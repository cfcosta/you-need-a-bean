//! Page 1, `Main.dc.html`: position, net worth, the next 30 days, goals
//! and the review queue.

use gpui::{
    AnyElement, Context, FontWeight, InteractiveElement, IntoElement,
    ParentElement, SharedString, StatefulInteractiveElement, Styled, div,
    prelude::*, px,
};
use rust_decimal::{Decimal, prelude::ToPrimitive};

use super::kit::{
    Fill, bar, bold, boxed, col_labels, cols, dash, line, row, run, sum_rule,
    tracked,
};
use crate::{
    fmt::{fixed, money, percent, signed, whole},
    model::overview::{Event, Overview, ReviewKind},
    root::Root,
    theme::Theme,
};

const MONTH_INITIALS: [&str; 12] =
    ["J", "F", "M", "A", "M", "J", "J", "A", "S", "O", "N", "D"];
const MONTHS: [&str; 12] = [
    "jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct",
    "nov", "dec",
];

fn f(v: Decimal) -> f32 {
    v.to_f32().unwrap_or(0.)
}

/// The floor a small chart stands on: a round number a little below the
/// lowest value, so changes read without the bars losing their base.
pub fn chart_floor(min: Decimal, max: Decimal, slack: f32) -> f32 {
    let (min, max) = (f(min), f(max));
    let range = (max - min).max(1.);
    let step = 10f32.powf(range.log10().floor());
    ((min - slack * range) / step).floor() * step
}

pub fn page(
    root: &Root,
    t: &Theme,
    width: f32,
    cx: &mut Context<Root>,
) -> AnyElement {
    let Some(data) = &root.data else {
        return div().into_any_element();
    };
    let o = &data.overview;
    let wide = width >= 1060.;
    let span = |n: u16| if wide { n } else { 12 };

    let grid = div()
        .grid()
        .grid_cols(12)
        .gap_x(px(40.))
        .gap_y(px(44.))
        .child(position(o, t).col_span(span(7)))
        .child(net_worth(o, t).col_span(span(5)))
        .child(next_days(root, o, t, cx).col_span(12))
        .child(goals(o, t).col_span(span(7)))
        .child(review(o, t).col_span(span(5)));

    div()
        .w_full()
        .flex()
        .justify_center()
        .child(
            div()
                .w_full()
                .max_w(px(1240.))
                .pt(px(40.))
                .px(px(if width < 640. { 16. } else { 40. }))
                .pb(px(72.))
                .child(grid),
        )
        .into_any_element()
}

fn huge(t: &Theme, figure: &str, color: gpui::Hsla) -> gpui::Div {
    div()
        .flex()
        .flex_row()
        .mt(px(14.))
        .mb(px(22.))
        .h(px(92.))
        .child(tracked(
            &[
                ("$", t.mut_, FontWeight::NORMAL),
                (figure, color, FontWeight::SEMIBOLD),
            ],
            92.,
            -0.04,
        ))
}

fn position(o: &Overview, t: &Theme) -> gpui::Div {
    let mut b = boxed(t, "position")
        .child(row(
            run_text("free after reserves", t.dim),
            run_text("USD", t.dim),
        ))
        .child(huge(t, &whole(o.free), t.ink))
        .child(bar(
            t,
            vec![
                (f(o.free), Fill::Solid(t.blue)),
                (f(o.reserved), Fill::Hatch(t.yellow)),
            ],
        ))
        .child(
            div()
                .mt(px(22.))
                .flex()
                .flex_col()
                .child(row(run_text("  held", t.dim), money(o.held)))
                .child(row(
                    line([run("−", t.yellow), run(" reserved", t.dim)]),
                    run_text(&money(-o.reserved), t.yellow),
                ))
                .child(sum_rule(t).child(row(
                    run_text("= free", t.ink),
                    line([bold(money(o.free), t.blue)]),
                ))),
        );
    if let Some(r) = &o.runway {
        let full = r.months.trunc().to_usize().unwrap_or(0);
        let part = f(r.months.fract());
        let start = o.today.0;
        b = b.child(
            dash(t, 26., 18.)
                .flex()
                .flex_col()
                .gap(px(6.))
                .child(row(
                    run_text("runway", t.dim),
                    line([
                        run(
                            format!("{} ÷ {}/mo", whole(o.free), whole(r.burn)),
                            t.ink2,
                        ),
                        run(" = ", t.mut_),
                        bold(format!("{} mo", fixed(r.months, 1)), t.blue),
                        run(" → ", t.mut_),
                        run(r.until.to_string(), t.cyan),
                    ]),
                ))
                .child(runway_years(t, full, part, start, o.today.1)),
        );
    }
    b
}

/// Four years of months as cells: lit while the cash lasts.
fn runway_years(
    t: &Theme,
    full: usize,
    part: f32,
    year: u16,
    month: u8,
) -> gpui::Div {
    let lit = full.min(48);
    div()
        .flex()
        .flex_row()
        .gap(px(10.))
        .children((0..4).map(|g| {
            let cells = (0..12).map(|c| {
                let i = g * 12 + c;
                let cell = div().flex_1().h(px(14.));
                if i < lit {
                    cell.bg(t.blue)
                } else if i == lit && part > 0. {
                    cell.flex()
                        .flex_row()
                        .child(
                            div().h_full().w(gpui::relative(part)).bg(t.blue),
                        )
                        .child(div().h_full().flex_1().bg(t.line2))
                } else {
                    cell.bg(t.line2)
                }
            });
            div()
                .flex_1()
                .flex()
                .flex_col()
                .gap(px(6.))
                .child(div().flex().flex_row().gap(px(3.)).children(cells))
                .child(div().text_size(px(12.)).text_color(t.mut_).child(
                    format!(
                        "{} {:02}",
                        MONTHS[usize::from(month) - 1],
                        (year + g as u16) % 100
                    ),
                ))
        }))
}

fn run_text(text: &str, color: gpui::Hsla) -> gpui::Div {
    div()
        .text_color(color)
        .whitespace_nowrap()
        .child(SharedString::from(text.to_owned()))
}

fn net_worth(o: &Overview, t: &Theme) -> gpui::Div {
    let values: Vec<Decimal> = o.history.iter().map(|p| p.1).collect();
    let (min, max) = (
        values.iter().copied().min().unwrap_or_default(),
        values.iter().copied().max().unwrap_or_default(),
    );
    let floor = chart_floor(min, max, 0.1);
    let top = f(max) - floor;
    let first = o.history.first().map(|p| p.0);
    let last = o.history.last().map(|p| p.0);
    let name = |m: Option<bean_core::model::MonthKey>| {
        m.map(|m| MONTHS[usize::from(m.month) - 1]).unwrap_or("")
    };
    let since = values.last().copied().unwrap_or_default()
        - values.first().copied().unwrap_or_default();

    let mut b = boxed(t, "net worth")
        .child(row(
            run_text(name(first), t.dim),
            run_text(name(last), t.dim),
        ))
        .child(
            div().mt(px(10.)).mb(px(6.)).child(cols(
                values
                    .iter()
                    .map(|v| ((f(*v) - floor) / top, Fill::Solid(t.purple)))
                    .collect(),
                72.,
                3.,
            )),
        )
        .child(col_labels(
            o.history.iter().map(|p| {
                SharedString::from(MONTH_INITIALS[usize::from(p.0.month) - 1])
            }),
            t.mut_,
            12.,
            3.,
        ))
        .child(div().mt(px(10.)).child(row(
            run_text(&format!("since {}", name(first)), t.dim),
            run_text(&signed(since), t.green),
        )))
        .child(
            div()
                .mt(px(22.))
                .flex()
                .flex_col()
                .child(row(run_text("  assets", t.dim), money(o.assets)))
                .child(row(
                    line([run("−", t.red), run(" liabilities", t.dim)]),
                    run_text(&money(-o.liabilities), t.red),
                ))
                .child(sum_rule(t).child(row(
                    "= net worth",
                    line([bold(money(o.net_worth), t.purple)]),
                ))),
        );

    let mut facts = dash(t, 26., 18.).flex().flex_col().gap(px(6.));
    facts = facts.child(row(
        run_text("saved / mo", t.dim),
        line(
            [run(money(o.saved_month), t.ink)].into_iter().chain(
                o.savings_rate
                    .map(|r| run(format!(" {}", percent(r, 0)), t.green)),
            ),
        ),
    ));
    if let Some(h) = o.holdings.first() {
        facts = facts.child(row(
            run_text(&format!("{} × {}", h.currency, fixed(h.units, 0)), t.dim),
            line([run(money(h.value), t.ink)].into_iter().chain(
                h.ret.map(|r| run(format!(" {}", plus_percent(r)), t.green)),
            )),
        ));
    }
    if let Some((p, target)) = o.independence {
        facts = facts.child(row(
            run_text("independence", t.dim),
            line([
                run(percent(p, 1), t.ink),
                run(format!(" of {}", whole(target)), t.mut_),
            ]),
        ));
    }
    b = b.child(facts);
    b
}

fn plus_percent(r: Decimal) -> String {
    let p = percent(r, 1);
    if r > Decimal::ZERO {
        format!("+{p}")
    } else {
        p
    }
}

fn next_days(
    root: &Root,
    o: &Overview,
    t: &Theme,
    cx: &mut Context<Root>,
) -> gpui::Div {
    let fc = &o.forecast[root.horizon.min(o.forecast.len().saturating_sub(1))];
    let min = fc.daily.iter().copied().min().unwrap_or_default();
    let max = fc.daily.iter().copied().max().unwrap_or_default();
    let floor = chart_floor(min, max, 0.2);
    let top = (f(max) - floor).max(1.);
    let start = fc.daily.first().copied().unwrap_or_default();
    let income: Vec<_> = o
        .events
        .iter()
        .filter(|e| e.amount > Decimal::ZERO)
        .map(|e| crate::model::ordinal(e.date) - crate::model::ordinal(o.today))
        .collect();
    let mut changed = false;
    let bars = fc
        .daily
        .iter()
        .enumerate()
        .map(|(i, v)| {
            changed |= *v != start;
            let color = if income.contains(&(i as i64)) {
                t.green
            } else if changed {
                t.blue
            } else {
                t.line2
            };
            ((f(*v) - floor) / top, Fill::Solid(color))
        })
        .collect();
    let end_day = crate::model::add_days(o.today, i64::from(fc.days));
    let short = |d: bean_core::model::Day| format!("{:02}-{:02}", d.1, d.2);

    let horizons = ["30d", "60d", "90d"];
    let selector = div()
        .flex()
        .flex_row()
        .border_1()
        .border_color(t.line2)
        .children(horizons.iter().enumerate().map(|(i, h)| {
            let on = i == root.horizon;
            div()
                .id(SharedString::from(format!("horizon-{h}")))
                .py(px(6.))
                .px(px(14.))
                .when(i > 0, |d| d.border_l_1().border_color(t.line2))
                .when(on, |d| d.bg(t.sel))
                .text_color(if on { t.ink } else { t.dim })
                .cursor_pointer()
                .on_click(
                    cx.listener(move |root, _, _, cx| root.set_horizon(i, cx)),
                )
                .child(*h)
        }));

    let horizon_events: Vec<&Event> = o.events.iter().collect();
    let shown = horizon_events.iter().take(4);
    let more = o.events.len().saturating_sub(4);

    boxed(t, format!("next {} days", fc.days))
        .child(
            div()
                .flex()
                .flex_row()
                .flex_wrap()
                .items_end()
                .gap(px(24.))
                .child(
                    div()
                        .flex_1()
                        .min_w(px(280.))
                        .flex()
                        .flex_col()
                        .gap(px(6.))
                        .child(cols(bars, 56., 2.))
                        .child(
                            row(
                                run_text(
                                    &format!(
                                        "{}  {}",
                                        short(o.today),
                                        whole(start)
                                    ),
                                    t.mut_,
                                ),
                                line([
                                    run(
                                        format!("{}  ", short(end_day)),
                                        t.mut_,
                                    ),
                                    run(whole(fc.end), t.ink),
                                ]),
                            )
                            .text_size(px(12.)),
                        ),
                )
                .child(selector),
        )
        .child(
            div()
                .mt(px(26.))
                .flex()
                .flex_col()
                .children(shown.map(|e| entry(t, e)))
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .gap(px(12.))
                        .py(px(5.))
                        .px(px(10.))
                        .mx(px(-10.))
                        .text_color(t.mut_)
                        .child(div().w(px(92.4)).child("⋮"))
                        .child(div().w(px(16.8)))
                        .child(format!(
                            "{more} more · {} recurring",
                            o.recurring
                        )),
                ),
        )
}

/// One upcoming transaction, written the way the ledger would write it.
fn entry(t: &Theme, e: &Event) -> impl IntoElement {
    let income = e.amount > Decimal::ZERO;
    div()
        .flex()
        .flex_row()
        .gap(px(12.))
        .py(px(5.))
        .px(px(10.))
        .mx(px(-10.))
        .whitespace_nowrap()
        .hover(|s| s.bg(t.hl))
        .child(
            div()
                .w(px(92.4))
                .flex_none()
                .text_color(t.cyan)
                .child(bean_core::home::date(e.date)),
        )
        .child(
            div()
                .w(px(16.8))
                .flex_none()
                .text_color(if e.scheduled { t.green } else { t.mut_ })
                .child(if e.scheduled { "*" } else { "~" }),
        )
        .child(div().w(px(201.6)).flex_shrink_1().overflow_hidden().child(
            line([
                run("\"", t.mut_),
                run(e.label.clone(), t.ink),
                run("\"", t.mut_),
            ]),
        ))
        .child(
            div()
                .flex_1()
                .min_w(px(0.))
                .overflow_hidden()
                .text_color(t.dim)
                .child(e.account.clone()),
        )
        .child(
            div()
                .w(px(109.2))
                .flex_none()
                .flex()
                .justify_end()
                .text_color(if income { t.green } else { t.ink })
                .child(signed(e.amount)),
        )
        .child(
            div()
                .w(px(33.6))
                .flex_none()
                .text_color(t.purple)
                .child("USD"),
        )
        .child(
            div()
                .w(px(84.))
                .flex_none()
                .flex()
                .justify_end()
                .text_color(t.mut_)
                .child(whole(e.balance)),
        )
}

fn goals(o: &Overview, t: &Theme) -> gpui::Div {
    boxed(t, "goals").child(
        div()
            .flex()
            .flex_col()
            .gap(px(18.))
            .children(o.goals.iter().map(|g| {
                let ratio = g.ratio();
                let color = if ratio >= Decimal::new(75, 2) {
                    t.green
                } else {
                    t.yellow
                };
                div()
                    .flex()
                    .flex_col()
                    .gap(px(4.))
                    .child(row(
                        g.label.to_lowercase(),
                        line([
                            bold(percent(ratio, 0), color),
                            run(" by ", t.mut_),
                            run(
                                format!("{:02}-{:02}", g.date.1, g.date.2),
                                t.cyan,
                            ),
                        ]),
                    ))
                    .child(bar(
                        t,
                        vec![
                            (f(ratio), Fill::Solid(color)),
                            ((1. - f(ratio)).max(0.), Fill::Hatch(t.line2)),
                        ],
                    ))
                    .child(
                        row(money(g.funded), money(g.target))
                            .text_size(px(12.))
                            .text_color(t.dim),
                    )
            }))
            .child(
                div().flex().child(
                    div()
                        .py(px(6.))
                        .px(px(12.))
                        .border_1()
                        .border_dashed()
                        .border_color(t.line2)
                        .text_color(t.dim)
                        .child("+ goal"),
                ),
            ),
    )
}

fn review(o: &Overview, t: &Theme) -> gpui::Div {
    let (current, total) = o.coverage;
    let dots: String = (0..total)
        .map(|i| if i < current { '●' } else { '○' })
        .collect();
    boxed(t, "review")
        .child(div().flex().flex_col().children(o.review.iter().map(|r| {
            div()
                .flex()
                .flex_row()
                .gap(px(12.))
                .py(px(5.))
                .px(px(10.))
                .mx(px(-10.))
                .hover(|s| s.bg(t.hl))
                .child(
                    div()
                        .w(px(16.8))
                        .text_color(t.yellow)
                        .font_weight(FontWeight::BOLD)
                        .child("!"),
                )
                .child(div().flex_1().child(r.label.clone()))
                .child(div().text_color(t.dim).child(match r.kind {
                    ReviewKind::Flagged => "payee →",
                    ReviewKind::Category => "spend →",
                    ReviewKind::Other => "→",
                }))
        })))
        .child(dash(t, 18., 16.).flex().flex_col().gap(px(6.)).child(row(
            run_text("coverage", t.dim),
            line([
                run(dots, if current == 0 { t.line2 } else { t.green }),
                run(
                    format!(" {current}/{total}"),
                    if current < total { t.yellow } else { t.green },
                ),
            ]),
        )))
}
