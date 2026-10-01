//! Each page against its board on the design canvas, rendered from the
//! same example ledger on the same day.

mod common;
mod support;

use std::sync::Arc;

use bean_desktop::{Page, Root};
use common::{TODAY, overview_ledger};
use support::{frame, headless, mismatch, shoot};

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
