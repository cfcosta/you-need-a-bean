use bean_core::{loader::load, model::Ledger};
use std::path::PathBuf;

fn ledger() -> Ledger {
    Ledger::build(
        load(
            &PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures/validation/main.beancount"),
        )
        .unwrap(),
    )
}

#[test]
fn unsupported_accounting_is_never_silently_accepted() {
    let l = ledger();
    assert!(
        l.warnings.iter().any(|w| w.contains("plugin")),
        "{:?}",
        l.warnings
    );
    assert!(
        l.warnings.iter().any(|w| w.contains("pad")),
        "{:?}",
        l.warnings
    );
}

#[test]
fn failed_balance_assertion_names_the_account_and_difference() {
    let l = ledger();
    assert!(
        l.warnings
            .iter()
            .any(|w| w.contains("Assets:Cash") && w.contains("899")),
        "{:?}",
        l.warnings
    );
}

#[test]
fn unbalanced_and_ambiguous_transactions_are_reported() {
    let l = ledger();
    assert!(
        l.warnings.iter().any(|w| w.contains("unbalanced")),
        "{:?}",
        l.warnings
    );
    assert!(
        l.warnings.iter().any(|w| w.contains("multiple omitted")),
        "{:?}",
        l.warnings
    );
}

#[test]
fn future_pay_is_not_cash_available_today() {
    let l = Ledger::build(
        load(
            &PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures/validation/future.beancount"),
        )
        .unwrap(),
    );
    let reports = l.reports_view((2026, 9, 7), 6, "USD");
    assert_eq!(reports.runway.liquid.to_string(), "100");
    let debts = l.liabilities_view((2026, 9, 7), 6, "USD");
    assert_eq!(debts.extra.now.to_string(), "100");
}
