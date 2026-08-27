use std::path::PathBuf;

use bean_core::loader::load;
use bean_core::model::{Day, Ledger, MonthKey};
use bean_core::reports::{YearGroup, YearView, months_to_fire};
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

fn at_group<'a>(y: &'a YearView, name: &str) -> &'a YearGroup {
    y.groups
        .iter()
        .find(|g| g.name == name)
        .unwrap_or_else(|| panic!("{name} is missing"))
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
fn the_year_card_lays_each_group_out_month_by_month() {
    let view = fixture("movers").reports_view((2026, 4, 15), 3, "USD");
    let y = &view.year;

    // The window's months are the axis every group is indexed by, so
    // a cell always knows which month it belongs to.
    let months: Vec<String> =
        y.months.iter().map(|m| m.month.to_string()).collect();
    assert_eq!(months.len(), 12);
    assert_eq!(months[0], "2025-04");
    assert_eq!(months[11], "2026-03");

    let group = |name: &str| {
        y.groups
            .iter()
            .find(|g| g.name == name)
            .unwrap_or_else(|| panic!("{name} is missing"))
    };

    // Rent is the same 2,000 every month, so every cell matches and
    // the typical month is that number.
    let housing = group("Housing");
    assert_eq!(housing.monthly, vec![dec("2000.00"); 12]);
    assert_eq!(housing.typical, Some(dec("2000.00")));

    // Travel is one 3,000 flight in November and nothing else. Its
    // year and its biggest month are the same number, which is the
    // whole reason to show the months: 3,000 a year reads like a
    // habit, 3,000 once reads like a trip.
    let travel = group("Travel");
    assert_eq!(travel.total, dec("3000.00"));
    assert_eq!(travel.monthly[7], dec("3000.00"));
    assert_eq!(
        travel
            .monthly
            .iter()
            .filter(|v| **v > Decimal::ZERO)
            .count(),
        1
    );

    // Groceries stepped 500 to 900 in January, so the last three
    // months stand apart from the nine before them — and the median
    // month is still the 510 it sat at for most of the year.
    let food = group("Food");
    assert_eq!(food.monthly[0], dec("510.00"));
    assert_eq!(food.monthly[11], dec("910.00"));
    assert_eq!(food.typical, Some(dec("510.00")));

    // A group that spent nothing this year keeps a full row of zeroes
    // rather than a short one, so the cells stay under their months.
    let edu = group("Education");
    assert_eq!(edu.monthly, vec![Decimal::ZERO; 12]);
    assert_eq!(edu.typical, None);
}

#[test]
fn the_year_card_measures_a_month_against_the_others_and_the_last_one() {
    let view = fixture("movers").reports_view((2026, 4, 15), 3, "USD");
    let y = &view.year;
    let at = |key: &str| {
        y.months
            .iter()
            .find(|x| x.month == m(key))
            .unwrap_or_else(|| panic!("{key} is missing"))
    };

    // Rent, groceries and coffee are what an ordinary month costs.
    assert_eq!(at("2025-04").total, dec("2510.00"));
    // November carries the flight on top, and is the year's biggest.
    assert_eq!(at("2025-11").total, dec("5510.00"));
    // March has the grocery step and the gym instead.
    assert_eq!(at("2026-03").total, dec("3110.00"));

    // Each month also carries the same calendar month a year earlier,
    // so a spike can be read against whether that month always spikes.
    // November's did not; July's was the year-earlier flight.
    assert_eq!(at("2025-11").prior, Some(dec("2510.00")));
    assert_eq!(at("2025-07").prior, Some(dec("7510.00")));

    // The typical month is the median of the twelve, which neither the
    // flight nor the step can drag.
    assert_eq!(y.typical, Some(dec("2510.00")));

    // And the two ways of adding the year up agree, because both are
    // summed from the same per-month cents.
    for g in &y.groups {
        assert_eq!(g.monthly.iter().sum::<Decimal>(), g.total);
    }
    let by_month: Decimal = y.months.iter().map(|x| x.total).sum();
    let by_group: Decimal = y.groups.iter().map(|g| g.total).sum();
    assert_eq!(by_month, by_group);
}

#[test]
fn a_year_with_nothing_before_it_compares_only_its_own_months() {
    let view = ledger().reports_view((2026, 4, 15), 3, "USD");
    let y = &view.year;

    // Three months of history: the axis is as long as the window, not
    // a padded twelve.
    let months: Vec<String> =
        y.months.iter().map(|x| x.month.to_string()).collect();
    assert_eq!(months, vec!["2026-01", "2026-02", "2026-03"]);
    assert!(y.groups.iter().all(|g| g.monthly.len() == 3));

    // Rent 2,000 a month; food 1,000, 1,000, then 2,000.
    assert_eq!(
        at_group(y, "Food").monthly,
        vec![dec("1000.00"), dec("1000.00"), dec("2000.00")]
    );
    assert_eq!(at_group(y, "Food").typical, Some(dec("1000.00")));

    // Months still compare against each other — that never needed a
    // year before them — but never against a year that isn't there.
    assert_eq!(y.typical, Some(dec("3000.00")));
    assert!(y.months.iter().all(|x| x.prior.is_none()));
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

#[test]
fn income_splits_by_source_and_measures_what_arrives_on_its_own() {
    let view = fixture("income").reports_view((2026, 4, 15), 3, "USD");
    let income = &view.income;

    assert_eq!(income.window, Some((m("2025-04"), m("2026-03"))));
    assert_eq!(income.total, dec("86500.00"));

    let rows: Vec<(&str, Decimal, Decimal, Decimal)> = income
        .sources
        .iter()
        .map(|s| (s.name.as_str(), s.total, s.share, s.passive))
        .collect();
    assert_eq!(
        rows,
        vec![
            ("Salary", dec("72000.00"), dec("0.8324"), dec("0.00")),
            // `income: "active"` on the open directive beats the name,
            // which would otherwise read a rent roll here.
            ("Rentals", dec("6000.00"), dec("0.0694"), dec("0.00")),
            ("Dividends", dec("4000.00"), dec("0.0462"), dec("4000.00")),
            // And the ledger can mark what no name would suggest.
            ("Consulting", dec("3000.00"), dec("0.0347"), dec("3000.00")),
            ("Interest", dec("1000.00"), dec("0.0116"), dec("1000.00")),
            // A rebate on spending stops when the spending does.
            ("Cashback", dec("500.00"), dec("0.0058"), dec("0.00")),
        ]
    );

    // Six sources, but five-sixths of it is one paycheque: the spread
    // is worth about one and a half independent sources.
    assert_eq!(income.effective_sources, Some(dec("1.43")));

    assert_eq!(income.passive, dec("8000.00"));
    // These two are kept at full precision, because they are read
    // beside each other and passive income spends years small enough
    // that a hundredth of a percent would round both to nothing and
    // make them look like one number said twice.
    assert_eq!(
        income.passive_share.map(|r| r.round_dp(6)),
        Some(dec("0.092486"))
    );
    // 8,000 against 40,000 of spending: a fifth of the bill already
    // pays itself.
    assert_eq!(income.passive_cover, Some(dec("0.2")));

    assert_eq!(income.declared, 1);
    assert_eq!(income.inferred, 2);
}

#[test]
fn the_trust_report_names_what_the_page_is_resting_on() {
    let ledger = fixture("trust");
    let view = ledger.reports_view((2026, 4, 15), 3, "USD");
    let trust = &view.trust;
    assert_eq!(trust.window, Some((m("2025-04"), m("2026-03"))));

    // GOLD hasn't been priced since January last year, and 20,000 of
    // net worth is standing on that price. VTI was priced a fortnight
    // ago, so it isn't a doubt; ADA was never priced at all, which is
    // a different problem the page reports separately.
    assert_eq!(view.unpriced, vec!["ADA".to_string()]);
    let stale: Vec<&str> =
        trust.stale.iter().map(|s| s.commodity.as_str()).collect();
    assert_eq!(stale, vec!["GOLD"]);
    let gold = &trust.stale[0];
    assert_eq!(gold.last, (2025, 1, 15));
    assert_eq!(gold.days, 455);
    assert_eq!(gold.value, dec("20000.00"));

    // Three transactions the ledger won't vouch for, but only the two
    // inside the window are behind the numbers on this page.
    assert_eq!(trust.flagged.total, 3);
    assert_eq!(trust.flagged.window, 2);
    assert_eq!(trust.flagged.amount, dec("1500.00"));
    let recent: Vec<(Day, &str)> = trust
        .flagged
        .recent
        .iter()
        .map(|t| (t.date, t.payee.as_deref().unwrap_or("")))
        .collect();
    assert_eq!(
        recent,
        vec![
            ((2026, 1, 15), "Diner"),
            ((2025, 12, 5), "Someone"),
            ((2025, 2, 10), "Nobody"),
        ]
    );

    // 500 of the window's 10,000 went somewhere unnamed. `Expenses:Misc`
    // is not a gap — miscellaneous is a decision.
    assert_eq!(trust.uncategorized.total, dec("500.00"));
    assert_eq!(trust.uncategorized.share, Some(dec("0.05")));
    assert_eq!(
        trust.uncategorized.accounts,
        vec!["Expenses:Uncategorized".to_string()]
    );

    assert!(trust.warnings.is_empty());
}

#[test]
fn a_ledger_with_nothing_to_hide_reports_nothing() {
    let ledger = ledger();
    let view = ledger.reports_view((2026, 4, 10), 3, "USD");
    let trust = &view.trust;
    assert!(trust.stale.is_empty());
    assert_eq!(trust.flagged.total, 0);
    assert_eq!(trust.flagged.amount, Decimal::ZERO);
    assert!(trust.flagged.recent.is_empty());
    assert_eq!(trust.uncategorized.total, Decimal::ZERO);
    // Nothing uncategorized, but the page can still say what share of
    // spend that is, because there was spend.
    assert_eq!(trust.uncategorized.share, Some(Decimal::ZERO));
    assert!(trust.warnings.is_empty());

    // VTI was last priced on 2026-03-01, so the same ledger crosses
    // into stale five days later — the point being that staleness is a
    // fact about today, not about the ledger.
    let later = ledger.reports_view((2026, 4, 15), 3, "USD");
    let stale = &later.trust.stale;
    assert_eq!(stale.len(), 1);
    assert_eq!(stale[0].commodity, "VTI");
    assert_eq!(stale[0].days, 45);
    assert_eq!(stale[0].value, dec("2200.00"));
}

#[test]
fn the_season_report_reads_the_shape_out_of_the_years() {
    let ledger = fixture("season");
    let view = ledger.reports_view((2026, 4, 15), 3, "USD");
    let season = &view.season;

    // Eight whole years of history, and the shape comes from the last
    // five: 2018 through 2022 ran an order of magnitude cheaper, and a
    // median that reached back that far would be a baseline for a life
    // this ledger stopped living.
    assert_eq!(season.years, Some((2021, 2025)));
    assert_eq!(season.months.len(), 12);
    assert!(season.months.iter().all(|p| p.samples == 5));
    let medians: Vec<Decimal> =
        season.months.iter().map(|p| p.median).collect();
    assert_eq!(
        medians,
        vec![
            dec("100.00"),
            dec("100.00"),
            dec("100.00"),
            dec("100.00"),
            dec("100.00"),
            dec("100.00"),
            dec("200.00"),
            dec("100.00"),
            dec("100.00"),
            dec("100.00"),
            dec("100.00"),
            dec("400.00"),
        ]
    );

    // The shape with the level divided out: December is a quarter of
    // an ordinary year on its own.
    assert_eq!(season.typical, dec("1600.00"));
    assert_eq!(season.months[11].share, dec("0.25"));
    assert_eq!(season.months[0].share, dec("0.0625"));

    // January through March are over and cost 150 each. April is still
    // running, so its 999 is nowhere in these numbers.
    assert_eq!(season.year, 2026);
    assert_eq!(season.elapsed, 3);
    assert_eq!(season.ytd, dec("450.00"));
    let actual: Vec<Option<Decimal>> =
        season.months.iter().map(|p| p.actual).collect();
    assert_eq!(actual[0], Some(dec("150.00")));
    assert_eq!(actual[2], Some(dec("150.00")));
    assert_eq!(actual[3], None);

    // 450 against the 300 those three months usually cost, and the
    // remaining 1,300 of a typical year priced at that same pace.
    assert_eq!(season.pace, Some(dec("1.5")));
    assert_eq!(season.projected, Some(dec("2400.00")));
}

#[test]
fn a_ledger_too_short_to_have_a_shape_refuses_to_guess_one() {
    let view = ledger().reports_view((2026, 4, 15), 3, "USD");
    let season = &view.season;

    // Four months is not a year, and eight empty calendar slots would
    // read as eight months that cost nothing.
    assert_eq!(season.years, None);
    assert!(season.months.is_empty());
    assert_eq!(season.typical, Decimal::ZERO);
    assert_eq!(season.pace, None);
    assert_eq!(season.projected, None);

    // What this year has spent is still a fact, and still reported.
    assert_eq!(season.year, 2026);
    assert_eq!(season.elapsed, 2);
}

#[test]
fn the_payee_leaderboard_ranks_merchants_by_what_they_took() {
    let ledger = fixture("payees");
    let view = ledger.reports_view((2026, 4, 15), 3, "USD");
    let payees = &view.payees;
    assert_eq!(payees.window, Some((m("2025-04"), m("2026-03"))));

    // 8,000 of spend in the window. The supermarket's 900 from
    // February is a month too early to be in it.
    assert_eq!(payees.total, dec("8000.00"));

    let top: Vec<(&str, Decimal, usize, Decimal)> = payees.items[..4]
        .iter()
        .map(|p| (p.name.as_str(), p.spent, p.count, p.average))
        .collect();
    assert_eq!(
        top,
        vec![
            ("Airline", dec("3000.00"), 1, dec("3000.00")),
            ("Supermarket", dec("2400.00"), 12, dec("200.00")),
            ("Streaming Co", dec("600.00"), 12, dec("50.00")),
            ("Corner Store", dec("500.00"), 2, dec("250.00")),
        ]
    );

    // The three numbers that tell the shapes apart: one flight, twelve
    // identical small charges, twelve larger ones.
    let air = &payees.items[0];
    assert_eq!(air.months, 1);
    assert_eq!(air.share, dec("0.375"));
    assert_eq!(air.first, (2025, 7, 20));
    assert_eq!(air.last, (2025, 7, 20));

    let market = &payees.items[1];
    assert_eq!(market.months, 12);
    assert_eq!(market.categories, 1);
    assert_eq!(market.share, dec("0.30"));
    assert_eq!(market.first, (2025, 4, 5));
    assert_eq!(market.last, (2026, 3, 5));

    // One name, two categories.
    let corner = &payees.items[3];
    assert_eq!(corner.categories, 2);
    assert_eq!(corner.months, 2);

    // Bought and returned nets to nothing, so it ranks nowhere at all —
    // not in the list and not in the tail behind it.
    assert!(payees.items.iter().all(|p| p.name != "Gadget Shop"));
    assert!(payees.items.iter().all(|p| p.name != "Bank"));

    // Sixteen names qualify and fifteen fit, so the last one is
    // reported rather than dropped.
    assert_eq!(payees.items.len(), 15);
    assert_eq!(payees.items[14].name, "Vendor K");
    assert_eq!(payees.others_items.len(), 1);
    assert_eq!(payees.others_spent, dec("45.00"));

    // Money that named nobody is money this card cannot rank, and it
    // says how much rather than ranking around it.
    assert_eq!(payees.anonymous, dec("630.00"));
    assert_eq!(payees.anonymous_count, 3);
}

#[test]
fn a_ledger_that_names_nobody_ranks_nobody() {
    // The trailing window clamps to the ledger's first activity rather
    // than refusing, the same way the income card's does: a merchant
    // is a fact about however long you have been recording, and three
    // months of it is still an answer.
    let payees = ledger().reports_view((2026, 4, 15), 3, "USD").payees;
    assert_eq!(payees.window, Some((m("2026-01"), m("2026-03"))));

    // This ledger writes narrations and never a payee, which is what a
    // whole class of importers produces. There is nobody to rank, and
    // the card says the spend is unattributed rather than showing an
    // empty list and leaving the reason to the reader.
    assert!(payees.items.is_empty());
    assert!(payees.others_items.is_empty());
    assert!(payees.total > Decimal::ZERO);
    assert_eq!(payees.anonymous, payees.total);
    assert_eq!(payees.anonymous_count, 8);
}

fn investments() -> Ledger {
    fixture("investments")
}

#[test]
fn investments_rank_positions_and_group_them_by_class() {
    let view = investments()
        .reports_view((2026, 6, 15), 3, "USD")
        .investments;

    // Cash is not a position: the operating currency the rest of the
    // page is denominated in is what these are being measured against.
    let held: Vec<&str> =
        view.items.iter().map(|p| p.currency.as_str()).collect();
    assert_eq!(held, vec!["VTI", "ETH", "ACME", "BOND"]);
    assert_eq!(view.total, dec("6080.00"));

    let vti = &view.items[0];
    assert_eq!(vti.label, "Vanguard Total Stock Market ETF");
    assert_eq!(vti.class.as_deref(), Some("etf"));
    // 20 bought, 10 more, 6 sold.
    assert_eq!(vti.units, dec("24"));
    assert_eq!(vti.price, dec("120.00"));
    assert_eq!(vti.value, dec("2880.00"));
    assert_eq!(vti.share, dec("0.4737"));
    assert_eq!(vti.accounts, 1);
    assert_eq!(vti.postings, 3);
    assert_eq!(vti.first, (2026, 1, 10));
    assert_eq!(vti.last, (2026, 3, 10));

    // One position, two brokers: moving a holding between accounts must
    // not split it into two smaller ones. A third broker bought a
    // tranche and sold it out again, which leaves it a place the
    // position has been rather than a place it is.
    let acme = &view.items[2];
    assert_eq!(acme.currency, "ACME");
    assert_eq!(acme.units, dec("150"));
    assert_eq!(acme.accounts, 2);
    assert_eq!(acme.postings, 4);
    assert_eq!(acme.value, dec("1200.00"));

    // Priced, and nothing declared what it is. It still ranks; it just
    // has nothing but its ticker to be called.
    let bond = &view.items[3];
    assert_eq!(bond.label, "BOND");
    assert_eq!(bond.class, None);

    let classes: Vec<(Option<&str>, String)> = view
        .classes
        .iter()
        .map(|c| (c.name.as_deref(), c.value.to_string()))
        .collect();
    assert_eq!(
        classes,
        vec![
            (Some("etf"), "2880.00".to_string()),
            (Some("crypto"), "1500.00".to_string()),
            (Some("stock"), "1200.00".to_string()),
            (None, "500.00".to_string()),
        ]
    );
    // Dust is folded out of the table but not out of its class.
    assert_eq!(view.classes[1].positions, 2);
    assert_eq!(view.classes[0].share, dec("0.4737"));

    // Four holdings that behave like three, the same `1 / Σ share²` the
    // income card uses on sources.
    assert_eq!(view.effective, Some(dec("3.02")));
}

#[test]
fn investments_report_a_basis_only_where_the_ledger_named_one() {
    let view = investments()
        .reports_view((2026, 6, 15), 3, "USD")
        .investments;

    // 20 × 100 plus a 1,100 lump is 3,100 over 30 units; selling 6 out
    // of whichever lot takes a fifth of the basis with it.
    let vti = &view.items[0];
    assert_eq!(vti.basis, Some(dec("2480.00")));
    assert_eq!(vti.gain, Some(dec("400.00")));
    assert_eq!(vti.ret, Some(dec("0.1613")));

    // Down is reported the same way up is. 100 at 10 and 50 at 12; the
    // ten the third broker bought at 50 and sold at 9 are charged to
    // that broker's own inventory, not blended into these — averaging
    // the whole position would report 1,968.75 for shares that cost
    // 1,600.
    let acme = &view.items[2];
    assert_eq!(acme.basis, Some(dec("1600.00")));
    assert_eq!(acme.gain, Some(dec("-400.00")));
    assert_eq!(acme.ret, Some(dec("-0.25")));

    // Staking rewards are not a purchase. There is no cost to be up
    // against, so no gain is reported rather than the +100% that
    // treating a missing basis as zero would invent.
    let eth = &view.items[1];
    assert_eq!(eth.currency, "ETH");
    assert_eq!(eth.value, dec("1500.00"));
    assert_eq!(eth.basis, None);
    assert_eq!(eth.gain, None);
    assert_eq!(eth.ret, None);

    // And the totals say how much of the page that covers, so a
    // portfolio return is never quoted over a part of the portfolio
    // without saying which part.
    assert_eq!(view.basis, dec("4530.00"));
    assert_eq!(view.based_value, dec("4580.00"));
    assert_eq!(view.gain, dec("50.00"));
    assert_eq!(view.ret, Some(dec("0.0110")));
    assert_eq!(view.coverage, Some(dec("0.7533")));
    assert_eq!(view.unbased, dec("1500.00"));
    assert_eq!(view.unbased_count, 2);
}

#[test]
fn investments_fold_dust_and_name_what_nothing_prices() {
    let view = investments()
        .reports_view((2026, 6, 15), 3, "USD")
        .investments;

    // Five DUST at a ten-thousandth is half a cent. It is still held
    // and still counted in the total; it just does not earn a row.
    assert!(view.items.iter().all(|p| p.currency != "DUST"));
    assert_eq!(view.dust_value, dec("0.00"));

    // Folded out of the ranking, not out of the report: the row that
    // counts them opens onto the positions themselves.
    let dust: Vec<&str> = view
        .dust_items
        .iter()
        .map(|p| p.currency.as_str())
        .collect();
    assert_eq!(dust, vec!["DUST"]);

    // Two GOLD is a real holding that nothing can value, so it is
    // missing from every figure above — and named for it rather than
    // quietly dropped.
    assert_eq!(view.unpriced, vec!["GOLD".to_string()]);
}

#[test]
fn payees_past_the_cut_are_kept_rather_than_counted() {
    let view = fixture("payees").reports_view((2026, 4, 15), 3, "USD");
    let payees = &view.payees;

    // The ranking stops at fifteen, but the names past it are still
    // carried, so the line that counts them can open onto them
    // instead of dead-ending in a number nobody can check.
    assert_eq!(payees.items.len(), 15);
    assert!(!payees.others_items.is_empty());

    // The tail carries on where the ranking stopped: nothing in it
    // outspends the cheapest name that made the cut.
    let cut = payees.items.last().unwrap().spent;
    assert!(payees.others_items.iter().all(|p| p.spent <= cut));

    // And it is the same money the summary claims, not a sample of it.
    let summed: Decimal = payees.others_items.iter().map(|p| p.spent).sum();
    assert_eq!(payees.others_spent, summed);
}

#[test]
fn movers_past_the_cut_are_kept_rather_than_dropped() {
    let view = fixture("movers-tail").reports_view((2026, 4, 15), 3, "USD");
    let movers = &view.movers;
    assert_eq!(movers.recent, Some((m("2026-01"), m("2026-03"))));

    // Twelve categories rose, each by 200 more than the one below it.
    // Eight earn a row; the other four used to vanish without the
    // card ever admitting they existed.
    assert_eq!(movers.items.len(), 8);
    let hidden: Vec<Decimal> =
        movers.hidden.iter().map(|mv| mv.delta).collect();
    assert_eq!(
        hidden,
        vec![dec("800.00"), dec("600.00"), dec("400.00"), dec("200.00")]
    );

    // The tail is ranked by the same measure as the list above it, so
    // opening it reads as one list rather than two.
    assert!(movers.items.last().unwrap().delta.abs() >= dec("800.00"));
}
