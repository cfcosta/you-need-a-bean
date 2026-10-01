//! The native window onto a Beancount ledger, in the Plain Text design:
//! everything set in one monospace face on a 14/22 rhythm, sections drawn
//! as outlined boxes, figures written as the sums they are.

use gpui::App;

pub mod fmt;
mod root;
pub mod theme;

pub use root::Root;

/// Everything a window needs before it opens: the fonts and the scheme.
pub fn init(cx: &mut App) {
    theme::load_fonts(cx);
    cx.set_global(theme::Theme::night());
}
