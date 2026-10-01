//! Each page against its board on the design canvas, rendered from the
//! same example ledger on the same day.

mod common;
mod support;

use std::sync::Arc;

use bean_desktop::{Page, Root};
use common::{TODAY, overview_ledger};
use support::{frame, headless, mismatch, mismatch_in, shoot};

fn page(page: Page, width: f32, height: f32) -> image::RgbaImage {
    let mut cx = headless();
    let ledger = Arc::new(overview_ledger());
    shoot(&mut cx, frame(width, height), move |_, cx| {
        let mut root = Root::new(ledger, TODAY, cx);
        root.page = page;
        root
    })
}

#[test]
fn overview_matches_the_canvas() {
    let shot = page(Page::Overview, 1440., 1220.);
    let off = mismatch(&shot, "Main-night");
    eprintln!("overview: {:.3}% off", off * 100.);
    assert!(
        off < 0.02,
        "{:.2}% of the overview is off the canvas",
        off * 100.
    );
}

#[test]
fn budget_matches_the_canvas() {
    let mut cx = headless();
    let ledger = Arc::new(overview_ledger());
    let shot = shoot(&mut cx, frame(1440., 1060.), move |_, cx| {
        let mut root = Root::new(ledger, TODAY, cx);
        root.page = Page::Budget;
        root.month = Some(bean_core::model::MonthKey::new(2026, 8));
        root
    });
    let off = mismatch(&shot, "Budget-night");
    eprintln!("budget: {:.3}% off", off * 100.);
    assert!(
        off < 0.02,
        "{:.2}% of the budget is off the canvas",
        off * 100.
    );
}

#[test]
fn account_register_matches_the_canvas() {
    let mut cx = headless();
    let ledger = Arc::new(overview_ledger());
    let shot = shoot(&mut cx, frame(1440., 820.), move |_, cx| {
        let mut root = Root::new(ledger, TODAY, cx);
        root.page = Page::Account;
        root.account = Some("Assets:Bank:Everyday".into());
        root.month = Some(bean_core::model::MonthKey::new(2026, 9));
        root
    });
    let off = mismatch(&shot, "Account-night");
    eprintln!("account: {:.3}% off", off * 100.);
    assert!(
        off < 0.02,
        "{:.2}% of the register is off the canvas",
        off * 100.
    );
}

#[test]
fn reports_match_the_canvas() {
    let shot = page(Page::Reports, 1440., 1560.);
    let off = mismatch(&shot, "Reports-night");
    eprintln!("reports: {:.3}% off", off * 100.);
    assert!(
        off < 0.02,
        "{:.2}% of the reports are off the canvas",
        off * 100.
    );
}

#[test]
fn investments_match_the_canvas() {
    let shot = page(Page::Investments, 1440., 1100.);
    let off = mismatch(&shot, "Investments-night");
    eprintln!("investments: {:.3}% off", off * 100.);
    assert!(
        off < 0.02,
        "{:.2}% of investments is off the canvas",
        off * 100.
    );
}

#[test]
fn liabilities_match_the_canvas() {
    let shot = page(Page::Liabilities, 1440., 1160.);
    let off = mismatch(&shot, "Liabilities-night");
    eprintln!("liabilities: {:.3}% off", off * 100.);
    assert!(
        off < 0.02,
        "{:.2}% of liabilities is off the canvas",
        off * 100.
    );
}

#[test]
fn search_matches_the_canvas() {
    let mut cx = headless();
    let ledger = Arc::new(overview_ledger());
    let shot = shoot(&mut cx, frame(1440., 640.), move |_, cx| {
        let mut root = Root::new(ledger, TODAY, cx);
        root.search = Some(bean_desktop::root::Search {
            query: "coffee".into(),
            ..Default::default()
        });
        root
    });
    // The canvas drew the palette over an empty page; the app dims the
    // page it was opened on. Count the status line and the palette.
    // The status line differs only by the example's file name.
    let off = mismatch_in(&shot, "Search-night", Some((200, 106, 1040, 413)));
    eprintln!("search: {:.3}% off", off * 100.);
    assert!(
        off < 0.03,
        "{:.2}% of the palette is off the canvas",
        off * 100.
    );
}

fn phone(
    page: Page,
    height: f32,
    setup: impl FnOnce(&mut Root) + 'static,
) -> image::RgbaImage {
    let mut cx = headless();
    let ledger = Arc::new(overview_ledger());
    shoot(&mut cx, frame(390., height), move |_, cx| {
        let mut root = Root::new(ledger, TODAY, cx);
        root.page = page;
        setup(&mut root);
        root
    })
}

/// A phone board is one tall screen; the app scrolls its page between a
/// fixed header and tab bar. Compare the header and the page above the
/// tab bar. Phones are held to 4%: at 13px the glyph-advance rounding
/// drifts dense text by a device pixel more often than at 14.
fn phone_off(shot: &image::RgbaImage, reference: &str, height: u32) -> f64 {
    let off = mismatch_in(shot, reference, Some((0, 0, 390, height)));
    eprintln!("{reference}: {:.3}% off", off * 100.);
    off
}

#[test]
fn phone_overview_matches_the_canvas() {
    let shot = phone(Page::Overview, 1460., |_| {});
    assert!(phone_off(&shot, "PlainTextPhone-night", 1400) < 0.04);
}

#[test]
fn phone_budget_matches_the_canvas() {
    let shot = phone(Page::Budget, 1600., |r| {
        r.month = Some(bean_core::model::MonthKey::new(2026, 8))
    });
    assert!(phone_off(&shot, "BudgetPhone-night", 1500) < 0.04);
}

#[test]
fn phone_account_matches_the_canvas() {
    let shot = phone(Page::Account, 920., |r| {
        r.account = Some("Assets:Bank:Everyday".into());
        r.month = Some(bean_core::model::MonthKey::new(2026, 9));
    });
    assert!(phone_off(&shot, "AccountPhone-night", 860) < 0.04);
}

#[test]
fn phone_reports_match_the_canvas() {
    let shot = phone(Page::Reports, 1180., |_| {});
    assert!(phone_off(&shot, "ReportsPhone-night", 1100) < 0.04);
}

#[test]
fn phone_investments_match_the_canvas() {
    let shot = phone(Page::Investments, 920., |_| {});
    assert!(phone_off(&shot, "InvestmentsPhone-night", 860) < 0.04);
}

#[test]
fn phone_liabilities_match_the_canvas() {
    let shot = phone(Page::Liabilities, 900., |_| {});
    assert!(phone_off(&shot, "LiabilitiesPhone-night", 840) < 0.04);
}

#[test]
fn phone_search_matches_the_canvas() {
    let shot = phone(Page::Overview, 844., |r| {
        r.search = Some(bean_desktop::root::Search {
            query: "coffee".into(),
            ..Default::default()
        });
    });
    assert!(phone_off(&shot, "SearchPhone-night", 600) < 0.04);
}

/// Every desktop page again in the Day scheme.
#[test]
fn day_scheme_matches_the_canvas() {
    type Board = (Page, &'static str, f32, Option<(u16, u8)>);
    let boards: [Board; 6] = [
        (Page::Overview, "Main-day", 1220., None),
        (Page::Budget, "Budget-day", 1060., Some((2026, 8))),
        (Page::Account, "Account-day", 820., Some((2026, 9))),
        (Page::Reports, "Reports-day", 1560., None),
        (Page::Investments, "Investments-day", 1100., None),
        (Page::Liabilities, "Liabilities-day", 1160., None),
    ];
    let mut worst = 0f64;
    for (page, reference, height, month) in boards {
        let mut cx = headless();
        cx.update(|cx| cx.set_global(bean_desktop::theme::Theme::day()));
        let ledger = Arc::new(overview_ledger());
        let shot = shoot(&mut cx, frame(1440., height), move |_, cx| {
            let mut root = Root::new(ledger, TODAY, cx);
            root.page = page;
            root.month =
                month.map(|(y, m)| bean_core::model::MonthKey::new(y, m));
            root.account = Some("Assets:Bank:Everyday".into());
            root
        });
        let off = mismatch(&shot, reference);
        eprintln!("{reference}: {:.3}% off", off * 100.);
        worst = worst.max(off);
    }
    assert!(
        worst < 0.02,
        "a Day page is {:.2}% off its board",
        worst * 100.
    );
}
