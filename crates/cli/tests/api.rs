use std::path::PathBuf;
use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use bean_cli::api::{AppState, router};
use bean_core::loader::load;
use bean_core::model::{Day, Ledger};
use serde_json::{Value, json};
use tower::ServiceExt;

fn app() -> Router {
    app_at("model/main", (2026, 8, 21))
}

/// A server over one of the test fixtures, with today pinned so the
/// trailing windows never move.
fn app_at(fixture: &str, today: Day) -> Router {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(format!("../core/tests/fixtures/{fixture}.beancount"));
    let ledger = Ledger::build(load(&path).unwrap());
    let state = AppState::new(ledger, 7).with_today(today);
    router(Arc::new(state))
}

async fn get(uri: &str) -> (StatusCode, Value) {
    get_at(app(), uri).await
}

async fn get_at(app: Router, uri: &str) -> (StatusCode, Value) {
    let response = app
        .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), 1 << 20)
        .await
        .unwrap();
    let value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, value)
}

#[tokio::test]
async fn summary_reports_ledger_shape() {
    let (status, body) = get("/api/summary").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["title"], json!("Model Ledger"));
    assert_eq!(body["root"], json!("main.beancount"));
    assert_eq!(body["files"], json!(1));
    assert_eq!(body["directives"], json!(30));
    assert_eq!(body["parse_ms"], json!(7));
    assert_eq!(body["operating_currencies"], json!(["USD"]));
    let months = body["months"].as_array().unwrap();
    assert_eq!(months.first().unwrap(), "2025-12");
    assert_eq!(months.last().unwrap(), "2026-08");
    assert_eq!(months.len(), 9);
    assert_eq!(body["today"], json!("2026-08-21"));
    assert_eq!(body["default_month"], json!("2026-08"));
}

#[tokio::test]
async fn month_endpoint_shapes_groups_and_accounts() {
    // No params: basis defaults to 6, cur to the operating currency.
    let (status, body) = get("/api/month/2026-01").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["month"], json!("2026-01"));
    assert_eq!(body["is_current"], json!(false));
    assert_eq!(body["day"], json!(31));
    assert_eq!(body["days_in_month"], json!(31));
    assert_eq!(body["income"], json!(1000.0));
    assert_eq!(body["spent"], json!(196.0));
    assert_eq!(body["typical"], json!(130.0));

    // Groups and categories arrive sorted by typical spend, biggest
    // first, so Fun (avg 100) precedes Food (avg 30).
    let food = &body["groups"][1];
    assert_eq!(food["name"], json!("Food"));
    assert_eq!(food["spent"], json!(106.0));
    assert_eq!(food["avg"], json!(30.0));
    let coffee = &food["categories"][1];
    assert_eq!(coffee["label"], json!("Dining · Coffee"));
    assert_eq!(coffee["split"], json!({"BRL": 30.0}));
    let groceries = &food["categories"][0];
    assert_eq!(groceries["account"], json!("Expenses:Food:Groceries"));
    assert_eq!(groceries["status"], json!("over"));
    assert_eq!(groceries["ratio"], json!(3.3333));

    let games = &body["groups"][0]["categories"][0];
    assert_eq!(games["ratio"], json!(0.9));
    assert_eq!(games["status"], json!("warn"));
    let rent = &body["groups"][2]["categories"][0];
    // No rent inside the window: no typical to show at all.
    assert_eq!(rent["avg"], Value::Null);
    assert_eq!(rent["status"], Value::Null);

    let cash = &body["accounts"]["budget"][0];
    assert_eq!(cash["account"], json!("Assets:Cash"));
    assert_eq!(cash["label"], json!("Cash"));
    // Cumulative through January: December's -30 plus January's -306.
    assert_eq!(cash["balances"], json!({"USD": -336.0}));
    assert_eq!(cash["converted"], json!(-336.0));
    let vea = &body["accounts"]["tracking"][0];
    assert_eq!(vea["account"], json!("Assets:ETrade:VEA"));
    assert_eq!(vea["converted"], Value::Null);

    // The current month reports today's progress through it.
    let (_, body) = get("/api/month/2026-08").await;
    assert_eq!(body["is_current"], json!(true));
    assert_eq!(body["day"], json!(21));
    assert_eq!(body["days_in_month"], json!(31));
}

#[tokio::test]
async fn category_endpoint_lists_txns() {
    let (status, body) =
        get("/api/category/Expenses:Food:Groceries/2026-02?basis=3").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["account"], json!("Expenses:Food:Groceries"));
    assert_eq!(body["label"], json!("Groceries"));
    assert_eq!(body["spent"], json!(30.0));
    assert_eq!(body["avg"], json!(65.0));
    assert_eq!(body["status"], json!("good"));
    assert_eq!(body["window"], json!(["2025-12", "2026-01"]));

    let history = body["history"].as_array().unwrap();
    assert_eq!(history.len(), 6);
    assert_eq!(history[5], json!({"month": "2026-02", "spent": 30.0}));

    let txns = body["txns"].as_array().unwrap();
    assert_eq!(txns.len(), 2);
    let refund = &txns[1];
    assert_eq!(refund["date"], json!("2026-02-14"));
    assert_eq!(refund["flag"], json!("*"));
    assert_eq!(refund["narration"], json!("Refund"));
    assert_eq!(refund["amount"], json!(-20.0));
    assert_eq!(refund["currency"], json!("USD"));
    assert_eq!(refund["converted"], json!(-20.0));
    assert_eq!(refund["tags"], json!([]));
    assert_eq!(refund["meta"], json!({}));
    let postings = refund["postings"].as_array().unwrap();
    assert_eq!(postings.len(), 2);
    assert_eq!(
        postings[0],
        json!({
            "account": "Expenses:Food:Groceries",
            "amount": -20.0,
            "currency": "USD"
        })
    );

    assert_eq!(body["split"], json!({"USD": 30.0}));
}

#[tokio::test]
async fn account_endpoint_reads_the_register() {
    let (status, body) = get("/api/account/Assets:Cash/2026-02?basis=3").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["account"], json!("Assets:Cash"));
    assert_eq!(body["label"], json!("Cash"));
    assert_eq!(body["kind"], json!("budget"));
    assert_eq!(body["opening"], json!(-336.0));
    assert_eq!(body["inflow"], json!(20.0));
    assert_eq!(body["outflow"], json!(50.0));
    assert_eq!(body["balance"], json!(-366.0));
    assert_eq!(body["balances"], json!({"USD": -366.0}));
    assert_eq!(body["unpriced"], json!([]));

    let history = body["history"].as_array().unwrap();
    assert_eq!(history.len(), 3);
    assert_eq!(
        history[2],
        json!({
            "month": "2026-02",
            "inflow": 20.0,
            "outflow": 50.0,
            "balance": -366.0
        })
    );

    // Each row is a full transaction plus the balance it left behind.
    let txns = body["txns"].as_array().unwrap();
    assert_eq!(txns.len(), 2);
    assert_eq!(txns[0]["narration"], json!("Feb shop"));
    assert_eq!(txns[0]["delta"], json!(-50.0));
    assert_eq!(txns[0]["balance"], json!(-386.0));
    assert_eq!(txns[1]["delta"], json!(20.0));
    assert_eq!(txns[1]["balance"], json!(-366.0));
    assert_eq!(txns[1]["postings"].as_array().unwrap().len(), 2);

    // An account holding something nothing prices says so, rather than
    // reporting a month in which nothing happened.
    let (status, body) = get("/api/account/Assets:Points/2026-03").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["kind"], json!("tracking"));
    assert_eq!(body["balance"], Value::Null);
    assert_eq!(body["unpriced"], json!(["VACHR"]));
    assert_eq!(body["txns"][0]["balance"], Value::Null);

    let (status, body) = get("/api/account/Assets:Nope/2026-01").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(body["error"].is_string());
}

#[tokio::test]
async fn reports_endpoint_shapes_series_and_fire() {
    let (status, body) = get("/api/reports").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["month"], json!("2026-08"));

    let net_worth = body["net_worth"].as_array().unwrap();
    assert_eq!(net_worth.len(), 9);
    // December: -30 cash, -100 on the card.
    assert_eq!(
        net_worth[0],
        json!({
            "month": "2025-12",
            "assets": -30.0,
            "cash": -30.0,
            "holdings": 0.0,
            "liabilities": -100.0,
            "net": -130.0
        })
    );
    // August carries balances forward; unpriced commodities (VEA,
    // VACHR) stay out of the convertible total.
    assert_eq!(net_worth[8]["month"], json!("2026-08"));
    assert_eq!(net_worth[8]["net"], json!(-56.0));

    // The commodities left out of that total are named rather than
    // silently dropped, and while any is missing the growth split
    // withholds the return it cannot compute honestly.
    assert_eq!(body["unpriced"], json!(["VACHR", "VEA"]));
    assert_eq!(body["growth"]["unpriced"], json!(["VACHR", "VEA"]));
    assert_eq!(body["growth"]["implied_return"], json!(null));

    // Every month's move splits into saving, capital in, and the rest.
    let growth = body["growth"]["points"].as_array().unwrap();
    assert_eq!(growth.len(), 9);
    for p in growth {
        let f = |k: &str| p[k].as_f64().unwrap();
        assert!(
            (f("saved") + f("equity") + f("market") - f("delta")).abs() < 1e-9,
            "{p} does not reconcile"
        );
    }

    // Recurring charges are no longer a report of their own: what
    // survives is the fixed nut the lean FIRE target and the lean
    // runway are built on, and the share of spending it accounts for
    // so those two are never read alone. The charge-by-charge list is
    // gone from the payload because nothing draws it.
    let rec = &body["recurring"];
    assert!(rec["monthly_fixed"].is_number());
    assert!(rec["annual_fixed"].is_number());
    assert_eq!(rec["items"], Value::Null);

    // Runway is cash against the same monthly spend FIRE uses.
    let runway = &body["runway"];
    assert!(runway["liquid"].is_number());
    assert_eq!(runway["lean_months"], json!(null));

    // Coasting is priced at the same three rates as the saving path.
    let rates = |k: &str| -> Vec<f64> {
        body["fire"][k]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s["rate"].as_f64().unwrap())
            .collect()
    };
    assert_eq!(rates("coast"), rates("scenarios"));
    assert_eq!(body["fire"]["lean_number"], json!(null));

    let cashflow = body["cashflow"].as_array().unwrap();
    assert_eq!(cashflow.len(), 9);
    assert_eq!(
        cashflow[1],
        json!({
            "month": "2026-01",
            "income": 1000.0,
            "expenses": 196.0,
            "net": 804.0
        })
    );

    let fire = &body["fire"];
    assert_eq!(fire["window"], json!(["2026-02", "2026-07"]));
    // Feb spent 530, nothing since: 530 / 6.
    assert_eq!(fire["monthly_spend"], json!(88.33));
    assert_eq!(fire["annual_spend"], json!(1059.96));
    assert_eq!(fire["fire_number"], json!(26499.0));
    assert_eq!(fire["net_worth"], json!(-56.0));
    assert_eq!(fire["progress"], json!(-0.0021));
    assert_eq!(fire["monthly_savings"], json!(-88.33));
    assert_eq!(fire["swr_monthly"], json!(-0.19));
    let scenarios = fire["scenarios"].as_array().unwrap();
    assert_eq!(scenarios.len(), 3);
    assert_eq!(scenarios[1]["rate"], json!(0.05));
    // Negative savings and negative net worth: FIRE never arrives.
    assert_eq!(scenarios[1]["months"], Value::Null);

    let year = &body["year"];
    assert_eq!(year["window"], json!(["2025-12", "2026-07"]));
    // Eight months of history is not a year, so there is nothing to
    // compare it against and every prior comes back null rather than a
    // number measured off however much ledger happens to precede it.
    assert_eq!(year["prior_window"], Value::Null);
    // The whole window month by month, empty ones included: this is the
    // axis every group's strip is indexed by, so it has to stay as long
    // as the window or the cells slide out from under their months.
    assert_eq!(
        year["months"],
        json!([
            {"month": "2025-12", "total": 130.0, "prior": null},
            {"month": "2026-01", "total": 196.0, "prior": null},
            {"month": "2026-02", "total": 530.0, "prior": null},
            {"month": "2026-03", "total": 0.0, "prior": null},
            {"month": "2026-04", "total": 0.0, "prior": null},
            {"month": "2026-05", "total": 0.0, "prior": null},
            {"month": "2026-06", "total": 0.0, "prior": null},
            {"month": "2026-07", "total": 0.0, "prior": null},
        ])
    );
    // The median of the months that saw spending, so the five empty
    // ones on the end cannot drag what a month usually costs to zero.
    assert_eq!(year["typical"], json!(196.0));
    // Vacation is absent: its only spending is VACHR with no price,
    // which never converts.
    assert_eq!(
        year["groups"],
        json!([
            {
                "name": "Home",
                "total": 500.0,
                "prior": null,
                "typical": 500.0,
                "monthly": [0.0, 0.0, 500.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            },
            {
                "name": "Fun",
                "total": 190.0,
                "prior": null,
                "typical": 95.0,
                "monthly": [100.0, 90.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            },
            {
                "name": "Food",
                "total": 166.0,
                "prior": null,
                "typical": 30.0,
                "monthly": [30.0, 106.0, 30.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            },
        ])
    );

    // Income split by source, with the passive slice inside each
    // total rather than beside it.
    let income = &body["income"];
    let shares: f64 = income["sources"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| {
            assert!(
                s["passive"].as_f64().unwrap() <= s["total"].as_f64().unwrap()
            );
            s["share"].as_f64().unwrap()
        })
        .sum();
    if income["total"].as_f64().unwrap() > 0.0 {
        assert!((shares - 1.0).abs() < 0.01, "shares summed to {shares}");
        assert!(income["effective_sources"].as_f64().unwrap() >= 1.0);
    }

    // Every tag and link that spans more than one transaction and
    // moved money, with what it took after refunds.
    let projects = &body["projects"];
    assert!(projects["singletons"].is_number());
    assert!(projects["markers"].is_number());
    for p in projects["items"].as_array().unwrap() {
        assert!(p["count"].as_u64().unwrap() >= 2);
        assert!(p["months"].as_u64().unwrap() >= 1);
        assert!(["#", "^"].contains(&p["sigil"].as_str().unwrap()));
        let (spent, income) =
            (p["spent"].as_f64().unwrap(), p["income"].as_f64().unwrap());
        assert!((p["net"].as_f64().unwrap() - (spent - income)).abs() < 1e-9);
    }

    // Six whole months do fit, so the quarters compare even though the
    // years cannot.
    let movers = &body["movers"];
    assert_eq!(movers["recent"], json!(["2026-05", "2026-07"]));
    assert_eq!(movers["prior"], json!(["2026-02", "2026-04"]));
    // Nothing at all was spent in the last three months, so every
    // category that had been running reads as the fall it is — and
    // ranked by money, rent leads groceries.
    assert_eq!(
        movers["items"],
        json!([
            {
                "account": "Expenses:Home:Rent",
                "label": "Monthly Rent",
                "group": "Home",
                "recent": 0.0,
                "prior": 500.0,
                "delta": -500.0,
                "ratio": -1.0,
            },
            {
                "account": "Expenses:Food:Groceries",
                "label": "Groceries",
                "group": "Food",
                "recent": 0.0,
                "prior": 30.0,
                "delta": -30.0,
                "ratio": -1.0,
            },
        ])
    );
}

#[tokio::test]
async fn bad_params_and_unknown_routes_error_as_json() {
    let (status, body) = get("/api/month/2026-13").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body["error"].is_string());

    let (status, _) = get("/api/month/garbage").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _) = get("/api/month/2026-01?basis=5").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _) = get("/api/month/2026-01?cur=usd").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    let (status, body) = get("/api/category/Expenses:Nope/2026-01").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(body["error"].is_string());

    let (status, body) = get("/api/definitely-not-a-route").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(body["error"].is_string());
}

#[tokio::test]
async fn reports_endpoint_names_what_it_is_unsure_of() {
    // The default fixture holds nothing it can price badly: what it
    // holds beyond dollars has no price at all, which the page reports
    // as missing rather than as out of date.
    let (_, body) = get("/api/reports").await;
    assert_eq!(body["trust"]["stale"], json!([]));
    assert_eq!(body["unpriced"], json!(["VACHR", "VEA"]));

    let app = app_at("reports/trust", (2026, 4, 15));
    let (status, body) = get_at(app, "/api/reports").await;
    assert_eq!(status, StatusCode::OK);
    let trust = &body["trust"];
    assert_eq!(trust["window"], json!(["2025-04", "2026-03"]));
    assert_eq!(
        trust["stale"],
        json!([{
            "commodity": "GOLD",
            "last": "2025-01-15",
            "days": 455,
            "value": 20000.0,
        }])
    );
    assert_eq!(
        trust["flagged"],
        json!({
            "total": 3,
            "window": 2,
            "amount": 1500.0,
            "recent": [
                {
                    "date": "2026-01-15",
                    "payee": "Diner",
                    "narration": "Is this the same place?",
                    "amount": 1000.0,
                },
                {
                    "date": "2025-12-05",
                    "payee": "Someone",
                    "narration": "Card charge, unidentified",
                    "amount": 500.0,
                },
                {
                    "date": "2025-02-10",
                    "payee": "Nobody",
                    "narration": "Left over from the old importer",
                    "amount": 250.0,
                },
            ],
        })
    );
    assert_eq!(
        trust["uncategorized"],
        json!({
            "total": 500.0,
            "share": 0.05,
            "accounts": ["Expenses:Uncategorized"],
        })
    );
    assert_eq!(trust["warnings"], json!([]));
}

#[tokio::test]
async fn reports_endpoint_carries_the_seasonal_shape() {
    // Four months of ledger is not a shape, and the endpoint says so
    // in the null rather than in twelve zeroes.
    let (_, body) = get("/api/reports").await;
    assert_eq!(body["season"]["years"], Value::Null);
    assert_eq!(body["season"]["months"], json!([]));
    assert_eq!(body["season"]["projected"], Value::Null);

    let app = app_at("reports/season", (2026, 4, 15));
    let (status, body) = get_at(app, "/api/reports").await;
    assert_eq!(status, StatusCode::OK);
    let season = &body["season"];
    assert_eq!(season["years"], json!([2021, 2025]));
    assert_eq!(season["typical"], json!(1600.0));
    assert_eq!(season["year"], json!(2026));
    assert_eq!(season["elapsed"], json!(3));
    assert_eq!(season["ytd"], json!(450.0));
    assert_eq!(season["pace"], json!(1.5));
    assert_eq!(season["projected"], json!(2400.0));

    let months = season["months"].as_array().unwrap();
    assert_eq!(months.len(), 12);
    assert_eq!(
        months[0],
        json!({
            "month": 1,
            "median": 100.0,
            "share": 0.0625,
            "samples": 5,
            "actual": 150.0,
        })
    );
    assert_eq!(
        months[11],
        json!({
            "month": 12,
            "median": 400.0,
            "share": 0.25,
            "samples": 5,
            "actual": null,
        })
    );
}

#[tokio::test]
async fn reports_endpoint_ranks_the_payees() {
    let app = app_at("reports/payees", (2026, 4, 15));
    let (status, body) = get_at(app, "/api/reports").await;
    assert_eq!(status, StatusCode::OK);
    let payees = &body["payees"];
    assert_eq!(payees["window"], json!(["2025-04", "2026-03"]));
    assert_eq!(payees["total"], json!(8000.0));
    // The names past the cut ride along rather than being counted,
    // so the line standing in for them can open onto them.
    let others = payees["others"].as_array().unwrap();
    assert_eq!(others.len(), 1);
    assert_eq!(others[0]["name"], json!("Vendor L"));
    assert_eq!(others[0]["spent"], json!(45.0));
    assert_eq!(payees["others_spent"], json!(45.0));
    assert_eq!(payees["anonymous"], json!(630.0));
    assert_eq!(payees["anonymous_count"], json!(3));

    let items = payees["items"].as_array().unwrap();
    assert_eq!(items.len(), 15);
    assert_eq!(
        items[0],
        json!({
            "name": "Airline",
            "spent": 3000.0,
            "count": 1,
            "average": 3000.0,
            "share": 0.375,
            "categories": 1,
            "months": 1,
            "first": "2025-07-20",
            "last": "2025-07-20",
        })
    );
    assert_eq!(
        items[1],
        json!({
            "name": "Supermarket",
            "spent": 2400.0,
            "count": 12,
            "average": 200.0,
            "share": 0.3,
            "categories": 1,
            "months": 12,
            "first": "2025-04-05",
            "last": "2026-03-05",
        })
    );
}

#[tokio::test]
async fn reports_endpoint_details_the_positions_held() {
    let app = app_at("reports/investments", (2026, 6, 15));
    let (status, body) = get_at(app, "/api/reports").await;
    assert_eq!(status, StatusCode::OK);
    let inv = &body["investments"];

    assert_eq!(inv["total"], json!(6080.0));
    assert_eq!(inv["basis"], json!(4530.0));
    assert_eq!(inv["based_value"], json!(4580.0));
    assert_eq!(inv["gain"], json!(50.0));
    assert_eq!(inv["ret"], json!(0.011));
    assert_eq!(inv["coverage"], json!(0.7533));
    assert_eq!(inv["unbased"], json!(1500.0));
    assert_eq!(inv["unbased_count"], json!(2));
    assert_eq!(inv["effective"], json!(3.02));
    let dust = inv["dust"].as_array().unwrap();
    assert_eq!(dust.len(), 1);
    assert_eq!(dust[0]["currency"], json!("DUST"));
    assert_eq!(inv["dust_value"], json!(0.0));
    assert_eq!(inv["unpriced"], json!(["GOLD"]));

    let items = inv["items"].as_array().unwrap();
    assert_eq!(items.len(), 4);
    assert_eq!(
        items[0],
        json!({
            "currency": "VTI",
            "label": "Vanguard Total Stock Market ETF",
            "class": "etf",
            "units": 24.0,
            "price": 120.0,
            "value": 2880.0,
            "share": 0.4737,
            "basis": 2480.0,
            "gain": 400.0,
            "ret": 0.1613,
            "accounts": 1,
            "postings": 3,
            "first": "2026-01-10",
            "last": "2026-03-10",
        })
    );
    // Fractional units survive the trip: rounding a holding to cents
    // reports three quarters of an ETH as 0.75 and a ten-thousandth of
    // a BTC as nothing at all.
    assert_eq!(items[1]["currency"], json!("ETH"));
    assert_eq!(items[1]["units"], json!(0.75));
    // Nothing bought it, so nothing is reported about what it cost.
    assert_eq!(items[1]["basis"], Value::Null);
    assert_eq!(items[1]["gain"], Value::Null);
    assert_eq!(items[1]["ret"], Value::Null);
    // Priced and undeclared: the ticker is the whole label.
    assert_eq!(items[3]["label"], json!("BOND"));
    assert_eq!(items[3]["class"], Value::Null);

    let classes = inv["classes"].as_array().unwrap();
    assert_eq!(classes.len(), 4);
    assert_eq!(
        classes[0],
        json!({ "name": "etf", "value": 2880.0, "share": 0.4737, "positions": 1 })
    );
    assert_eq!(classes[1]["name"], json!("crypto"));
    assert_eq!(classes[1]["positions"], json!(2));
    assert_eq!(classes[3]["name"], Value::Null);
}
