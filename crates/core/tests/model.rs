use std::path::PathBuf;

use bean_core::loader::load;
use bean_core::model::{AccountKind, Ledger, MonthKey};
use rust_decimal::Decimal;

fn dec(s: &str) -> Decimal {
    s.parse().unwrap()
}

fn ledger() -> Ledger {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/model/main.beancount");
    Ledger::build(load(&path).unwrap())
}

fn m(s: &str) -> MonthKey {
    MonthKey::parse(s).unwrap()
}

#[test]
fn month_key_parses_formats_and_orders() {
    assert_eq!(m("2026-08"), MonthKey::new(2026, 8));
    assert_eq!(m("2026-08").to_string(), "2026-08");
    assert!(MonthKey::parse("2026-13").is_none());
    assert!(MonthKey::parse("2026-0").is_none());
    assert!(MonthKey::parse("garbage").is_none());
    assert!(m("2025-12") < m("2026-01"));
    assert_eq!(m("2025-12").next(), m("2026-01"));
    assert_eq!(m("2026-02").days_in_month(), 28);
    assert_eq!(m("2024-02").days_in_month(), 29);
    assert_eq!(m("2026-04").days_in_month(), 30);
    assert_eq!(m("2026-01").days_in_month(), 31);
}

#[test]
fn derives_category_groups_and_labels() {
    let ledger = ledger();

    let groceries = ledger.account("Expenses:Food:Groceries").unwrap();
    assert_eq!(groceries.group.as_deref(), Some("Food"));
    assert_eq!(groceries.label, "Groceries");

    let coffee = ledger.account("Expenses:Food:Dining:Coffee").unwrap();
    assert_eq!(coffee.group.as_deref(), Some("Food"));
    assert_eq!(coffee.label, "Dining · Coffee");

    let vacation = ledger.account("Expenses:Vacation").unwrap();
    assert_eq!(vacation.group.as_deref(), Some("Vacation"));
    assert_eq!(vacation.label, "Vacation");

    // name: metadata on the open directive wins over the fallback.
    let rent = ledger.account("Expenses:Home:Rent").unwrap();
    assert_eq!(rent.label, "Monthly Rent");
}

#[test]
fn labels_assets_with_metadata_and_fallbacks() {
    let ledger = ledger();
    let checking = ledger.account("Assets:US:BofA:Checking").unwrap();
    assert_eq!(checking.label, "Checking");
    assert!(checking.group.is_none());

    // Depth > 2 without metadata: last two segments joined with a space.
    let slate = ledger.account("Liabilities:US:Chase:Slate").unwrap();
    assert_eq!(slate.label, "Chase Slate");

    // Depth 2: just the last segment.
    let cash = ledger.account("Assets:Cash").unwrap();
    assert_eq!(cash.label, "Cash");
}

#[test]
fn classifies_budget_tracking_and_hidden() {
    let ledger = ledger();
    let kind = |name: &str| ledger.account(name).unwrap().kind;

    assert_eq!(kind("Assets:Cash"), AccountKind::Budget);
    assert_eq!(kind("Liabilities:US:Chase:Slate"), AccountKind::Budget);
    // Holds a non-operating commodity (VEA / VACHR) → tracking.
    assert_eq!(kind("Assets:ETrade:VEA"), AccountKind::Tracking);
    assert_eq!(kind("Assets:Points"), AccountKind::Tracking);
    // ynab: metadata overrides both ways.
    assert_eq!(kind("Assets:Vault"), AccountKind::Tracking);
    assert_eq!(kind("Assets:Old"), AccountKind::Hidden);
}

#[test]
fn sums_postings_per_account_month_and_currency() {
    let ledger = ledger();
    let sum = |account: &str, month: &str, cur: &str| {
        ledger.sum(account, m(month), cur)
    };

    assert_eq!(sum("Expenses:Food:Groceries", "2026-01", "USD"), dec("100"));
    // Refunds subtract.
    assert_eq!(sum("Expenses:Food:Groceries", "2026-02", "USD"), dec("30"));
    // Native currency of the posting, not the operating currency.
    assert_eq!(
        sum("Expenses:Food:Dining:Coffee", "2026-01", "BRL"),
        dec("30")
    );
    assert_eq!(sum("Expenses:Home:Rent", "2026-02", "USD"), dec("500"));
    assert_eq!(sum("Income:Salary", "2026-01", "USD"), dec("-1000"));
    assert_eq!(sum("Assets:ETrade:VEA", "2026-01", "VEA"), dec("2"));

    // Elided postings absorb the residual: -40 -60 (groceries), -6 (coffee
    // paid at @@ price), -200 (VEA bought at cost).
    assert_eq!(sum("Assets:Cash", "2026-01", "USD"), dec("-306"));
    // Elided residual in a non-operating commodity.
    assert_eq!(sum("Assets:Points", "2026-03", "VACHR"), dec("-3"));
    // Absent (account, month) pairs are zero.
    assert_eq!(sum("Assets:Cash", "2027-05", "USD"), dec("0"));
}

#[test]
fn tracks_first_and_last_transaction_months() {
    let ledger = ledger();
    assert_eq!(ledger.first_txn_month, Some(m("2025-12")));
    assert_eq!(ledger.last_txn_month, Some(m("2026-03")));
}

#[test]
fn lists_transactions_per_category_month() {
    let ledger = ledger();
    let txns = ledger.txns("Expenses:Food:Groceries", m("2026-01"));
    assert_eq!(txns.len(), 2);
    // Sorted by date.
    assert_eq!(txns[0].date, (2026, 1, 3));
    assert_eq!(txns[0].payee.as_deref(), Some("SuperMart"));
    assert_eq!(txns[0].narration.as_deref(), Some("Weekly shop"));
    assert_eq!(txns[0].flag, '*');

    let topup = txns[1];
    assert_eq!(topup.tags, vec!["food"]);
    assert_eq!(topup.links, vec!["trip-1"]);
    assert_eq!(
        topup.meta,
        vec![("note".to_string(), "with coupons".to_string())]
    );
    assert_eq!(topup.postings.len(), 2);
    assert_eq!(topup.postings[0].account, "Expenses:Food:Groceries");
    assert_eq!(topup.postings[0].amounts, vec![(dec("60"), "USD".into())]);
    // The elided posting carries its resolved residual.
    assert_eq!(topup.postings[1].account, "Assets:Cash");
    assert_eq!(topup.postings[1].amounts, vec![(dec("-60"), "USD".into())]);

    assert!(
        ledger
            .txns("Expenses:Food:Groceries", m("2027-05"))
            .is_empty()
    );
}

#[test]
fn parses_amounts_written_with_a_unary_plus() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/plus/main.beancount");
    let ledger = Ledger::build(load(&path).unwrap());

    // `+123.45 USD` is valid beancount; the posting must not be dropped.
    assert_eq!(ledger.sum("Assets:A", m("2030-02"), "USD"), dec("-118.45"));
    assert_eq!(ledger.sum("Assets:B", m("2030-02"), "USD"), dec("133.45"));
    // Elided legs still absorb the residual of `+`-signed amounts.
    assert_eq!(ledger.sum("Assets:C", m("2030-02"), "USD"), dec("-15"));
}

#[test]
fn weighs_total_cost_postings_and_prefers_cost_over_price() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/total-cost/main.beancount");
    let loaded = load(&path).unwrap();
    assert!(
        loaded.warnings.is_empty(),
        "warnings: {:?}",
        loaded.warnings
    );
    let ledger = Ledger::build(loaded);

    // `{{ 700.00 USD }}` is a total cost; the posting must parse and its
    // elided leg must absorb the lump sum, not quantity × total.
    assert_eq!(
        ledger.sum("Assets:Investments:ExampleBroker", m("2030-03"), "MOCK"),
        dec("7")
    );
    // When a posting carries both `{cost}` and `@ price`, beancount weighs
    // it by the cost: 2 × 5.00, not 2 × 6.00. Residual: -700.00 - 10.00.
    assert_eq!(
        ledger.sum("Assets:Cash:ExampleBroker", m("2030-03"), "USD"),
        dec("-710.00")
    );
}

#[test]
fn converts_with_direct_inverse_and_pivot_rates() {
    let ledger = ledger();
    let end_jan = (2026, 1, 31);

    // Same currency is identity.
    assert_eq!(
        ledger.convert(dec("7"), "USD", "USD", end_jan),
        Some(dec("7"))
    );
    // Direct rate, latest on or before the date.
    assert_eq!(
        ledger.convert(dec("100"), "EUR", "USD", end_jan),
        Some(dec("110"))
    );
    assert_eq!(
        ledger.convert(dec("100"), "EUR", "USD", (2026, 1, 5)),
        Some(dec("105"))
    );
    // Inverse rate (division, so exact).
    assert_eq!(
        ledger.convert(dec("11"), "USD", "EUR", end_jan),
        Some(dec("10"))
    );
    // One-hop pivot through the operating currency: BRL → USD → EUR.
    assert_eq!(
        ledger.convert(dec("55"), "BRL", "EUR", end_jan),
        Some(dec("10"))
    );
    // Pivot with two direct legs: EUR → USD → BRL.
    assert_eq!(
        ledger.convert(dec("2"), "EUR", "BRL", end_jan),
        Some(dec("11.00"))
    );
    // No price known yet at that date.
    assert_eq!(ledger.convert(dec("1"), "EUR", "USD", (2026, 1, 1)), None);
    // No price at all.
    assert_eq!(
        ledger.convert(dec("5"), "VACHR", "USD", (2026, 3, 31)),
        None
    );
}
