//! The handful of parts every page is built from, each a direct port of
//! a class in the design canvas (`.box`, `.row`, `.cb`, `.cols`, `.vb`,
//! `.seg`) so a page reads like the HTML it was drawn as.

use gpui::{
    AnyElement, Bounds, ContentMask, Div, FontWeight, Hsla, IntoElement,
    ParentElement, PathBuilder, Pixels, SharedString, Styled, Window, canvas,
    div, fill, point, px, relative, size,
};

use crate::theme::Theme;

/// The body text: 14px on a 22px line.
pub const SIZE: f32 = 14.;
pub const LINE: f32 = 22.;

/// A run of text in one colour and weight; a line is a row of runs.
#[derive(Clone)]
pub struct Run {
    pub text: SharedString,
    pub color: Hsla,
    pub weight: FontWeight,
}

pub fn run(text: impl Into<SharedString>, color: Hsla) -> Run {
    Run {
        text: text.into(),
        color,
        weight: FontWeight::NORMAL,
    }
}

pub fn bold(text: impl Into<SharedString>, color: Hsla) -> Run {
    Run {
        text: text.into(),
        color,
        weight: FontWeight::BOLD,
    }
}

impl Run {
    pub fn weight(mut self, weight: FontWeight) -> Self {
        self.weight = weight;
        self
    }
}

/// Runs laid end to end, no gaps: the monospace grid keeps them aligned.
pub fn line(runs: impl IntoIterator<Item = Run>) -> Div {
    div()
        .flex()
        .flex_row()
        .flex_none()
        .whitespace_nowrap()
        .children(runs.into_iter().map(|r| {
            div()
                .flex_none()
                .text_color(r.color)
                .font_weight(r.weight)
                .child(r.text)
        }))
}

/// A label on the left and a figure on the right of one line.
pub fn row(left: impl IntoElement, right: impl IntoElement) -> Div {
    div()
        .flex()
        .flex_row()
        .justify_between()
        .gap(px(16.))
        .whitespace_nowrap()
        .child(left)
        .child(right)
}

/// A section: an outlined box with its label set into the top border.
pub fn boxed(t: &Theme, label: impl Into<SharedString>) -> Div {
    // gpui paints an element's border over its children; the outline
    // lives on a backdrop child instead so the legend can cut it.
    div()
        .relative()
        .min_w(px(0.))
        .pt(px(27.))
        .px(px(25.))
        .pb(px(21.))
        .child(div().absolute().inset_0().border_1().border_color(t.line2))
        .child(legend(t, label.into(), false))
}

/// A section with a second, quieter label on the right of the border.
pub fn boxed_right(
    t: &Theme,
    label: impl Into<SharedString>,
    right: impl Into<SharedString>,
) -> Div {
    boxed(t, label).child(legend(t, right.into(), true))
}

fn legend(t: &Theme, text: SharedString, right: bool) -> Div {
    let d = div()
        .absolute()
        // CSS measures these from inside the 1px border.
        .top(px(-10.))
        .line_height(px(LINE))
        .px(px(8.))
        .bg(t.bg)
        .text_color(if right { t.mut_ } else { t.dim })
        .whitespace_nowrap()
        .child(text);
    if right {
        d.right(px(17.))
    } else {
        d.left(px(17.))
    }
}

/// The rule that closes a sum: `= free`.
pub fn sum_rule(t: &Theme) -> Div {
    div()
        .border_t_1()
        .border_color(t.line2)
        .mt(px(6.))
        .pt(px(6.))
}

/// The dashed rule between the parts of one box.
pub fn dash(t: &Theme, top: f32, pad: f32) -> Div {
    div()
        .mt(px(top))
        .pt(px(pad))
        .border_t_1()
        .border_dashed()
        .border_color(t.line2)
}

/// JetBrains Mono advances every glyph by 600/1000 em.
pub const ADVANCE: f32 = 0.6;

/// Large figures are set tight (−0.04em in the design); gpui has no
/// letter spacing, so each glyph gets a box exactly as wide as CSS would
/// give it: the advance plus the (negative) tracking.
pub fn tracked(
    runs: &[(&str, Hsla, FontWeight)],
    size_px: f32,
    tracking_em: f32,
) -> Div {
    let w = px(size_px * (ADVANCE + tracking_em));
    div()
        .flex()
        .flex_row()
        .flex_none()
        .text_size(px(size_px))
        .line_height(px(size_px))
        .children(runs.iter().flat_map(|(text, color, weight)| {
            let (color, weight) = (*color, *weight);
            text.chars().map(move |c| {
                div()
                    .flex_none()
                    .w(w)
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .text_color(color)
                    .font_weight(weight)
                    .child(c.to_string())
            })
        }))
}

/// How a stretch of a bar is painted.
#[derive(Clone, Copy)]
pub enum Fill {
    Solid(Hsla),
    /// `repeating-linear-gradient(135deg, c 0 2px, transparent 2px 5px)`.
    Hatch(Hsla),
}

fn paint_fill(window: &mut Window, bounds: Bounds<Pixels>, f: Fill) {
    match f {
        Fill::Solid(c) => window.paint_quad(fill(bounds, c)),
        Fill::Hatch(c) => hatch(window, bounds, c),
    }
}

/// Diagonal bands two pixels thick every five, measured across the band,
/// running from bottom left to top right.
fn hatch(window: &mut Window, bounds: Bounds<Pixels>, color: Hsla) {
    let w = f32::from(bounds.size.width);
    let h = f32::from(bounds.size.height);
    let period = 5.0 * std::f32::consts::SQRT_2;
    let band = 2.0 * std::f32::consts::SQRT_2;
    let o = bounds.origin;
    window.with_content_mask(Some(ContentMask { bounds }), |window| {
        let mut k = 0.0;
        while k < w + h {
            let (a, b) = (k, k + band);
            let mut p = PathBuilder::fill();
            p.move_to(point(o.x + px(a), o.y));
            p.line_to(point(o.x + px(b), o.y));
            p.line_to(point(o.x + px(b - h), o.y + px(h)));
            p.line_to(point(o.x + px(a - h), o.y + px(h)));
            p.close();
            if let Ok(path) = p.build() {
                window.paint_path(path, color);
            }
            k += period;
        }
    });
}

/// A box painted with one fill, sized by its parent.
pub fn filled(f: Fill) -> impl IntoElement {
    canvas(
        |_, _, _| (),
        move |bounds, _, window, _| paint_fill(window, bounds, f),
    )
    .size_full()
}

/// `.cb`: a bar of cells — stretches in proportion, cut every
/// `cell + gap` pixels by a gap in the page colour.
pub fn cells(
    t: &Theme,
    parts: Vec<(f32, Fill)>,
    height: f32,
    cell: f32,
    gap: f32,
) -> impl IntoElement {
    let bg = t.bg;
    let total: f32 = parts.iter().map(|p| p.0).sum::<f32>().max(f32::EPSILON);
    canvas(
        |_, _, _| (),
        move |bounds, _, window, _| {
            let w = f32::from(bounds.size.width);
            let mut x = 0.0;
            for (flex, f) in &parts {
                let pw = w * flex / total;
                let b = Bounds::new(
                    point(bounds.origin.x + px(x), bounds.origin.y),
                    size(px(pw), bounds.size.height),
                );
                paint_fill(window, b, *f);
                x += pw;
            }
            let mut g = cell;
            while g < w {
                let b = Bounds::new(
                    point(bounds.origin.x + px(g), bounds.origin.y),
                    size(px(gap.min(w - g)), bounds.size.height),
                );
                window.paint_quad(fill(b, bg));
                g += cell + gap;
            }
        },
    )
    .w_full()
    .h(px(height))
}

/// The standard 20px bar of 9px cells.
pub fn bar(t: &Theme, parts: Vec<(f32, Fill)>) -> impl IntoElement {
    cells(t, parts, 20., 9., 2.)
}

/// The slim 12px variant used inside tables.
pub fn slim(t: &Theme, parts: Vec<(f32, Fill)>) -> impl IntoElement {
    cells(t, parts, 12., 9., 2.)
}

/// `.cols`: columns standing on a baseline, each a share of `height`.
pub fn cols(items: Vec<(f32, Fill)>, height: f32, gap: f32) -> Div {
    div()
        .flex()
        .flex_row()
        .items_end()
        .gap(px(gap))
        .h(px(height))
        .children(items.into_iter().map(|(share, f)| {
            div()
                .flex_1()
                .h(relative(share.clamp(0., 1.)))
                .child(filled(f))
        }))
}

/// Labels centred under `cols`, one per column.
pub fn col_labels(
    labels: impl IntoIterator<Item = SharedString>,
    color: Hsla,
    text_px: f32,
    gap: f32,
) -> Div {
    div()
        .flex()
        .flex_row()
        .gap(px(gap))
        .text_color(color)
        .text_size(px(text_px))
        .children(
            labels
                .into_iter()
                .map(|l| div().flex_1().flex().justify_center().child(l)),
        )
}

/// `.vb`: spend against typical — a track that reaches 150%, a tick at
/// 100%, cut into 6px cells.
pub fn vs_bar(t: &Theme, ratio: Option<f32>, color: Hsla) -> impl IntoElement {
    let (hl, bg, tick) = (t.hl, t.bg, t.ink2);
    canvas(
        |_, _, _| (),
        move |bounds, _, window, _| {
            let w = f32::from(bounds.size.width);
            let o = bounds.origin;
            let hgt = bounds.size.height;
            window.paint_quad(fill(bounds, hl));
            if let Some(r) = ratio {
                let fw = w * r.clamp(0., 1.5) / 1.5;
                window
                    .paint_quad(fill(Bounds::new(o, size(px(fw), hgt)), color));
            }
            let mut g = 6.0;
            while g < w {
                window.paint_quad(fill(
                    Bounds::new(
                        point(o.x + px(g), o.y),
                        size(px(2.0f32.min(w - g)), hgt),
                    ),
                    bg,
                ));
                g += 8.0;
            }
            let tx = (w * 2.0 / 3.0).round();
            window.paint_quad(fill(
                Bounds::new(
                    point(o.x + px(tx), o.y - px(3.)),
                    size(px(2.), hgt + px(6.)),
                ),
                tick,
            ));
        },
    )
    .w_full()
    .h(px(12.))
}

/// `.seg`: a joined row of choices, the chosen one lit.
pub fn seg(
    t: &Theme,
    options: impl IntoIterator<Item = (SharedString, bool)>,
    pad_y: f32,
    pad_x: f32,
) -> Div {
    let (sel, ink, dim, line2) = (t.sel, t.ink, t.dim, t.line2);
    div()
        .flex()
        .flex_row()
        .border_1()
        .border_color(line2)
        .children(options.into_iter().enumerate().map(
            move |(i, (label, on))| {
                let d = div()
                    .py(px(pad_y))
                    .px(px(pad_x))
                    .text_color(if on { ink } else { dim })
                    .when(on, |d| d.bg(sel));
                let d = if i > 0 {
                    d.border_l_1().border_color(line2)
                } else {
                    d
                };
                d.child(label)
            },
        ))
}

/// Anything, boxed so heterogeneous branches can share a type.
pub fn any(e: impl IntoElement) -> AnyElement {
    e.into_any_element()
}

use gpui::prelude::FluentBuilder as _;

/// `grid-template-columns: minmax(0, a fr) minmax(0, b fr)` with a gap:
/// two columns sharing what the gap leaves in proportion `a : b`.
pub fn split(
    a: f32,
    b: f32,
    gap: f32,
    left: impl IntoElement,
    right: impl IntoElement,
) -> Div {
    let total = a + b;
    div()
        .flex()
        .flex_row()
        .gap(px(gap))
        .child(
            div()
                .flex_basis(relative(a / total))
                .flex_shrink_1()
                .min_w(px(0.))
                .child(left),
        )
        .child(
            div()
                .flex_basis(relative(b / total))
                .flex_shrink_1()
                .min_w(px(0.))
                .child(right),
        )
}
