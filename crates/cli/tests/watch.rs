//! The server follows the ledger on disk: edit a file it read and the
//! next request answers from the new one, without dropping what it has
//! when an edit leaves the ledger unparseable.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::body::Body;
use axum::http::Request;
use bean_cli::api::{AppState, router};
use bean_cli::watch::follow;
use bean_core::loader::load;
use bean_core::model::Ledger;
use serde_json::{Value, json};
use tower::util::ServiceExt;

const POLL: Duration = Duration::from_millis(15);

const ROOT: &str = "\
option \"title\" \"Watched\"
option \"operating_currency\" \"USD\"

include \"august.beancount\"
";

const AUGUST: &str = "\
2026-08-02 * \"Baker\" \"Bread\"
  Expenses:Food:Groceries   12.00 USD
  Assets:Bank:Checking
";

/// A directory of this test's own, emptied first so a previous run cannot
/// leak into this one.
fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir()
        .join(format!("ynab-follow-{}", std::process::id()))
        .join(name);
    fs::remove_dir_all(&dir).ok();
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn serve(root: &Path) -> Arc<AppState> {
    let ledger = Ledger::build(load(root).expect("the fixture loads"));
    Arc::new(AppState::new(ledger, 0).with_today((2026, 8, 21)))
}

async fn summary(state: &Arc<AppState>) -> Value {
    let app: Router = router(Arc::clone(state));
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/summary")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let bytes = axum::body::to_bytes(response.into_body(), 1 << 20)
        .await
        .unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

/// Poll the summary until `done` accepts it, or give up. Generous enough
/// that a slow machine does not fail, short enough that a broken watcher
/// does not hang the suite.
async fn until(
    state: &Arc<AppState>,
    what: &str,
    done: impl Fn(&Value) -> bool,
) -> Value {
    for _ in 0..400 {
        let body = summary(state).await;
        if done(&body) {
            return body;
        }
        tokio::time::sleep(POLL).await;
    }
    panic!("timed out waiting for {what}: {:?}", summary(state).await);
}

#[tokio::test]
async fn picks_up_an_edit_to_an_included_file() {
    let dir = scratch("edit");
    let root = dir.join("index.beancount");
    fs::write(&root, ROOT).unwrap();
    fs::write(dir.join("august.beancount"), AUGUST).unwrap();

    let state = serve(&root);
    let before = summary(&state).await;
    assert_eq!(before["revision"], json!(0));
    assert_eq!(before["reload_error"], Value::Null);
    let directives = before["directives"].as_u64().unwrap();

    tokio::spawn(follow(root.clone(), Arc::clone(&state), POLL));

    fs::write(
        dir.join("august.beancount"),
        format!(
            "{AUGUST}
2026-08-03 * \"Baker\" \"More bread\"
  Expenses:Food:Groceries   8.00 USD
  Assets:Bank:Checking
"
        ),
    )
    .unwrap();

    let after = until(&state, "the edit to be picked up", |b| {
        b["revision"] == json!(1)
    })
    .await;
    assert_eq!(after["directives"], json!(directives + 1));
    assert_eq!(after["reload_error"], Value::Null);
}

#[tokio::test]
async fn follows_a_file_that_was_only_just_included() {
    let dir = scratch("new-include");
    let root = dir.join("index.beancount");
    fs::write(&root, ROOT).unwrap();
    fs::write(dir.join("august.beancount"), AUGUST).unwrap();

    let state = serve(&root);
    assert_eq!(summary(&state).await["files"], json!(2));

    tokio::spawn(follow(root.clone(), Arc::clone(&state), POLL));

    // Writing a file nobody mentions changes nothing; naming it does.
    fs::write(dir.join("september.beancount"), "; nothing yet\n").unwrap();
    fs::write(&root, format!("{ROOT}include \"september.beancount\"\n"))
        .unwrap();
    until(&state, "the new include", |b| b["files"] == json!(3)).await;

    // And from then on that file is watched like any other.
    fs::write(
        dir.join("september.beancount"),
        "2026-09-01 * \"Baker\" \"Bread\"
  Expenses:Food:Groceries   5.00 USD
  Assets:Bank:Checking
",
    )
    .unwrap();
    let after = until(&state, "the new file to be followed", |b| {
        b["revision"] == json!(2)
    })
    .await;
    assert!(
        after["months"]
            .as_array()
            .unwrap()
            .contains(&json!("2026-09")),
        "September should be in range now: {after:?}"
    );
}

#[tokio::test]
async fn keeps_the_last_good_ledger_when_an_edit_breaks_it() {
    let dir = scratch("broken");
    let root = dir.join("index.beancount");
    fs::write(&root, ROOT).unwrap();
    fs::write(dir.join("august.beancount"), AUGUST).unwrap();

    let state = serve(&root);
    let before = summary(&state).await;

    tokio::spawn(follow(root.clone(), Arc::clone(&state), POLL));

    fs::write(dir.join("august.beancount"), "2026-08-02 * this is not\n")
        .unwrap();
    let broken = until(&state, "the failure to be reported", |b| {
        b["reload_error"] != Value::Null
    })
    .await;

    // Still answering from the ledger that last loaded cleanly.
    assert_eq!(broken["revision"], json!(0));
    assert_eq!(broken["directives"], before["directives"]);
    let message = broken["reload_error"].as_str().unwrap();
    assert!(message.contains("august.beancount"), "{message}");

    // And it recovers on its own once the file parses again.
    fs::write(dir.join("august.beancount"), AUGUST).unwrap();
    let fixed = until(&state, "the ledger to recover", |b| {
        b["revision"] == json!(1)
    })
    .await;
    assert_eq!(fixed["reload_error"], Value::Null);
    assert_eq!(fixed["directives"], before["directives"]);
}
