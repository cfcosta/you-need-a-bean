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
