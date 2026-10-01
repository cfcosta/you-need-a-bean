//! The overview's figures, read from the example ledger on the day the
//! design canvas was drawn.

mod common;

use bean_core::model::MonthKey;
use bean_desktop::model::overview::{Overview, ReviewKind};
use common::{TODAY, d, overview_ledger};

fn overview() -> Overview {
    Overview::build(&overview_ledger(), TODAY, "USD")
}

#[test]
fn position_is_held_minus_reserved() {
    let o = overview();
    assert_eq!(o.held, d("89054.58"));
    assert_eq!(o.reserved, d("12800"));
    assert_eq!(o.free, d("76254.58"));
    assert_eq!(o.out_30, d("407"));
}

#[test]
fn runway_names_the_month_the_cash_runs_out() {
    let r = overview().runway.expect("a runway");
    assert_eq!(r.months, d("36.8"));
    assert_eq!(r.burn, d("2074.73"));
    assert_eq!(r.until, MonthKey::new(2029, 10));
}

#[test]
fn net_worth_is_assets_minus_liabilities_with_its_history() {
    let o = overview();
    assert_eq!(o.assets, d("142814.58"));
    assert_eq!(o.liabilities, d("9679.69"));
    assert_eq!(o.net_worth, d("133134.89"));
    assert_eq!(o.history.len(), 11);
    assert_eq!(o.history[0], (MonthKey::new(2025, 12), d("86500")));
    assert_eq!(o.history[10].1, d("133134.89"));
}

#[test]
fn savings_and_holdings_come_from_the_reports() {
    let o = overview();
    assert_eq!(o.saved_month, d("4125.27"));
    assert_eq!(o.savings_rate.unwrap().round_dp(4), d("0.6654"));
    let h = &o.holdings[0];
    assert_eq!(
        (h.currency.as_str(), h.units, h.value),
        ("BEAN", d("420"), d("53760"))
    );
    assert_eq!(h.ret.unwrap().round_dp(2), d("0.28"));
    let (progress, target) = o.independence.unwrap();
    assert_eq!(progress.round_dp(4), d("0.1933"));
    assert_eq!(target.round(), d("622419"));
}

#[test]
fn the_forecast_has_one_balance_per_day() {
    let o = overview();
    let f = &o.forecast[0];
    assert_eq!(f.days, 30);
    assert_eq!(f.daily.len(), 31);
    assert_eq!(f.daily[0], d("76254.58"));
    assert_eq!(f.daily[3], d("76254.58"));
    assert_eq!(f.daily[4], d("82454.58"));
    assert_eq!(f.daily[30], d("82047.58"));
    assert_eq!(f.end, d("82047.58"));
    assert_eq!(o.forecast[2].days, 90);
    assert_eq!(o.forecast[2].end, d("90733.58"));
}

#[test]
fn events_carry_the_balance_they_leave() {
    let o = overview();
    assert_eq!(o.events.len(), 11);
    let first = &o.events[0];
    assert_eq!(first.date, (2026, 10, 5));
    assert_eq!(first.label, "Northstar Payroll");
    assert_eq!(first.account, "Income:Salary");
    assert_eq!(first.amount, d("6200"));
    assert!(first.scheduled);
    assert_eq!(first.balance, d("82454.58"));
    assert!(!o.events[1].scheduled);
    assert_eq!(o.events[3].balance, d("80597.58"));
    assert_eq!(o.recurring, 6);
}

#[test]
fn goals_review_and_coverage() {
    let o = overview();
    assert_eq!(o.goals.len(), 2);
    assert_eq!(o.goals[0].label, "A little breathing room");
    assert_eq!(
        (o.goals[0].funded, o.goals[0].target),
        (d("21200"), d("24000"))
    );
    assert_eq!(o.goals[0].date, (2027, 6, 1));
    let review: Vec<_> = o
        .review
        .iter()
        .map(|r| (r.label.as_str(), r.kind))
        .collect();
    assert_eq!(
        review,
        [
            ("Corner Market", ReviewKind::Flagged),
            ("Uncategorized spending", ReviewKind::Category)
        ]
    );
    assert_eq!(o.coverage, (0, 6));
}
