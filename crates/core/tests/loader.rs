use std::path::PathBuf;

use bean_core::loader::{LoadError, load};
use miette::Diagnostic;

/// The text an error actually points at, which is what a reader sees
/// underlined. Every assertion below goes through this rather than through the
/// message, because the span is the part that has to be right.
fn underlined(err: &dyn Diagnostic) -> String {
    let src = err
        .source_code()
        .expect("a located error carries its source");
    let span = err.labels().expect("a located error is labelled").next();
    let span = *span.expect("at least one label").inner();
    let contents = src
        .read_span(&span, 0, 0)
        .expect("the span is inside the source");
    String::from_utf8(contents.data().to_vec()).expect("utf-8 source")
}

fn fixture(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(rel)
}

fn file_names(paths: &[PathBuf]) -> Vec<String> {
    paths
        .iter()
        .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
        .collect()
}

#[test]
fn loads_a_standalone_file() {
    let ledger = load(&fixture("simple/main.beancount")).unwrap();
    assert_eq!(ledger.files.len(), 1);
    assert_eq!(ledger.directives.len(), 3);
    assert_eq!(ledger.title(), Some("Simple Ledger"));
    assert_eq!(ledger.operating_currencies(), vec!["USD", "BRL"]);
    assert!(ledger.warnings.is_empty());
}

#[test]
fn follows_includes_recursively_with_dedup_and_cycles() {
    let ledger = load(&fixture("includes/main.beancount")).unwrap();
    // main, accounts, years/2026, months/feb, months/jan, shared — each once,
    // even though accounts is included twice and shared includes main again.
    assert_eq!(ledger.files.len(), 6, "files: {:?}", ledger.files);
    let names = file_names(&ledger.files);
    assert_eq!(names[0], "main.beancount");
    assert!(names.contains(&"shared.beancount".to_string()));
    // 2 opens + 5 transactions across all files.
    assert_eq!(ledger.directives.len(), 7);
}

#[test]
fn expands_glob_includes_in_sorted_order() {
    let ledger = load(&fixture("includes/main.beancount")).unwrap();
    let names = file_names(&ledger.files);
    let feb = names.iter().position(|n| n == "feb.beancount").unwrap();
    let jan = names.iter().position(|n| n == "jan.beancount").unwrap();
    assert!(
        feb < jan,
        "glob matches must load in sorted order: {names:?}"
    );
}

#[test]
fn zero_match_glob_is_a_warning_not_an_error() {
    let ledger = load(&fixture("includes/main.beancount")).unwrap();
    assert_eq!(ledger.warnings.len(), 1, "warnings: {:?}", ledger.warnings);
    assert!(ledger.warnings[0].contains("missing/*.beancount"));
}

#[test]
fn unparseable_posting_is_an_error() {
    // Money must never disappear silently. A posting the parser cannot read
    // used to be skipped along with the rest of its transaction, which the
    // loader could only detect after the fact; it is a syntax error now, and
    // the error names the line.
    let err = load(&fixture("unparseable/main.beancount")).unwrap_err();
    let LoadError::Syntax { .. } = err else {
        panic!("expected Syntax error, got {err:?}");
    };
    assert!(err.path().ends_with("main.beancount"), "{}", err.path());
    // Line 7 is `  Expenses:Stuff  10..0 USD`, and the underline covers the
    // whole of it: the parser knows where it stopped, not how much is wrong.
    assert_eq!(underlined(&err), "Expenses:Stuff  10..0 USD");
}

#[test]
fn clean_ledgers_load_without_warnings() {
    let ledger = load(&fixture("model/main.beancount")).unwrap();
    assert!(
        ledger.warnings.is_empty(),
        "warnings: {:?}",
        ledger.warnings
    );
}

#[test]
fn missing_include_points_at_the_line_that_asked_for_it() {
    // A ledger is hundreds of files deep. Naming the file that could not be
    // read says nothing about which of them wanted it.
    let err = load(&fixture("broken/missing.beancount")).unwrap_err();
    let LoadError::UnreadableInclude { path, .. } = &err else {
        panic!("expected UnreadableInclude, got {err:?}");
    };
    assert!(path.ends_with("does-not-exist.beancount"), "{path}");
    assert!(err.path().ends_with("missing.beancount"), "{}", err.path());
    assert_eq!(underlined(&err), "\"does-not-exist.beancount\"");
}

#[test]
fn options_collect_in_load_order_and_first_wins() {
    let ledger = load(&fixture("includes/main.beancount")).unwrap();
    assert_eq!(ledger.title(), Some("Multi-file"));
    assert_eq!(ledger.operating_currencies(), vec!["USD", "EUR"]);
}
