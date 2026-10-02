//! The ledger as DuckDB tables: every posting arrives, the views agree
//! with the ledger's own sums, and a query comes back as typed cells.

use std::{path::PathBuf, sync::Arc, time::Duration};

use bean_core::{
    loader::load,
    model::{Ledger, MonthKey},
};
use bean_sql::{Cell, Database};
use rust_decimal::Decimal;

fn ledger(path: &str) -> Ledger {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(path);
    Ledger::build(load(&path).expect("the fixture loads"))
}

fn small() -> Database {
    Database::load(&ledger("tests/fixtures/small.beancount")).unwrap()
}

fn d(s: &str) -> Decimal {
    s.parse().unwrap()
}

/// The single value a query answers with.
fn one(db: &Database, sql: &str) -> Cell {
    let answer = db.run(sql, 10).unwrap_or_else(|e| panic!("{sql}: {e:?}"));
    answer.rows[0][0].clone()
}

#[test]
fn every_posting_becomes_a_row_with_its_transaction() {
    let db = small();
    assert_eq!(one(&db, "SELECT count(*) FROM postings"), Cell::Int(8));
    assert_eq!(one(&db, "SELECT count(*) FROM txns"), Cell::Int(4));
    assert_eq!(
        one(
            &db,
            "SELECT amount FROM postings WHERE account = 'Assets:Bank' \
             AND payee = 'Payroll'"
        ),
        Cell::Decimal(d("1000.00")),
        "the elided posting carries the balancing residual"
    );
    assert_eq!(
        one(
            &db,
            "SELECT kind FROM postings WHERE account = 'Income:Salary'"
        ),
        Cell::Text("Income".into())
    );
    assert_eq!(
        one(&db, "SELECT flag FROM txns WHERE payee = 'Bakery'"),
        Cell::Text("!".into())
    );
}

#[test]
fn tags_links_and_metadata_are_tables_of_their_own() {
    let db = small();
    assert_eq!(
        one(&db, "SELECT tag FROM tags JOIN txns USING (txn_id)"),
        Cell::Text("work".into())
    );
    assert_eq!(
        one(&db, "SELECT link FROM links"),
        Cell::Text("pay-1".into())
    );
    assert_eq!(
        one(&db, "SELECT value FROM meta WHERE key = 'source'"),
        Cell::Text("bank".into())
    );
}

#[test]
fn prices_accounts_and_commodities_are_there_to_join() {
    let db = small();
    assert_eq!(
        one(&db, "SELECT rate FROM prices ORDER BY date DESC LIMIT 1"),
        Cell::Decimal(d("1.20"))
    );
    assert_eq!(
        one(&db, "SELECT name FROM commodities WHERE currency = 'EUR'"),
        Cell::Text("Euro".into())
    );
    assert_eq!(one(&db, "SELECT count(*) FROM accounts"), Cell::Int(4));
}

#[test]
fn converted_values_each_posting_at_the_rate_of_its_day() {
    let db = small();
    let answer = db
        .run(
            "SELECT date, value FROM converted \
             WHERE account = 'Expenses:Food' ORDER BY date",
            10,
        )
        .unwrap();
    let values: Vec<_> = answer.rows.iter().map(|r| r[1].clone()).collect();
    assert_eq!(
        values,
        [
            Cell::Decimal(d("22.00")),
            Cell::Decimal(d("12.00")),
            Cell::Decimal(d("4.50")),
        ]
    );
    assert_eq!(answer.rows[0][0], Cell::Date((2026, 1, 10)));
}

#[test]
fn balances_run_through_each_account_in_date_order() {
    let db = small();
    let answer = db
        .run(
            "SELECT balance FROM balances WHERE account = 'Assets:Bank' \
             ORDER BY date, txn_id",
            10,
        )
        .unwrap();
    let running: Vec<_> = answer.rows.iter().map(|r| r[0].clone()).collect();
    assert_eq!(
        running,
        [Cell::Decimal(d("1000.00")), Cell::Decimal(d("995.50"))]
    );
}

/// The `monthly` view must agree with the ledger's own index for every
/// account, month and currency, on a ledger shaped like a real one.
#[test]
fn monthly_agrees_with_the_ledger_everywhere() {
    for path in [
        "../../examples/overview.beancount",
        "../desktop/tests/fixtures/rough.beancount",
    ] {
        let ledger = ledger(path);
        let db = Database::load(&ledger).unwrap();
        let answer = db
            .run(
                "SELECT account, month, currency, amount FROM monthly",
                1_000_000,
            )
            .unwrap();
        let mut checked = 0;
        for row in &answer.rows {
            let (
                Cell::Text(account),
                Cell::Date((y, m, _)),
                Cell::Text(cur),
                Cell::Decimal(sum),
            ) = (&row[0], &row[1], &row[2], &row[3])
            else {
                panic!("unexpected row {row:?}");
            };
            assert_eq!(
                *sum,
                ledger.sum(account, MonthKey::new(*y, *m), cur),
                "{path}: {account} {y}-{m} {cur}"
            );
            checked += 1;
        }
        let expected: std::collections::HashSet<_> = ledger
            .txns
            .iter()
            .flat_map(|t| {
                t.postings.iter().flat_map(move |p| {
                    p.amounts.iter().map(move |(_, c)| {
                        (p.account.clone(), t.date.0, t.date.1, c.clone())
                    })
                })
            })
            .collect();
        let expected = expected.len();
        assert_eq!(checked, expected, "{path}: every month of every account");
    }
}

#[test]
fn a_capped_answer_still_counts_every_row() {
    let db = small();
    let answer = db.run("SELECT * FROM range(1000)", 50).unwrap();
    assert_eq!(answer.rows.len(), 50);
    assert_eq!(answer.total, 1000);
    assert_eq!(answer.columns[0].name, "range");
    assert_eq!(answer.columns[0].ty, "BIGINT");
}

#[test]
fn columns_carry_their_types() {
    let db = small();
    let answer = db
        .run(
            "SELECT date, payee, amount, NULL AS nothing FROM postings LIMIT 1",
            10,
        )
        .unwrap();
    let types: Vec<_> = answer.columns.iter().map(|c| c.ty.as_str()).collect();
    assert_eq!(types, ["DATE", "VARCHAR", "DECIMAL(38,18)", "INTEGER"]);
    assert_eq!(answer.rows[0][3], Cell::Null);
}

#[test]
fn a_mistake_comes_back_as_duckdbs_own_error() {
    let db = small();
    let failure = db.run("SELECT *\nFROM posting\nLIMIT 20;", 10).unwrap_err();
    assert_eq!(failure.kind, "Catalog");
    assert!(
        failure
            .message
            .contains("Table with name posting does not exist"),
        "{}",
        failure.message
    );
    assert!(failure.message.contains("postings"), "it suggests the fix");
}

#[test]
fn the_schema_lists_tables_then_views_with_their_columns() {
    let db = small();
    let schema = db.schema().unwrap();
    let names: Vec<_> =
        schema.iter().map(|r| (r.name.as_str(), r.view)).collect();
    assert_eq!(
        names,
        [
            ("postings", false),
            ("txns", false),
            ("tags", false),
            ("links", false),
            ("meta", false),
            ("prices", false),
            ("accounts", false),
            ("commodities", false),
            ("converted", true),
            ("monthly", true),
            ("balances", true),
        ]
    );
    let postings = &schema[0];
    assert_eq!(postings.rows, Some(8));
    assert_eq!(postings.columns[0].name, "txn_id");
    assert!(schema[8].rows.is_none(), "views are not counted");
}

#[test]
fn an_interrupt_stops_a_long_query() {
    let db = Arc::new(std::sync::Mutex::new(small()));
    let handle = db.lock().unwrap().interrupt_handle();
    let runner = {
        let db = db.clone();
        std::thread::spawn(move || {
            db.lock()
                .unwrap()
                .run("SELECT count(*) FROM range(100000000000) a", 10)
        })
    };
    std::thread::sleep(Duration::from_millis(200));
    handle.interrupt();
    let failure = runner.join().unwrap().unwrap_err();
    assert!(
        failure.message.to_lowercase().contains("interrupt"),
        "{failure:?}"
    );
    assert!(
        db.lock().unwrap().run("SELECT 1", 1).is_ok(),
        "the database still answers afterwards"
    );
}
