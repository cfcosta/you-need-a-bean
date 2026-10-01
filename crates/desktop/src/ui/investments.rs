//! Page 4, `Investments.dc.html`.

use gpui::{
    AnyElement, Context, Div, FontWeight, Hsla, InteractiveElement,
    IntoElement, ParentElement, SharedString, StatefulInteractiveElement,
    Styled, div, prelude::*, px,
};
use rust_decimal::{Decimal, prelude::ToPrimitive};

use super::{
    budget::{CH, wrap},
    kit::sign_color,
    kit::{
        Fill, bar, boxed, col_labels, cols, dash, line, row, run, slim,
        sum_rule, tracked,
    },
};
use crate::{
    fmt::{money, percent, signed, whole},
    model::investments::{Investments, Range, Sort},
    root::{Field, Root},
    theme::Theme,
};

const CH12: f32 = 12. * 0.6;

fn f(v: Decimal) -> f32 {
    v.to_f32().unwrap_or(0.)
}

fn text(s: impl Into<SharedString>, color: Hsla) -> Div {
    div().text_color(color).whitespace_nowrap().child(s.into())
}

fn class_colors(t: &Theme) -> [Hsla; 5] {
    [t.purple, t.blue, t.cyan, t.teal, t.mut_]
}

/// A position's units as the ledger would write them: whole when whole.
pub fn units(v: Decimal) -> String {
    crate::fmt::units(v)
}

pub fn page(
    root: &mut Root,
    t: &Theme,
    width: f32,
    cx: &mut Context<Root>,
) -> AnyElement {
    let Some(i) = root.investments() else {
        return div().into_any_element();
    };
    let wide = width >= 1060.;
    let filter = root.holding_filter.clone();
    let editing = root.typing == Some(Field::HoldingFilter);
    let sort = root.holding_sort;
    let today = root.data.as_ref().map(|d| d.today).unwrap_or((1970, 1, 1));
    let grid = div()
        .grid()
        .grid_cols(12)
        .gap_x(px(40.))
        .gap_y(px(44.))
        .items_start()
        .child(portfolio(&i, today, t).col_span(if wide { 7 } else { 12 }))
        .child(allocation(&i, t).col_span(if wide { 5 } else { 12 }))
        .child(performance(&i, t, cx).col_span(12))
        .child(holdings(&i, &filter, editing, sort, t, cx).col_span(12));
    wrap(width, grid)
}

fn portfolio(i: &Investments, today: bean_core::model::Day, t: &Theme) -> Div {
    let mut sums = div().flex().flex_col();
    for p in &i.positions {
        sums = sums.child(row(
            text(
                format!(
                    "  {} {} × {}",
                    units(p.units),
                    p.currency,
                    money(p.price)
                ),
                t.dim,
            ),
            money(p.value),
        ));
    }
    let cost = match i.positions.as_slice() {
        [only] if only.basis.is_some() && !only.units.is_zero() => {
            format!(
                " cost {} × {}",
                units(only.units),
                money(only.basis.unwrap_or_default() / only.units)
            )
        }
        _ => " cost".into(),
    };
    sums = sums
        .child(row(
            line([run("−", t.dim), run(cost, t.dim)]),
            money(-i.basis),
        ))
        .child(
            sum_rule(t).child(row(
                "= unrealized gain",
                line(
                    i.ret
                        .map(|r| {
                            run(format!("{}  ", plus(percent(r, 1), r)), t.mut_)
                        })
                        .into_iter()
                        .chain([run(signed(i.gain), t.green)
                            .weight(FontWeight::BOLD)]),
                ),
            )),
        );
    let largest = i.positions.first();
    boxed(t, "portfolio")
        .child(row(
            text(
                format!("market value · as of {:02}-{:02}", today.1, today.2),
                t.dim,
            ),
            text(super::kit::currency(), t.dim),
        ))
        .child(div().mt(px(14.)).mb(px(22.)).h(px(92.)).child(tracked(
            &[
                (&super::kit::symbol(), t.mut_, FontWeight::NORMAL),
                (&whole(i.value), t.purple, FontWeight::SEMIBOLD),
            ],
            92.,
            -0.04,
        )))
        .child(sums)
        .child(
            dash(t, 22., 16.)
                .flex()
                .flex_col()
                .gap(px(6.))
                .children(i.coverage.map(|c| {
                    row(text("value with known cost", t.dim), percent(c, 0))
                }))
                .children(largest.map(|p| {
                    row(
                        text("largest holding", t.dim),
                        line([
                            run(p.currency.clone(), t.ink),
                            run(format!(" {}", percent(p.share, 0)), t.mut_),
                        ]),
                    )
                })),
        )
}

fn plus(text: String, v: Decimal) -> String {
    if v > Decimal::ZERO {
        format!("+{text}")
    } else {
        text
    }
}

fn allocation(i: &Investments, t: &Theme) -> Div {
    let colors = class_colors(t);
    let segments = i
        .classes
        .iter()
        .enumerate()
        .map(|(k, c)| (f(c.share), Fill::Solid(colors[k.min(4)])))
        .collect();
    let mut b = boxed(t, "allocation");
    for c in &i.classes {
        b = b.child(row(
            c.name.clone().unwrap_or_else(|| "unclassified".into()),
            share_whole(c.share),
        ));
    }
    let holdings = i.positions.len();
    b.child(div().mt(px(10.)).mb(px(18.)).child(bar(t, segments)))
        .child(row(
            text(
                format!(
                    "{holdings} holding{} · {} class{}",
                    if holdings == 1 { "" } else { "s" },
                    i.classes.len(),
                    if i.classes.len() == 1 { "" } else { "es" }
                ),
                t.mut_,
            ),
            text(money(i.value), t.mut_),
        ))
        .child(
            dash(t, 22., 16.).flex().flex_col().gap(px(4.)).children(
                // Each account once, with what it holds under it.
                by_account(i)
                    .into_iter()
                    .flat_map(|(account, label, held)| {
                        std::iter::once(row(text(account, t.dim), ""))
                            .chain(std::iter::once(row(
                                text(format!("  {label}"), t.ink),
                                "",
                            )))
                            .chain(held.into_iter().map(
                                move |(amount, currency)| {
                                    row(
                                        "",
                                        text(
                                            format!(
                                                "{} {currency}",
                                                units(amount)
                                            ),
                                            t.purple,
                                        ),
                                    )
                                },
                            ))
                    }),
            ),
        )
}

/// An account, its name, and the amounts of each commodity it holds.
type Placed = (String, String, Vec<(Decimal, String)>);

/// The holdings grouped by the account they sit in, in first-seen order.
fn by_account(i: &Investments) -> Vec<Placed> {
    let mut out: Vec<Placed> = Vec::new();
    for p in &i.positions {
        for l in &p.locations {
            match out.iter_mut().find(|(a, _, _)| *a == l.account) {
                Some((_, _, held)) => held.push((l.units, p.currency.clone())),
                None => out.push((
                    l.account.clone(),
                    l.label.clone(),
                    vec![(l.units, p.currency.clone())],
                )),
            }
        }
    }
    out
}

fn share_whole(r: Decimal) -> String {
    percent(r, 0)
}

fn performance(i: &Investments, t: &Theme, cx: &mut Context<Root>) -> Div {
    let p = &i.performance;
    // Every point keeps its place, priced or not, so bars and labels
    // stay in step.
    let values: Vec<Option<Decimal>> =
        p.points.iter().map(|x| x.value).collect();
    let priced = values.iter().flatten().copied();
    let min = priced.clone().min().unwrap_or_default();
    let max = priced.max().unwrap_or_default();
    // The canvas stands these bars on 90% of the lowest value.
    let floor = f(min) * 0.9;
    let top = (f(max) - floor).max(1.);
    let last = values.len().saturating_sub(1);
    let bars = values
        .iter()
        .enumerate()
        .map(|(k, v)| match v {
            Some(v) => (
                ((f(*v) - floor) / top).max(0.01),
                Fill::Solid(if k == last { t.purple } else { t.line2 }),
            ),
            None => (0., Fill::Solid(t.line2)),
        })
        .collect();
    let days: Vec<_> = p.points.iter().map(|x| x.date).collect();
    let labels = crate::model::investments::month_labels(&days)
        .into_iter()
        .map(SharedString::from);
    let gain = p.gain.unwrap_or_default();
    let known = p.gain.is_some();
    let ranges = div()
        .flex()
        .flex_row()
        .border_1()
        .border_color(t.line2)
        .children(Range::CHOICES.iter().enumerate().map(
            |(k, (range, label))| {
                let on = *range == i.range;
                let range = *range;
                div()
                    .id(SharedString::from(format!("range-{label}")))
                    .py(px(5.))
                    .px(px(12.))
                    .when(k > 0, |d| d.border_l_1().border_color(t.line2))
                    .when(on, |d| d.bg(t.sel))
                    .text_color(if on { t.ink } else { t.dim })
                    .cursor_pointer()
                    .on_click(
                        cx.listener(move |r, _, _, cx| r.set_range(range, cx)),
                    )
                    .child(*label)
            },
        ));
    let fact = |label: String, value: String, color: Hsla, bold: bool| {
        div()
            .flex()
            .flex_col()
            .child(text(label, if bold { t.ink } else { t.dim }))
            .child(
                div()
                    .text_size(px(18.))
                    .text_color(color)
                    .when(bold, |d| d.font_weight(FontWeight::BOLD))
                    .child(value),
            )
    };
    let day = |d: bean_core::model::Day| format!("{:02}-{:02}", d.1, d.2);
    boxed(t, "performance")
        .child(
            div()
                .flex()
                .flex_row()
                .flex_wrap()
                .items_center()
                .gap(px(16.))
                .child(text("price gain", t.dim))
                .child(
                    div()
                        .text_size(px(30.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(if gain < Decimal::ZERO {
                            t.red
                        } else {
                            t.green
                        })
                        .child(if known { signed(gain) } else { "—".into() }),
                )
                .children(p.ret.map(|r| text(plus(percent(r, 0), r), t.green)))
                .child(div().flex_1())
                .child(ranges),
        )
        .child(div().mt(px(22.)).child(cols(bars, 120., 6.)))
        .child(div().mt(px(6.)).child(col_labels(labels, t.mut_, 12., 6.)))
        .child(
            dash(t, 22., 16.)
                .grid()
                .grid_cols(4)
                .gap_x(px(16.))
                .child(fact(
                    format!("opening {}", day(p.start)),
                    p.opening.map(money).unwrap_or_else(|| "—".into()),
                    t.ink,
                    false,
                ))
                .child(fact(
                    "+ net flows".into(),
                    p.net_flows.map(money).unwrap_or_else(|| "—".into()),
                    t.ink,
                    false,
                ))
                .child(fact(
                    "+ price gain".into(),
                    if known { money(gain) } else { "—".into() },
                    sign_color(t, gain),
                    false,
                ))
                .child(fact(
                    format!("= closing {}", day(p.end)),
                    p.closing.map(money).unwrap_or_default(),
                    t.purple,
                    true,
                )),
        )
}

fn hold_cols(ch: f32, cells: [AnyElement; 6]) -> Div {
    let [name, units, price, value, alloc, gain] = cells;
    let right = |w: f32, e: AnyElement| {
        div().w(px(w)).flex_none().flex().justify_end().child(e)
    };
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(16.))
        .whitespace_nowrap()
        .child(div().flex_1().min_w(px(0.)).overflow_hidden().child(name))
        .child(right(9. * ch, units))
        .child(right(15. * ch, price))
        .child(right(12. * ch, value))
        .child(
            div()
                .w(px(14. * ch))
                .flex_shrink_1()
                .min_w(px(0.))
                .child(alloc),
        )
        .child(right(12. * ch, gain))
}

fn holdings(
    i: &Investments,
    filter: &str,
    editing: bool,
    sort: Sort,
    t: &Theme,
    cx: &mut Context<Root>,
) -> Div {
    let shown = i.holdings(filter, sort);
    let input = div()
        .id("holding-filter")
        .flex()
        .flex_row()
        .items_center()
        .gap(px(10.))
        .flex_1()
        .min_w(px(220.))
        .py(px(6.))
        .px(px(12.))
        .border_1()
        .border_color(if editing { t.blue } else { t.line2 })
        .cursor_text()
        .on_click(
            cx.listener(|r, _, _, cx| r.start_typing(Field::HoldingFilter, cx)),
        )
        .child(text("/", t.blue))
        .child(if filter.is_empty() {
            text("holding, account…", t.mut_)
        } else {
            text(filter.to_owned(), t.ink)
        })
        .when(editing, |d| d.child(div().w(px(2.)).h(px(18.)).bg(t.ink)));
    let sorts = div()
        .flex()
        .flex_row()
        .border_1()
        .border_color(t.line2)
        .children(
            [
                (Sort::Value, "value"),
                (Sort::Gain, "gain"),
                (Sort::Name, "name"),
            ]
            .into_iter()
            .enumerate()
            .map(|(k, (s, label))| {
                let on = s == sort;
                div()
                    .id(SharedString::from(format!("sort-{label}")))
                    .py(px(5.))
                    .px(px(12.))
                    .when(k > 0, |d| d.border_l_1().border_color(t.line2))
                    .when(on, |d| d.bg(t.sel))
                    .text_color(if on { t.ink } else { t.dim })
                    .cursor_pointer()
                    .on_click(cx.listener(move |r, _, _, cx| r.set_sort(s, cx)))
                    .child(label)
            }),
        );
    let head = hold_cols(
        CH12,
        [
            "holding",
            "units",
            "price · date",
            "value",
            "allocation",
            "gain",
        ]
        .map(|s| div().child(s).into_any_element()),
    )
    .text_size(px(12.))
    .text_color(t.mut_)
    .pb(px(6.));
    let rows = shown.iter().enumerate().map(|(k, p)| {
        let s = f(p.share).clamp(0., 1.);
        hold_cols(
            CH,
            [
                line([
                    run(p.currency.clone(), t.purple).weight(FontWeight::BOLD),
                    run(format!(" {}", p.label), t.dim),
                ])
                .into_any_element(),
                div().child(units(p.units)).into_any_element(),
                line([run(money(p.price), t.ink)].into_iter().chain(
                    p.price_date.map(|d| {
                        run(format!(" {:02}-{:02}", d.1, d.2), t.cyan)
                    }),
                ))
                .into_any_element(),
                div().child(money(p.value)).into_any_element(),
                slim(
                    t,
                    vec![
                        (s, Fill::Solid(t.purple)),
                        (1. - s, Fill::Hatch(t.line2)),
                    ],
                )
                .into_any_element(),
                text(
                    p.gain.map(signed).unwrap_or_else(|| "—".into()),
                    if p.gain.unwrap_or_default() < Decimal::ZERO {
                        t.red
                    } else {
                        t.green
                    },
                )
                .into_any_element(),
            ],
        )
        .py(px(8.))
        .px(px(10.))
        .mx(px(-10.))
        .border_t_1()
        .border_color(t.line)
        .id(("holding", k))
        .hover(|s| s.bg(t.hl))
    });
    boxed(t, "holdings")
        .child(
            div()
                .flex()
                .flex_row()
                .flex_wrap()
                .gap(px(12.))
                .mb(px(18.))
                .child(input)
                .child(sorts),
        )
        .child(head)
        .children(rows)
        .child(
            row(
                format!("{} of {}", shown.len(), i.positions.len()),
                "cost: average, per account",
            )
            .mt(px(10.))
            .text_size(px(12.))
            .text_color(t.mut_),
        )
}
