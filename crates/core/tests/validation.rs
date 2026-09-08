use bean_core::{loader::load, model::Ledger};
use std::path::PathBuf;

#[test]
fn maximum_precision_loads_and_normal_rounding_still_passes() {
    let l = Ledger::build(
        load(
            &PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures/validation/precision.beancount"),
        )
        .unwrap(),
    );
    assert!(l.audit.issues.is_empty(), "{:?}", l.audit.issues);
    assert_eq!(l.audit.balances.len(), 2);
    assert!(l.audit.balances.iter().all(|b| b.passed));
}

#[test]
fn maximum_precision_still_detects_the_smallest_representable_mismatch() {
    let l = Ledger::build(
        load(
            &PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures/validation/precision-mismatch.beancount"),
        )
        .unwrap(),
    );
    let codes: Vec<_> =
        l.audit.issues.iter().map(|i| i.code.as_str()).collect();
    assert_eq!(codes, ["unbalanced", "balance"]);
    assert_eq!(l.audit.balances.len(), 1);
    assert!(!l.audit.balances[0].passed);
}

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

#[test]
fn cash_includes_every_account_owner_but_excludes_nonliquid_assets() {
    let l = Ledger::build(
        load(
            &PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures/validation/purpose.beancount"),
        )
        .unwrap(),
    );
    let reports = l.reports_view((2026, 9, 7), 6, "USD");
    assert_eq!(reports.runway.liquid.to_string(), "20075");
    let debts = l.liabilities_view((2026, 9, 7), 6, "USD");
    assert_eq!(debts.extra.now.to_string(), "20075");
}

#[test]
fn retirement_capital_includes_all_eligible_accounts() {
    let l = Ledger::build(
        load(
            &PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures/validation/purpose.beancount"),
        )
        .unwrap(),
    );
    assert_eq!(
        l.reports_view((2026, 9, 7), 6, "USD")
            .fire
            .net_worth
            .to_string(),
        "20075"
    );
}

#[test]
fn invalid_financial_metadata_is_reported_instead_of_ignored() {
    let l = ledger();
    assert!(
        l.audit
            .issues
            .iter()
            .filter(|i| i.code == "metadata")
            .count()
            == 3
    );
    assert!(!l.audit.issues.iter().any(|i| i.message.contains("scope")));
    let plugin = l
        .audit
        .issues
        .iter()
        .find(|i| i.message.contains("plugin"))
        .unwrap();
    assert_eq!(plugin.source.as_ref().unwrap().line, 2);
}

#[test]
fn planning_uses_income_and_spending_from_the_whole_ledger() {
    let l = Ledger::build(
        load(
            &PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures/validation/whole-ledger.beancount"),
        )
        .unwrap(),
    );
    assert!(l.audit.issues.is_empty(), "{:?}", l.audit.issues);
    let r = l.reports_view((2026, 2, 1), 1, "USD");
    assert_eq!(r.fire.monthly_spend.to_string(), "400");
    assert_eq!(r.fire.monthly_savings.to_string(), "2600");
    assert_eq!(r.fire.net_worth.to_string(), "2600");
}
