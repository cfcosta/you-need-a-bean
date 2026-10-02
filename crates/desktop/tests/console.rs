//! The SQL console as a reader drives it: `6` opens it with the editor
//! taking the keys, ctrl-enter runs what it holds, escape lets the page
//! keys back in.

mod common;

use std::sync::Arc;

use bean_desktop::{Page, Root};
use bean_sql::Cell;
use common::{TODAY, overview_ledger};
use gpui::{Focusable, TestAppContext};

fn open(
    cx: &mut TestAppContext,
) -> (gpui::Entity<Root>, &mut gpui::VisualTestContext) {
    cx.update(bean_desktop::init);
    let ledger = Arc::new(overview_ledger());
    let (root, cx) = cx.add_window_view(|_, cx| Root::new(ledger, TODAY, cx));
    cx.update(|window, cx| {
        let focus = root.read(cx).focus_handle(cx);
        window.focus(&focus, cx);
    });
    (root, cx)
}

fn text(root: &gpui::Entity<Root>, cx: &mut gpui::VisualTestContext) -> String {
    root.read_with(cx, |r, _| r.console.editor.text())
}

#[gpui::test]
fn six_opens_the_console_on_the_first_saved_query(cx: &mut TestAppContext) {
    let (root, cx) = open(cx);
    cx.simulate_keystrokes("6");
    assert_eq!(root.read_with(cx, |r, _| r.page), Page::Query);
    assert_eq!(
        root.read_with(cx, |r, _| r.console.title()),
        ("food-by-month".to_string(), false)
    );
    assert!(text(&root, cx).starts_with("SELECT date_trunc('month', date)"));
}

#[gpui::test]
fn the_editor_takes_the_keys_until_escape(cx: &mut TestAppContext) {
    let (root, cx) = open(cx);
    cx.simulate_keystrokes("6");
    root.update(cx, |r, cx| r.load_sql("", cx));
    cx.simulate_input("SELECT 1");
    cx.simulate_keystrokes("enter");
    cx.simulate_input("t2");
    assert_eq!(
        text(&root, cx),
        "SELECT 1\nt2",
        "t and 2 typed, not followed"
    );
    assert_eq!(root.read_with(cx, |r, _| r.page), Page::Query);
    cx.simulate_keystrokes("backspace left backspace");
    assert_eq!(text(&root, cx), "SELECT 1t");
    cx.simulate_keystrokes("escape 1");
    assert_eq!(root.read_with(cx, |r, _| r.page), Page::Overview);
}

#[gpui::test]
fn ctrl_enter_runs_the_query_and_remembers_it(cx: &mut TestAppContext) {
    let (root, cx) = open(cx);
    cx.simulate_keystrokes("6");
    cx.run_until_parked();
    root.update(cx, |r, cx| {
        r.load_sql("SELECT count(*) AS n FROM postings", cx)
    });
    cx.simulate_keystrokes("ctrl-enter");
    cx.run_until_parked();
    let (rows, history, running) = root.read_with(cx, |r, _| {
        let rows = r.console.result.clone().unwrap().unwrap().rows;
        (rows, r.console.history.len(), r.console.running)
    });
    assert!(matches!(rows[0][0], Cell::Int(n) if n > 100));
    assert_eq!(history, 2, "the first visit's answer, then this one");
    assert!(!running);
    assert!(
        !root.read_with(cx, |r, _| r.console.schema.is_empty()),
        "the schema list is read once the database is up"
    );
}

#[gpui::test]
fn a_saved_query_loads_into_the_editor(cx: &mut TestAppContext) {
    let (root, cx) = open(cx);
    cx.simulate_keystrokes("6");
    root.update(cx, |r, cx| r.open_saved(1, cx));
    assert_eq!(
        root.read_with(cx, |r, _| r.console.title()),
        ("top-payees".to_string(), false)
    );
    cx.simulate_keystrokes("ctrl-enter");
    cx.run_until_parked();
    let rows = root
        .read_with(cx, |r, _| r.console.result.clone().unwrap().unwrap().rows);
    assert_eq!(rows[0][0], Cell::Text("Willow House".into()));
}

#[gpui::test]
fn a_mistake_shows_and_its_suggestion_fixes_it(cx: &mut TestAppContext) {
    let (root, cx) = open(cx);
    cx.simulate_keystrokes("6");
    root.update(cx, |r, cx| {
        r.load_sql("SELECT *\nFROM posting\nLIMIT 20;", cx)
    });
    cx.simulate_keystrokes("ctrl-enter");
    cx.run_until_parked();
    let failure = root.read_with(cx, |r, _| r.console.result.clone().unwrap());
    assert_eq!(failure.unwrap_err().kind, "Catalog");
    root.update(cx, |r, cx| r.apply_suggestion(cx));
    cx.run_until_parked();
    assert_eq!(text(&root, cx), "SELECT *\nFROM postings\nLIMIT 20;");
    assert!(
        root.read_with(cx, |r, _| r.console.result.clone().unwrap().is_ok()),
        "the fixed query runs straight away"
    );
}

#[gpui::test]
fn opening_the_console_answers_its_first_query(cx: &mut TestAppContext) {
    let (root, cx) = open(cx);
    cx.simulate_keystrokes("6");
    cx.run_until_parked();
    let (running, result) = root
        .read_with(cx, |r, _| (r.console.running, r.console.result.clone()));
    assert!(!running);
    let rows = result.expect("an answer").expect("no failure").rows;
    assert_eq!(rows[0][1], Cell::Text("Expenses:Food:Groceries".into()));
}
