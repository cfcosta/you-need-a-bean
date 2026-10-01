//! The canonical dump: a deterministic, line-oriented rendering of everything
//! a parse observes.
//!
//! This is the pin. Two parsers agree exactly when they produce the same dump
//! for the same input, so a from-scratch implementation only has to reproduce
//! this function's output — no API shape, no internal types, nothing about how
//! the parse is done.
//!
//! Format, two-space indents:
//!
//! ```text
//! option "operating_currency" "USD"
//! include "months/2026-01.beancount"
//! plugin "beancount.plugins.auto" "config"
//! directive line=7 2026-01-15 txn
//!   flag *
//!   payee "Whole Foods"
//!   narration "groceries"
//!   tag food
//!   link inv-1
//!   meta rate number 4.55
//!   posting
//!     account Assets:Cash
//!     amount -10.00 CHF
//!     cost unit amount=2 PLN date=2026-01-01
//!     price total 25.00 EUR
//!     meta lot string "A"
//! error line=12 expected="the end of the line"
//! ```
//!
//! Rules that make it canonical, and that a replacement must therefore match:
//!
//! - Sets and maps the parser stores unordered — tags, links, `open`
//!   currencies, metadata — are emitted sorted by their string form, so the
//!   dump never depends on hash iteration order.
//! - Every directive ends with its tags, then its links, then its metadata,
//!   whatever kind it is. A transaction's tags come from the transaction (they
//!   absorb the pushed tag stack); every other kind's come from the directive
//!   line itself. The dump does not distinguish the two.
//! - Absent optional fields print no line at all. Every present one prints,
//!   including empty strings (`payee ""`), so presence is never ambiguous. The
//!   one field that shares a line with another is a plugin's config, which is
//!   still unambiguous because both are quoted.
//! - Numbers print as [`rust_decimal::Decimal`] does, which preserves the
//!   scale the input was written with: `10.00` stays `10.00`, and an
//!   expression prints the value it evaluates to.
//! - An error prints `error line=N expected="…"`, naming what the parser was
//!   looking for where it stopped, or just `error line=N` when nothing more
//!   specific is known. Parsing then carries on at the next line that starts
//!   in column 1, so a dump can hold several errors, each where the parser's
//!   own iterator yields it.

use std::{collections::HashSet, fmt::Write as _};

use beancount_parser::{
    Amount, Cost, CustomValue, Date, Directive, DirectiveContent, Entry, Link,
    Posting, PostingPrice, Tag, metadata, parse_iter,
};
use rust_decimal::Decimal;

/// Parse `input` and render the canonical dump of everything observed.
///
/// Never fails and never panics: a syntax error becomes an `error line=N`
/// line where it was found, and the dump goes on from there.
#[must_use]
pub fn dump(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for entry in parse_iter::<Decimal>(input) {
        match entry {
            Ok(Entry::Option(option)) => {
                let _ = writeln!(
                    out,
                    "option {} {}",
                    quote(&option.name),
                    quote(&option.value)
                );
            }
            Ok(Entry::Include(include)) => {
                let _ = writeln!(
                    out,
                    "include {}",
                    quote(&include.path.to_string_lossy())
                );
            }
            Ok(Entry::Plugin(plugin)) => {
                let _ = write!(out, "plugin {}", quote(&plugin.name));
                if let Some(config) = &plugin.config {
                    let _ = write!(out, " {}", quote(config));
                }
                let _ = writeln!(out);
            }
            Ok(Entry::Directive(directive)) => {
                directive_to(&mut out, &directive)
            }
            Ok(_) => {
                // `Entry` is `#[non_exhaustive]`; a variant this dump does not
                // know about is a contract change, not something to swallow.
                let _ = writeln!(out, "unknown-entry");
            }
            Err(err) => {
                let _ = write!(out, "error line={}", err.line_number());
                if let Some(expected) = err.expected() {
                    let _ = write!(out, " expected={}", quote(expected));
                }
                let _ = writeln!(out);
            }
        }
    }
    out
}

/// The dump with every `line=N` removed, for properties about everything
/// *except* where things sit in the file.
#[must_use]
pub fn without_line_numbers(dumped: &str) -> String {
    dumped
        .lines()
        .map(|line| match line.strip_prefix("directive line=") {
            Some(rest) => {
                let tail = rest.split_once(' ').map_or("", |(_, t)| t);
                format!("directive {tail}\n")
            }
            None => match line.strip_prefix("error line=") {
                Some(rest) => match rest.split_once(' ') {
                    Some((_, tail)) => format!("error {tail}\n"),
                    None => "error\n".to_string(),
                },
                None => format!("{line}\n"),
            },
        })
        .collect()
}

fn directive_to(out: &mut String, directive: &Directive<Decimal>) {
    let kind = match &directive.content {
        DirectiveContent::Transaction(_) => "txn",
        DirectiveContent::Price(_) => "price",
        DirectiveContent::Balance(_) => "balance",
        DirectiveContent::Open(_) => "open",
        DirectiveContent::Close(_) => "close",
        DirectiveContent::Pad(_) => "pad",
        DirectiveContent::Commodity(_) => "commodity",
        DirectiveContent::Event(_) => "event",
        DirectiveContent::Note(_) => "note",
        DirectiveContent::Document(_) => "document",
        DirectiveContent::Query(_) => "query",
        DirectiveContent::Custom(_) => "custom",
        _ => "unknown",
    };
    let _ = writeln!(
        out,
        "directive line={} {} {kind}",
        directive.line_number,
        date(directive.date)
    );

    match &directive.content {
        DirectiveContent::Transaction(txn) => {
            if let Some(flag) = txn.flag {
                let _ = writeln!(out, "  flag {flag}");
            }
            if let Some(payee) = &txn.payee {
                let _ = writeln!(out, "  payee {}", quote(payee));
            }
            if let Some(narration) = &txn.narration {
                let _ = writeln!(out, "  narration {}", quote(narration));
            }
            tags_and_links_to(out, &txn.tags, &txn.links);
            tail_to(out, directive);
            for posting in &txn.postings {
                posting_to(out, posting);
            }
        }
        DirectiveContent::Open(open) => {
            let _ = writeln!(out, "  account {}", open.account);
            let mut currencies: Vec<&str> =
                open.currencies.iter().map(AsRef::as_ref).collect();
            currencies.sort_unstable();
            for currency in currencies {
                let _ = writeln!(out, "  currency {currency}");
            }
            if let Some(booking) = &open.booking_method {
                let _ = writeln!(out, "  booking {}", quote(booking.as_ref()));
            }
            tail_to(out, directive);
        }
        DirectiveContent::Close(close) => {
            let _ = writeln!(out, "  account {}", close.account);
            tail_to(out, directive);
        }
        DirectiveContent::Balance(balance) => {
            let _ = writeln!(out, "  account {}", balance.account);
            let _ = writeln!(out, "  amount {}", amount(&balance.amount));
            if let Some(tolerance) = &balance.tolerance {
                let _ = writeln!(out, "  tolerance {tolerance}");
            }
            tail_to(out, directive);
        }
        DirectiveContent::Pad(pad) => {
            let _ = writeln!(out, "  account {}", pad.account);
            let _ = writeln!(out, "  source {}", pad.source_account);
            tail_to(out, directive);
        }
        DirectiveContent::Price(price) => {
            let _ = writeln!(out, "  currency {}", price.currency);
            let _ = writeln!(out, "  amount {}", amount(&price.amount));
            tail_to(out, directive);
        }
        DirectiveContent::Commodity(currency) => {
            let _ = writeln!(out, "  currency {currency}");
            tail_to(out, directive);
        }
        DirectiveContent::Event(event) => {
            let _ = writeln!(out, "  name {}", quote(&event.name));
            let _ = writeln!(out, "  value {}", quote(&event.value));
            tail_to(out, directive);
        }
        DirectiveContent::Note(note) => {
            let _ = writeln!(out, "  account {}", note.account);
            let _ = writeln!(out, "  comment {}", quote(&note.comment));
            tail_to(out, directive);
        }
        DirectiveContent::Document(document) => {
            let _ = writeln!(out, "  account {}", document.account);
            let _ = writeln!(out, "  path {}", quote(&document.path));
            tail_to(out, directive);
        }
        DirectiveContent::Query(query) => {
            let _ = writeln!(out, "  name {}", quote(&query.name));
            let _ = writeln!(out, "  query {}", quote(&query.query));
            tail_to(out, directive);
        }
        DirectiveContent::Custom(custom) => {
            let _ = writeln!(out, "  name {}", quote(&custom.name));
            // Order is meaning here, so these are emitted as written.
            for value in &custom.values {
                let _ = writeln!(out, "  value {}", custom_value(value));
            }
            tail_to(out, directive);
        }
        _ => {
            tail_to(out, directive);
        }
    }
}

/// Every directive ends the same way: the tags and links written on its line,
/// then its metadata. A transaction's own tags are rendered by its arm instead,
/// because they absorb the pushed tag stack and so live on the `Transaction`;
/// `directive.tags` is empty for one.
fn tail_to(out: &mut String, directive: &Directive<Decimal>) {
    tags_and_links_to(out, &directive.tags, &directive.links);
    metadata_to(out, "  ", &directive.metadata);
}

fn tags_and_links_to(
    out: &mut String,
    tags: &HashSet<Tag>,
    links: &HashSet<Link>,
) {
    let mut tags: Vec<&str> = tags.iter().map(AsRef::as_ref).collect();
    tags.sort_unstable();
    for tag in tags {
        let _ = writeln!(out, "  tag {tag}");
    }
    let mut links: Vec<&str> = links.iter().map(AsRef::as_ref).collect();
    links.sort_unstable();
    for link in links {
        let _ = writeln!(out, "  link {link}");
    }
}

fn posting_to(out: &mut String, posting: &Posting<Decimal>) {
    let _ = writeln!(out, "  posting");
    if let Some(flag) = posting.flag {
        let _ = writeln!(out, "    flag {flag}");
    }
    let _ = writeln!(out, "    account {}", posting.account);
    if let Some(value) = &posting.amount {
        let _ = writeln!(out, "    amount {}", amount(value));
    }
    if let Some(cost) = &posting.cost {
        let _ = writeln!(out, "    cost {}", cost_body(cost));
    }
    match &posting.price {
        Some(PostingPrice::Unit(price)) => {
            let _ = writeln!(out, "    price unit {}", amount(price));
        }
        Some(PostingPrice::Total(price)) => {
            let _ = writeln!(out, "    price total {}", amount(price));
        }
        None => {}
    }
    metadata_to(out, "    ", &posting.metadata);
}

fn metadata_to(out: &mut String, indent: &str, map: &metadata::Map<Decimal>) {
    let mut entries: Vec<(&str, String)> = map
        .iter()
        .map(|(key, value)| (key.as_ref(), metadata_value(value)))
        .collect();
    entries.sort_unstable();
    for (key, value) in entries {
        let _ = writeln!(out, "{indent}meta {key} {value}");
    }
}

fn metadata_value(value: &metadata::Value<Decimal>) -> String {
    match value {
        metadata::Value::String(text) => format!("string {}", quote(text)),
        metadata::Value::Number(number) => format!("number {number}"),
        metadata::Value::Currency(currency) => format!("currency {currency}"),
        _ => "unknown".to_string(),
    }
}

fn custom_value(value: &CustomValue<Decimal>) -> String {
    match value {
        CustomValue::String(text) => format!("string {}", quote(text)),
        CustomValue::Date(value) => format!("date {}", date(*value)),
        CustomValue::Bool(value) => format!("bool {value}"),
        CustomValue::Amount(value) => format!("amount {}", amount(value)),
        CustomValue::Number(number) => format!("number {number}"),
        CustomValue::Account(account) => format!("account {account}"),
        CustomValue::Currency(currency) => format!("currency {currency}"),
        _ => "unknown".to_string(),
    }
}

fn cost_body(cost: &Cost<Decimal>) -> String {
    // `{ … }` and `{{ … }}` are different postings; the dump has to say which.
    let mut body = if cost.total { "total" } else { "unit" }.to_string();
    if let Some(value) = &cost.amount {
        let _ = write!(body, " amount={}", amount(value));
    }
    if let Some(date) = cost.date {
        let _ = write!(body, " date={}", self::date(date));
    }
    body
}

fn amount(amount: &Amount<Decimal>) -> String {
    format!("{} {}", amount.value, amount.currency)
}

fn date(date: Date) -> String {
    format!("{:04}-{:02}-{:02}", date.year, date.month, date.day)
}

/// Quote a string so that no content can be confused with the dump's own
/// structure — every byte survives, including newlines the parser accepted.
pub(crate) fn quote(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for ch in text.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => {
                let _ = write!(out, "\\u{{{:x}}}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}
