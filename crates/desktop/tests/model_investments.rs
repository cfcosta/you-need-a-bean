//! The investments page, from the example ledger's one fund.

mod common;

use bean_desktop::model::investments::{Investments, Range, Sort};
use common::{TODAY, d, overview_ledger};

fn ytd() -> Investments {
    Investments::build(&overview_ledger(), TODAY, "USD", Range::Ytd)
}

#[test]
fn value_minus_cost_is_the_gain() {
    let i = ytd();
    assert_eq!(i.value, d("53760"));
    assert_eq!(i.basis, d("42000"));
    assert_eq!(i.gain, d("11760"));
    assert_eq!(i.ret.unwrap().round_dp(2), d("0.28"));
    assert_eq!(i.coverage.unwrap(), d("1"));
}

#[test]
fn positions_and_classes() {
    let i = ytd();
    let p = &i.positions[0];
    assert_eq!(
        (p.currency.as_str(), p.units, p.price),
        ("BEAN", d("420"), d("128"))
    );
    assert_eq!(p.price_date, Some((2026, 9, 7)));
    assert_eq!(i.classes[0].name.as_deref(), Some("Global equities"));
    assert_eq!(i.classes[0].share, d("1"));
}

#[test]
fn ranges_start_where_they_say() {
    assert_eq!(Range::Ytd.start(TODAY, None), (2025, 12, 31));
    assert_eq!(Range::Months(3).start(TODAY, None), (2026, 7, 1));
    assert_eq!(Range::Months(12).start(TODAY, None), (2025, 10, 1));
    let first = bean_core::model::MonthKey::new(2025, 12);
    assert_eq!(Range::All.start(TODAY, Some(first)), (2025, 11, 30));
}

#[test]
fn performance_is_opening_plus_flows_plus_gain() {
    let p = ytd().performance;
    assert_eq!(p.opening, Some(d("42000")));
    assert_eq!(p.net_flows, Some(d("0")));
    assert_eq!(p.gain, Some(d("11760")));
    assert_eq!(p.closing, Some(d("53760")));
    assert_eq!(p.points.len(), 11);
}

#[test]
fn holdings_filter_and_sort() {
    let i = ytd();
    assert_eq!(i.holdings("bean", Sort::Value).len(), 1);
    assert_eq!(
        i.holdings("broad", Sort::Value).len(),
        1,
        "matches the label too"
    );
    assert_eq!(
        i.holdings("index", Sort::Value).len(),
        1,
        "and the accounts it sits in"
    );
    assert!(i.holdings("cash", Sort::Value).is_empty());
}
