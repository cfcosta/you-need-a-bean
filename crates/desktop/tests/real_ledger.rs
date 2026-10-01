//! Renders every page from the ledger named by `BEAN_LEDGER` into
//! `target/visual/real-*.png`, for looking at the app against a real,
//! private ledger. Ignored by default; nothing it reads is kept in the
//! repository.
//!
//!     BEAN_LEDGER=~/finances/index.beancount \
//!         cargo test -p you-need-a-bean-desktop --test real_ledger -- --ignored

mod support;

use std::{path::PathBuf, sync::Arc};

use bean_core::{loader::load, model::Ledger};
use bean_desktop::{Page, Root};
use support::{frame, headless, shoot};

#[test]
#[ignore = "needs BEAN_LEDGER"]
fn render_every_page() {
    let Ok(path) = std::env::var("BEAN_LEDGER") else {
        return;
    };
    let ledger = Arc::new(Ledger::build(
        load(&PathBuf::from(path)).expect("the ledger loads"),
    ));
    let today = bean_core::home::parse_date(
        &std::env::var("BEAN_TODAY").unwrap_or("2026-10-01".into()),
    )
    .unwrap();
    let out =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/visual");
    std::fs::create_dir_all(&out).unwrap();
    let pages = [
        (Page::Overview, "overview"),
        (Page::Budget, "budget"),
        (Page::Reports, "reports"),
        (Page::Investments, "investments"),
        (Page::Liabilities, "liabilities"),
    ];
    for (width, height, form) in
        [(1440., 2400., "desktop"), (390., 2400., "phone")]
    {
        for (page, name) in pages {
            let mut cx = headless();
            let ledger = ledger.clone();
            let shot = shoot(&mut cx, frame(width, height), move |_, cx| {
                let mut root = Root::new(ledger, today, cx);
                root.page = page;
                root
            });
            shot.save(out.join(format!("real-{form}-{name}.png")))
                .unwrap();
        }
    }
}
