//! `j` and `k` walk the rows of the open page's main table; `enter`
//! does what the row leads to. The highlighted row stays in view.

mod common;

use std::sync::Arc;

use bean_desktop::{Page, Root};
use common::{TODAY, overview_ledger};
use gpui::{Focusable, TestAppContext, px, size};

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

fn row(
    root: &gpui::Entity<Root>,
    cx: &mut gpui::VisualTestContext,
) -> Option<usize> {
    root.read_with(cx, |r, _| r.row)
}

#[gpui::test]
fn j_and_k_walk_the_days_ahead_and_stop_at_the_ends(cx: &mut TestAppContext) {
    let (root, cx) = open(cx);
    root.update(cx, |r, cx| r.set_horizon(2, cx));
    assert_eq!(row(&root, cx), None, "nothing is lit until asked");
    cx.simulate_keystrokes("j");
    assert_eq!(row(&root, cx), Some(0));
    cx.simulate_keystrokes("j j k");
    assert_eq!(row(&root, cx), Some(1));
    cx.simulate_keystrokes("k k k");
    assert_eq!(row(&root, cx), Some(0));
    let count = root.update(cx, |r, _| r.row_count());
    assert!(count > 3);
    for _ in 0..count + 5 {
        cx.simulate_keystrokes("j");
    }
    assert_eq!(row(&root, cx), Some(count - 1));
}

#[gpui::test]
fn another_page_starts_with_nothing_lit(cx: &mut TestAppContext) {
    let (root, cx) = open(cx);
    cx.simulate_keystrokes("j j 3");
    assert_eq!(row(&root, cx), None);
    cx.simulate_keystrokes("j");
    assert_eq!(row(&root, cx), Some(0), "the first payee");
}

#[gpui::test]
fn on_the_budget_the_inspector_follows_the_categories(cx: &mut TestAppContext) {
    let (root, cx) = open(cx);
    cx.simulate_keystrokes("2");
    let shown = |root: &gpui::Entity<Root>,
                 cx: &mut gpui::VisualTestContext| {
        root.update(cx, |r, _| {
            r.budget().and_then(|b| b.inspector.map(|i| i.account))
        })
    };
    let first = shown(&root, cx).expect("a category is shown");
    let order: Vec<String> = root.update(cx, |r, _| {
        r.budget()
            .unwrap()
            .lines
            .iter()
            .filter_map(|l| l.account.clone())
            .collect()
    });
    let at = order.iter().position(|a| *a == first).unwrap();
    cx.simulate_keystrokes("j");
    assert_eq!(shown(&root, cx).as_ref(), order.get(at + 1));
    cx.simulate_keystrokes("k");
    assert_eq!(shown(&root, cx), Some(first.clone()));
    cx.simulate_keystrokes("enter");
    assert_eq!(root.read_with(cx, |r, _| r.page), Page::Account);
    assert_eq!(root.read_with(cx, |r, _| r.account.clone()), Some(first));
}

#[gpui::test]
fn enter_on_a_payee_searches_for_it(cx: &mut TestAppContext) {
    let (root, cx) = open(cx);
    cx.simulate_keystrokes("3 j enter");
    let query =
        root.read_with(cx, |r, _| r.search.as_ref().map(|s| s.query.clone()));
    let first = root.update(cx, |r, _| {
        r.reports().unwrap().view.payees.items[0].name.clone()
    });
    assert_eq!(query, Some(first));
}

#[gpui::test]
fn enter_on_a_debt_opens_its_register(cx: &mut TestAppContext) {
    let (root, cx) = open(cx);
    cx.simulate_keystrokes("5 j enter");
    assert_eq!(root.read_with(cx, |r, _| r.page), Page::Account);
    assert_eq!(
        root.read_with(cx, |r, _| r.account.clone()).as_deref(),
        Some("Liabilities:Loan:Car")
    );
}

#[gpui::test]
fn the_console_answer_is_walked_once_the_editor_lets_go(
    cx: &mut TestAppContext,
) {
    let (root, cx) = open(cx);
    cx.simulate_keystrokes("6");
    cx.run_until_parked();
    cx.simulate_keystrokes("j");
    assert_eq!(row(&root, cx), None, "j typed into the editor");
    cx.simulate_keystrokes("backspace escape j j");
    assert_eq!(row(&root, cx), Some(1));
}

#[gpui::test]
fn the_lit_row_is_scrolled_into_view(cx: &mut TestAppContext) {
    let (root, cx) = open(cx);
    cx.simulate_resize(size(px(1440.), px(600.)));
    // The app has always drawn the page before a key reaches it, and
    // draws again when the lit row asks; a test draws those frames.
    let frame = |cx: &mut gpui::VisualTestContext| {
        cx.update(|window, cx| {
            let _ = window.draw(cx);
        })
    };
    frame(cx);
    cx.simulate_keystrokes("j");
    frame(cx);
    let at = root.read_with(cx, |r, _| -f32::from(r.scroll.offset().y));
    assert!(at > 0., "the days ahead sit below the fold");
}
