//! The reports page's derived figures, from the example ledger.

mod common;

use bean_core::model::MonthKey;
use bean_desktop::model::reports::{Reports, duration};
use common::{TODAY, d, overview_ledger};

fn reports() -> Reports {
    Reports::build(&overview_ledger(), TODAY, 6, "USD")
}

#[test]
fn durations_read_as_years_and_months() {
    assert_eq!(duration(101), "8y 5m");
    assert_eq!(duration(83), "6y 11m");
    assert_eq!(duration(5), "5m");
    assert_eq!(duration(24), "2y 0m");
}

#[test]
fn scenarios_say_when_independence_arrives() {
    let r = reports();
    let rows: Vec<_> = r
        .scenarios()
        .iter()
        .map(|s| (s.rate_pct, s.reach.clone(), s.around, s.coast.clone()))
        .collect();
    assert_eq!(
        rows[0],
        (
            3,
            "8y 5m".to_string(),
            Some(MonthKey::new(2035, 3)),
            "55y 8m".to_string()
        )
    );
    assert_eq!(rows[1].2, Some(MonthKey::new(2034, 5)));
    assert_eq!(rows[2].1, "6y 11m");
}

#[test]
fn net_worth_moved_is_opening_plus_saved_plus_markets() {
    let r = reports();
    let m = r.moved();
    assert_eq!(m.opening, d("86500"));
    assert_eq!(m.saved, d("37004.89"));
    assert_eq!(m.markets, d("11760"));
    assert_eq!(m.opening + m.saved + m.markets, m.closing);
    assert_eq!(m.closing, d("135264.89"));
    assert_eq!(m.months.first().map(|p| p.0), Some(MonthKey::new(2026, 1)));
    assert_eq!(m.months.last().map(|p| p.0), Some(MonthKey::new(2026, 9)));
    assert_eq!(m.so_far, Some((MonthKey::new(2026, 10), d("-2130"))));
}

#[test]
fn cashflow_rows_start_where_money_starts_moving() {
    let rows = reports().cashflow();
    assert_eq!(rows.first().map(|r| r.month), Some(MonthKey::new(2026, 1)));
    let jan = &rows[0];
    assert_eq!((jan.income, jan.expenses), (d("6200"), d("2111")));
    assert_eq!(jan.rate.unwrap().round_dp(2), d("0.66"));
    let oct = rows.last().unwrap();
    assert_eq!(oct.month, MonthKey::new(2026, 10));
    assert_eq!(oct.rate, None);
}

#[test]
fn doubt_is_the_flagged_and_the_uncategorized() {
    let r = reports();
    assert_eq!(r.doubt(), (d("78.42"), d("78.42")));
}
