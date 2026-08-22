use std::path::PathBuf;

use bean_core::loader::load;
use bean_core::model::{Ledger, MonthKey};
use bean_core::reports::months_to_fire;
use rust_decimal::Decimal;

fn dec(s: &str) -> Decimal {
    s.parse().unwrap()
}

fn ledger() -> Ledger {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/reports/main.beancount");
    Ledger::build(load(&path).unwrap())
}

fn m(s: &str) -> MonthKey {
    MonthKey::parse(s).unwrap()
}

#[test]
fn reports_track_net_worth_and_cashflow_by_month() {
    let ledger = ledger();
    let view = ledger.reports_view((2026, 4, 15), 3, "USD");
    assert_eq!(view.month, m("2026-04"));

    let months: Vec<String> =
        view.net_worth.iter().map(|p| p.month.to_string()).collect();
    assert_eq!(months, vec!["2026-01", "2026-02", "2026-03", "2026-04"]);

    // January: 11,000 cash + 20 VTI at the January price, 1,000 on the
    // card. Equity postings stay out of net worth.
    let jan = &view.net_worth[0];
    assert_eq!(jan.assets, dec("13000.00"));
    assert_eq!(jan.liabilities, dec("-1000.00"));
    assert_eq!(jan.net, dec("12000.00"));
    assert_eq!(view.net_worth[1].net, dec("14000.00"));

    // March picks up the newer VTI price (20 × 110).
    let mar = &view.net_worth[2];
    assert_eq!(mar.assets, dec("16200.00"));
    assert_eq!(mar.liabilities, dec("0"));

    // April has no postings; balances carry forward.
    assert_eq!(view.net_worth[3].net, dec("16200.00"));

    let cf = &view.cashflow;
    assert_eq!(cf[0].income, dec("5000.00"));
    assert_eq!(cf[0].expenses, dec("3000.00"));
    assert_eq!(cf[0].net, dec("2000.00"));
    assert_eq!(cf[2].income, dec("6000.00"));
    assert_eq!(cf[2].expenses, dec("4000.00"));
    assert_eq!(cf[3].income, dec("0"));
    assert_eq!(cf[3].expenses, dec("0"));
}

#[test]
fn year_breakdown_totals_the_trailing_year_by_group() {
    let view = ledger().reports_view((2026, 4, 15), 3, "USD");

    // A fixed twelve-month window clamped to first activity — the
    // display basis (3 here) plays no part.
    assert_eq!(view.year_window, Some((m("2026-01"), m("2026-03"))));

    let rows: Vec<(&str, Decimal)> = view
        .year_groups
        .iter()
        .map(|g| (g.name.as_str(), g.total))
        .collect();
    // Rent 3 × 2,000, Food 1,000 + 1,000 + 2,000. Refunds bought and
    // returned the same gear, nets to zero, and drops out.
    assert_eq!(
        rows,
        vec![("Rent", dec("6000.00")), ("Food", dec("4000.00"))]
    );
}

#[test]
fn fire_numbers_follow_the_four_percent_rule() {
    let view = ledger().reports_view((2026, 4, 15), 3, "USD");
    let fire = &view.fire;

    assert_eq!(fire.window, Some((m("2026-01"), m("2026-03"))));
    // (3,000 + 3,000 + 4,000) / 3, quantized at the leaf.
    assert_eq!(fire.monthly_spend, dec("3333.33"));
    assert_eq!(fire.annual_spend, dec("39999.96"));
    // 25× annual spend — the 4% rule.
    assert_eq!(fire.fire_number, dec("999999.00"));
    assert_eq!(fire.net_worth, dec("16200.00"));
    assert_eq!(fire.progress, Some(dec("0.0162")));
    assert_eq!(fire.monthly_savings, dec("2000.00"));
    // What the current stash sustains today: 16,200 × 4% / 12.
    assert_eq!(fire.swr_monthly, dec("54.00"));

    let rates: Vec<f64> = fire.scenarios.iter().map(|s| s.rate).collect();
    assert_eq!(rates, vec![0.03, 0.05, 0.07]);

    // The reported month is the first one where compounding the stash
    // and adding monthly savings reaches the target — one month less
    // must fall short.
    for s in &fire.scenarios {
        let k = s.months.unwrap();
        let rm = (1.0 + s.rate).powf(1.0 / 12.0) - 1.0;
        let balance = |n: u32| {
            let mut b = 16200.0;
            for _ in 0..n {
                b = b * (1.0 + rm) + 2000.0;
            }
            b
        };
        assert!(balance(k) >= 999_999.0, "rate {}: {k} too late", s.rate);
        assert!(balance(k - 1) < 999_999.0, "rate {}: {k} too early", s.rate);
    }
    // Higher real returns reach FIRE sooner.
    assert!(fire.scenarios[2].months < fire.scenarios[1].months);
    assert!(fire.scenarios[1].months < fire.scenarios[0].months);
}

#[test]
fn months_to_fire_handles_edges() {
    // Already there.
    assert_eq!(months_to_fire(1000.0, 0.0, 900.0, 0.05), Some(0));
    // No returns: pure saving, 100/month toward 1,200.
    assert_eq!(months_to_fire(0.0, 100.0, 1200.0, 0.0), Some(12));
    // Pure growth, no savings: 1.07^(k/12) ≥ 2 → 123 months.
    assert_eq!(months_to_fire(1000.0, 0.0, 2000.0, 0.07), Some(123));
    // Nothing saved, nothing owned: never.
    assert_eq!(months_to_fire(0.0, 0.0, 1000.0, 0.05), None);
    // Spending more than returns: never.
    assert_eq!(months_to_fire(1000.0, -50.0, 2000.0, 0.05), None);
}
