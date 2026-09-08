use bean_core::{loader::load, model::Ledger};
use rust_decimal::Decimal;
use std::path::PathBuf;
fn dec(s: &str) -> Decimal {
    s.parse().unwrap()
}
fn ledger() -> Ledger {
    Ledger::build(
        load(
            &PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures/reports/performance.beancount"),
        )
        .unwrap(),
    )
}
#[test]
fn performance_separates_additions_from_growth_and_ignores_internal_transfers()
{
    let l = ledger();
    let flat = l.investment_performance((2025, 12, 31), (2026, 1, 16), "USD");
    assert_eq!(flat.opening, Some(dec("1000")));
    assert_eq!(flat.closing, Some(dec("2000")));
    assert_eq!(flat.net_flows, Some(dec("1000")));
    assert_eq!(flat.gain, Some(Decimal::ZERO));
    assert_eq!(flat.ret, Some(Decimal::ZERO));
    let p = l.investment_performance((2025, 12, 31), (2026, 1, 31), "USD");
    assert_eq!(p.closing, Some(dec("1800")));
    assert_eq!(p.net_flows, Some(dec("375")));
    assert_eq!(p.gain, Some(dec("425")));
    // End-of-day flows: 20/31 days for the purchase, 10/31 for the sale.
    let expected = dec("425")
        / (dec("1000") + dec("1000") * dec("20") / dec("31")
            - dec("625") * dec("10") / dec("31"));
    assert!((p.ret.unwrap() - expected).abs() < dec("0.0000001"));
    assert_eq!(p.holdings[0].gain, p.gain);
    assert_eq!(p.points.first().unwrap().value, p.opening);
    assert_eq!(p.points.last().unwrap().value, p.closing);
}
#[test]
fn performance_keeps_sold_holdings_and_uses_sale_prices_instead_of_old_cost() {
    let p =
        ledger().investment_performance((2025, 12, 31), (2026, 2, 28), "USD");
    assert_eq!(p.closing, Some(Decimal::ZERO));
    assert_eq!(p.net_flows, Some(dec("-1575")));
    assert_eq!(p.gain, Some(dec("575")));
    assert_eq!(p.holdings.len(), 1);
    assert_eq!(p.holdings[0].closing, Some(Decimal::ZERO));
}
#[test]
fn performance_does_not_use_future_quotes_or_invent_zero_capital_returns() {
    let p =
        ledger().investment_performance((2025, 12, 30), (2025, 12, 31), "USD");
    assert_eq!(p.gain, Some(Decimal::ZERO));
    assert_eq!(p.ret, None); // opening zero, acquisition on final day
    let p =
        ledger().investment_performance((2026, 1, 31), (2026, 1, 31), "USD");
    assert_eq!(p.ret, None);
}
#[test]
fn performance_withholds_totals_when_historical_prices_are_missing() {
    let mut l = ledger();
    // No FUND quote can be converted to this reporting currency.
    let p = l.investment_performance((2025, 12, 31), (2026, 1, 31), "EUR");
    assert_eq!(p.gain, None);
    assert_eq!(p.ret, None);
    assert_eq!(p.holdings[0].gain, None);
    assert!(!p.issues.is_empty());
    // Unsupported negative inventory is not a positive holding or a valid gain.
    l.txns[0].postings[0].amounts[0].0 = dec("-100");
    let p = l.investment_performance((2025, 12, 31), (2026, 1, 31), "USD");
    assert_eq!(p.gain, None);
    assert!(!p.issues.is_empty());
}
