//! `Search.dc.html`: the ⌘K palette over the dimmed page.

use gpui::{
    AnyElement, BoxShadow, Context, Div, FontWeight, Hsla, InteractiveElement,
    IntoElement, ParentElement, SharedString, StatefulInteractiveElement,
    Styled, div, hsla, point, prelude::*, px,
};
use rust_decimal::Decimal;

use super::kit::{line, run};
use crate::{
    fmt::{money, signed},
    model::search::Hit,
    root::Root,
    theme::Theme,
};

const CH: f32 = 8.4;

/// `text`, with every stretch matching one of `terms` marked.
pub fn marked(text: &str, terms: &[String], color: Hsla, t: &Theme) -> Div {
    marked_at(text, terms, color, t, 14., 22.)
}

/// `marked` for text of `size` on a `line`-tall line.
pub fn marked_at(
    text: &str,
    terms: &[String],
    color: Hsla,
    t: &Theme,
    size: f32,
    line: f32,
) -> Div {
    // JetBrains Mono's content area is 1.32em: ascent 1.02, descent 0.3.
    // Chromium snaps the box to device pixels (half pixels at 2x).
    let content = (size * 1.32 * 2.).round() / 2.;
    let top = ((line - content) / 2. * 2.).floor() / 2.;
    let lower = text.to_lowercase();
    let mut marks = vec![false; text.len()];
    for term in terms.iter().filter(|s| !s.is_empty()) {
        let mut from = 0;
        while let Some(i) = lower[from..].find(term.as_str()) {
            let start = from + i;
            for m in &mut marks[start..start + term.len()] {
                *m = true;
            }
            from = start + term.len().max(1);
        }
    }
    let mut out = div().flex().flex_row().flex_none();
    let mut start = 0;
    while start < text.len() {
        let on = marks[start];
        let mut end = start;
        while end < text.len() && marks[end] == on {
            end += 1;
        }
        while !text.is_char_boundary(end) {
            end += 1;
        }
        let piece = SharedString::from(text[start..end].to_owned());
        // `<mark>` paints over the font's content area, not the whole
        // line, centred in it.
        out = out.child(
            div()
                .relative()
                .text_color(if on { t.ink } else { color })
                .when(on, |d| {
                    d.child(
                        div()
                            .absolute()
                            .left_0()
                            .right_0()
                            .top(px(top))
                            .h(px(content))
                            .bg(t.match_),
                    )
                })
                .child(div().relative().child(piece)),
        );
        start = end;
    }
    out
}

fn quoted(text: &str, terms: &[String], color: Hsla, t: &Theme) -> Div {
    div()
        .flex()
        .flex_row()
        .child(div().text_color(t.mut_).child("\""))
        .child(marked(text, terms, color, t))
        .child(div().text_color(t.mut_).child("\""))
}

fn chip(t: &Theme, s: &'static str) -> Div {
    div().px(px(8.)).bg(t.hl).child(s)
}

pub fn overlay(
    query: &str,
    selected: usize,
    hits: &[Hit],
    t: &Theme,
    narrow: bool,
    cx: &mut Context<Root>,
) -> AnyElement {
    let terms: Vec<String> = query
        .split_whitespace()
        .map(|s| s.trim_start_matches(['#', '^']).to_lowercase())
        .collect();
    let selected = selected.min(hits.len().saturating_sub(1));
    let total: Decimal = hits.iter().map(|h| h.amount).sum();
    let shown = hits.iter().take(50).enumerate().map(|(k, h)| {
        let on = k == selected;
        let who = if h.payee.is_empty() {
            &h.narration
        } else {
            &h.payee
        };
        div()
            .id(("hit", k))
            .relative()
            .flex()
            .flex_row()
            .gap(px(12.))
            .py(px(5.))
            .px(px(16.))
            .whitespace_nowrap()
            .when(on, |d| {
                d.bg(t.sel).child(
                    div()
                        .absolute()
                        .left_0()
                        .top_0()
                        .bottom_0()
                        .w(px(2.))
                        .bg(t.blue),
                )
            })
            .hover(|s| s.bg(t.hl))
            .cursor_pointer()
            .on_click(cx.listener(move |r, _, _, cx| r.pick_hit(k, cx)))
            .child(
                div()
                    .w(px(11. * CH))
                    .flex_none()
                    .text_color(t.cyan)
                    .child(bean_core::home::date(h.date)),
            )
            .child(
                div()
                    .w(px(2. * CH))
                    .flex_none()
                    .text_color(if h.flag == '!' { t.yellow } else { t.green })
                    .child(h.flag.to_string()),
            )
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.))
                    .overflow_hidden()
                    .child(quoted(who, &terms, t.ink, t)),
            )
            .child(
                div()
                    .w(px(22. * CH))
                    .flex_shrink_1()
                    .min_w(px(0.))
                    .overflow_hidden()
                    .child(marked(&h.category, &terms, t.dim, t)),
            )
            .child(
                div()
                    .w(px(9. * CH))
                    .flex_none()
                    .flex()
                    .justify_end()
                    .text_color(if h.amount > Decimal::ZERO {
                        t.green
                    } else {
                        t.ink
                    })
                    .child(signed(h.amount)),
            )
    });

    let preview = hits.get(selected).map(|h| {
        // Laid out the way bean-format lays a transaction out: accounts
        // padded to the longest, then the amounts right-aligned.
        let width = h
            .postings
            .iter()
            .map(|p| p.0.chars().count())
            .max()
            .unwrap_or(0);
        let values: Vec<String> = h
            .postings
            .iter()
            .map(|p| money(p.1).replace('−', "-"))
            .collect();
        let amount_width =
            values.iter().map(|v| v.chars().count()).max().unwrap_or(0);
        let mut lines = div()
            .flex()
            .flex_col()
            .text_size(px(13.))
            .line_height(px(21.))
            .text_color(t.ink2);
        lines = lines.child(line([
            run(bean_core::home::date(h.date), t.cyan),
            run(" ", t.ink2),
            run(
                h.flag.to_string(),
                if h.flag == '!' { t.yellow } else { t.green },
            ),
            run(" \"", t.mut_),
            run(h.payee.clone(), t.ink),
            run("\" \"", t.mut_),
            run(h.narration.clone(), t.ink),
            run("\"", t.mut_),
        ]));
        for ((account, _, currency), value) in h.postings.iter().zip(&values) {
            let pad = width - account.chars().count();
            lines = lines.child(line([
                run("  ", t.ink2),
                run(account.clone(), t.dim),
                run(
                    format!("{}{value:>amount_width$}", " ".repeat(pad + 3)),
                    t.ink2,
                ),
                run(" ", t.ink2),
                run(currency.clone(), t.purple),
            ]));
        }
        if let (Some(file), Some(line_no)) = (&h.file, h.line) {
            lines = lines.child(div().h(px(21.))).child(
                div()
                    .text_color(t.mut_)
                    .child(format!("; {file}:{line_no}")),
            );
        }
        lines
    });

    let palette = div()
        .w_full()
        .max_w(px(1040.))
        .flex()
        .flex_col()
        .bg(t.bg)
        .border_1()
        .border_color(t.line2)
        .shadow(vec![BoxShadow {
            color: hsla(0., 0., 0., 0.45),
            offset: point(px(0.), px(30.)),
            blur_radius: px(80.),
            spread_radius: px(0.),
            inset: false,
        }])
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(12.))
                .py(px(14.))
                .px(px(16.))
                .border_b_1()
                .border_color(t.line2)
                .child(
                    div()
                        .text_color(t.blue)
                        .font_weight(FontWeight::BOLD)
                        .child("/"),
                )
                .child(
                    div()
                        .flex_1()
                        .flex()
                        .flex_row()
                        .items_center()
                        .text_size(px(18.))
                        // An <input> keeps 2px of its own padding.
                        .pl(px(2.))
                        .child(query.to_owned())
                        .child(div().w(px(2.)).h(px(22.)).bg(t.ink)),
                )
                .child(
                    div()
                        .text_color(t.dim)
                        .child(format!("{} txns", hits.len())),
                )
                .child(
                    div()
                        .px(px(8.))
                        .border_1()
                        .border_color(t.line2)
                        .text_color(t.mut_)
                        .child("esc"),
                ),
        )
        .child(
            div()
                .flex()
                .flex_row()
                .flex_wrap()
                .gap(px(8.))
                .py(px(10.))
                .px(px(16.))
                .border_b_1()
                .border_color(t.line)
                .text_size(px(12.))
                .text_color(t.mut_)
                .children(
                    [
                        "payee:",
                        "account:",
                        "#tag",
                        "^link",
                        "key:value",
                        "2026-08",
                    ]
                    .map(|c| chip(t, c)),
                ),
        )
        .child(
            div()
                .flex()
                .flex_row()
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.))
                        .py(px(8.))
                        .when(!narrow, |d| d.border_r_1().border_color(t.line))
                        .children(shown),
                )
                .when(!narrow, |d| {
                    d.child(
                        div()
                            .w(px(42. * CH))
                            .flex_none()
                            .p(px(16.))
                            .overflow_hidden()
                            .children(preview),
                    )
                }),
        )
        .child(
            div()
                .flex()
                .flex_row()
                .flex_wrap()
                .gap(px(18.))
                .py(px(10.))
                .px(px(16.))
                .border_t_1()
                .border_color(t.line2)
                .text_size(px(12.))
                .text_color(t.mut_)
                .child(line([run("↑↓", t.ink2), run(" move", t.mut_)]))
                .child(line([
                    run("enter", t.ink2),
                    run(" open register", t.mut_),
                ]))
                .child(line([run("⇥", t.ink2), run(" more", t.mut_)]))
                .child(div().flex_1())
                .child(format!("{} total", money(total))),
        );

    div()
        .id("search-scrim")
        .absolute()
        .inset_0()
        .bg(t.scrim)
        .flex()
        .flex_col()
        .items_center()
        .pt(px(72.))
        .px(px(24.))
        .on_click(cx.listener(|r, _, _, cx| r.close_search(cx)))
        .child(
            div()
                .id("palette")
                .w_full()
                .flex()
                .justify_center()
                .on_click(|_, _, cx| cx.stop_propagation())
                .child(palette),
        )
        .into_any_element()
}
