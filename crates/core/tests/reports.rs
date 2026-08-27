use std::path::PathBuf;

use bean_core::loader::load;
use bean_core::model::{Ledger, MonthKey};
use bean_core::reports::months_to_fire;
use rust_decimal::Decimal;

fn dec(s: &str) -> Decimal {
    s.parse().unwrap()
}

fn ledger() -> Ledger {
    fixture("main")
}

fn fixture(name: &str) -> Ledger {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(format!("tests/fixtures/reports/{name}.beancount"));
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
    assert_eq!(view.year.window, Some((m("2026-01"), m("2026-03"))));

    let rows: Vec<(&str, Decimal)> = view
        .year
        .groups
        .iter()
        .map(|g| (g.name.as_str(), g.total))
        .collect();
    // Rent 3 × 2,000, Food 1,000 + 1,000 + 2,000. Refunds bought and
    // returned the same gear, nets to zero, and drops out.
    assert_eq!(
        rows,
        vec![("Rent", dec("6000.00")), ("Food", dec("4000.00"))]
    );

    // Three months of history can't be measured against the year
    // before it, and the view says so rather than comparing a quarter
    // to nothing and calling everything new.
    assert_eq!(view.year.prior_window, None);
    assert!(view.year.groups.iter().all(|g| g.prior.is_none()));
    assert_eq!(view.movers.recent, None);
    assert!(view.movers.items.is_empty());
}

#[test]
fn the_year_card_carries_the_year_before_it() {
    let view = fixture("movers").reports_view((2026, 4, 15), 3, "USD");

    assert_eq!(view.year.window, Some((m("2025-04"), m("2026-03"))));
    assert_eq!(view.year.prior_window, Some((m("2024-04"), m("2025-03"))));

    let rows: Vec<(&str, Decimal, Option<Decimal>)> = view
        .year
        .groups
        .iter()
        .map(|g| (g.name.as_str(), g.total, g.prior))
        .collect();
    assert_eq!(
        rows,
        vec![
            // Rent never moved: 12 × 2,000 either year.
            ("Housing", dec("24000.00"), Some(dec("24000.00"))),
            // Groceries stepped 500 → 900 for the last three months.
            ("Food", dec("7320.00"), Some(dec("6120.00"))),
            ("Travel", dec("3000.00"), Some(dec("5000.00"))),
            ("Health", dec("400.00"), Some(dec("0.00"))),
            // Spent nothing this year, and that is the whole point of
            // keeping the row: 2,400 last year, gone.
            ("Education", dec("0.00"), Some(dec("2400.00"))),
        ]
    );
}

#[test]
fn movers_rank_the_quarter_by_money_not_percentage() {
    let view = fixture("movers").reports_view((2026, 4, 15), 3, "USD");
    let movers = &view.movers;

    assert_eq!(movers.recent, Some((m("2026-01"), m("2026-03"))));
    assert_eq!(movers.prior, Some((m("2025-10"), m("2025-12"))));
    assert_eq!(movers.recent_total, dec("9130.00"));
    assert_eq!(movers.prior_total, dec("10530.00"));

    let rows: Vec<(&str, Decimal, Decimal, Decimal, Option<Decimal>)> = movers
        .items
        .iter()
        .map(|i| (i.label.as_str(), i.recent, i.prior, i.delta, i.ratio))
        .collect();
    assert_eq!(
        rows,
        vec![
            // One trip last quarter, none this one: the biggest single
            // reason the quarter came in lighter.
            (
                "Flights",
                dec("0.00"),
                dec("3000.00"),
                dec("-3000.00"),
                Some(dec("-1"))
            ),
            (
                "Groceries",
                dec("2700.00"),
                dec("1500.00"),
                dec("1200.00"),
                Some(dec("0.8"))
            ),
            // Nothing to divide by, so no percentage is offered.
            ("Gym", dec("400.00"), dec("0.00"), dec("400.00"), None),
        ]
    );

    // Rent didn't move and coffee moved by nothing worth a sentence;
    // neither earns a row.
    assert!(!movers.items.iter().any(|i| i.label == "Rent"));
    assert!(!movers.items.iter().any(|i| i.label == "Coffee"));
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

#[test]
fn runway_measures_liquid_cash_against_monthly_spend() {
    let view = ledger().reports_view((2026, 4, 15), 3, "USD");

    // Checking only: the broker account holds VTI, so it is tracking
    // and its 2,200 stays out of the runway even though net worth
    // counts it.
    assert_eq!(view.runway.liquid, dec("14000.00"));
    // 14,000 against a 3,333.33 month.
    assert_eq!(view.runway.months, Some(dec("4.2")));
    // Fixed costs are not priced yet.
    assert_eq!(view.runway.lean_months, None);
}

#[test]
fn fire_prices_coasting_and_saving_more() {
    let view = ledger().reports_view((2026, 4, 15), 3, "USD");
    let fire = &view.fire;

    // Coasting is the same projection with contributions turned off,
    // so it always lands later than the saving path.
    let rates: Vec<f64> = fire.coast.iter().map(|s| s.rate).collect();
    assert_eq!(rates, vec![0.03, 0.05, 0.07]);
    for (coast, saving) in fire.coast.iter().zip(&fire.scenarios) {
        match (coast.months, saving.months) {
            (Some(c), Some(s)) => assert!(c > s, "rate {}", coast.rate),
            // Never getting there on its own is later still.
            (None, Some(_)) => {}
            (c, s) => panic!("rate {}: coast {c:?}, saving {s:?}", coast.rate),
        }
    }
    // 16,200 growing at 3% never reaches a million inside a century.
    assert_eq!(fire.coast[0].months, None);
    assert_eq!(
        fire.coast[2].months,
        months_to_fire(16200.0, 0.0, 999_999.0, 0.07)
    );

    // Saving 25% and 50% more, priced at the middle rate.
    let extra: Vec<Decimal> = fire.steps.iter().map(|s| s.extra).collect();
    assert_eq!(extra, vec![dec("500.00"), dec("1000.00")]);
    let middle = fire.scenarios[1].months.unwrap();
    for step in &fire.steps {
        assert!(step.months.unwrap() < middle);
    }
    assert!(fire.steps[1].months < fire.steps[0].months);

    // No fixed costs priced yet, so there is no lean target.
    assert_eq!(fire.lean_number, None);
}

#[test]
fn net_worth_splits_cash_from_holdings() {
    let view = ledger().reports_view((2026, 4, 15), 3, "USD");

    // January: 11,000 in checking, 2,000 of VTI at the January price.
    let jan = &view.net_worth[0];
    assert_eq!(jan.cash, dec("11000.00"));
    assert_eq!(jan.holdings, dec("2000.00"));
    assert_eq!(jan.cash + jan.holdings, jan.assets);

    // March revalues the holding without touching the cash.
    let mar = &view.net_worth[2];
    assert_eq!(mar.cash, dec("14000.00"));
    assert_eq!(mar.holdings, dec("2200.00"));
}

#[test]
fn growth_separates_what_you_saved_from_what_the_market_did() {
    let view = ledger().reports_view((2026, 4, 15), 3, "USD");
    let points = &view.growth.points;

    // January: net worth 0 → 12,000. 2,000 of it was saved, 10,000
    // walked in as an opening balance, and the market did nothing.
    assert_eq!(points[0].month, m("2026-01"));
    assert_eq!(points[0].delta, dec("12000.00"));
    assert_eq!(points[0].saved, dec("2000.00"));
    assert_eq!(points[0].equity, dec("10000.00"));
    assert_eq!(points[0].market, dec("0"));

    // February: pure saving, no equity, no price move.
    assert_eq!(points[1].delta, dec("2000.00"));
    assert_eq!(points[1].saved, dec("2000.00"));
    assert_eq!(points[1].market, dec("0"));

    // March: saved 2,000 and VTI went 100 → 110 on 20 shares.
    assert_eq!(points[2].saved, dec("2000.00"));
    assert_eq!(points[2].market, dec("200.00"));
    assert_eq!(points[2].delta, dec("2200.00"));

    // April is quiet on every axis.
    assert_eq!(points[3].delta, dec("0"));
    assert_eq!(points[3].market, dec("0"));

    // Every month reconciles.
    for p in points {
        assert_eq!(p.delta, p.saved + p.equity + p.market, "{}", p.month);
    }

    // The window totals cover January through March.
    let g = &view.growth;
    assert_eq!(g.window, Some((m("2026-01"), m("2026-03"))));
    assert_eq!(g.saved, dec("6000.00"));
    assert_eq!(g.equity, dec("10000.00"));
    assert_eq!(g.market, dec("200.00"));
    assert_eq!(g.delta, dec("16200.00"));
    // 200 over the mean of 12,000 / 14,000 / 16,200 — 14,066.67.
    assert_eq!(g.implied_return, Some(dec("0.0142")));
}

#[test]
fn unpriced_commodities_are_named_and_void_the_implied_return() {
    let view = fixture("unpriced").reports_view((2026, 2, 15), 3, "USD");

    // Nothing prices ADA, so every ADA amount drops out on the way to
    // USD — on both sides of the ledger.
    assert_eq!(view.unpriced, vec!["ADA".to_string()]);
    assert_eq!(view.growth.unpriced, vec!["ADA".to_string()]);

    // The grant never lands as income, but the rent it paid for does
    // land as spend.
    let cf = &view.cashflow[0];
    assert_eq!(cf.income, dec("0"));
    assert_eq!(cf.expenses, dec("2000.00"));

    // Meanwhile 30,000 walked into checking with no flow to explain it,
    // so the residual reports a month of pure market gain that never
    // happened.
    let g = &view.growth.points[0];
    assert_eq!(g.delta, dec("28000.00"));
    assert_eq!(g.saved, dec("-2000.00"));
    assert_eq!(g.market, dec("30000.00"));

    // Which is exactly why the return is not reported: the inputs it
    // would be computed from are known to be incomplete.
    assert_eq!(view.growth.implied_return, None);
}

#[test]
fn a_fully_priced_ledger_reports_nothing_unpriced() {
    let view = ledger().reports_view((2026, 4, 15), 3, "USD");
    assert!(view.unpriced.is_empty());
    assert!(view.growth.unpriced.is_empty());
    assert!(view.growth.implied_return.is_some());
}

#[test]
fn recurring_finds_the_charges_that_repeat_on_a_cadence() {
    let view = fixture("recurring").reports_view((2026, 8, 20), 6, "USD");
    let r = &view.recurring;

    let names: Vec<(&str, &str)> = r
        .items
        .iter()
        .map(|i| (i.payee.as_str(), i.cadence.label()))
        .collect();
    // Still charging first, biggest first inside that, so the top of
    // the list is the top of next month's bill and the one that stopped
    // falls to the bottom whatever it used to cost. Groceries repeat
    // under one payee every month but never for the same amount, so
    // they are not a charge you can plan around and stay out.
    assert_eq!(
        names,
        vec![
            ("Landlord", "monthly"),
            ("Assurance", "yearly"),
            ("Cloud Host", "monthly"),
            ("Netflix", "monthly"),
            ("City Gym", "monthly"),
        ]
    );

    let rent = &r.items[0];
    assert_eq!(rent.account, "Expenses:Housing:Rent");
    assert_eq!(rent.amount, dec("2000.00"));
    assert_eq!(rent.monthly, dec("2000.00"));
    assert!(rent.active);
    assert!(rent.change.is_none());

    // A yearly premium is worth a twelfth of itself each month.
    let car = &r.items[1];
    assert_eq!(car.amount, dec("1200.00"));
    assert_eq!(car.monthly, dec("100.00"));
    assert!(car.active);

    // The gym stopped charging nine months ago; it is still worth
    // showing, but it is not part of what next month costs.
    let gym = &r.items[4];
    assert!(!gym.active);
    assert_eq!(gym.last, (2025, 11, 5));

    // Netflix went up in March and has stayed there since.
    let netflix = &r.items[3];
    assert_eq!(netflix.amount, dec("17.99"));
    let change = netflix.change.as_ref().unwrap();
    assert_eq!(change.from, dec("15.99"));
    assert_eq!(change.to, dec("17.99"));
    // Two dollars a month, twelve months a year.
    assert_eq!(change.annual, dec("24.00"));

    // The fixed nut counts only what is still charging.
    assert_eq!(r.monthly_fixed, dec("2176.13"));
    assert_eq!(r.annual_fixed, dec("26113.56"));
}

#[test]
fn fixed_costs_drive_the_lean_target_and_the_lean_runway() {
    let view = fixture("recurring").reports_view((2026, 8, 20), 6, "USD");

    // 25× a year of fixed costs — the stash that covers the bills but
    // not the lifestyle.
    assert_eq!(view.fire.lean_number, Some(dec("652839.00")));
    assert!(view.fire.lean_progress.is_some());

    // And the same cash lasts longer once you only have to cover them.
    let runway = &view.runway;
    let (months, lean) = (runway.months.unwrap(), runway.lean_months.unwrap());
    assert!(lean > months, "lean {lean} should outlast {months}");
}

#[test]
fn the_fixed_nut_says_how_much_of_the_spending_it_covers() {
    let view = fixture("recurring").reports_view((2026, 8, 20), 6, "USD");

    // The nut is only ever what could be recognized as recurring, so it
    // is reported against the spending it is a share of — a ledger with
    // thin payees gets a small number here and every figure built on it
    // inherits that.
    let coverage = view.recurring.coverage.unwrap();
    let spend = view.fire.monthly_spend;
    assert_eq!(coverage, (view.recurring.monthly_fixed / spend).round_dp(4));
    assert!(coverage > Decimal::ZERO && coverage < Decimal::ONE);
}

#[test]
fn exchange_rate_drift_is_not_a_price_rise() {
    let view = fixture("recurring").reports_view((2026, 8, 20), 6, "USD");
    let find = |payee: &str| {
        view.recurring
            .items
            .iter()
            .find(|i| i.payee == payee)
            .unwrap_or_else(|| panic!("{payee} was not detected"))
    };

    // A charge billed abroad lands on a different number every month
    // once it is converted. Cents on fifty-eight dollars are the
    // exchange rate moving, not a decision anyone made.
    let hosting = find("Cloud Host");
    assert_eq!(hosting.cadence.label(), "monthly");
    assert!(
        hosting.change.is_none(),
        "cent drift reported as a rise: {:?}",
        hosting.change
    );

    // Two dollars on sixteen is.
    assert!(find("Netflix").change.is_some());
}

#[test]
fn tags_and_links_add_up_as_topics() {
    let view = fixture("projects").reports_view((2026, 4, 15), 3, "USD");
    let projects = &view.projects;

    let rows: Vec<(char, &str, Decimal, Decimal, usize, usize, u32)> = projects
        .items
        .iter()
        .map(|p| {
            (
                p.kind.sigil(),
                p.name.as_str(),
                p.spent,
                p.net,
                p.count,
                p.categories,
                p.months,
            )
        })
        .collect();
    assert_eq!(
        rows,
        vec![
            // 4,000 tile + 6,000 labour, less the 500 of spare boxes
            // that went back.
            ('#', "renovation", dec("9500.00"), dec("9500.00"), 3, 2, 3),
            // Charged 5,400, but the employer paid half of it back —
            // reporting the gross would say the trip cost what it
            // never did.
            ('^', "trip-japan", dec("5400.00"), dec("3400.00"), 3, 2, 2),
        ]
    );

    let renovation = &projects.items[0];
    assert_eq!(renovation.first, (2026, 1, 10));
    assert_eq!(renovation.last, (2026, 3, 2));

    // Two payment ids on one transaction each, and one tag that only
    // marks transfers between accounts already owned. Neither is a
    // topic, and both are counted rather than quietly dropped.
    assert_eq!(projects.singletons, 2);
    assert_eq!(projects.markers, 1);
}
