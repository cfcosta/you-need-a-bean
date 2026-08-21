use std::path::PathBuf;

use bean_core::loader::{LoadError, load};

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
fn warns_when_posting_like_lines_are_dropped() {
    // The parser skips lines it cannot read instead of failing; money must
    // never disappear silently, so the loader flags them.
    let ledger = load(&fixture("skipped/main.beancount")).unwrap();
    assert_eq!(ledger.warnings.len(), 1, "warnings: {:?}", ledger.warnings);
    let warning = &ledger.warnings[0];
    assert!(warning.contains("main.beancount"), "{warning}");
    assert!(warning.contains("2 posting"), "{warning}");
    assert!(warning.contains("line 7"), "{warning}");
    assert!(warning.contains("Expenses:Stuff"), "{warning}");

    // The intact transaction still loads normally.
    assert_eq!(ledger.directives.len(), 4);
}

#[test]
fn clean_ledgers_load_without_posting_warnings() {
    let ledger = load(&fixture("model/main.beancount")).unwrap();
    assert!(
        ledger.warnings.is_empty(),
        "warnings: {:?}",
        ledger.warnings
    );
}

#[test]
fn missing_include_is_an_error() {
    let err = load(&fixture("broken/missing.beancount")).unwrap_err();
    match err {
        LoadError::Io { path, .. } => {
            assert!(
                path.to_string_lossy().contains("does-not-exist.beancount")
            );
        }
        other => panic!("expected Io error, got {other:?}"),
    }
}

#[test]
fn options_collect_in_load_order_and_first_wins() {
    let ledger = load(&fixture("includes/main.beancount")).unwrap();
    assert_eq!(ledger.title(), Some("Multi-file"));
    assert_eq!(ledger.operating_currencies(), vec!["USD", "EUR"]);
}
