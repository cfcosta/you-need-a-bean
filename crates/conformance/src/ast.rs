//! A beancount AST that knows both how to write itself as source and what
//! parsing that source must produce.
//!
//! This is the oracle. [`Ledger::render`] makes one pass and emits two things
//! at once: the `.beancount` text, and the exact [`crate::dump`] output that
//! parsing that text has to yield — including line numbers, the `pushtag`
//! stack, and the value of every arithmetic expression. Nothing here calls the
//! parser, so the two sides are genuinely independent: a parser that drops a
//! posting, mis-folds `2 + 3 * 4`, forgets a pushed tag, or shifts a line
//! number makes them disagree.
//!
//! Everything is deliberately explicit rather than convenient. [`Num`] carries
//! the literal text *and* its value so `1,234.50` and `+7` can be written
//! without the model having to re-derive what the parser does with them.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use rust_decimal::Decimal;

use crate::dump::quote;

/// A whole file.
#[derive(Debug, Clone, Default)]
pub struct Ledger {
    pub entries: Vec<Entry>,
}

/// One top-level construct, in source order.
#[derive(Debug, Clone)]
pub enum Entry {
    /// An empty line. Produces no dump output.
    Blank,
    /// A `;` comment line. Produces no dump output.
    Comment(String),
    Option {
        name: String,
        value: String,
    },
    Include(String),
    /// Pushes onto the tag stack applied to later transactions.
    PushTag(String),
    /// Pops from the tag stack. Popping a tag that was never pushed is a
    /// no-op, matching the parser.
    PopTag(String),
    Plugin {
        name: String,
        config: Option<String>,
    },
    /// Pushes a key onto the metadata stack applied to later directives.
    PushMeta(String, MetaValue),
    /// Pops the innermost push of a key. Popping a key that was never pushed
    /// is a no-op, matching the parser.
    PopMeta(String),
    Directive(Directive),
}

#[derive(Debug, Clone)]
pub struct Directive {
    pub date: Date,
    pub content: Content,
    /// Metadata attached to the directive itself. Keys must be unique; the
    /// parser keeps a map, so duplicates would collapse.
    pub meta: Vec<(String, MetaValue)>,
    /// Tags written on the directive line. Left empty for a transaction,
    /// whose tags absorb the pushed tag stack and so live on [`Txn::tags`].
    pub tags: Vec<String>,
    /// Links written on the directive line. Empty for a transaction, for the
    /// same reason as [`Directive::tags`].
    pub links: Vec<String>,
}

#[derive(Debug, Clone)]
pub enum Content {
    Transaction(Txn),
    Open {
        account: String,
        currencies: Vec<String>,
        booking: Option<String>,
    },
    Close {
        account: String,
    },
    Balance {
        account: String,
        amount: Amount,
        tolerance: Option<Expr>,
    },
    Pad {
        account: String,
        source: String,
    },
    Price {
        currency: String,
        amount: Amount,
    },
    Commodity {
        currency: String,
    },
    Event {
        name: String,
        value: String,
    },
    Note {
        account: String,
        comment: String,
    },
    Document {
        account: String,
        path: String,
    },
    Query {
        name: String,
        query: String,
    },
    Custom {
        name: String,
        values: Vec<CustomValue>,
    },
}

/// One argument of a `custom` directive.
///
/// The arguments are read by type, in order, and the types overlap: `TRUE` and
/// a bare currency lex the same way, and a bare number followed by either of
/// them lexes as a single amount. Writing a sequence that re-reads as
/// something else is possible, so the generator does not — see
/// [`crate::generate`].
#[derive(Debug, Clone)]
pub enum CustomValue {
    Str(String),
    Date(Date),
    Bool(bool),
    Amount(Amount),
    Num(Num),
    Account(String),
    Currency(String),
}

impl CustomValue {
    fn render(&self) -> String {
        match self {
            CustomValue::Str(text) => quote(text),
            CustomValue::Date(date) => date.render(),
            CustomValue::Bool(true) => "TRUE".to_string(),
            CustomValue::Bool(false) => "FALSE".to_string(),
            CustomValue::Amount(amount) => amount.render(),
            CustomValue::Num(number) => number.literal.clone(),
            CustomValue::Account(account) | CustomValue::Currency(account) => {
                account.clone()
            }
        }
    }

    fn dump(&self) -> String {
        match self {
            CustomValue::Str(text) => format!("string {}", quote(text)),
            CustomValue::Date(date) => format!("date {}", date.render()),
            CustomValue::Bool(value) => format!("bool {value}"),
            CustomValue::Amount(amount) => format!("amount {}", amount.dump()),
            CustomValue::Num(number) => format!("number {}", number.value),
            CustomValue::Account(account) => format!("account {account}"),
            CustomValue::Currency(currency) => format!("currency {currency}"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Txn {
    pub flag: Flag,
    /// A payee can only be written when there is also a narration — the
    /// grammar reads one string as the narration and two as payee then
    /// narration.
    pub payee: Option<String>,
    pub narration: Option<String>,
    pub tags: Vec<String>,
    pub links: Vec<String>,
    pub postings: Vec<Posting>,
}

/// A transaction's flag. Anything that is not an ASCII lowercase letter is a
/// flag; the `txn` keyword means "no flag".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Flag {
    /// The `txn` keyword: parses to no flag at all.
    Keyword,
    Char(char),
}

#[derive(Debug, Clone)]
pub struct Posting {
    pub flag: Option<char>,
    pub account: String,
    pub amount: Option<Amount>,
    pub cost: Option<Cost>,
    pub price: Option<Price>,
    pub meta: Vec<(String, MetaValue)>,
}

/// `{ … }` (per unit) or `{{ … }}` (total). Either part may be absent, and
/// `{}` with nothing inside is legal.
#[derive(Debug, Clone)]
pub struct Cost {
    pub total: bool,
    pub amount: Option<Amount>,
    pub date: Option<Date>,
    /// Write the date before the amount rather than after.
    pub date_first: bool,
}

#[derive(Debug, Clone)]
pub enum Price {
    Unit(Amount),
    Total(Amount),
}

#[derive(Debug, Clone)]
pub struct Amount {
    pub value: Expr,
    pub currency: String,
}

#[derive(Debug, Clone)]
pub enum MetaValue {
    Str(String),
    Num(Expr),
    Currency(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Date {
    pub year: u16,
    pub month: u8,
    pub day: u8,
}

impl Date {
    #[must_use]
    pub fn new(year: u16, month: u8, day: u8) -> Self {
        Self { year, month, day }
    }

    fn render(self) -> String {
        format!("{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }
}

/// A number, kept as both the text to write and the value it stands for.
///
/// The parser strips `,` and a leading `+` from literals, so `1,234.50` and
/// `+7` need no special handling anywhere else: the model already knows the
/// answer.
#[derive(Debug, Clone)]
pub struct Num {
    pub literal: String,
    pub value: Decimal,
}

impl Num {
    /// A number written the plain way, with `scale` decimal places.
    #[must_use]
    pub fn plain(value: Decimal) -> Self {
        Self {
            literal: value.to_string(),
            value,
        }
    }

    /// The same value written with `,` thousands separators, which the parser
    /// discards.
    #[must_use]
    pub fn grouped(value: Decimal) -> Self {
        let text = value.to_string();
        let (sign, rest) = match text.strip_prefix('-') {
            Some(rest) => ("-", rest),
            None => ("", text.as_str()),
        };
        let (int, frac) = rest.split_once('.').unwrap_or((rest, ""));
        let mut grouped = String::new();
        for (i, ch) in int.chars().enumerate() {
            if i > 0 && (int.len() - i) % 3 == 0 {
                grouped.push(',');
            }
            grouped.push(ch);
        }
        let mut literal = format!("{sign}{grouped}");
        if !frac.is_empty() {
            let _ = write!(literal, ".{frac}");
        }
        Self { literal, value }
    }

    /// The same value written with an explicit leading `+`.
    #[must_use]
    pub fn signed_plus(value: Decimal) -> Self {
        debug_assert!(value >= Decimal::ZERO, "a `+` on a negative is a typo");
        Self {
            literal: format!("+{value}"),
            value,
        }
    }
}

/// An arithmetic expression.
///
/// Only shapes the parser actually accepts are representable. In particular
/// [`Expr::Signed`] always wraps a group, because the grammar's unary sign
/// rule only applies to a parenthesised expression — a bare `-5` is part of
/// the literal itself.
#[derive(Debug, Clone)]
pub enum Expr {
    Lit(Num),
    /// `( … )`
    Group(Box<Expr>),
    /// `-( … )` or `+( … )`
    Signed(char, Box<Expr>),
    /// A left-associative run of operators, all of the same precedence:
    /// either `+`/`-` or `*`/`/`. Mixing the two levels is done by nesting,
    /// which keeps evaluation order unambiguous.
    Chain {
        head: Box<Expr>,
        tail: Vec<(char, Expr)>,
    },
}

impl Expr {
    #[must_use]
    pub fn lit(num: Num) -> Self {
        Expr::Lit(num)
    }

    #[must_use]
    pub fn int(value: i64) -> Self {
        Expr::Lit(Num::plain(Decimal::from(value)))
    }

    /// The value this expression evaluates to, folded left exactly the way
    /// the grammar folds it.
    #[must_use]
    pub fn eval(&self) -> Decimal {
        match self {
            Expr::Lit(num) => num.value,
            Expr::Group(inner) => inner.eval(),
            Expr::Signed(sign, inner) => {
                let value = inner.eval();
                if *sign == '-' { -value } else { value }
            }
            Expr::Chain { head, tail } => {
                tail.iter().fold(head.eval(), |acc, (op, rhs)| {
                    let rhs = rhs.eval();
                    match op {
                        '+' => acc + rhs,
                        '-' => acc - rhs,
                        '*' => acc * rhs,
                        '/' => acc / rhs,
                        other => unreachable!("bad operator {other}"),
                    }
                })
            }
        }
    }

    fn render(&self) -> String {
        match self {
            Expr::Lit(num) => num.literal.clone(),
            Expr::Group(inner) => format!("({})", inner.render()),
            Expr::Signed(sign, inner) => {
                format!("{sign}({})", inner.render())
            }
            Expr::Chain { head, tail } => {
                let mut out = head.render();
                for (op, rhs) in tail {
                    let _ = write!(out, " {op} {}", rhs.render());
                }
                out
            }
        }
    }
}

impl Amount {
    fn render(&self) -> String {
        format!("{} {}", self.value.render(), self.currency)
    }

    fn dump(&self) -> String {
        format!("{} {}", self.value.eval(), self.currency)
    }
}

/// The source text of a ledger paired with the dump parsing it must produce.
#[derive(Debug, Clone)]
pub struct Rendered {
    pub text: String,
    pub dump: String,
}

struct Writer {
    text: String,
    dump: String,
    /// Line number the next `text` line will occupy.
    line: u32,
    /// Tags currently pushed, applied to every transaction written.
    stack: BTreeSet<String>,
    /// Metadata currently pushed, applied to every directive written. The
    /// pushes nest, so each key holds a stack and the innermost one wins.
    meta_stack: BTreeMap<String, Vec<MetaValue>>,
}

impl Writer {
    fn source(&mut self, line: &str) -> u32 {
        let at = self.line;
        self.text.push_str(line);
        self.text.push('\n');
        self.line += 1;
        at
    }

    fn expect(&mut self, line: &str) {
        self.dump.push_str(line);
        self.dump.push('\n');
    }
}

impl Ledger {
    /// Write the ledger as source together with the dump it must parse to.
    #[must_use]
    pub fn render(&self) -> Rendered {
        let mut w = Writer {
            text: String::new(),
            dump: String::new(),
            line: 1,
            stack: BTreeSet::new(),
            meta_stack: BTreeMap::new(),
        };
        for entry in &self.entries {
            render_entry(&mut w, entry);
        }
        Rendered {
            text: w.text,
            dump: w.dump,
        }
    }

    /// Number of directives, for benchmark labelling.
    #[must_use]
    pub fn directives(&self) -> usize {
        self.entries
            .iter()
            .filter(|e| matches!(e, Entry::Directive(_)))
            .count()
    }
}

fn render_entry(w: &mut Writer, entry: &Entry) {
    match entry {
        Entry::Blank => {
            w.source("");
        }
        Entry::Comment(text) => {
            w.source(&format!("; {text}"));
        }
        Entry::Option { name, value } => {
            w.source(&format!("option {} {}", quote(name), quote(value)));
            w.expect(&format!("option {} {}", quote(name), quote(value)));
        }
        Entry::Include(path) => {
            w.source(&format!("include {}", quote(path)));
            w.expect(&format!("include {}", quote(path)));
        }
        Entry::PushTag(tag) => {
            w.source(&format!("pushtag #{tag}"));
            w.stack.insert(tag.clone());
        }
        Entry::PopTag(tag) => {
            w.source(&format!("poptag #{tag}"));
            w.stack.remove(tag);
        }
        Entry::Plugin { name, config } => {
            let mut line = format!("plugin {}", quote(name));
            if let Some(config) = config {
                let _ = write!(line, " {}", quote(config));
            }
            w.source(&line.clone());
            w.expect(&line);
        }
        Entry::PushMeta(key, value) => {
            w.source(&format!("pushmeta {key}: {}", render_meta_value(value)));
            w.meta_stack
                .entry(key.clone())
                .or_default()
                .push(value.clone());
        }
        Entry::PopMeta(key) => {
            w.source(&format!("popmeta {key}:"));
            if let Some(stack) = w.meta_stack.get_mut(key) {
                stack.pop();
            }
        }
        Entry::Directive(directive) => render_directive(w, directive),
    }
}

fn render_directive(w: &mut Writer, directive: &Directive) {
    let date = directive.date.render();
    match &directive.content {
        Content::Transaction(txn) => {
            let mut header = format!("{date} ");
            match txn.flag {
                Flag::Keyword => header.push_str("txn"),
                Flag::Char(flag) => header.push(flag),
            }
            if let Some(narration) = &txn.narration {
                if let Some(payee) = &txn.payee {
                    let _ = write!(header, " {}", quote(payee));
                }
                let _ = write!(header, " {}", quote(narration));
            }
            for tag in &txn.tags {
                let _ = write!(header, " #{tag}");
            }
            for link in &txn.links {
                let _ = write!(header, " ^{link}");
            }
            let line = w.source(&header);

            w.expect(&format!("directive line={line} {date} txn"));
            if let Flag::Char(flag) = txn.flag {
                w.expect(&format!("  flag {flag}"));
            }
            if txn.narration.is_some()
                && let Some(payee) = &txn.payee
            {
                w.expect(&format!("  payee {}", quote(payee)));
            }
            if let Some(narration) = &txn.narration {
                w.expect(&format!("  narration {}", quote(narration)));
            }
            let mut tags: BTreeSet<String> = txn.tags.iter().cloned().collect();
            tags.extend(w.stack.iter().cloned());
            for tag in tags {
                w.expect(&format!("  tag {tag}"));
            }
            let links: BTreeSet<&str> =
                txn.links.iter().map(String::as_str).collect();
            for link in links {
                w.expect(&format!("  link {link}"));
            }
            render_directive_meta(w, &directive.meta);
            for posting in &txn.postings {
                render_posting(w, posting);
            }
        }
        Content::Open {
            account,
            currencies,
            booking,
        } => {
            let mut header = format!("{date} open {account}");
            if !currencies.is_empty() {
                let _ = write!(header, " {}", currencies.join(","));
            }
            if let Some(booking) = booking {
                let _ = write!(header, " {}", quote(booking));
            }
            let line = w.source(&tagged(&header, directive));
            w.expect(&format!("directive line={line} {date} open"));
            w.expect(&format!("  account {account}"));
            for currency in currencies.iter().collect::<BTreeSet<_>>() {
                w.expect(&format!("  currency {currency}"));
            }
            if let Some(booking) = booking {
                w.expect(&format!("  booking {}", quote(booking)));
            }
            render_tail(w, directive);
        }
        Content::Close { account } => {
            let header = format!("{date} close {account}");
            let line = w.source(&tagged(&header, directive));
            w.expect(&format!("directive line={line} {date} close"));
            w.expect(&format!("  account {account}"));
            render_tail(w, directive);
        }
        Content::Balance {
            account,
            amount,
            tolerance,
        } => {
            let mut header =
                format!("{date} balance {account} {}", amount.value.render());
            if let Some(tolerance) = tolerance {
                let _ = write!(header, " ~ {}", tolerance.render());
            }
            let _ = write!(header, " {}", amount.currency);
            let line = w.source(&tagged(&header, directive));
            w.expect(&format!("directive line={line} {date} balance"));
            w.expect(&format!("  account {account}"));
            w.expect(&format!("  amount {}", amount.dump()));
            if let Some(tolerance) = tolerance {
                w.expect(&format!("  tolerance {}", tolerance.eval()));
            }
            render_tail(w, directive);
        }
        Content::Pad { account, source } => {
            let header = format!("{date} pad {account} {source}");
            let line = w.source(&tagged(&header, directive));
            w.expect(&format!("directive line={line} {date} pad"));
            w.expect(&format!("  account {account}"));
            w.expect(&format!("  source {source}"));
            render_tail(w, directive);
        }
        Content::Price { currency, amount } => {
            let header = format!("{date} price {currency} {}", amount.render());
            let line = w.source(&tagged(&header, directive));
            w.expect(&format!("directive line={line} {date} price"));
            w.expect(&format!("  currency {currency}"));
            w.expect(&format!("  amount {}", amount.dump()));
            render_tail(w, directive);
        }
        Content::Commodity { currency } => {
            let header = format!("{date} commodity {currency}");
            let line = w.source(&tagged(&header, directive));
            w.expect(&format!("directive line={line} {date} commodity"));
            w.expect(&format!("  currency {currency}"));
            render_tail(w, directive);
        }
        Content::Event { name, value } => {
            let header =
                format!("{date} event {} {}", quote(name), quote(value));
            let line = w.source(&tagged(&header, directive));
            w.expect(&format!("directive line={line} {date} event"));
            w.expect(&format!("  name {}", quote(name)));
            w.expect(&format!("  value {}", quote(value)));
            render_tail(w, directive);
        }
        Content::Note { account, comment } => {
            let header = format!("{date} note {account} {}", quote(comment));
            let line = w.source(&tagged(&header, directive));
            w.expect(&format!("directive line={line} {date} note"));
            w.expect(&format!("  account {account}"));
            w.expect(&format!("  comment {}", quote(comment)));
            render_tail(w, directive);
        }
        Content::Document { account, path } => {
            let header = format!("{date} document {account} {}", quote(path));
            let line = w.source(&tagged(&header, directive));
            w.expect(&format!("directive line={line} {date} document"));
            w.expect(&format!("  account {account}"));
            w.expect(&format!("  path {}", quote(path)));
            render_tail(w, directive);
        }
        Content::Query { name, query } => {
            let header =
                format!("{date} query {} {}", quote(name), quote(query));
            let line = w.source(&tagged(&header, directive));
            w.expect(&format!("directive line={line} {date} query"));
            w.expect(&format!("  name {}", quote(name)));
            w.expect(&format!("  query {}", quote(query)));
            render_tail(w, directive);
        }
        Content::Custom { name, values } => {
            let mut header = format!("{date} custom {}", quote(name));
            for value in values {
                let _ = write!(header, " {}", value.render());
            }
            let line = w.source(&tagged(&header, directive));
            w.expect(&format!("directive line={line} {date} custom"));
            w.expect(&format!("  name {}", quote(name)));
            for value in values {
                w.expect(&format!("  value {}", value.dump()));
            }
            render_tail(w, directive);
        }
    }
}

/// A directive's source line with its tags and links appended, in the order
/// they were written.
fn tagged(header: &str, directive: &Directive) -> String {
    let mut line = header.to_string();
    for tag in &directive.tags {
        let _ = write!(line, " #{tag}");
    }
    for link in &directive.links {
        let _ = write!(line, " ^{link}");
    }
    line
}

/// The dump lines every directive ends with: its tags, its links, then its
/// metadata. Mirrors `dump::tail_to`, which is the thing being pinned.
fn render_tail(w: &mut Writer, directive: &Directive) {
    let tags: BTreeSet<&str> =
        directive.tags.iter().map(String::as_str).collect();
    for tag in tags {
        w.expect(&format!("  tag {tag}"));
    }
    let links: BTreeSet<&str> =
        directive.links.iter().map(String::as_str).collect();
    for link in links {
        w.expect(&format!("  link {link}"));
    }
    render_directive_meta(w, &directive.meta);
}

fn render_posting(w: &mut Writer, posting: &Posting) {
    let mut line = String::from("  ");
    if let Some(flag) = posting.flag {
        let _ = write!(line, "{flag} ");
    }
    line.push_str(&posting.account);
    if let Some(amount) = &posting.amount {
        let _ = write!(line, "  {}", amount.render());
        if let Some(cost) = &posting.cost {
            let _ = write!(line, " {}", render_cost(cost));
        }
        match &posting.price {
            Some(Price::Unit(price)) => {
                let _ = write!(line, " @ {}", price.render());
            }
            Some(Price::Total(price)) => {
                let _ = write!(line, " @@ {}", price.render());
            }
            None => {}
        }
    }
    w.source(&line);

    w.expect("  posting");
    if let Some(flag) = posting.flag {
        w.expect(&format!("    flag {flag}"));
    }
    w.expect(&format!("    account {}", posting.account));
    if let Some(amount) = &posting.amount {
        w.expect(&format!("    amount {}", amount.dump()));
        if let Some(cost) = &posting.cost {
            w.expect(&format!("    cost {}", dump_cost(cost)));
        }
        match &posting.price {
            Some(Price::Unit(price)) => {
                w.expect(&format!("    price unit {}", price.dump()));
            }
            Some(Price::Total(price)) => {
                w.expect(&format!("    price total {}", price.dump()));
            }
            None => {}
        }
    }
    render_meta(w, "    ", &posting.meta);
}

fn render_cost(cost: &Cost) -> String {
    let mut body = String::new();
    match (&cost.amount, cost.date, cost.date_first) {
        (Some(amount), Some(date), false) => {
            let _ = write!(body, "{}, {}", amount.render(), date.render());
        }
        (Some(amount), Some(date), true) => {
            let _ = write!(body, "{}, {}", date.render(), amount.render());
        }
        (Some(amount), None, _) => body.push_str(&amount.render()),
        (None, Some(date), _) => body.push_str(&date.render()),
        (None, None, _) => {}
    }
    if cost.total {
        format!("{{{{ {body} }}}}")
    } else {
        format!("{{ {body} }}")
    }
}

fn dump_cost(cost: &Cost) -> String {
    let mut body = if cost.total { "total" } else { "unit" }.to_string();
    if let Some(amount) = &cost.amount {
        let _ = write!(body, " amount={}", amount.dump());
    }
    if let Some(date) = cost.date {
        let _ = write!(body, " date={}", date.render());
    }
    body
}

fn render_meta_value(value: &MetaValue) -> String {
    match value {
        MetaValue::Str(text) => quote(text),
        MetaValue::Num(expr) => expr.render(),
        MetaValue::Currency(currency) => currency.clone(),
    }
}

fn dump_meta_value(value: &MetaValue) -> String {
    match value {
        MetaValue::Str(text) => format!("string {}", quote(text)),
        MetaValue::Num(expr) => format!("number {}", expr.eval()),
        MetaValue::Currency(currency) => format!("currency {currency}"),
    }
}

/// Metadata on a posting. Nothing is inherited: `pushmeta` reaches directives
/// only.
fn render_meta(w: &mut Writer, indent: &str, meta: &[(String, MetaValue)]) {
    for (key, value) in meta {
        w.source(&format!("{indent}{key}: {}", render_meta_value(value)));
    }
    let mut sorted: Vec<(&str, String)> = meta
        .iter()
        .map(|(key, value)| (key.as_str(), dump_meta_value(value)))
        .collect();
    sorted.sort_unstable();
    for (key, value) in sorted {
        w.expect(&format!("{indent}meta {key} {value}"));
    }
}

/// Metadata on a directive, which also inherits whatever `pushmeta` has left
/// on the stack. A key the directive sets itself keeps its own value: the
/// pushed one only fills a gap.
fn render_directive_meta(w: &mut Writer, meta: &[(String, MetaValue)]) {
    for (key, value) in meta {
        w.source(&format!("  {key}: {}", render_meta_value(value)));
    }
    let mut sorted: Vec<(String, String)> = meta
        .iter()
        .map(|(key, value)| (key.clone(), dump_meta_value(value)))
        .collect();
    for (key, stack) in &w.meta_stack {
        if let Some(value) = stack.last()
            && !meta.iter().any(|(written, _)| written == key)
        {
            sorted.push((key.clone(), dump_meta_value(value)));
        }
    }
    sorted.sort_unstable();
    for (key, value) in sorted {
        w.expect(&format!("  meta {key} {value}"));
    }
}
