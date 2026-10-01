//! The keyboard is the fastest way round a ledger: digits open pages,
//! `t` flips the scheme, `/` opens search and escape closes it.

mod common;

use std::sync::Arc;

use bean_desktop::{Page, Root, theme::Scheme};
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

#[gpui::test]
fn digits_open_the_numbered_pages(cx: &mut TestAppContext) {
    let (root, cx) = open(cx);
    cx.simulate_keystrokes("2");
    assert_eq!(root.read_with(cx, |r, _| r.page), Page::Budget);
    cx.simulate_keystrokes("5");
    assert_eq!(root.read_with(cx, |r, _| r.page), Page::Liabilities);
    cx.simulate_keystrokes("1");
    assert_eq!(root.read_with(cx, |r, _| r.page), Page::Overview);
}

#[gpui::test]
fn t_flips_between_night_and_day(cx: &mut TestAppContext) {
    let (_root, cx) = open(cx);
    cx.simulate_keystrokes("t");
    assert_eq!(
        cx.update(|_, cx| bean_desktop::theme::theme(cx).scheme),
        Scheme::Day
    );
    cx.simulate_keystrokes("t");
    assert_eq!(
        cx.update(|_, cx| bean_desktop::theme::theme(cx).scheme),
        Scheme::Night
    );
}

#[gpui::test]
fn slash_opens_search_and_escape_closes_it(cx: &mut TestAppContext) {
    let (root, cx) = open(cx);
    cx.simulate_keystrokes("/");
    assert!(root.read_with(cx, |r, _| r.search.is_some()));
    cx.simulate_keystrokes("escape");
    assert!(root.read_with(cx, |r, _| r.search.is_none()));
    cx.simulate_keystrokes("ctrl-k");
    assert!(root.read_with(cx, |r, _| r.search.is_some()));
}

#[gpui::test]
fn a_page_field_takes_the_keys_until_escape(cx: &mut TestAppContext) {
    let (root, cx) = open(cx);
    cx.simulate_keystrokes("4");
    root.update(cx, |r, cx| {
        r.start_typing(bean_desktop::root::Field::HoldingFilter, cx)
    });
    cx.simulate_keystrokes("b 2 backspace e");
    assert_eq!(root.read_with(cx, |r, _| r.holding_filter.clone()), "be");
    assert_eq!(
        root.read_with(cx, |r, _| r.page),
        Page::Investments,
        "digits typed, not followed"
    );
    cx.simulate_keystrokes("escape 1");
    assert_eq!(root.read_with(cx, |r, _| r.page), Page::Overview);
}
