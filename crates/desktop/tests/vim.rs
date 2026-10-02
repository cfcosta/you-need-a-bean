//! Vim's motions on the pages: `h`/`l` step through the tabs, `d`/`u`
//! scroll half a page, `G` to the end, `gg` to the top. (`j`/`k` walk a
//! table's rows; see `rows.rs`.)

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

fn page(root: &gpui::Entity<Root>, cx: &mut gpui::VisualTestContext) -> Page {
    root.read_with(cx, |r, _| r.page)
}

#[gpui::test]
fn h_and_l_step_through_the_tabs_and_wrap(cx: &mut TestAppContext) {
    let (root, cx) = open(cx);
    cx.simulate_keystrokes("l");
    assert_eq!(page(&root, cx), Page::Budget);
    cx.simulate_keystrokes("l l");
    assert_eq!(page(&root, cx), Page::Investments);
    cx.simulate_keystrokes("h h h");
    assert_eq!(page(&root, cx), Page::Overview);
    cx.simulate_keystrokes("h");
    assert_eq!(
        page(&root, cx),
        Page::Query,
        "left of the first is the last"
    );
    cx.simulate_keystrokes("escape l");
    assert_eq!(
        page(&root, cx),
        Page::Overview,
        "right of the last is the first"
    );
}

#[gpui::test]
fn an_account_steps_from_its_budget_tab(cx: &mut TestAppContext) {
    let (root, cx) = open(cx);
    root.update(cx, |r, cx| {
        r.open_account("Assets:Bank:Everyday".into(), cx)
    });
    cx.simulate_keystrokes("l");
    assert_eq!(page(&root, cx), Page::Reports);
}

/// (offset scrolled down, furthest it can go, half the visible height)
fn scroll(
    root: &gpui::Entity<Root>,
    cx: &mut gpui::VisualTestContext,
) -> (f32, f32, f32) {
    cx.run_until_parked();
    root.read_with(cx, |r, _| {
        (
            -f32::from(r.scroll.offset().y),
            f32::from(r.scroll.max_offset().y),
            f32::from(r.scroll.bounds().size.height) / 2.,
        )
    })
}

#[gpui::test]
fn the_motions_scroll_the_page(cx: &mut TestAppContext) {
    let (root, cx) = open(cx);
    cx.simulate_resize(size(px(1440.), px(600.)));
    cx.simulate_keystrokes("3");
    let (at, max, half) = scroll(&root, cx);
    assert_eq!(at, 0.);
    assert!(max > 600., "the reports run past the window");

    cx.simulate_keystrokes("d");
    assert_eq!(scroll(&root, cx).0, half);
    cx.simulate_keystrokes("u");
    assert_eq!(scroll(&root, cx).0, 0.);

    cx.simulate_keystrokes("shift-g");
    assert_eq!(scroll(&root, cx).0, max);
    cx.simulate_keystrokes("d");
    assert_eq!(scroll(&root, cx).0, max, "never past the end");
    cx.simulate_keystrokes("g g");
    assert_eq!(scroll(&root, cx).0, 0.);
}

#[gpui::test]
fn a_new_page_opens_at_its_top(cx: &mut TestAppContext) {
    let (root, cx) = open(cx);
    cx.simulate_resize(size(px(1440.), px(600.)));
    cx.simulate_keystrokes("3");
    scroll(&root, cx);
    cx.simulate_keystrokes("shift-g h");
    assert_eq!(scroll(&root, cx).0, 0.);
}

#[gpui::test]
fn the_motions_type_while_a_field_has_the_keys(cx: &mut TestAppContext) {
    let (root, cx) = open(cx);
    cx.simulate_keystrokes("6");
    root.update(cx, |r, cx| r.load_sql("", cx));
    cx.simulate_input("hjkl du G gg");
    assert_eq!(
        root.read_with(cx, |r, _| r.console.editor.text()),
        "hjkl du G gg"
    );
    assert_eq!(page(&root, cx), Page::Query);
}
