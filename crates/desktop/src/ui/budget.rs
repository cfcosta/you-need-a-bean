//! Page 2, `Budget.dc.html`: the month's categories against typical, the
//! month as a sum, the selected category up close, and the accounts.

use bean_core::{model::MonthKey, query::Status};
use gpui::{
    AnyElement, Context, Div, FontWeight, Hsla, InteractiveElement,
    IntoElement, ParentElement, SharedString, StatefulInteractiveElement,
    Styled, div, prelude::*, px, relative,
};
use rust_decimal::{Decimal, prelude::ToPrimitive};

use super::{
    kit::{
        Fill, bar, bold, boxed, boxed_right, col_labels, cols, dash, line, row,
        run, sum_rule, vs_bar,
    },
    overview::chart_floor,
};
use crate::{
    fmt::{money, percent},
    model::budget::{Budget, Inspector, Line},
    root::{Page, Root},
    theme::Theme,
};

/// One character of the 14px face.
pub const CH: f32 = 8.4;

pub const MONTHS: [&str; 12] = [
    "jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct",
    "nov", "dec",
];
const MONTH_NAMES: [&str; 12] = [
    "january",
    "february",
    "march",
    "april",
    "may",
    "june",
    "july",
    "august",
    "september",
    "october",
    "november",
    "december",
];

fn f(v: Decimal) -> f32 {
    v.to_f32().unwrap_or(0.)
}

pub fn status_color(t: &Theme, status: Option<Status>) -> Hsla {
    match status {
        Some(Status::Good) => t.green,
        Some(Status::Warn) => t.yellow,
        Some(Status::Over) => t.red,
        None => t.mut_,
    }
}

fn pct(ratio: Option<Decimal>) -> String {
    ratio.map_or_else(|| "—".into(), |r| percent(r, 0))
}

/// The page frame every page but the overview shares.
pub fn wrap(width: f32, body: impl IntoElement) -> AnyElement {
    div()
        .w_full()
        .flex()
        .justify_center()
        .child(
            div()
                .w_full()
                .max_w(px(1240.))
                .pt(px(36.))
                .px(px(if width < 640. { 16. } else { 40. }))
                .pb(px(72.))
                .child(body),
        )
        .into_any_element()
}

/// `◀ 2026-08 ▶`
pub fn month_switch(t: &Theme, month: MonthKey, cx: &mut Context<Root>) -> Div {
    let prev = month.prev();
    let next = month.next();
    div()
        .flex()
        .flex_row()
        .border_1()
        .border_color(t.line2)
        .child(
            div()
                .id("month-prev")
                .py(px(5.))
                .px(px(12.))
                .text_color(t.dim)
                .cursor_pointer()
                .hover(|s| s.bg(t.hl))
                .on_click(cx.listener(move |r, _, _, cx| r.set_month(prev, cx)))
                .child("◀"),
        )
        .child(
            div()
                .py(px(5.))
                .px(px(14.))
                .border_l_1()
                .border_r_1()
                .border_color(t.line2)
                .font_weight(FontWeight::BOLD)
                .child(month.to_string()),
        )
        .child(
            div()
                .id("month-next")
                .py(px(5.))
                .px(px(12.))
                .text_color(t.dim)
                .cursor_pointer()
                .hover(|s| s.bg(t.hl))
                .on_click(cx.listener(move |r, _, _, cx| r.set_month(next, cx)))
                .child("▶"),
        )
}

/// `typical over 3 [6] 12 mo`
pub fn basis_switch(
    t: &Theme,
    basis: u32,
    pad_y: f32,
    pad_x: f32,
    cx: &mut Context<Root>,
) -> Div {
    div()
        .flex()
        .flex_row()
        .border_1()
        .border_color(t.line2)
        .children([3u32, 6, 12].into_iter().enumerate().map(|(i, b)| {
            let on = b == basis;
            div()
                .id(SharedString::from(format!("basis-{b}")))
                .py(px(pad_y))
                .px(px(pad_x))
                .when(i > 0, |d| d.border_l_1().border_color(t.line2))
                .when(on, |d| d.bg(t.sel))
                .text_color(if on { t.ink } else { t.dim })
                .cursor_pointer()
                .on_click(cx.listener(move |r, _, _, cx| r.set_basis(b, cx)))
                .child(b.to_string())
        }))
}

pub fn page(
    root: &mut Root,
    t: &Theme,
    width: f32,
    cx: &mut Context<Root>,
) -> AnyElement {
    let Some(b) = root.budget() else {
        return div().into_any_element();
    };
    let wide = width >= 1060.;
    let selected = b.inspector.as_ref().map(|i| i.account.clone());

    let monthbar = div()
        .flex()
        .flex_row()
        .flex_wrap()
        .items_center()
        .gap(px(18.))
        .child(month_switch(t, b.month, cx))
        .child(div().text_color(t.dim).child(format!(
            "{} · {}",
            MONTH_NAMES[usize::from(b.month.month) - 1],
            if b.closed { "closed" } else { "open" }
        )))
        .child(div().flex_1())
        .child(div().text_color(t.dim).child("typical over"))
        .child(basis_switch(t, b.basis, 5., 12., cx))
        .child(div().text_color(t.dim).child("mo"));

    let left = div()
        .flex()
        .flex_col()
        .gap(px(44.))
        .min_w(px(0.))
        .child(categories(&b, selected.as_deref(), &root.follow, t, cx))
        .child(month_box(&b, t));
    let right = div()
        .flex()
        .flex_col()
        .gap(px(44.))
        .min_w(px(0.))
        .children(b.inspector.as_ref().map(|i| inspector(i, t)))
        .child(accounts(&b, t, cx));

    let grid = div()
        .grid()
        .grid_cols(12)
        .gap_x(px(40.))
        .gap_y(px(44.))
        .items_start()
        .child(left.col_span(if wide { 8 } else { 12 }))
        .child(right.col_span(if wide { 4 } else { 12 }));

    wrap(
        width,
        div()
            .flex()
            .flex_col()
            .gap(px(30.))
            .child(monthbar)
            .child(grid),
    )
}

/// The five columns of the category table. The header is set at 12px,
/// and the canvas sizes its columns in `ch` of that smaller face, so
/// they are narrower than the rows' and the labels sit left of the
/// figures they head.
fn columns(
    ch: f32,
    name: impl IntoElement,
    typical: impl IntoElement,
    spent: impl IntoElement,
    bar: impl IntoElement,
    pct: impl IntoElement,
) -> Div {
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(14.))
        .child(
            div()
                .flex_1()
                .min_w(px(0.))
                .overflow_hidden()
                .whitespace_nowrap()
                .child(name),
        )
        .child(
            div()
                .w(px(11. * ch))
                .flex_none()
                .flex()
                .justify_end()
                .child(typical),
        )
        .child(
            div()
                .w(px(11. * ch))
                .flex_none()
                .flex()
                .justify_end()
                .child(spent),
        )
        .child(div().w(px(15. * ch)).flex_none().child(bar))
        .child(
            div()
                .w(px(5. * ch))
                .flex_none()
                .flex()
                .justify_end()
                .child(pct),
        )
}

fn categories(
    b: &Budget,
    selected: Option<&str>,
    follow: &super::kit::Follow,
    t: &Theme,
    cx: &mut Context<Root>,
) -> Div {
    let count = b.lines.iter().filter(|l| l.account.is_some()).count();
    let groups = b
        .lines
        .iter()
        .filter(|l| l.depth == 0 && l.account.is_none())
        .count();
    let head = columns(
        12. * 0.6,
        "Expenses",
        "typical",
        "spent",
        div().child("vs typical"),
        "",
    )
    .pb(px(8.))
    .text_size(px(12.))
    .text_color(t.mut_);

    let rows = b
        .lines
        .iter()
        .enumerate()
        .map(|(i, l)| tree_row(i, l, selected, follow, t, cx));
    let (ratio, status) =
        crate::model::budget::ratio_status(b.spent, b.typical);
    let color = status_color(t, status);
    let total = columns(
        CH,
        "= total",
        div()
            .text_color(t.dim)
            .child(b.typical.map(money).unwrap_or_default()),
        div().font_weight(FontWeight::BOLD).child(money(b.spent)),
        vs_bar(t, ratio.map(f), color),
        div().text_color(color).child(pct(ratio)),
    )
    .pt(px(8.))
    .mt(px(6.))
    .border_t_1()
    .border_color(t.line2);

    boxed_right(t, "categories", format!("{count} · {groups} groups"))
        .child(head)
        .children(rows)
        .child(total)
}

fn tree_row(
    i: usize,
    l: &Line,
    selected: Option<&str>,
    follow: &super::kit::Follow,
    t: &Theme,
    cx: &mut Context<Root>,
) -> AnyElement {
    let top = l.depth == 0;
    let on = l.account.is_some() && l.account.as_deref() == selected;
    let color = status_color(t, l.status);
    let name = line(
        [
            run("  ".repeat(usize::from(l.depth)), t.ink),
            run(l.name.clone(), if top { t.ink } else { t.ink2 }).weight(
                if top {
                    FontWeight::BOLD
                } else {
                    FontWeight::NORMAL
                },
            ),
        ]
        .into_iter()
        .chain(
            (!l.label.is_empty()).then(|| run(format!(" {}", l.label), t.mut_)),
        ),
    );
    let weight = if top {
        FontWeight::BOLD
    } else {
        FontWeight::NORMAL
    };
    let cells = columns(
        CH,
        name,
        div().text_color(t.dim).child(
            l.typical.map(money).unwrap_or_else(|| money(Decimal::ZERO)),
        ),
        div().font_weight(weight).child(money(l.spent)),
        vs_bar(t, l.ratio.map(f), color),
        div().text_color(color).child(if l.ratio.is_some() {
            pct(l.ratio)
        } else {
            "  —".into()
        }),
    )
    .py(px(6.))
    .px(px(10.))
    .mx(px(-10.))
    .relative();
    let cells = super::kit::lit(cells, on, true, t, follow);
    let row = div().id(("category", i)).child(cells).hover(|s| s.bg(t.hl));
    match l.account.clone() {
        Some(account) => row
            .cursor_pointer()
            .on_click(cx.listener(move |r, _, _, cx| {
                r.select_category(account.clone(), cx)
            }))
            .into_any_element(),
        None => row.into_any_element(),
    }
}

fn month_box(b: &Budget, t: &Theme) -> Div {
    let kept = b.kept();
    let rate = (b.income > Decimal::ZERO).then(|| kept / b.income);
    let mut sums = div()
        .flex()
        .flex_col()
        .child(row(
            div().text_color(t.dim).child("  income"),
            div().text_color(t.green).child(money(b.income)),
        ))
        .child(row(
            line([run("−", t.red), run(" spent", t.dim)]),
            money(b.spent),
        ))
        .child(
            sum_rule(t).child(row(
                "= kept",
                line(
                    rate.map(|r| run(format!("{}  ", percent(r, 0)), t.mut_))
                        .into_iter()
                        .chain([bold(money(kept), t.blue)]),
                ),
            )),
        );
    let mut out = boxed(t, "month");
    if let Some(typical) = b.typical {
        let (ratio, status) =
            crate::model::budget::ratio_status(b.spent, Some(typical));
        let color = status_color(t, status);
        let parts = if b.spent > typical {
            vec![
                (f(typical), Fill::Solid(t.line2)),
                (f(b.spent - typical), Fill::Solid(t.red)),
            ]
        } else {
            vec![
                (f(b.spent), Fill::Solid(t.blue)),
                (f(typical - b.spent), Fill::Hatch(t.line2)),
            ]
        };
        sums = sums.child(
            dash(t, 22., 16.)
                .flex()
                .flex_col()
                .gap(px(8.))
                .child(row(
                    div().text_color(t.dim).child("typical month"),
                    money(typical),
                ))
                .child(bar(t, parts))
                .child(row(
                    div().text_color(t.dim).child("spent ÷ typical"),
                    line([bold(
                        ratio.map(|r| percent(r, 1)).unwrap_or_default(),
                        color,
                    )]),
                )),
        );
    }
    out = out.child(sums);
    out
}

fn inspector(i: &Inspector, t: &Theme) -> Div {
    let color = status_color(t, i.status);
    let word = match i.status {
        Some(Status::Over) => "over",
        Some(Status::Warn) => "near",
        Some(Status::Good) => "under",
        None => "",
    };
    let values: Vec<Decimal> = i.history.iter().map(|h| h.1).collect();
    let min = values.iter().copied().min().unwrap_or_default();
    let max = values
        .iter()
        .copied()
        .max()
        .unwrap_or_default()
        .max(i.typical.unwrap_or_default());
    let floor = chart_floor(min, max, 1.0);
    let top = (f(max) - floor).max(1.);
    let last = values.len().saturating_sub(1);
    let bars = values
        .iter()
        .enumerate()
        .map(|(k, v)| {
            (
                (f(*v) - floor) / top,
                Fill::Solid(if k == last { color } else { t.line2 }),
            )
        })
        .collect();
    let typical_at = i.typical.map(|v| (f(v) - floor) / top * 80.);

    boxed_right(t, i.account.clone(), "◀ ▶")
        .child(row(
            div()
                .font_weight(FontWeight::BOLD)
                .child(i.label.to_lowercase()),
            div().text_color(color).child(word),
        ))
        .child(div().mt(px(10.)).child(row(
            div().text_color(t.dim).child(format!(
                "{} ÷ {}",
                money(i.spent),
                i.typical.map(money).unwrap_or_default()
            )),
            line([bold(format!("= {}", pct(i.ratio)), color)]),
        )))
        .child(
            div()
                .relative()
                .mt(px(18.))
                .child(cols(bars, 80., 6.))
                .children(typical_at.map(|y| {
                    div()
                        .absolute()
                        .left_0()
                        .right_0()
                        .bottom(px(y))
                        .border_t_1()
                        .border_dashed()
                        .border_color(t.ink2)
                })),
        )
        .child(col_labels(
            i.history.iter().map(|h| {
                SharedString::from(MONTHS[usize::from(h.0.month) - 1])
            }),
            t.mut_,
            12.,
            6.,
        ))
        .child(
            row(
                format!(
                    "- - typical {}",
                    i.typical.map(money).unwrap_or_default()
                ),
                format!("{} mo", i.history.len()),
            )
            .text_size(px(12.))
            .text_color(t.mut_),
        )
        .child(dash(t, 22., 16.).flex().flex_col().gap(px(2.)).children(
            i.txns.iter().map(|x| {
                row(
                    line([
                        run(format!("{:02}-{:02}", x.date.1, x.date.2), t.cyan),
                        run(" ", t.ink),
                        run(
                            x.flag.to_string(),
                            if x.flag == '!' { t.yellow } else { t.green },
                        ),
                        run(" \"", t.mut_),
                        run(x.payee.clone(), t.ink),
                        run("\"", t.mut_),
                    ]),
                    money(x.amount),
                )
                .py(px(4.))
                .px(px(8.))
                .mx(px(-8.))
                .hover(|s| s.bg(t.hl))
            }),
        ))
}

fn accounts(b: &Budget, t: &Theme, cx: &mut Context<Root>) -> Div {
    let account_row = |id: usize,
                       label: String,
                       value: String,
                       color: Hsla,
                       cx: &mut Context<Root>| {
        row(
            div().text_color(color).child(label),
            div().text_color(color).child(value),
        )
        .py(px(3.))
        .px(px(8.))
        .mx(px(-8.))
        .id(("account", id))
        .hover(|s| s.bg(t.hl))
        .cursor_pointer()
        .on_click(cx.listener(|r, _, _, cx| r.open(Page::Budget, cx)))
    };
    let mut col = div().flex().flex_col();
    for (k, a) in b.accounts.iter().enumerate() {
        let color = if a.value < Decimal::ZERO {
            t.red
        } else {
            t.ink
        };
        let label = div().child(a.label.clone());
        let value = div().text_color(color).child(money(a.value));
        let account = a.account.clone();
        col = col.child(
            row(label, value)
                .py(px(3.))
                .px(px(8.))
                .mx(px(-8.))
                .id(("account", k))
                .hover(|s| s.bg(t.hl))
                .cursor_pointer()
                .on_click(cx.listener(move |r, _, _, cx| {
                    r.open_account(account.clone(), cx)
                })),
        );
    }
    col = col.child(
        sum_rule(t).child(row(
            "= budget",
            div()
                .font_weight(FontWeight::BOLD)
                .child(money(b.budget_total)),
        )),
    );
    for (k, a) in b.tracking.iter().enumerate() {
        let r = account_row(
            100 + k,
            a.label.clone(),
            a.native.clone(),
            t.purple,
            cx,
        );
        col = col.child(div().mt(px(if k == 0 { 10. } else { 0. })).child(r));
    }
    let _ = relative;
    boxed(t, "accounts").child(col)
}
