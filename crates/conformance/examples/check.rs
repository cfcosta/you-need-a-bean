//! Point the parser at real ledgers and report what it made of them.
//!
//! ```text
//! cargo run -p you-need-a-bean-conformance --example check -- examples ~/example-ledger
//! ```
//!
//! Each argument is a file or a directory to search for `*.beancount`. For
//! every file this prints one line: either the count of each entry kind, or
//! the line the parse stopped on and the text of that line.
//!
//! This is the check the corpus cannot do. The corpus pins behaviour on inputs
//! we thought to write down; this says whether a parser handles the ledger
//! someone actually has, which is the only question that matters when swapping
//! one implementation for another. Run it before and after a rewrite and
//! compare: the totals should be identical, and no file should regress into an
//! error.

use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    process::ExitCode,
};

use beancount_parser::{DirectiveContent, Entry, parse_iter};
use rust_decimal::Decimal;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        eprintln!("usage: check <file-or-directory>...");
        return ExitCode::FAILURE;
    }

    let mut totals: BTreeMap<&'static str, usize> = BTreeMap::new();
    let mut files = 0usize;
    let mut failed = 0usize;

    for arg in &args {
        for path in ledgers(Path::new(arg)) {
            files += 1;
            let text = match std::fs::read_to_string(&path) {
                Ok(text) => text,
                Err(err) => {
                    failed += 1;
                    println!("{}: cannot read: {err}", path.display());
                    continue;
                }
            };
            match count(&text) {
                Ok(counts) => {
                    for (kind, n) in counts {
                        *totals.entry(kind).or_default() += n;
                    }
                }
                Err(line_number) => {
                    failed += 1;
                    let line = text
                        .lines()
                        .nth(line_number.saturating_sub(1) as usize)
                        .unwrap_or("")
                        .trim();
                    println!(
                        "{}:{line_number}: parse stopped here: {line}",
                        path.display()
                    );
                }
            }
        }
    }

    println!("\n{files} file(s), {failed} with errors");
    for (kind, n) in &totals {
        println!("{n:>8}  {kind}");
    }

    if failed > 0 {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

/// Every entry kind, counted. `Err(line)` if the parse stopped early.
fn count(text: &str) -> Result<BTreeMap<&'static str, usize>, u32> {
    let mut counts = BTreeMap::new();
    for entry in parse_iter::<Decimal>(text) {
        let kind = match entry.map_err(|err| err.line_number())? {
            Entry::Option(_) => "option",
            Entry::Include(_) => "include",
            Entry::Plugin(_) => "plugin",
            Entry::Directive(directive) => match directive.content {
                DirectiveContent::Transaction(txn) => {
                    *counts.entry("posting").or_default() += txn.postings.len();
                    "transaction"
                }
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
                _ => "unknown-directive",
            },
            _ => "unknown-entry",
        };
        *counts.entry(kind).or_default() += 1;
    }
    Ok(counts)
}

fn ledgers(path: &Path) -> Vec<PathBuf> {
    if path.is_file() {
        return vec![path.to_path_buf()];
    }
    let mut found = Vec::new();
    let Ok(entries) = std::fs::read_dir(path) else {
        return found;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            found.extend(ledgers(&path));
        } else if path.extension().is_some_and(|e| e == "beancount") {
            found.push(path);
        }
    }
    found.sort();
    found
}
