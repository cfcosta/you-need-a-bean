//! Local addition vs upstream 2.6.0: the beancount directives upstream does
//! not model.
//!
//! Upstream recognises eight directive kinds. Everything else beancount
//! defines — `note`, `document`, `query`, `custom` and the top-level `plugin`,
//! `pushmeta` and `popmeta` — matched no rule and fell through the "skip the
//! line" fallback, producing no entry and no error. See VENDOR.md.

use nom::{
    branch::alt,
    bytes::complete::{tag, take_while},
    character::complete::{char, space0, space1},
    combinator::{iterator, map, opt, peek, value},
    sequence::preceded,
    Parser,
};

use crate::{
    account, amount, date, metadata, string, Account, Amount, Currency, Date,
    Decimal, IResult, Span,
};

/// A `note` directive: free text attached to an account.
///
/// # Example
///
/// ```
/// # use beancount_parser::{BeancountFile, DirectiveContent};
/// let input = r#"2023-05-27 note Assets:Cash "called the bank""#;
/// let beancount: BeancountFile<f64> = input.parse().unwrap();
/// let DirectiveContent::Note(note) = &beancount.directives[0].content else { unreachable!() };
/// assert_eq!(note.account.as_str(), "Assets:Cash");
/// assert_eq!(note.comment, "called the bank");
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Note {
    /// Account the note is attached to
    pub account: Account,
    /// The note's text
    pub comment: String,
}

/// A `document` directive: an external file attached to an account.
///
/// # Example
///
/// ```
/// # use beancount_parser::{BeancountFile, DirectiveContent};
/// let input = r#"2023-05-27 document Assets:Cash "statement.pdf""#;
/// let beancount: BeancountFile<f64> = input.parse().unwrap();
/// let DirectiveContent::Document(doc) = &beancount.directives[0].content else { unreachable!() };
/// assert_eq!(doc.path, "statement.pdf");
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Document {
    /// Account the document is attached to
    pub account: Account,
    /// Path to the document, as written (relative paths are not resolved)
    pub path: String,
}

/// A `query` directive: a named BQL query stored in the ledger.
///
/// # Example
///
/// ```
/// # use beancount_parser::{BeancountFile, DirectiveContent};
/// let input = r#"2023-05-27 query "expenses" "SELECT account""#;
/// let beancount: BeancountFile<f64> = input.parse().unwrap();
/// let DirectiveContent::Query(query) = &beancount.directives[0].content else { unreachable!() };
/// assert_eq!(query.name, "expenses");
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Query {
    /// Name of the query
    pub name: String,
    /// The query text itself, uninterpreted
    pub query: String,
}

/// A `custom` directive: a named directive with a free-form list of values,
/// used by plugins.
///
/// # Example
///
/// ```
/// # use beancount_parser::{BeancountFile, DirectiveContent, CustomValue};
/// let input = r#"2023-05-27 custom "budget" Expenses:Food "monthly" 400.00 USD"#;
/// let beancount: BeancountFile<f64> = input.parse().unwrap();
/// let DirectiveContent::Custom(custom) = &beancount.directives[0].content else { unreachable!() };
/// assert_eq!(custom.name, "budget");
/// assert_eq!(custom.values.len(), 3);
/// ```
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct Custom<D> {
    /// Name of the custom directive
    pub name: String,
    /// Its arguments, in the order they were written
    pub values: Vec<CustomValue<D>>,
}

/// One argument of a [`Custom`] directive.
#[allow(missing_docs)]
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum CustomValue<D> {
    String(String),
    Date(Date),
    Bool(bool),
    Amount(Amount<D>),
    Number(D),
    Account(Account),
    Currency(Currency),
}

/// A `plugin` directive: a Python plugin beancount should load.
///
/// # Example
///
/// ```
/// # use beancount_parser::{BeancountFile, Entry};
/// let input = r#"plugin "beancount.plugins.auto" "config""#;
/// let beancount: BeancountFile<f64> = input.parse().unwrap();
/// assert_eq!(beancount.plugins[0].name, "beancount.plugins.auto");
/// assert_eq!(beancount.plugins[0].config.as_deref(), Some("config"));
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Plugin {
    /// Module name of the plugin
    pub name: String,
    /// Optional configuration string
    pub config: Option<String>,
}

pub(crate) fn note(input: Span<'_>) -> IResult<'_, Note> {
    let (input, account) = account::parse(input)?;
    let (input, _) = space1(input)?;
    let (input, comment) = string(input)?;
    Ok((input, Note { account, comment }))
}

pub(crate) fn document(input: Span<'_>) -> IResult<'_, Document> {
    let (input, account) = account::parse(input)?;
    let (input, _) = space1(input)?;
    let (input, path) = string(input)?;
    Ok((input, Document { account, path }))
}

pub(crate) fn query(input: Span<'_>) -> IResult<'_, Query> {
    let (input, name) = string(input)?;
    let (input, _) = space1(input)?;
    let (input, query) = string(input)?;
    Ok((input, Query { name, query }))
}

pub(crate) fn custom<D: Decimal>(input: Span<'_>) -> IResult<'_, Custom<D>> {
    let (input, name) = string(input)?;
    let mut iter = iterator(input, preceded(space1, custom_value));
    let values: Vec<CustomValue<D>> = iter.by_ref().collect();
    let (input, ()) = iter.finish()?;
    Ok((input, Custom { name, values }))
}

/// Order matters. A date and an amount both start with a digit, and
/// `amount::expression` would happily read `2023-05-27` as arithmetic, so dates
/// go first. `TRUE`/`FALSE` would otherwise lex as currencies, so booleans go
/// before both currencies and accounts.
fn custom_value<D: Decimal>(input: Span<'_>) -> IResult<'_, CustomValue<D>> {
    alt((
        map(date::parse, CustomValue::Date),
        map(string, CustomValue::String),
        value(CustomValue::Bool(true), tag("TRUE")),
        value(CustomValue::Bool(false), tag("FALSE")),
        map(amount::parse, CustomValue::Amount),
        map(amount::expression, CustomValue::Number),
        map(account_value, CustomValue::Account),
        map(amount::currency, CustomValue::Currency),
    ))
    .parse(input)
}

/// `account::parse` commits (via `cut`) once the first segment matches, which
/// would turn a bare currency into an unrecoverable failure instead of letting
/// `alt` move on. Only hand it a token that actually contains a colon.
fn account_value(input: Span<'_>) -> IResult<'_, Account> {
    let (_, token) =
        peek(take_while(|c: char| !c.is_whitespace())).parse(input)?;
    if !token.fragment().contains(':') {
        return Err(nom::Err::Error(nom::error::Error::new(
            input,
            nom::error::ErrorKind::Tag,
        )));
    }
    account::parse(input)
}

pub(crate) fn plugin(input: Span<'_>) -> IResult<'_, Plugin> {
    let (input, _) = tag("plugin")(input)?;
    let (input, name) = preceded(space1, string).parse(input)?;
    let (input, config) = opt(preceded(space1, string)).parse(input)?;
    Ok((input, Plugin { name, config }))
}

/// `pushmeta key: value`. The value is required: `popmeta` pops one entry, so
/// a valueless push would leave the stack unbalanced.
pub(crate) fn pushmeta<D: Decimal>(
    input: Span<'_>,
) -> IResult<'_, (metadata::Key, metadata::Value<D>)> {
    let (input, _) = tag("pushmeta")(input)?;
    let (input, _) = space1(input)?;
    let (input, key) = metadata::key(input)?;
    let (input, _) = char(':')(input)?;
    let (input, _) = space1(input)?;
    let (input, value) = metadata::value(input)?;
    Ok((input, (key, value)))
}

pub(crate) fn popmeta(input: Span<'_>) -> IResult<'_, metadata::Key> {
    let (input, _) = tag("popmeta")(input)?;
    let (input, _) = space1(input)?;
    let (input, key) = metadata::key(input)?;
    let (input, _) = space0(input)?;
    let (input, _) = char(':')(input)?;
    Ok((input, key))
}
