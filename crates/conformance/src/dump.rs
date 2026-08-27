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
//! error line=12
//! ```
//!
//! Rules that make it canonical, and that a replacement must therefore match:
//!
//! - Sets and maps the parser stores unordered — tags, links, `open`
//!   currencies, metadata — are emitted sorted by their string form, so the
//!   dump never depends on hash iteration order.
//! - Absent optional fields print no line at all. Every present one prints,
//!   including empty strings (`payee ""`), so presence is never ambiguous.
//! - Numbers print as [`rust_decimal::Decimal`] does, which preserves the
//!   scale the input was written with: `10.00` stays `10.00`, and an
//!   expression prints the value it evaluates to.
//! - Parsing stops at the first error, and the dump ends with `error line=N`.
//!   That mirrors the parser's own iterator, which yields at most one error
//!   and then stops.

use std::fmt::Write as _;

use beancount_parser::{
    Amount, Cost, Date, Directive, DirectiveContent, Entry, Posting,
    PostingPrice, metadata, parse_iter,
};
use rust_decimal::Decimal;

/// Parse `input` and render the canonical dump of everything observed.
///
/// Never fails and never panics: a syntax error becomes a trailing
/// `error line=N` line.
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
            Ok(Entry::Include(path)) => {
                let _ =
                    writeln!(out, "include {}", quote(&path.to_string_lossy()));
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
                let _ = writeln!(out, "error line={}", err.line_number());
                break;
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
            None if line.starts_with("error line=") => "error\n".to_string(),
            None => format!("{line}\n"),
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
            let mut tags: Vec<&str> =
                txn.tags.iter().map(AsRef::as_ref).collect();
            tags.sort_unstable();
            for tag in tags {
                let _ = writeln!(out, "  tag {tag}");
            }
            let mut links: Vec<&str> =
                txn.links.iter().map(AsRef::as_ref).collect();
            links.sort_unstable();
            for link in links {
                let _ = writeln!(out, "  link {link}");
            }
            metadata_to(out, "  ", &directive.metadata);
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
                let _ =
                    writeln!(out, "  booking {}", quote(&booking.to_string()));
            }
            metadata_to(out, "  ", &directive.metadata);
        }
        DirectiveContent::Close(close) => {
            let _ = writeln!(out, "  account {}", close.account);
            metadata_to(out, "  ", &directive.metadata);
        }
        DirectiveContent::Balance(balance) => {
            let _ = writeln!(out, "  account {}", balance.account);
            let _ = writeln!(out, "  amount {}", amount(&balance.amount));
            if let Some(tolerance) = &balance.tolerance {
                let _ = writeln!(out, "  tolerance {tolerance}");
            }
            metadata_to(out, "  ", &directive.metadata);
        }
        DirectiveContent::Pad(pad) => {
            let _ = writeln!(out, "  account {}", pad.account);
            let _ = writeln!(out, "  source {}", pad.source_account);
            metadata_to(out, "  ", &directive.metadata);
        }
        DirectiveContent::Price(price) => {
            let _ = writeln!(out, "  currency {}", price.currency);
            let _ = writeln!(out, "  amount {}", amount(&price.amount));
            metadata_to(out, "  ", &directive.metadata);
        }
        DirectiveContent::Commodity(currency) => {
            let _ = writeln!(out, "  currency {currency}");
            metadata_to(out, "  ", &directive.metadata);
        }
        DirectiveContent::Event(event) => {
            let _ = writeln!(out, "  name {}", quote(&event.name));
            let _ = writeln!(out, "  value {}", quote(&event.value));
            metadata_to(out, "  ", &directive.metadata);
        }
        _ => {
            metadata_to(out, "  ", &directive.metadata);
        }
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
