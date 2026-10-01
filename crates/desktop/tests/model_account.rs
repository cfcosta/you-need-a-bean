//! The Everyday account's register for September 2026.

mod common;

use bean_core::model::MonthKey;
use bean_desktop::model::account::Register;
use common::{TODAY, d, overview_ledger};

fn september() -> Register {
    Register::build(
        &overview_ledger(),
        TODAY,
        "Assets:Bank:Everyday",
        MonthKey::new(2026, 9),
        6,
        "USD",
    )
    .expect("the account exists")
}

#[test]
fn the_balance_is_opening_plus_in_minus_out() {
    let r = september();
    assert_eq!(r.label, "Everyday account");
    assert_eq!(r.kind, "budget");
    assert_eq!(r.opening, d("53858"));
    assert_eq!((r.inflow, r.ins), (d("6200"), 1));
    assert_eq!((r.outflow, r.outs), (d("2073.42"), 4));
    assert_eq!(r.balance, d("57984.58"));
}

#[test]
fn six_months_of_flows() {
    let r = september();
    let months: Vec<_> = r.history.iter().map(|h| h.month).collect();
    assert_eq!(months.first(), Some(&MonthKey::new(2026, 4)));
    assert_eq!(months.len(), 6);
    let april = &r.history[0];
    assert_eq!(
        (april.inflow, april.outflow, april.balance),
        (d("6200"), d("2714"), Some(d("39980")))
    );
}

#[test]
fn entries_name_the_other_leg_and_carry_a_running_balance() {
    let r = september();
    let rows: Vec<_> = r
        .entries
        .iter()
        .map(|e| {
            (
                e.date,
                e.flag,
                e.payee.as_str(),
                e.narration.as_str(),
                e.other.as_str(),
                e.amount,
                e.balance,
            )
        })
        .collect();
    assert_eq!(
        rows[0],
        (
            (2026, 9, 1),
            '*',
            "Willow House",
            "",
            "Expenses:Home:Rent",
            d("-1450"),
            Some(d("52408"))
        )
    );
    assert_eq!(rows[2].3, "Monthly salary");
    assert_eq!(
        rows[3],
        (
            (2026, 9, 5),
            '*',
            "",
            "Room for tomorrow",
            "Assets:Bank:Emergency",
            d("-300"),
            Some(d("58063"))
        )
    );
    assert_eq!(rows[4].1, '!');
    assert_eq!(rows.len(), 5);
}
