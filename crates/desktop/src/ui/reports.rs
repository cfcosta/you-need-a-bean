//! Page 3, `Reports.dc.html`.

use bean_core::model::MonthKey;
use gpui::{
    AnyElement, Context, Div, FontWeight, Hsla, InteractiveElement,
    IntoElement, ParentElement, SharedString, Styled, div, prelude::*, px,
    relative,
};
use rust_decimal::{Decimal, prelude::ToPrimitive};

use super::{
    budget::{CH, MONTHS, wrap},
    kit::{
        Fill, bar, bold, boxed, boxed_right, col_labels, dash, line, row, run,
        sign_color, slim, split, sum_rule, tracked,
    },
};
use crate::{
    fmt::{fixed, money, percent, signed, whole},
    model::reports::{Reports, duration},
    root::Root,
    theme::Theme,
};

const CH12: f32 = 12. * 0.6;

fn f(v: Decimal) -> f32 {
    v.to_f32().unwrap_or(0.)
}

fn short(m: MonthKey) -> &'static str {
    MONTHS[usize::from(m.month) - 1]
}

fn span((a, b): (MonthKey, MonthKey)) -> String {
    format!("{}–{}", short(a), short(b))
}

/// A share as the canvas writes it: one decimal, two under one percent.
pub fn share(ratio: Decimal) -> String {
    let p = ratio * Decimal::ONE_HUNDRED;
    let places = if p < Decimal::ONE { 2 } else { 1 };
    format!("{}%", p.round_dp(places).normalize())
}

/// Fixed-width columns in `ch` of a face, then flexible ones.
fn grid_row(gap: f32, cells: Vec<(Option<f32>, bool, AnyElement)>) -> Div {
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(gap))
        .whitespace_nowrap()
        .children(cells.into_iter().map(|(w, right, e)| {
            let d = match w {
                Some(w) => div().w(px(w)).flex_none(),
                None => div().flex_1().min_w(px(0.)),
            };
            d.overflow_hidden()
                .when(right, |d| d.flex().justify_end())
                .child(e)
        }))
}

fn text(s: impl Into<SharedString>, color: Hsla) -> AnyElement {
    div().text_color(color).child(s.into()).into_any_element()
}

pub fn page(
    root: &mut Root,
    t: &Theme,
    width: f32,
    _cx: &mut Context<Root>,
) -> AnyElement {
    let Some(r) = root.reports() else {
        return div().into_any_element();
    };
    let wide = width >= 1060.;
    let span7 = if wide { 7 } else { 12 };
    let span5 = if wide { 5 } else { 12 };
    let grid = div()
        .grid()
        .grid_cols(12)
        .gap_x(px(40.))
        .gap_y(px(44.))
        .items_start()
        .child(independence(&r, t, wide).col_span(12))
        .child(moved(&r, t).col_span(span7))
        .child(cashflow(&r, t).col_span(span5))
        .child(payees(&r, t).col_span(span7))
        .child(year(&r, t).col_span(span5))
        .child(projects(&r, t).col_span(span7))
        .child(rests(&r, t).col_span(span5));
    wrap(width, grid)
}

fn independence(r: &Reports, t: &Theme, wide: bool) -> Div {
    let fire = &r.view.fire;
    let progress = fire.progress.unwrap_or_default();
    let lean = fire
        .lean_number
        .filter(|_| fire.fire_number > Decimal::ZERO)
        .map(|l| f(l / fire.fire_number));
    let p = f(progress).clamp(0., 1.);
    let mut track = div().relative().mt(px(8.)).child(bar(
        t,
        vec![(p, Fill::Solid(t.blue)), (1. - p, Fill::Hatch(t.line2))],
    ));
    if let Some(l) = lean {
        track = track.child(
            div()
                .absolute()
                .left(relative(l))
                .top(px(-6.))
                .bottom(px(-6.))
                .w(px(2.))
                .bg(t.ink2),
        );
    }
    let marks = div()
        .relative()
        .h(px(22.))
        .text_size(px(12.))
        .text_color(t.mut_)
        .child(div().absolute().left_0().child(line([
            run(format!("{} ", whole(fire.net_worth)), t.mut_),
            run(percent(progress, 1), t.blue),
        ])))
        .children(fire.lean_number.zip(lean).map(|(n, l)| {
            // Centred on the mark, as `translateX(-50%)` would.
            let label = format!("lean {}", whole(n));
            let half = label.chars().count() as f32 * 12. * 0.6 / 2.;
            div()
                .absolute()
                .left(relative(l))
                .ml(px(-half))
                .child(label)
        }))
        .child(div().absolute().right_0().child(whole(fire.fire_number)));

    let left = div()
        .flex()
        .flex_col()
        .gap(px(16.))
        .min_w(px(0.))
        .child(row(
            div().text_color(t.dim).child("fire number · 4% rule"),
            div()
                .text_color(t.dim)
                .child(format!("25 × {}/yr", money(fire.annual_spend))),
        ))
        .child(div().h(px(76.)).child(tracked(
            &[
                (&super::kit::symbol(), t.mut_, FontWeight::NORMAL),
                (&whole(fire.fire_number), t.ink, FontWeight::SEMIBOLD),
            ],
            76.,
            -0.04,
        )))
        .child(track)
        .child(marks);
    let right = div()
        .flex()
        .flex_col()
        .gap(px(6.))
        .min_w(px(0.))
        .child(row(text("investable", t.dim), money(fire.net_worth)))
        .child(row(text("spend / mo", t.dim), money(fire.monthly_spend)))
        .child(row(
            text("saved / mo", t.dim),
            text(
                money(fire.monthly_savings),
                sign_color(t, fire.monthly_savings),
            ),
        ))
        .child(row(
            text("4% pays today", t.dim),
            format!("{}/mo", money(fire.swr_monthly)),
        ))
        .children(r.view.runway.months.map(|m| {
            row(text("runway", t.dim), format!("{} mo", fixed(m, 1)))
        }));

    let scenarios = r.scenarios();
    let scale = scenarios
        .iter()
        .filter_map(|s| s.months)
        .max()
        .unwrap_or(1)
        .div_ceil(60) as f32
        * 60.;
    let middle = scenarios.len() / 2;
    let cols = |ch: f32| {
        [
            Some(14. * ch),
            Some(10. * ch),
            Some(10. * ch),
            None,
            Some(12. * ch),
        ]
    };
    let head = {
        let w = cols(CH12);
        grid_row(
            16.,
            vec![
                (w[0], false, text("real return", t.mut_)),
                (w[1], false, text("reach", t.mut_)),
                (w[2], false, text("around", t.mut_)),
                (w[3], false, text("", t.mut_)),
                (w[4], true, text("coasting", t.mut_)),
            ],
        )
        .text_size(px(12.))
        .pb(px(6.))
    };
    let w = cols(CH);
    let rows = scenarios.iter().enumerate().map(|(i, s)| {
        let mid = i == middle;
        let filled = s.months.map_or(0., |m| m as f32 / scale);
        grid_row(
            16.,
            vec![
                (
                    w[0],
                    false,
                    line(
                        [run(format!("{}%", s.rate_pct), t.ink)]
                            .into_iter()
                            .chain(mid.then(|| run(" ←", t.mut_))),
                    )
                    .into_any_element(),
                ),
                (
                    w[1],
                    false,
                    div()
                        .when(mid, |d| d.font_weight(FontWeight::BOLD))
                        .child(s.reach.clone())
                        .into_any_element(),
                ),
                (
                    w[2],
                    false,
                    text(
                        s.around.map(|m| m.to_string()).unwrap_or_default(),
                        t.cyan,
                    ),
                ),
                (
                    w[3],
                    false,
                    slim(
                        t,
                        vec![
                            (filled, Fill::Solid(t.blue)),
                            (1. - filled, Fill::Hatch(t.line2)),
                        ],
                    )
                    .into_any_element(),
                ),
                (w[4], true, text(s.coast.clone(), t.dim)),
            ],
        )
        .py(px(3.))
        .when(mid, |d| d.bg(t.hl).mx(px(-10.)).px(px(10.)))
    });
    let steps = &fire.steps;
    let mut more = vec![];
    for (k, s) in steps.iter().enumerate() {
        if k > 0 {
            more.push(run("    ", t.dim));
        }
        more.push(run(signed(s.extra), t.green));
        more.push(run(
            format!(
                "/mo → {}",
                s.months.map(duration).unwrap_or_else(|| "—".into())
            ),
            t.dim,
        ));
    }
    let rate = scenarios.get(middle).map_or(5, |s| s.rate_pct);

    boxed_right(
        t,
        "independence",
        fire.window
            .map(|w| format!("basis {}", span(w)))
            .unwrap_or_default(),
    )
    .child(if wide {
        split(7., 5., 48., left, right)
    } else {
        div()
            .flex()
            .flex_col()
            .gap(px(24.))
            .child(left)
            .child(right)
    })
    .child(dash(t, 22., 16.).child(head).children(rows).when(
        !steps.is_empty(),
        |d| {
            d.child(
                row(text(format!("save more @ {rate}%"), t.dim), line(more))
                    .mt(px(12.)),
            )
        },
    ))
}

fn moved(r: &Reports, t: &Theme) -> Div {
    let m = r.moved_recent(12);
    let stacks = diverging(t, &m.months, 150., 6.);
    let first = r.view.growth.window.map(|w| short(w.0)).unwrap_or("");
    let last = m.months.last().map(|p| short(p.0)).unwrap_or("");
    boxed(t, "net worth moved")
        .child(
            div()
                .flex()
                .flex_col()
                .child(row(
                    text(format!("  opening · {first}"), t.dim),
                    money(m.opening),
                ))
                .child(row(
                    line([run("+", t.blue), run(" you saved", t.dim)]),
                    text(money(m.saved), t.blue),
                ))
                .child(row(
                    line([run("+", t.purple), run(" markets moved", t.dim)]),
                    text(money(m.markets), t.purple),
                ))
                .child(sum_rule(t).child(row(
                    format!("= {last}"),
                    line([bold(money(m.closing), t.ink)]),
                )))
                .children(m.implied_return.map(|ret| {
                    row(
                        text("return on the pot", t.dim),
                        text(percent(ret, 1), sign_color(t, ret)),
                    )
                    .mt(px(6.))
                })),
        )
        .child(
            dash(t, 22., 16.)
                .child(stacks)
                .child(div().mt(px(6.)).child(col_labels(
                    m.months.iter().map(|p| SharedString::from(short(p.0))),
                    t.mut_,
                    12.,
                    6.,
                )))
                .child(
                    row(
                        line([
                            run("■", t.blue),
                            run(" saved  ", t.mut_),
                            run("■", t.purple),
                            run(" markets", t.mut_),
                        ]),
                        m.so_far
                            .map(|(month, v)| {
                                format!("{} so far {}", short(month), money(v))
                            })
                            .unwrap_or_default(),
                    )
                    .mt(px(8.))
                    .text_size(px(12.))
                    .text_color(t.mut_),
                ),
        )
}

fn cashflow(r: &Reports, t: &Theme) -> Div {
    let rows = r.cashflow_recent(12);
    let window = match (rows.first(), rows.last()) {
        (Some(a), Some(b)) => span((a.month, b.month)),
        _ => String::new(),
    };
    let w = |ch: f32| {
        [
            Some(4. * ch),
            Some(9. * ch),
            Some(9. * ch),
            None,
            Some(4. * ch),
        ]
    };
    let wh = w(CH12);
    let head = grid_row(
        12.,
        vec![
            (wh[0], false, text("", t.mut_)),
            (wh[1], true, text("in", t.mut_)),
            (wh[2], true, text("out", t.mut_)),
            (wh[3], false, text("kept", t.mut_)),
            (wh[4], false, text("", t.mut_)),
        ],
    )
    .text_size(px(12.))
    .pb(px(6.));
    let wr = w(CH);
    let body = rows.iter().map(|c| {
        let (bar_parts, pct, pct_color) = match c.rate {
            // More went out than came in: the bar is all overspend.
            Some(rate) if rate.is_sign_negative() => {
                (vec![(1., Fill::Hatch(t.red))], "over".to_string(), t.red)
            }
            Some(rate) => {
                let p = f(rate).clamp(0., 1.);
                (
                    vec![
                        (p, Fill::Solid(t.green)),
                        (1. - p, Fill::Hatch(t.line2)),
                    ],
                    percent(rate, 0),
                    t.dim,
                )
            }
            None => (vec![(1., Fill::Hatch(t.red))], "—".to_string(), t.red),
        };
        grid_row(
            12.,
            vec![
                (wr[0], false, text(short(c.month), t.dim)),
                (
                    wr[1],
                    true,
                    text(
                        whole(c.income),
                        if c.income.is_zero() { t.mut_ } else { t.green },
                    ),
                ),
                (wr[2], true, text(whole(c.expenses), t.ink)),
                (wr[3], false, slim(t, bar_parts).into_any_element()),
                (wr[4], true, text(pct, pct_color)),
            ],
        )
        .py(px(2.))
    });
    let income = &r.view.income;
    boxed_right(t, "cashflow", window)
        .child(head)
        .children(body)
        .child(
            dash(t, 22., 16.)
                .children(income.sources.iter().take(LISTED).map(|s| {
                    row(
                        text(format!("Income:{}", s.name), t.dim),
                        line([
                            run(money(s.total), t.ink),
                            run(format!(" {}", percent(s.share, 0)), t.mut_),
                        ]),
                    )
                }))
                .when(income.sources.len() > LISTED, |d| {
                    let rest: Decimal = income
                        .sources
                        .iter()
                        .skip(LISTED)
                        .map(|s| s.total)
                        .sum();
                    d.child(row(
                        text(
                            format!("⋮ {} more", income.sources.len() - LISTED),
                            t.mut_,
                        ),
                        text(money(rest), t.mut_),
                    ))
                })
                .child(row(
                    text("passive", t.dim),
                    text(money(income.passive), t.mut_),
                )),
        )
}

fn payees(r: &Reports, t: &Theme) -> Div {
    let p = &r.view.payees;
    let rows = p.items.iter().enumerate().map(|(k, i)| {
        let sh = f(i.share).clamp(0., 1.);
        grid_row(
            14.,
            vec![
                (
                    Some(18. * CH),
                    false,
                    line([
                        run("\"", t.mut_),
                        run(super::kit::clip(&i.name, 16), t.ink),
                        run("\"", t.mut_),
                    ])
                    .into_any_element(),
                ),
                (
                    Some(10. * CH),
                    true,
                    text(format!("{}× {}", i.count, whole(i.average)), t.dim),
                ),
                (
                    None,
                    false,
                    slim(
                        t,
                        vec![
                            (sh, Fill::Solid(t.blue)),
                            (1. - sh, Fill::Hatch(t.line2)),
                        ],
                    )
                    .into_any_element(),
                ),
                (Some(11. * CH), true, text(money(i.spent), t.ink)),
                (Some(6. * CH), true, text(share(i.share), t.dim)),
            ],
        )
        .py(px(4.))
        .px(px(10.))
        .mx(px(-10.))
        .id(("payee", k))
        .hover(|s| s.bg(t.hl))
    });
    boxed_right(
        t,
        "where the money goes",
        p.window.map(span).unwrap_or_default(),
    )
    .children(rows)
    .child(sum_rule(t).child(row(
        format!("= {} payees", p.items.len()),
        div().font_weight(FontWeight::BOLD).child(money(p.total)),
    )))
}

fn year(r: &Reports, t: &Theme) -> Div {
    let y = &r.view.year;
    let total: Decimal = y.groups.iter().map(|g| g.total).sum();
    let rows = y.groups.iter().map(|g| {
        let ratio = if total.is_zero() {
            Decimal::ZERO
        } else {
            g.total / total
        };
        let sh = (f(ratio)).max(0.008);
        let color = if g.name == "Uncategorized" {
            t.yellow
        } else {
            t.blue
        };
        grid_row(
            12.,
            vec![
                (Some(14. * CH), false, text(g.name.clone(), t.ink)),
                (
                    None,
                    false,
                    slim(
                        t,
                        vec![
                            (sh, Fill::Solid(color)),
                            (1. - sh, Fill::Hatch(t.line2)),
                        ],
                    )
                    .into_any_element(),
                ),
                (Some(8. * CH), true, text(whole(g.total), t.ink)),
                (Some(6. * CH), true, text(share(ratio), t.dim)),
            ],
        )
        .py(px(3.))
    });
    let mv = &r.view.movers;
    let changed = mv.recent_total - mv.prior_total;
    let movers = mv.items.iter().map(|m| {
        let tail = match m.ratio {
            Some(ratio) => format!(" {}", percent(ratio, 0)),
            None => " new".into(),
        };
        row(
            text(format!("  {}", m.label), t.ink),
            line([
                run(
                    fixed(m.delta, 0).replace('-', "−"),
                    if m.delta < Decimal::ZERO {
                        t.green
                    } else {
                        t.red
                    },
                )
                .weight(FontWeight::NORMAL),
                run(tail, t.mut_),
            ]),
        )
    });
    let movers_title = match (mv.recent, mv.prior) {
        (Some(a), Some(b)) => format!("{} vs {}", span(a), span(b)),
        _ => String::new(),
    };
    boxed(t, "where the year went").children(rows).when(
        !mv.items.is_empty(),
        |d| {
            d.child(
                dash(t, 22., 16.)
                    .flex()
                    .flex_col()
                    .gap(px(4.))
                    .child(row(
                        text(movers_title, t.dim),
                        text(
                            money(changed),
                            if changed <= Decimal::ZERO {
                                t.green
                            } else {
                                t.red
                            },
                        ),
                    ))
                    .children(movers),
            )
        },
    )
}

fn projects(r: &Reports, t: &Theme) -> Div {
    let p = &r.view.projects;
    boxed(t, "projects")
        .child(row(
            text("#tags · ^links over one txn", t.dim),
            p.items.len().to_string(),
        ))
        .children(
            p.items
                .iter()
                .take(LISTED)
                .map(|i| row(super::kit::clip(&i.name, 40), money(i.spent))),
        )
        .when(p.items.len() > LISTED, |d| {
            d.child(text(format!("⋮ {} more", p.items.len() - LISTED), t.mut_))
        })
}

/// The longest a list runs inside a box before the rest is counted.
const LISTED: usize = 6;

fn rests(r: &Reports, t: &Theme) -> Div {
    let (flagged, uncategorized) = r.doubt();
    let trust = &r.view.trust;
    let uncat_name = trust
        .uncategorized
        .accounts
        .first()
        .cloned()
        .unwrap_or_else(|| "uncategorized".into());
    boxed(t, "what these numbers rest on").child(
        div()
            .flex()
            .flex_col()
            .child(
                row(
                    line([
                        bold("!", t.yellow),
                        run(" unconfirmed ", t.ink),
                        run(format!("{} flagged", trust.flagged.total), t.mut_),
                    ]),
                    money(flagged),
                )
                .py(px(4.))
                .px(px(10.))
                .mx(px(-10.)),
            )
            .child(
                row(
                    line([
                        bold("?", t.yellow),
                        run(format!(" {uncat_name}"), t.ink),
                    ]),
                    money(uncategorized),
                )
                .py(px(4.))
                .px(px(10.))
                .mx(px(-10.)),
            )
            .child(sum_rule(t).child(row(
                "= on a doubt",
                line([bold(money(flagged + uncategorized), t.yellow)]),
            ))),
    )
}

/// Each month's saving and market move as stacked columns, gains above
/// a baseline and losses below it, both on one scale.
pub fn diverging(
    t: &Theme,
    months: &[(MonthKey, Decimal, Decimal)],
    height: f32,
    gap: f32,
) -> Div {
    let up = |v: Decimal| f(v).max(0.);
    let down = |v: Decimal| (-f(v)).max(0.);
    let rise = months
        .iter()
        .map(|(_, s, k)| up(*s) + up(*k))
        .fold(0., f32::max);
    let fall = months
        .iter()
        .map(|(_, s, k)| down(*s) + down(*k))
        .fold(0., f32::max);
    let span = (rise + fall).max(1.);
    let above = height * rise / span;
    let below = height - above;
    let column = |s: Decimal, k: Decimal| {
        let piece =
            |v: f32, color| div().w_full().h(px(v / span * height)).bg(color);
        div()
            .flex_1()
            .flex()
            .flex_col()
            .child(
                div()
                    .h(px(above))
                    .flex()
                    .flex_col()
                    .justify_end()
                    .child(piece(up(k), t.purple))
                    .child(piece(up(s), t.blue)),
            )
            .child(
                div()
                    .h(px(below))
                    .flex()
                    .flex_col()
                    .child(piece(down(s), t.red))
                    .child(piece(down(k), t.purple.opacity(0.5))),
            )
    };
    div()
        .flex()
        .flex_row()
        .gap(px(gap))
        .h(px(height))
        .children(months.iter().map(|(_, s, k)| column(*s, *k)))
}
