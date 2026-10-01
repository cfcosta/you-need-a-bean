//! The pages' figures on a ledger with the rough edges a real one has.

mod common;

use common::{TODAY, rough_ledger};

#[test]
fn the_rough_ledger_is_clean() {
    let l = rough_ledger();
    assert!(l.warnings.is_empty(), "{:#?}", l.warnings);
    assert_eq!(l.operating_currencies[0], "BRL");
}

use bean_core::model::MonthKey;
use bean_desktop::{
    fmt::{symbol, units},
    model::{
        budget::Budget,
        investments::{Investments, Range, month_labels},
        liabilities::Liabilities,
        overview::Overview,
        reports::Reports,
    },
};

#[test]
fn money_carries_the_ledgers_own_symbol() {
    assert_eq!(symbol("BRL"), "R$");
    assert_eq!(symbol("USD"), "$");
    assert_eq!(symbol("XYZ"), "XYZ");
}

#[test]
fn units_keep_a_readable_number_of_decimals() {
    let d = |s: &str| s.parse().unwrap();
    assert_eq!(units(d("420")), "420");
    assert_eq!(units(d("0.005537110808728093")), "0.00553711");
    assert_eq!(units(d("0.00011627")), "0.00011627");
    assert_eq!(units(d("12.3456789")), "12.3457");
    assert_eq!(units(d("1234.5")), "1,234.5");
}

#[test]
fn the_overview_charts_a_year_and_lists_review_newest_first() {
    let o = Overview::build(&rough_ledger(), TODAY, "BRL");
    let year = o.recent_history(12);
    assert_eq!(year.len(), 12);
    assert_eq!(year.last().map(|p| p.0), Some(MonthKey::new(2026, 10)));
    let first = &o.review[0];
    assert_eq!(first.date, Some((2026, 7, 14)), "newest flagged first");
    assert!(
        o.review
            .windows(2)
            .all(|w| w[0].date >= w[1].date || w[1].date.is_none())
    );
}

#[test]
fn a_tracking_account_reads_as_its_holdings_not_a_list_of_zeroes() {
    let b = Budget::build(
        &rough_ledger(),
        TODAY,
        MonthKey::new(2026, 9),
        6,
        "BRL",
        None,
    );
    for a in &b.tracking {
        assert!(!a.native.contains(" 0 "), "{}: {}", a.label, a.native);
        assert!(a.native.chars().count() <= 20, "{}: {}", a.label, a.native);
    }
}

#[test]
fn reports_read_the_last_year_not_the_whole_history() {
    let r = Reports::build(&rough_ledger(), TODAY, 6, "BRL");
    let rows = r.cashflow_recent(12);
    assert_eq!(rows.len(), 12);
    assert_eq!(rows.last().map(|c| c.month), Some(MonthKey::new(2026, 10)));
    let moved = r.moved_recent(12);
    assert!(moved.months.len() <= 12);
    assert_eq!(
        moved.months.last().map(|p| p.0),
        Some(MonthKey::new(2026, 9))
    );
}

#[test]
fn performance_points_stay_in_step_with_their_labels() {
    let i = Investments::build(&rough_ledger(), TODAY, "BRL", Range::All);
    let days: Vec<_> = i.performance.points.iter().map(|p| p.date).collect();
    let labels = month_labels(&days);
    assert_eq!(labels.len(), days.len());
    // A month is named once, where it starts.
    let named: Vec<_> = labels.iter().filter(|l| !l.is_empty()).collect();
    let mut unique = named.clone();
    unique.dedup();
    assert_eq!(named, unique);
}

#[test]
fn owed_splits_into_loans_and_cards_and_paid_off_debts_step_aside() {
    let l = Liabilities::build(&rough_ledger(), TODAY, 6, "BRL");
    let (loans, cards) = l.owed_parts();
    assert_eq!(loans + cards, l.view.owed);
    assert!(cards.is_zero());
    let open: Vec<_> =
        l.open_debts().iter().map(|d| d.account.as_str()).collect();
    assert_eq!(open, ["Liabilities:Friend"]);
    let friend = l.open_debts()[0];
    assert!(friend.rate.is_none());
    assert!(
        l.payoff(friend, rust_decimal::Decimal::ZERO).is_some(),
        "no rate still pays off"
    );
}

#[test]
fn a_loan_without_a_rate_still_projects_to_zero_from_where_it_began() {
    let l = Liabilities::build(&rough_ledger(), TODAY, 6, "BRL");
    let friend = l.open_debts()[0];
    let path = l.projection(friend, rust_decimal::Decimal::ZERO);
    assert!(!path.is_empty());
    assert!(path.last().unwrap().is_zero());
    let history = l.history(friend, 24);
    assert_eq!(
        history.first().map(|p| p.month),
        Some(MonthKey::new(2026, 6)),
        "no months before the loan"
    );
    assert!(history.len() <= 24);
}

#[test]
fn net_worth_moved_charts_consecutive_months() {
    let r = Reports::build(&rough_ledger(), TODAY, 6, "BRL");
    let months: Vec<MonthKey> =
        r.moved_recent(12).months.iter().map(|p| p.0).collect();
    assert_eq!(months.len(), 12);
    assert!(months.windows(2).all(|w| w[0].next() == w[1]), "{months:?}");
}
