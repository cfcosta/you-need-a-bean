//! Every ledger under examples/ must load, build, and answer the queries
//! the UI makes — this is the app's compatibility contract with real
//! beancount files.

use std::path::PathBuf;

use bean_core::loader::load;
use bean_core::model::Ledger;

/// Top-level example ledgers plus the entry files of multi-file examples.
fn corpus() -> Vec<PathBuf> {
    let examples =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples");
    let mut found: Vec<PathBuf> = std::fs::read_dir(&examples)
        .expect("examples/ exists")
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "beancount"))
        .collect();
    found.push(examples.join("multi/main.beancount"));
    found.sort();
    found
}

#[test]
fn every_example_loads_and_serves_a_month_view() {
    let corpus = corpus();
    assert!(corpus.len() >= 5, "corpus too small: {corpus:?}");

    for path in corpus {
        let loaded = load(&path).unwrap_or_else(|e| panic!("{path:?}: {e}"));
        assert!(
            loaded.warnings.is_empty(),
            "{path:?} warned: {:?}",
            loaded.warnings
        );
        let ledger = Ledger::build(loaded);
        assert!(ledger.directives > 0, "{path:?} is empty");

        let today = (2026, 8, 21);
        let month = ledger.default_month(today);
        let view = ledger.month_view(
            month,
            6,
            ledger
                .operating_currencies
                .first()
                .map_or("USD", |c| c.as_str()),
        );
        // Digging into every category must never panic either.
        for group in &view.groups {
            for cat in &group.categories {
                ledger.category_view(
                    &cat.account,
                    month,
                    6,
                    ledger
                        .operating_currencies
                        .first()
                        .map_or("USD", |c| c.as_str()),
                );
            }
        }
    }
}

#[test]
fn the_multi_file_example_follows_its_includes() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/multi/main.beancount");
    let loaded = load(&root).expect("multi example loads");
    assert!(
        loaded.files.len() >= 5,
        "expected several included files, got {:?}",
        loaded.files
    );

    let ledger = Ledger::build(loaded);
    assert_eq!(ledger.title.as_deref(), Some("Multi-File Household"));
    assert_eq!(ledger.operating_currencies, vec!["USD", "BRL"]);

    // Data from a glob-included month file is visible in the model.
    let view = ledger.month_view(
        bean_core::model::MonthKey::parse("2026-07").unwrap(),
        3,
        "USD",
    );
    assert!(view.spent > rust_decimal::Decimal::ZERO);
    assert!(
        view.groups.iter().any(|g| g.name == "Food"),
        "Food group missing: {:?}",
        view.groups.iter().map(|g| &g.name).collect::<Vec<_>>()
    );
}
