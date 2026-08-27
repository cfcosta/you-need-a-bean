use std::path::PathBuf;

use bean_core::loader::load;
use bean_core::model::{AccountKind, Ledger, MonthKey};
use bean_core::query::Status;
use rust_decimal::Decimal;

fn dec(s: &str) -> Decimal {
    s.parse().unwrap()
}

fn ledger() -> Ledger {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/model/main.beancount");
    Ledger::build(load(&path).unwrap())
}

fn m(s: &str) -> MonthKey {
    MonthKey::parse(s).unwrap()
}

#[test]
fn typical_uses_trailing_window_clamped_to_first_activity() {
    let ledger = ledger();
    let typ = |account: &str, month: &str, basis: u32| {
        ledger.typical(account, m(month), basis, "USD")
    };

    // Window [2025-11, 2026-01] clamps to first activity (2025-12):
    // the median of {30, 100}.
    assert_eq!(
        typ("Expenses:Food:Groceries", "2026-02", 3),
        Some(dec("65"))
    );
    // Only December before January: that one month is the median.
    assert_eq!(
        typ("Expenses:Food:Groceries", "2026-01", 6),
        Some(dec("30"))
    );
    // The first month has no window at all.
    assert_eq!(typ("Expenses:Food:Groceries", "2025-12", 6), None);
    // Months without spend don't count: rent was paid once inside the
    // window, so typical rent is that payment.
    assert_eq!(typ("Expenses:Home:Rent", "2026-04", 12), Some(dec("500")));
    // A window whose months never saw spending has no typical at all.
    assert_eq!(typ("Expenses:Home:Rent", "2026-02", 3), None);
    // History converts at each month's own end date: 30 BRL → 6 USD.
    assert_eq!(
        typ("Expenses:Food:Dining:Coffee", "2026-02", 6),
        Some(dec("6"))
    );
}

#[test]
fn typical_is_the_median_month_with_real_payments() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/outliers/main.beancount");
    let ledger = Ledger::build(load(&path).unwrap());

    // Vet runs 150/mo with one 900 emergency. The typical month is the
    // median, 150 — the spike doesn't drag it anywhere.
    assert_eq!(
        ledger.typical("Expenses:Spiky:Vet", m("2026-03"), 12, "USD"),
        Some(dec("150"))
    );
    // Same inside a shorter window: median of {150, 150, 900}.
    assert_eq!(
        ledger.typical("Expenses:Spiky:Vet", m("2026-03"), 3, "USD"),
        Some(dec("150"))
    );
    // Even count: the median averages the two middle months.
    assert_eq!(
        ledger.typical("Expenses:Old:Server", m("2026-01"), 12, "USD"),
        Some(dec("1200"))
    );
    // A net-refund month is a month without payments: it doesn't count,
    // so Gadgets' typical is its one real purchase, not median{-50, 300}.
    assert_eq!(
        ledger.typical("Expenses:Spiky:Gadgets", m("2026-03"), 3, "USD"),
        Some(dec("300"))
    );

    // The headline typical is the median real month — the median of
    // the window months' totals {1350, 1350, 150, 510, 910} — not the
    // sum of every category's median (1,710), which would bill each
    // sporadic category every month.
    let view = ledger.month_view(m("2026-03"), 12, "USD");
    assert_eq!(view.typical, Some(dec("910")));
}

#[test]
fn table_sorts_on_trailing_year_spend_whatever_the_basis() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/outliers/main.beancount");
    let ledger = Ledger::build(load(&path).unwrap());
    let view = ledger.month_view(m("2026-03"), 3, "USD");

    // The table ranks by what the year actually cost. Server spent
    // nothing in the 3-month window, but its trailing-year total
    // (2,400) tops the table; switching the basis must never
    // reshuffle the order.
    let names: Vec<&str> =
        view.groups.iter().map(|g| g.name.as_str()).collect();
    assert_eq!(names, vec!["Old", "Spiky", "New"]);

    let old = &view.groups[0];
    let labels: Vec<&str> =
        old.categories.iter().map(|c| c.label.as_str()).collect();
    assert_eq!(labels, vec!["Server", "Lamp"]);

    // Rank and typical are decoupled: Vet's year (1,500) outweighs
    // Gadgets' (250) even though Gadgets' typical month is bigger.
    let spiky = &view.groups[1];
    let labels: Vec<&str> =
        spiky.categories.iter().map(|c| c.label.as_str()).collect();
    assert_eq!(labels, vec!["Vet", "Gadgets"]);
    assert_eq!(spiky.categories[0].avg, Some(dec("150")));
    assert_eq!(spiky.categories[1].avg, Some(dec("300")));

    // The displayed typical still honors the chosen basis: nothing in
    // the window means no typical, top slot or not.
    assert_eq!(old.categories[0].avg, None);
    assert_eq!(old.categories[1].avg, Some(dec("10")));
}

#[test]
fn month_view_aggregates_groups_and_totals() {
    let ledger = ledger();
    let view = ledger.month_view(m("2026-01"), 6, "USD");

    assert_eq!(view.income, dec("1000"));
    assert_eq!(view.spent, dec("196"));
    assert_eq!(view.typical, Some(dec("130")));

    // Biggest typical spend reads first: Fun averages 100 to Food's 30,
    // and groups the window knows nothing about sink to the bottom in
    // name order.
    let names: Vec<&str> =
        view.groups.iter().map(|g| g.name.as_str()).collect();
    assert_eq!(names, vec!["Fun", "Food", "Home", "Vacation"]);

    let food = &view.groups[1];
    assert_eq!(food.spent, dec("106"));
    assert_eq!(food.avg, Some(dec("30")));
    // Same rule inside the group: coffee has no average yet, so it
    // trails groceries despite being alphabetically first.
    let labels: Vec<&str> =
        food.categories.iter().map(|c| c.label.as_str()).collect();
    assert_eq!(labels, vec!["Groceries", "Dining · Coffee"]);

    let coffee = &food.categories[1];
    assert_eq!(coffee.spent, dec("6"));
    assert_eq!(coffee.split, vec![("BRL".to_string(), dec("30"))]);

    let groceries = &food.categories[0];
    assert_eq!(groceries.spent, dec("100"));
    assert_eq!(groceries.avg, Some(dec("30")));
    assert_eq!(groceries.status, Some(Status::Over));

    let games = &view.groups[0].categories[0];
    assert_eq!(games.spent, dec("90"));
    assert_eq!(games.avg, Some(dec("100")));
    assert_eq!(games.ratio, Some(dec("0.9")));
    assert_eq!(games.status, Some(Status::Warn));

    // A window that exists but never saw rent gives no typical.
    let rent = &view.groups[2].categories[0];
    assert_eq!(rent.avg, None);
    assert_eq!(rent.ratio, None);
    assert_eq!(rent.status, None);
}

#[test]
fn sidebar_balances_accumulate_and_convert() {
    let ledger = ledger();
    let view = ledger.month_view(m("2026-02"), 6, "USD");

    let budget: Vec<&str> = view
        .budget_accounts
        .iter()
        .map(|a| a.account.as_str())
        .collect();
    assert_eq!(
        budget,
        vec![
            "Assets:Cash",
            "Assets:US:BofA:Checking",
            "Liabilities:US:Chase:Slate"
        ]
    );
    let cash = &view.budget_accounts[0];
    assert_eq!(cash.balances, vec![("USD".to_string(), dec("-366"))]);
    assert_eq!(cash.converted, Some(dec("-366")));
    assert_eq!(view.budget_accounts[1].converted, Some(dec("500")));
    assert_eq!(view.budget_accounts[2].converted, Some(dec("-190")));

    let tracking: Vec<&str> = view
        .tracking_accounts
        .iter()
        .map(|a| a.account.as_str())
        .collect();
    // Assets:Points has no postings yet by February and Assets:Vault
    // never has any; zero balances stay out of the sidebar.
    assert_eq!(tracking, vec!["Assets:ETrade:VEA"]);
    let vea = &view.tracking_accounts[0];
    assert_eq!(vea.balances, vec![("VEA".to_string(), dec("2"))]);
    assert_eq!(vea.converted, None);

    // Hidden accounts never appear.
    assert!(
        !view
            .budget_accounts
            .iter()
            .any(|a| a.account == "Assets:Old")
    );
    assert!(
        !view
            .tracking_accounts
            .iter()
            .any(|a| a.account == "Assets:Old")
    );

    // March: the unconvertible VACHR balance shows natively.
    let march = ledger.month_view(m("2026-03"), 6, "USD");
    let points = march
        .tracking_accounts
        .iter()
        .find(|a| a.account == "Assets:Points")
        .unwrap();
    assert_eq!(points.balances, vec![("VACHR".to_string(), dec("-3"))]);
    assert_eq!(points.converted, None);
}

#[test]
fn category_view_lists_history_window_and_txns() {
    let ledger = ledger();
    let view = ledger
        .category_view("Expenses:Food:Groceries", m("2026-02"), 3, "USD")
        .unwrap();

    assert_eq!(view.label, "Groceries");
    assert_eq!(view.spent, dec("30"));
    assert_eq!(view.avg, Some(dec("65")));
    assert_eq!(view.status, Some(Status::Good));
    assert_eq!(view.window, Some((m("2025-12"), m("2026-01"))));

    let months: Vec<String> =
        view.history.iter().map(|p| p.month.to_string()).collect();
    assert_eq!(
        months,
        vec![
            "2025-09", "2025-10", "2025-11", "2025-12", "2026-01", "2026-02"
        ]
    );
    let spent: Vec<Decimal> = view.history.iter().map(|p| p.spent).collect();
    assert_eq!(
        spent,
        vec![
            dec("0"),
            dec("0"),
            dec("0"),
            dec("30"),
            dec("100"),
            dec("30")
        ]
    );

    assert_eq!(view.txns.len(), 2);
    assert_eq!(view.txns[1].narration.as_deref(), Some("Refund"));
    assert_eq!(view.split, vec![("USD".to_string(), dec("30"))]);

    assert!(
        ledger
            .category_view("Expenses:Nope", m("2026-02"), 3, "USD")
            .is_none()
    );
}

#[test]
fn sidebar_hides_accounts_with_all_zero_balances() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sidebar/main.beancount");
    let ledger = Ledger::build(load(&path).unwrap());
    let names = |view: &bean_core::query::MonthView| -> Vec<String> {
        view.budget_accounts
            .iter()
            .chain(&view.tracking_accounts)
            .map(|a| a.account.clone())
            .collect()
    };

    // January: the wallet holds money, the points balance is nonzero
    // even though PTS never converts, and the never-used account stays
    // out of the way.
    let jan = ledger.month_view(m("2026-01"), 6, "USD");
    assert_eq!(
        names(&jan),
        vec!["Assets:Cash", "Assets:Wallet", "Assets:Points"]
    );

    // February: the wallet went back to exactly zero, so it disappears;
    // the untouched points balance carries over and stays visible.
    let feb = ledger.month_view(m("2026-02"), 6, "USD");
    assert_eq!(names(&feb), vec!["Assets:Cash", "Assets:Points"]);
}

#[test]
fn views_quantize_money_at_the_leaves_so_sums_add_up() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/rounding/main.beancount");
    let ledger = Ledger::build(load(&path).unwrap());

    // Each category converts to 3.333 USD exactly; displayed cents must
    // sum to the displayed totals, so quantization happens at the leaf.
    let view = ledger.month_view(m("2026-01"), 6, "USD");
    for group in &view.groups {
        for c in &group.categories {
            assert!(c.spent.scale() <= 2, "{}: {}", c.account, c.spent);
        }
        assert_eq!(
            group.spent,
            group.categories.iter().map(|c| c.spent).sum::<Decimal>()
        );
    }
    assert_eq!(view.groups[0].spent, dec("6.66"));
    assert_eq!(view.spent, dec("9.99"));
    assert_eq!(view.income, dec("100.00"));

    // Typicals round at the leaf too, and skip no-spend months: only
    // January in the [Jan, Feb] window saw spending, so the typical is
    // January itself — 3.333 → 3.33 per category. The headline typical
    // medians the raw month totals instead of summing rounded leaves,
    // so it reads 9.999 → 10.00.
    let march = ledger.month_view(m("2026-03"), 3, "USD");
    let food = &march.groups[0];
    assert_eq!(food.categories[0].avg, Some(dec("3.33")));
    assert_eq!(food.avg, Some(dec("6.66")));
    assert_eq!(march.typical, Some(dec("10.00")));

    // The inspector shows the same cents as the table row.
    let cat = ledger
        .category_view("Expenses:Food:Dining", m("2026-01"), 6, "USD")
        .unwrap();
    assert_eq!(cat.spent, dec("3.33"));
    assert!(cat.history.iter().all(|p| p.spent.scale() <= 2));

    // Sidebar conversions are display values as well: the cash residual
    // is 90.0011 USD exactly, shown as 90.00.
    let cash = &view.budget_accounts[0];
    assert_eq!(cash.converted, Some(dec("90.00")));
    // Native balances stay exact — they are ledger truth, not display.
    assert_eq!(cash.balances, vec![("USD".to_string(), dec("90.0011"))]);
}

#[test]
fn months_range_and_default_month_clamp() {
    let ledger = ledger();

    // Today after the last transaction extends the range to today.
    let months = ledger.months_range((2026, 8, 21));
    assert_eq!(months.first().unwrap().to_string(), "2025-12");
    assert_eq!(months.last().unwrap().to_string(), "2026-08");
    assert_eq!(months.len(), 9);
    assert_eq!(ledger.default_month((2026, 8, 21)), m("2026-08"));

    // Today before the ledger clamps to the first month.
    let months = ledger.months_range((2025, 1, 15));
    assert_eq!(months.first().unwrap().to_string(), "2025-12");
    assert_eq!(months.last().unwrap().to_string(), "2026-03");
    assert_eq!(ledger.default_month((2025, 1, 15)), m("2025-12"));

    // Today inside the range is the default as-is.
    assert_eq!(ledger.default_month((2026, 2, 10)), m("2026-02"));
}

#[test]
fn account_view_reads_a_month_as_money_in_and_out() {
    let ledger = ledger();
    let view = ledger
        .account_view("Assets:Cash", m("2026-02"), 3, "USD")
        .unwrap();

    assert_eq!(view.label, "Cash");
    assert_eq!(view.kind, AccountKind::Budget);
    // February opened on what January closed at, took 20 back and let
    // 50 go. The header prints that as a sentence, so it has to add up.
    assert_eq!(view.opening, Some(dec("-336")));
    assert_eq!(view.inflow, dec("20"));
    assert_eq!(view.outflow, dec("50"));
    assert_eq!(view.converted, Some(dec("-366")));
    assert_eq!(view.balances, vec![("USD".to_string(), dec("-366"))]);
    assert!(view.unpriced.is_empty());

    // The chart's window is the basis, ending at the month on screen.
    let months: Vec<String> =
        view.history.iter().map(|p| p.month.to_string()).collect();
    assert_eq!(months, vec!["2025-12", "2026-01", "2026-02"]);
    let flows: Vec<(Decimal, Decimal)> =
        view.history.iter().map(|p| (p.inflow, p.outflow)).collect();
    assert_eq!(
        flows,
        vec![
            (dec("0"), dec("30")),
            (dec("0"), dec("306")),
            (dec("20"), dec("50")),
        ]
    );
    let closes: Vec<Option<Decimal>> =
        view.history.iter().map(|p| p.balance).collect();
    assert_eq!(
        closes,
        vec![Some(dec("-30")), Some(dec("-336")), Some(dec("-366"))]
    );

    // Every row carries the balance it left behind, so the register
    // reads down its last column the way a statement does.
    let rows: Vec<(&str, Option<Decimal>, Option<Decimal>)> = view
        .entries
        .iter()
        .map(|e| (e.txn.narration.as_deref().unwrap_or(""), e.delta, e.balance))
        .collect();
    assert_eq!(
        rows,
        vec![
            ("Feb shop", Some(dec("-50")), Some(dec("-386"))),
            ("Refund", Some(dec("20")), Some(dec("-366"))),
        ]
    );

    assert!(
        ledger
            .account_view("Assets:Nope", m("2026-02"), 3, "USD")
            .is_none()
    );
}

#[test]
fn account_view_leaves_a_hole_where_a_price_is_missing() {
    let ledger = ledger();
    let view = ledger
        .account_view("Assets:Points", m("2026-03"), 3, "USD")
        .unwrap();

    assert_eq!(view.kind, AccountKind::Tracking);
    assert_eq!(view.balances, vec![("VACHR".to_string(), dec("-3"))]);
    // Nothing prices a voucher hour. A zero in the money columns would
    // read as "nothing moved", which is the one thing that isn't true.
    assert_eq!(view.converted, None);
    assert_eq!(view.unpriced, vec!["VACHR".to_string()]);
    assert_eq!(view.inflow, dec("0"));
    assert_eq!(view.outflow, dec("0"));
    assert_eq!(view.entries.len(), 1);
    assert_eq!(view.entries[0].delta, None);
    assert_eq!(view.entries[0].balance, None);

    // Holding nothing yet is a balance of zero, not an unknown one:
    // the hole starts where the unpriced commodity does.
    assert_eq!(view.opening, Some(dec("0")));
    let closes: Vec<Option<Decimal>> =
        view.history.iter().map(|p| p.balance).collect();
    assert_eq!(closes, vec![Some(dec("0")), Some(dec("0")), None]);
}
