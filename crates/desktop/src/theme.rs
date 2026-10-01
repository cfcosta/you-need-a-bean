//! The Plain Text palette: Tokyo Night for the dark scheme, Tokyo Night
//! Day for the light one, with the same token names the design canvas
//! uses so a value can be checked against it by eye.

use gpui::{App, Global, Hsla, Rgba, rgb, rgba};

/// The one family the design is set in.
pub const MONO: &str = "JetBrains Mono";

const FONTS: [&[u8]; 4] = [
    include_bytes!("../assets/fonts/JetBrainsMono-Regular.ttf"),
    include_bytes!("../assets/fonts/JetBrainsMono-Medium.ttf"),
    include_bytes!("../assets/fonts/JetBrainsMono-SemiBold.ttf"),
    include_bytes!("../assets/fonts/JetBrainsMono-Bold.ttf"),
];

pub fn load_fonts(cx: &App) {
    let fonts = FONTS.into_iter().map(std::borrow::Cow::Borrowed).collect();
    if let Err(error) = cx.text_system().add_fonts(fonts) {
        eprintln!(
            "you-need-a-bean: could not load the embedded fonts: {error}"
        );
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scheme {
    Night,
    Day,
}

impl Scheme {
    pub fn toggled(self) -> Self {
        match self {
            Scheme::Night => Scheme::Day,
            Scheme::Day => Scheme::Night,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Theme {
    pub scheme: Scheme,
    pub bg: Hsla,
    pub bar: Hsla,
    pub hl: Hsla,
    pub sel: Hsla,
    pub ink: Hsla,
    pub ink2: Hsla,
    pub mut_: Hsla,
    pub dim: Hsla,
    pub line: Hsla,
    pub line2: Hsla,
    pub blue: Hsla,
    pub cyan: Hsla,
    pub purple: Hsla,
    pub green: Hsla,
    pub yellow: Hsla,
    pub orange: Hsla,
    pub red: Hsla,
    pub teal: Hsla,
    pub match_: Hsla,
    pub scrim: Hsla,
    /// Text on the yellow and blue status blocks, dark in both schemes.
    pub on_block: Hsla,
    /// The range slider's track, as Chromium draws one in each scheme.
    pub range_track: Hsla,
    pub range_edge: Hsla,
}

fn c(color: Rgba) -> Hsla {
    color.into()
}

impl Theme {
    pub fn night() -> Self {
        Self {
            scheme: Scheme::Night,
            bg: c(rgb(0x1a1b26)),
            bar: c(rgb(0x16161e)),
            hl: c(rgb(0x1f2335)),
            sel: c(rgb(0x283457)),
            ink: c(rgb(0xc0caf5)),
            ink2: c(rgb(0xa9b1d6)),
            mut_: c(rgb(0x565f89)),
            dim: c(rgb(0x737aa2)),
            line: c(rgb(0x292e42)),
            line2: c(rgb(0x3b4261)),
            blue: c(rgb(0x7aa2f7)),
            cyan: c(rgb(0x7dcfff)),
            purple: c(rgb(0xbb9af7)),
            green: c(rgb(0x9ece6a)),
            yellow: c(rgb(0xe0af68)),
            orange: c(rgb(0xff9e64)),
            red: c(rgb(0xf7768e)),
            teal: c(rgb(0x73daca)),
            match_: c(rgba(0xe0af6847)),
            scrim: c(rgba(0x0d0e14b8)),
            on_block: c(rgb(0x1a1b26)),
            range_track: c(rgb(0x3b3b3b)),
            range_edge: c(rgb(0x858585)),
        }
    }

    pub fn day() -> Self {
        Self {
            scheme: Scheme::Day,
            bg: c(rgb(0xe1e2e7)),
            bar: c(rgb(0xd0d5e3)),
            hl: c(rgb(0xd5d6db)),
            sel: c(rgb(0xb7c1e3)),
            ink: c(rgb(0x3760bf)),
            ink2: c(rgb(0x343b58)),
            mut_: c(rgb(0x8990b3)),
            dim: c(rgb(0x6172b0)),
            line: c(rgb(0xc4c8da)),
            line2: c(rgb(0xa8aecb)),
            blue: c(rgb(0x2e7de9)),
            cyan: c(rgb(0x007197)),
            purple: c(rgb(0x9854f1)),
            green: c(rgb(0x587539)),
            yellow: c(rgb(0x8c6c3e)),
            orange: c(rgb(0xb15c00)),
            red: c(rgb(0xf52a65)),
            teal: c(rgb(0x118c74)),
            match_: c(rgba(0x8c6c3e40)),
            scrim: c(rgba(0xa1a6c599)),
            on_block: c(rgb(0x1a1b26)),
            range_track: c(rgb(0xefefef)),
            range_edge: c(rgb(0xb2b2b2)),
        }
    }

    pub fn of(scheme: Scheme) -> Self {
        match scheme {
            Scheme::Night => Self::night(),
            Scheme::Day => Self::day(),
        }
    }
}

impl Global for Theme {}

pub fn theme(cx: &App) -> &Theme {
    cx.global::<Theme>()
}
