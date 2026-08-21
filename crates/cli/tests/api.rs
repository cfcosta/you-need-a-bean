use std::path::PathBuf;
use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use bean_cli::api::{AppState, router};
use bean_core::loader::load;
use bean_core::model::Ledger;
use serde_json::{Value, json};
use tower::ServiceExt;

fn app() -> Router {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../core/tests/fixtures/model/main.beancount");
    let ledger = Ledger::build(load(&path).unwrap());
    let state = AppState::new(ledger, 7).with_today((2026, 8, 21));
    router(Arc::new(state))
}

async fn get(uri: &str) -> (StatusCode, Value) {
    let response = app()
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
    assert_eq!(body["directives"], json!(28));
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

    let food = &body["groups"][0];
    assert_eq!(food["name"], json!("Food"));
    assert_eq!(food["spent"], json!(106.0));
    assert_eq!(food["avg"], json!(30.0));
    let coffee = &food["categories"][0];
    assert_eq!(coffee["label"], json!("Dining · Coffee"));
    assert_eq!(coffee["split"], json!({"BRL": 30.0}));
    let groceries = &food["categories"][1];
    assert_eq!(groceries["account"], json!("Expenses:Food:Groceries"));
    assert_eq!(groceries["status"], json!("over"));
    assert_eq!(groceries["ratio"], json!(3.3333));

    let games = &body["groups"][1]["categories"][0];
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
            "liabilities": -100.0,
            "net": -130.0
        })
    );
    // August carries balances forward; unpriced commodities (VEA,
    // VACHR) stay out of the convertible total.
    assert_eq!(net_worth[8]["month"], json!("2026-08"));
    assert_eq!(net_worth[8]["net"], json!(-56.0));

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
