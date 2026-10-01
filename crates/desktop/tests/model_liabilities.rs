//! The car loan on the liabilities page.

mod common;

use bean_core::model::MonthKey;
use bean_desktop::model::liabilities::Liabilities;
use common::{TODAY, d, overview_ledger};

fn debts() -> Liabilities {
    Liabilities::build(&overview_ledger(), TODAY, 6, "USD")
}

#[test]
fn owed_is_peak_minus_principal_paid() {
    let l = debts();
    let car = &l.view.debts[0];
    assert_eq!(car.label, "The little blue car");
    assert_eq!(car.peak, d("12000"));
    assert_eq!(car.principal_paid, d("2320.31"));
    assert_eq!(car.owed, d("9679.69"));
    assert_eq!(car.peak - car.principal_paid, car.owed);
}

#[test]
fn what_debt_costs_and_how_well_cash_covers_it() {
    let l = debts();
    assert_eq!(l.cost_month().round_dp(2), d("48.40"));
    assert_eq!(l.covered().unwrap().round_dp(1), d("7.9"));
    assert_eq!(l.view.debt_free, Some(MonthKey::new(2029, 5)));
    assert_eq!(l.view.interest_year, d("439.69"));
}

#[test]
fn the_slider_reworks_the_payoff() {
    let l = debts();
    let car = &l.view.debts[0];
    assert_eq!(
        l.payoff(car, d("0")),
        Some((MonthKey::new(2029, 5), d("776.24")))
    );
    let faster = l.payoff(car, d("100")).unwrap();
    assert!(faster.0 < MonthKey::new(2029, 5));
    assert!(faster.1 < d("776.24"));
}

#[test]
fn the_projection_runs_from_today_to_zero() {
    let l = debts();
    let car = &l.view.debts[0];
    let path = l.projection(car, d("0"));
    assert_eq!(path.len(), 31);
    assert_eq!(path.last().copied(), Some(d("0")));
    assert!(path[0] < car.owed);
}
