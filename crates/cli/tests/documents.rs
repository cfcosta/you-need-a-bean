//! Serving the files a `document` directive points at.

use std::path::PathBuf;
use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::http::{HeaderMap, Request, StatusCode};
use bean_cli::api::{AppState, router};
use bean_core::loader::load;
use bean_core::model::Ledger;
use serde_json::{Value, json};
use tower::ServiceExt;

fn app() -> Router {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../core/tests/fixtures/documents/main.beancount");
    let ledger = Ledger::build(load(&path).unwrap());
    let state = AppState::new(ledger, 1).with_today((2026, 1, 31));
    router(Arc::new(state))
}

async fn get(uri: &str) -> (StatusCode, HeaderMap, Vec<u8>) {
    let response = app()
        .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let bytes = axum::body::to_bytes(response.into_body(), 1 << 20)
        .await
        .unwrap();
    (status, headers, bytes.to_vec())
}

async fn json(uri: &str) -> Value {
    let (status, _, bytes) = get(uri).await;
    assert_eq!(status, StatusCode::OK, "{uri}");
    serde_json::from_slice(&bytes).unwrap()
}

fn header(headers: &HeaderMap, name: &str) -> String {
    headers
        .get(name)
        .map(|v| v.to_str().unwrap().to_owned())
        .unwrap_or_default()
}

/// The transaction the fixture hangs three documents off.
async fn subscription() -> Value {
    let body = json("/api/category/Expenses:Software/2026-01").await;
    body["txns"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["payee"] == json!("Acme"))
        .expect("the subscription")
        .clone()
}

#[tokio::test]
async fn a_transaction_carries_the_documents_it_produced() {
    let txn = subscription().await;
    let docs = txn["documents"].as_array().unwrap();
    let names: Vec<&str> =
        docs.iter().map(|d| d["name"].as_str().unwrap()).collect();
    assert_eq!(names, ["invoice.pdf", "gone.pdf", "statement.csv"]);

    // Which account each hangs off, so the panel can say why the card's
    // statement is sitting under a software expense.
    assert_eq!(docs[0]["account"], json!("Expenses:Software"));
    assert_eq!(docs[2]["account"], json!("Liabilities:Card"));

    // The full path, because the answer to "which file is this?" is a
    // path, and it is the thing a person copies out.
    assert!(
        docs[0]["path"]
            .as_str()
            .unwrap()
            .ends_with("core/tests/fixtures/documents/files/invoice.pdf"),
        "{}",
        docs[0]["path"],
    );
}

#[tokio::test]
async fn a_transaction_without_paperwork_carries_an_empty_list() {
    // Not a missing key: the UI should not have to tell absent from empty.
    let body = json("/api/category/Expenses:Coffee/2026-01").await;
    let txns = body["txns"].as_array().unwrap();
    let bank = json("/api/account/Assets:Bank/2026-01").await;
    assert!(txns.iter().all(|t| t["documents"].is_array()));
    assert!(
        bank["txns"]
            .as_array()
            .unwrap()
            .iter()
            .all(|t| t["documents"].is_array())
    );
}

#[tokio::test]
async fn serves_the_file_under_the_type_its_name_implies() {
    let txn = subscription().await;
    let id = txn["documents"][2]["id"].as_u64().unwrap();

    let (status, headers, bytes) = get(&format!("/api/document/{id}")).await;
    assert_eq!(status, StatusCode::OK);
    assert!(header(&headers, "content-type").starts_with("text/csv"));
    assert!(
        header(&headers, "content-disposition")
            .contains("inline; filename=\"statement.csv\""),
        "{}",
        header(&headers, "content-disposition"),
    );
    assert_eq!(
        String::from_utf8(bytes).unwrap(),
        "date,amount\n2026-01-05,120.00\n"
    );
}

#[tokio::test]
async fn serves_documents_sandboxed() {
    // Some ledgers contain saved HTML receipts. Served from the app's own
    // origin they could read and drive the app; the
    // sandbox directive drops them into an origin of their own, and
    // `nosniff` stops a mislabelled one from being reinterpreted.
    let txn = subscription().await;
    let id = txn["documents"][0]["id"].as_u64().unwrap();

    let (status, headers, _) = get(&format!("/api/document/{id}")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(header(&headers, "content-security-policy"), "sandbox");
    assert_eq!(header(&headers, "x-content-type-options"), "nosniff");
}

#[tokio::test]
async fn a_document_whose_file_is_gone_says_which_file() {
    let txn = subscription().await;
    let id = txn["documents"][1]["id"].as_u64().unwrap();

    let (status, _, bytes) = get(&format!("/api/document/{id}")).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let body: Value = serde_json::from_slice(&bytes).unwrap();
    assert!(
        body["error"].as_str().unwrap().contains("gone.pdf"),
        "{}",
        body["error"],
    );
}

#[tokio::test]
async fn an_id_the_ledger_never_handed_out_is_not_found() {
    // Ids are indexes into the loaded documents, so nothing a caller can
    // type reaches a file the ledger did not name.
    for uri in ["/api/document/9999", "/api/document/nope"] {
        let (status, _, _) = get(uri).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{uri}");
    }
}
