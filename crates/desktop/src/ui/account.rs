//! `Account.dc.html`: one account's month, as the arithmetic of its
//! balance, six months of flows and the register itself.

use gpui::{
    AnyElement, Context, Div, FontWeight, Hsla, InteractiveElement,
    IntoElement, ParentElement, SharedString, StatefulInteractiveElement,
    Styled, div, prelude::*, px,
};
use rust_decimal::{Decimal, prelude::ToPrimitive};

use super::{
    budget::{CH, MONTHS, month_switch, wrap},
    kit::{
        bold, boxed, boxed_right, col_labels, dash, line, row, run, sum_rule,
        tracked,
    },
};
use crate::{
    fmt::{money, signed, whole},
    model::account::{Entry, Register},
    root::{Page, Root},
    theme::Theme,
};

fn f(v: Decimal) -> f32 {
    v.to_f32().unwrap_or(0.)
}

/// What the status line calls an account: `Everyday account` → `everyday`.
pub fn crumb(label: &str) -> String {
    let l = label.to_lowercase();
    l.strip_suffix(" account").map(str::to_owned).unwrap_or(l)
}

pub fn page(
    root: &mut Root,
    t: &Theme,
    width: f32,
    cx: &mut Context<Root>,
) -> AnyElement {
    let Some(r) = root.register() else {
        return div().into_any_element();
    };
    let wide = width >= 1060.;

    let monthbar = div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(18.))
        .child(month_switch(t, r.month, cx))
        .child(
            div()
                .id("back-to-budget")
                .text_color(t.dim)
                .cursor_pointer()
                .hover(|s| s.text_color(t.ink))
                .on_click(
                    cx.listener(|root, _, _, cx| root.open(Page::Budget, cx)),
                )
                .child("← budget"),
        );

    let grid = div()
        .grid()
        .grid_cols(12)
        .gap_x(px(40.))
        .gap_y(px(44.))
        .items_start()
        .child(balance(&r, t).col_span(if wide { 7 } else { 12 }))
        .child(flows(&r, t).col_span(if wide { 5 } else { 12 }))
        .child(register(&r, t).col_span(12));

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

fn balance(r: &Register, t: &Theme) -> Div {
    boxed(t, r.account.clone())
        .child(row(
            div()
                .text_color(t.dim)
                .child(format!("{} · {}", r.label, r.kind)),
            div().text_color(t.dim).child(r.month.to_string()),
        ))
        .child(div().mt(px(14.)).mb(px(22.)).h(px(76.)).child(tracked(
            &[
                (&super::kit::symbol(), t.mut_, FontWeight::NORMAL),
                (&money(r.balance), t.ink, FontWeight::SEMIBOLD),
            ],
            76.,
            -0.04,
        )))
        .child(row(
            div().text_color(t.dim).child("  opening"),
            money(r.opening),
        ))
        .child(row(
            line([run("+", t.green), run(format!(" in · {}", r.ins), t.dim)]),
            div().text_color(t.green).child(money(r.inflow)),
        ))
        .child(row(
            line([run("−", t.red), run(format!(" out · {}", r.outs), t.dim)]),
            money(-r.outflow),
        ))
        .child(
            sum_rule(t).child(row(
                "= balance",
                line([bold(money(r.balance), t.blue)]),
            )),
        )
}

fn flows(r: &Register, t: &Theme) -> Div {
    let top = r
        .history
        .iter()
        .map(|h| f(h.inflow.max(h.outflow)))
        .fold(1., f32::max);
    let last = r.history.len().saturating_sub(1);
    let pairs = div()
        .flex()
        .flex_row()
        .items_end()
        .gap(px(14.))
        .h(px(150.))
        .children(r.history.iter().enumerate().map(|(k, h)| {
            let o = if k == last { 1. } else { 0.45 };
            div()
                .flex_1()
                .h_full()
                .flex()
                .flex_row()
                .items_end()
                .gap(px(3.))
                .child(
                    div()
                        .flex_1()
                        .h(gpui::relative(f(h.inflow) / top))
                        .bg(t.green.opacity(o)),
                )
                .child(
                    div()
                        .flex_1()
                        .h(gpui::relative(f(h.outflow) / top))
                        .bg(t.red.opacity(o)),
                )
        }));
    let first = r
        .history
        .first()
        .and_then(|h| h.balance)
        .unwrap_or_default();
    let end = r
        .history
        .last()
        .and_then(|h| h.balance)
        .unwrap_or(r.balance);
    let short = |k: usize| {
        r.history
            .get(k)
            .map_or("", |h| MONTHS[usize::from(h.month.month) - 1])
    };
    boxed(t, format!("{} months", r.history.len()))
        .child(pairs)
        .child(div().mt(px(6.)).child(col_labels(
            r.history.iter().map(|h| {
                SharedString::from(MONTHS[usize::from(h.month.month) - 1])
            }),
            t.mut_,
            12.,
            14.,
        )))
        .child(
            dash(t, 22., 16.)
                .flex()
                .flex_col()
                .gap(px(4.))
                .child(row(
                    div()
                        .text_color(t.dim)
                        .child(format!("net {}", short(last))),
                    div()
                        .text_color(t.green)
                        .child(signed(r.inflow - r.outflow)),
                ))
                .child(row(
                    div().text_color(t.dim).child(format!(
                        "balance {} → {}",
                        short(0),
                        short(last)
                    )),
                    format!("{} → {}", whole(first), whole(end)),
                )),
        )
}

/// date · flag · who · other leg · amount · balance.
fn reg_cols(ch: f32, cells: [AnyElement; 6]) -> Div {
    let [date, flag, who, other, amount, balance] = cells;
    div()
        .flex()
        .flex_row()
        .gap(px(14.))
        .whitespace_nowrap()
        .child(div().w(px(6. * ch)).flex_none().child(date))
        .child(div().w(px(2. * ch)).flex_none().child(flag))
        .child(div().flex_1().min_w(px(0.)).overflow_hidden().child(who))
        .child(
            div()
                .w(px(24. * ch))
                .flex_shrink_1()
                .min_w(px(0.))
                .overflow_hidden()
                .child(other),
        )
        .child(
            div()
                .w(px(11. * ch))
                .flex_none()
                .flex()
                .justify_end()
                .child(amount),
        )
        .child(
            div()
                .w(px(11. * ch))
                .flex_none()
                .flex()
                .justify_end()
                .child(balance),
        )
}

fn leg_color(t: &Theme, account: &str) -> Hsla {
    if account.starts_with("Assets") {
        t.blue
    } else if account.starts_with("Income") {
        t.green
    } else {
        t.dim
    }
}

fn entry(k: usize, e: &Entry, t: &Theme) -> AnyElement {
    let flagged = e.flag == '!';
    let who = if e.payee.is_empty() {
        line([
            run("\"", t.mut_),
            run(e.narration.clone(), t.dim),
            run("\"", t.mut_),
        ])
    } else {
        line(
            [
                run("\"", t.mut_),
                run(e.payee.clone(), t.ink),
                run("\"", t.mut_),
            ]
            .into_iter()
            .chain(
                (!e.narration.is_empty())
                    .then(|| run(format!(" {}", e.narration), t.dim)),
            ),
        )
    };
    reg_cols(
        CH,
        [
            div()
                .text_color(t.cyan)
                .child(format!("{:02}-{:02}", e.date.1, e.date.2))
                .into_any_element(),
            div()
                .text_color(if flagged { t.yellow } else { t.green })
                .font_weight(FontWeight::BOLD)
                .child(e.flag.to_string())
                .into_any_element(),
            who.into_any_element(),
            div()
                .text_color(leg_color(t, &e.other))
                .child(e.other.clone())
                .into_any_element(),
            div()
                .text_color(if e.amount > Decimal::ZERO {
                    t.green
                } else {
                    t.ink
                })
                .child(signed(e.amount))
                .into_any_element(),
            div()
                .text_color(t.dim)
                .child(e.balance.map(money).unwrap_or_default())
                .into_any_element(),
        ],
    )
    .py(px(6.))
    .px(px(10.))
    .mx(px(-10.))
    .when(flagged, |d| d.bg(t.hl))
    .id(("entry", k))
    .hover(|s| s.bg(t.hl))
    .into_any_element()
}

fn register(r: &Register, t: &Theme) -> Div {
    let text = |s: &str| {
        div()
            .child(SharedString::from(s.to_owned()))
            .into_any_element()
    };
    let head = reg_cols(
        12. * 0.6,
        [
            text("date"),
            text(""),
            text("payee · narration"),
            text("other leg"),
            text("amount"),
            text("balance"),
        ],
    )
    .pb(px(6.))
    .text_size(px(12.))
    .text_color(t.mut_);
    let opening = reg_cols(
        CH,
        [
            text(&format!("{:02}-01", r.month.month)),
            text(""),
            text("opening balance"),
            text(""),
            text(""),
            text(&money(r.opening)),
        ],
    )
    .py(px(6.))
    .text_color(t.dim)
    .border_t_1()
    .border_color(t.line);
    boxed_right(t, "register", format!("{} txns", r.entries.len()))
        .child(head)
        .child(opening)
        .children(r.entries.iter().enumerate().map(|(k, e)| entry(k, e, t)))
}
