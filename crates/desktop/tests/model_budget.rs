//! August 2026 on the budget page, as the canvas drew it.

mod common;

use bean_core::{model::MonthKey, query::Status};
use bean_desktop::model::budget::{Budget, Line};
use common::{TODAY, d, overview_ledger};

fn august() -> Budget {
    Budget::build(
        &overview_ledger(),
        TODAY,
        MonthKey::new(2026, 8),
        6,
        "USD",
        Some("Expenses:Food:Groceries"),
    )
}

#[test]
fn the_month_is_income_minus_spent() {
    let b = august();
    assert_eq!(b.month, MonthKey::new(2026, 8));
    assert!(b.closed);
    assert_eq!(b.income, d("6200"));
    assert_eq!(b.spent, d("2145.87"));
    assert_eq!(b.kept(), d("4054.13"));
    assert_eq!(b.typical, Some(d("2127.47")));
}

#[test]
fn groups_with_one_category_collapse_into_one_line() {
    let lines = august().lines;
    let names: Vec<_> = lines
        .iter()
        .map(|l| (l.depth, l.name.as_str(), l.label.as_str()))
        .collect();
    assert_eq!(
        names,
        [
            (0, "Home", ""),
            (1, "Rent", "A place to call home"),
            (1, "Utilities", "Lights & warmth"),
            (0, "Food", ""),
            (1, "Groceries", "Good food"),
            (1, "Coffee", "Coffee with friends"),
            (0, "Interest", "Loan interest"),
            (0, "Subscriptions:Music", "A soundtrack for life"),
            (0, "Travel", "Places to go"),
            (0, "Uncategorized", "A receipt to remember"),
        ]
    );
}

#[test]
fn each_line_carries_typical_spent_and_status() {
    let lines = august().lines;
    let line = |name: &str| -> &Line {
        lines.iter().find(|l| l.name == name).unwrap()
    };
    let food = line("Food");
    assert_eq!((food.typical, food.spent), (Some(d("492")), d("513")));
    assert_eq!(food.status, Some(Status::Over));
    let groceries = line("Groceries");
    assert_eq!(
        groceries.account.as_deref(),
        Some("Expenses:Food:Groceries")
    );
    assert_eq!(groceries.ratio.unwrap().round_dp(4), d("1.0404"));
    assert_eq!(line("Home").status, Some(Status::Warn));
    assert_eq!(line("Interest").status, Some(Status::Warn));
    assert_eq!(line("Travel").ratio, None);
}

#[test]
fn the_inspector_shows_the_selected_category() {
    let i = august().inspector.expect("groceries is selected");
    assert_eq!(i.account, "Expenses:Food:Groceries");
    assert_eq!(i.label, "Good food");
    assert_eq!((i.spent, i.typical), (d("451"), Some(d("433.5"))));
    let months: Vec<_> = i.history.iter().map(|h| h.0).collect();
    assert_eq!(months.first(), Some(&MonthKey::new(2026, 3)));
    assert_eq!(months.last(), Some(&MonthKey::new(2026, 8)));
    assert_eq!(i.history.last().unwrap().1, d("451"));
    let txns: Vec<_> = i
        .txns
        .iter()
        .map(|t| (t.date, t.payee.as_str(), t.amount))
        .collect();
    assert_eq!(
        txns,
        [
            ((2026, 8, 3), "Green Basket", d("242")),
            ((2026, 8, 17), "Green Basket", d("209"))
        ]
    );
}

#[test]
fn accounts_add_up_to_the_budget_total() {
    let b = august();
    assert_eq!(b.accounts.len(), 5);
    assert_eq!(b.accounts[0].label, "A little breathing room");
    assert_eq!(b.budget_total, d("77078.31"));
    assert_eq!(b.tracking[0].label, "Long-term portfolio");
    assert_eq!(b.tracking[0].native, "420 BEAN");
}
