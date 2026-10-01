mod common;
mod support;

use bean_desktop::Root;
use support::{frame, headless, pixel, shoot};

#[test]
fn the_window_is_painted_in_tokyo_night() {
    let mut cx = headless();
    let shot = shoot(&mut cx, frame(640., 400.), |_, cx| Root::empty(cx));
    // --bg in the Night scheme: #1a1b26.
    assert_eq!(pixel(&shot, 600, 380), [0x1a, 0x1b, 0x26]);
}

#[test]
fn the_status_line_keeps_today_in_view_on_a_narrow_window() {
    let mut cx = headless();
    let ledger = std::sync::Arc::new(common::overview_ledger());
    for width in [720., 900., 1060.] {
        let ledger = ledger.clone();
        let shot = shoot(&mut cx, frame(width, 300.), move |_, cx| {
            Root::new(ledger, common::TODAY, cx)
        });
        // The date block, in --blue, sits flush with the right edge.
        let right = shot.width() - 4;
        assert_eq!(pixel(&shot, right, 8), [0x7a, 0xa2, 0xf7], "at {width}px");
    }
}
