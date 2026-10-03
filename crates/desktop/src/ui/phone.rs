//! The phone boards: the same pages on a 390px screen — a 13/21 rhythm,
//! tighter boxes, a header in place of the status line and the pages
//! along the bottom.

use bean_core::model::MonthKey;
use gpui::{
    AnyElement, Context, Div, FontWeight, Hsla, InteractiveElement,
    IntoElement, ParentElement, SharedString, StatefulInteractiveElement,
    Styled, div, prelude::*, px, relative,
};
use rust_decimal::{Decimal, prelude::ToPrimitive};

use super::{
    budget::{MONTHS, basis_switch, status_color},
    investments::units,
    kit::{
        Fill, bold, cells, col_labels, cols, line, row, run, sum_rule, tracked,
        vs_bar,
    },
    reports::share,
    search::marked_at,
};
use crate::{
    fmt::{fixed, money, percent, signed, whole},
    model::{account::Register, investments::Range, search::Hit},
    root::{Page, Root},
    theme::Theme,
};

pub const SIZE: f32 = 13.;
pub const LINE: f32 = 21.;
/// A phone is anything this narrow.
pub const WIDTH: f32 = 640.;

fn f(v: Decimal) -> f32 {
    v.to_f32().unwrap_or(0.)
}

fn text(s: impl Into<SharedString>, color: Hsla) -> Div {
    div().text_color(color).whitespace_nowrap().child(s.into())
}

fn short(m: MonthKey) -> &'static str {
    MONTHS[usize::from(m.month) - 1]
}

/// The phone box: the legend sits 12px in, padded 6.
pub(super) fn pbox(t: &Theme, label: impl Into<SharedString>) -> Div {
    div()
        .relative()
        .min_w(px(0.))
        .pt(px(23.))
        .px(px(17.))
        .pb(px(17.))
        .child(div().absolute().inset_0().border_1().border_color(t.line2))
        .child(
            div()
                .absolute()
                .top(px(-10.))
                .left(px(13.))
                .px(px(6.))
                .bg(t.bg)
                .text_color(t.dim)
                .whitespace_nowrap()
                .child(label.into()),
        )
}

/// The phone's 16px bar of 7px cells.
fn pbar(t: &Theme, parts: Vec<(f32, Fill)>, height: f32) -> impl IntoElement {
    cells(t, parts, height, 7., 2.)
}

fn dash(t: &Theme, top: f32, pad: f32) -> Div {
    div()
        .mt(px(top))
        .pt(px(pad))
        .border_t_1()
        .border_dashed()
        .border_color(t.line2)
}

fn huge(
    t: &Theme,
    figure: &str,
    size: f32,
    color: Hsla,
    above: f32,
    below: f32,
    line_height: f32,
) -> Div {
    let lh = size * line_height;
    div()
        .mt(px(above + (lh - size) / 2.))
        .mb(px(below + (lh - size) / 2.))
        .h(px(size))
        .child(tracked(
            &[
                (&super::kit::symbol(), t.mut_, FontWeight::NORMAL),
                (figure, color, FontWeight::SEMIBOLD),
            ],
            size,
            -0.04,
        ))
}

pub fn header(
    title: &str,
    t: &Theme,
    today: (u16, u8, u8),
    cx: &mut Context<Root>,
) -> AnyElement {
    div()
        .flex()
        .flex_row()
        .flex_none()
        .h(px(44.))
        .bg(t.bar)
        .border_b_1()
        .border_color(t.line)
        .whitespace_nowrap()
        .child(
            div()
                .id("phone-bean")
                .flex()
                .items_center()
                .px(px(14.))
                .bg(t.yellow)
                .text_color(t.on_block)
                .font_weight(FontWeight::BOLD)
                .on_click(cx.listener(|r, _, _, cx| r.open(Page::Overview, cx)))
                .child("◆ bean"),
        )
        .child(
            div()
                .flex()
                .items_center()
                .px(px(12.))
                .text_color(t.dim)
                .child(title.to_owned()),
        )
        .child(div().flex_1())
        .child(
            div()
                .id("phone-search")
                .w(px(52.))
                .flex()
                .items_center()
                .justify_center()
                .border_l_1()
                .border_color(t.line)
                .text_color(t.ink2)
                .on_click(
                    cx.listener(|r, _, window, cx| r.open_search(window, cx)),
                )
                .child("/"),
        )
        .child(
            div()
                .flex()
                .items_center()
                .px(px(12.))
                .bg(t.blue)
                .text_color(t.on_block)
                .font_weight(FontWeight::BOLD)
                .child(format!("{:02}-{:02}", today.1, today.2)),
        )
        .into_any_element()
}

pub fn nav(page: Page, t: &Theme, cx: &mut Context<Root>) -> AnyElement {
    div()
        .grid()
        .grid_cols(Page::TABS.len() as u16)
        .flex_none()
        .bg(t.bar)
        .border_t_1()
        .border_color(t.line)
        .text_size(px(11.))
        .children(Page::TABS.iter().enumerate().map(|(i, p)| {
            let on = *p == page.tab();
            let target = *p;
            div()
                .id(SharedString::from(format!("phone-tab-{}", p.name())))
                .flex()
                .flex_col()
                .items_center()
                .pt(px(8.))
                .pb(px(18.))
                .when(on, |d| d.bg(t.sel))
                .text_color(if on { t.ink } else { t.dim })
                .on_click(cx.listener(move |r, _, _, cx| r.open(target, cx)))
                .child(
                    div()
                        .text_color(if on { t.blue } else { t.mut_ })
                        .child((i + 1).to_string()),
                )
                .child(p.name())
        }))
        .into_any_element()
}

/// The page body under the header, padded the way each board pads it.
fn body(top: f32, gap: f32) -> Div {
    div()
        .flex()
        .flex_col()
        .pt(px(top))
        .px(px(16.))
        .pb(px(32.))
        .gap(px(gap))
}

pub fn page(root: &mut Root, t: &Theme, cx: &mut Context<Root>) -> AnyElement {
    match root.page {
        Page::Overview => overview(root, t),
        Page::Budget => budget(root, t, cx),
        Page::Account => account(root, t, cx),
        Page::Reports => reports(root, t),
        Page::Investments => investments(root, t, cx),
        Page::Liabilities => liabilities(root, t, cx),
        Page::Query => super::console::phone(root, t, cx),
    }
}

pub fn title(root: &mut Root) -> String {
    match root.page {
        Page::Account => {
            let crumb = root
                .register()
                .map(|r| super::account::crumb(&r.label))
                .unwrap_or_default();
            format!("budget › {crumb}")
        }
        page => page.name().to_owned(),
    }
}

fn overview(root: &Root, t: &Theme) -> AnyElement {
    let Some(data) = &root.data else {
        return div().into_any_element();
    };
    let o = &data.overview;
    let mut position =
        pbox(t, "position")
            .child(row(
                text("free after reserves", t.dim),
                text(super::kit::currency(), t.dim),
            ))
            .child(huge(t, &whole(o.free), 60., t.ink, 12., 18., 1.))
            .child(pbar(
                t,
                vec![
                    (f(o.free), Fill::Solid(t.blue)),
                    (f(o.reserved), Fill::Hatch(t.yellow)),
                ],
                16.,
            ))
            .child(
                div()
                    .mt(px(16.))
                    .child(row(text("  held", t.dim), money(o.held)))
                    .child(row(
                        line([run("−", t.yellow), run(" reserved", t.dim)]),
                        text(money(-o.reserved), t.yellow),
                    ))
                    .child(sum_rule(t).child(row(
                        "= free",
                        line([bold(money(o.free), t.blue)]),
                    ))),
            );
    if let Some(r) = &o.runway {
        position = position.child(dash(t, 18., 14.).child(row(
            text("runway", t.dim),
            line([
                bold(format!("{} mo", fixed(r.months, 1)), t.blue),
                run(" → ", t.mut_),
                run(r.until.to_string(), t.cyan),
            ]),
        )));
    }

    let history = o.recent_history(12);
    let values: Vec<Decimal> = history.iter().map(|p| p.1).collect();
    let (min, max) = (
        values.iter().copied().min().unwrap_or_default(),
        values.iter().copied().max().unwrap_or_default(),
    );
    let floor = super::overview::chart_floor(min, max, 0.1);
    let top = (f(max) - floor).max(1.);
    let first = history.first().map(|p| short(p.0)).unwrap_or("");
    let last = history.last().map(|p| short(p.0)).unwrap_or("");
    let net = pbox(t, "net worth")
        .child(row(
            text(format!("{first} → {last}"), t.dim),
            line([bold(money(o.net_worth), t.purple)]),
        ))
        .child(
            div().mt(px(12.)).mb(px(6.)).child(cols(
                values
                    .iter()
                    .map(|v| ((f(*v) - floor) / top, Fill::Solid(t.purple)))
                    .collect(),
                56.,
                2.,
            )),
        )
        .child(row(
            text(format!("since {first}"), t.dim),
            text(
                signed(
                    values.last().copied().unwrap_or_default()
                        - values.first().copied().unwrap_or_default(),
                ),
                t.green,
            ),
        ))
        .child(row(
            text("saved / mo", t.dim),
            line([run(money(o.saved_month), t.ink)].into_iter().chain(
                o.savings_rate.map(|r| {
                    run(
                        format!(" {}", percent(r, 0)),
                        super::kit::sign_color(t, r),
                    )
                }),
            )),
        ));

    let fc = &o.forecast[root.horizon.min(o.forecast.len().saturating_sub(1))];
    let fmin = fc.daily.iter().copied().min().unwrap_or_default();
    let fmax = fc.daily.iter().copied().max().unwrap_or_default();
    let ffloor = super::overview::chart_floor(fmin, fmax, 0.2);
    let ftop = (f(fmax) - ffloor).max(1.);
    let start = fc.daily.first().copied().unwrap_or_default();
    let income: Vec<i64> = o
        .events
        .iter()
        .filter(|e| e.amount > Decimal::ZERO)
        .map(|e| bean_core::date::days_between(o.today, e.date))
        .collect();
    let mut changed = false;
    let bars = fc
        .daily
        .iter()
        .enumerate()
        .map(|(i, v)| {
            changed |= *v != start;
            let c = if income.contains(&(i as i64)) {
                t.green
            } else if changed {
                t.blue
            } else {
                t.line2
            };
            ((f(*v) - ffloor) / ftop, Fill::Solid(c))
        })
        .collect();
    let end = crate::model::add_days(o.today, i64::from(fc.days));
    let ahead = o.upcoming(fc.days);
    let shown: Vec<_> = ahead.iter().take(super::kit::EVENTS_SHOWN).collect();
    let more = ahead.len() - shown.len();
    let count = shown.len();
    let next = pbox(t, format!("next {} days", fc.days))
        .child(cols(bars, 88., 2.))
        .child(
            row(
                format!("{:02}-{:02}", o.today.1, o.today.2),
                line([
                    run(format!("{:02}-{:02} ", end.1, end.2), t.mut_),
                    run(whole(fc.end), t.ink),
                ]),
            )
            .mt(px(4.))
            .text_size(px(11.))
            .text_color(t.mut_),
        )
        .child(div().mt(px(16.)).flex().flex_col().gap(px(4.)).children(
            shown.into_iter().enumerate().map(|(k, e)| {
                div()
                    .flex()
                    .flex_col()
                    .py(px(8.))
                    .when(k + 1 < count, |d| {
                        d.border_b_1().border_color(t.line)
                    })
                    .child(row(
                        line([
                            run(
                                format!("{:02}-{:02}", e.date.1, e.date.2),
                                t.cyan,
                            ),
                            run(" ", t.ink),
                            run(
                                if e.scheduled { "*" } else { "~" },
                                if e.scheduled { t.green } else { t.mut_ },
                            ),
                            run(
                                format!(" {}", super::kit::clip(&e.label, 22)),
                                t.ink,
                            ),
                        ]),
                        text(
                            signed(e.amount),
                            if e.amount > Decimal::ZERO {
                                t.green
                            } else {
                                t.ink
                            },
                        ),
                    ))
                    .child(
                        row(e.account.clone(), whole(e.balance))
                            .text_size(px(11.))
                            .text_color(t.mut_),
                    )
            }),
        ))
        .when(more > 0, |d| {
            d.child(
                text(format!("⋮ {more} more"), t.mut_)
                    .min_h(px(36.))
                    .flex()
                    .items_center(),
            )
        });

    body(28., 36.)
        .child(position)
        .child(net)
        .child(next)
        .into_any_element()
}

fn month_bar(t: &Theme, month: MonthKey, cx: &mut Context<Root>) -> Div {
    let (prev, next) = (month.prev(), month.next());
    div()
        .flex()
        .flex_row()
        .border_1()
        .border_color(t.line2)
        .child(
            div()
                .id("p-month-prev")
                .py(px(8.))
                .px(px(14.))
                .text_color(t.dim)
                .on_click(cx.listener(move |r, _, _, cx| r.set_month(prev, cx)))
                .child("◀"),
        )
        .child(
            div()
                .py(px(8.))
                .px(px(12.))
                .border_l_1()
                .border_r_1()
                .border_color(t.line2)
                .font_weight(FontWeight::BOLD)
                .child(month.to_string()),
        )
        .child(
            div()
                .id("p-month-next")
                .py(px(8.))
                .px(px(14.))
                .text_color(t.dim)
                .on_click(cx.listener(move |r, _, _, cx| r.set_month(next, cx)))
                .child("▶"),
        )
}

fn budget(root: &mut Root, t: &Theme, cx: &mut Context<Root>) -> AnyElement {
    let Some(b) = root.budget() else {
        return div().into_any_element();
    };
    let top = div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(10.))
        .child(month_bar(t, b.month, cx))
        .child(div().flex_1())
        .child(basis_switch(t, b.basis, 8., 10., cx));
    let (ratio, status) =
        crate::model::budget::ratio_status(b.spent, b.typical);
    let month = pbox(t, "month")
        .child(row(text("  income", t.dim), text(money(b.income), t.green)))
        .child(row(
            line([run("−", t.red), run(" spent", t.dim)]),
            money(b.spent),
        ))
        .child(
            sum_rule(t)
                .child(row("= kept", line([bold(money(b.kept()), t.blue)]))),
        )
        .children(b.typical.map(|typ| {
            row(
                text(format!("vs typical {}", money(typ)), t.dim),
                text(
                    ratio.map(|r| percent(r, 1)).unwrap_or_default(),
                    status_color(t, status),
                ),
            )
            .mt(px(12.))
        }));
    let lines = b.lines.iter().map(|l| {
        let top = l.depth == 0;
        let ind = "  ".repeat(usize::from(l.depth));
        let color = status_color(t, l.status);
        let weight = if top {
            FontWeight::BOLD
        } else {
            FontWeight::NORMAL
        };
        div()
            .flex()
            .flex_col()
            .gap(px(4.))
            .py(px(8.))
            .border_b_1()
            .border_color(t.line)
            .child(row(
                div().font_weight(weight).child(format!("{ind}{}", l.name)),
                line([
                    run(money(l.spent), t.ink).weight(weight),
                    run(
                        format!(
                            " {}",
                            l.ratio
                                .map(|r| percent(r, 0))
                                .unwrap_or_else(|| "—".into())
                        ),
                        if l.ratio.is_some() { color } else { t.mut_ },
                    ),
                ]),
            ))
            .when(l.ratio.is_some(), |d| {
                d.child(vs_bar(t, l.ratio.map(f), color))
            })
            .child(
                row(
                    format!("{ind}{}", l.label),
                    format!(
                        "typ {}",
                        l.typical
                            .map(money)
                            .unwrap_or_else(|| money(Decimal::ZERO))
                    ),
                )
                .text_size(px(11.))
                .text_color(t.mut_),
            )
    });
    body(20., 32.)
        .child(top)
        .child(month)
        .child(pbox(t, "categories").children(lines))
        .into_any_element()
}

fn account(root: &mut Root, t: &Theme, cx: &mut Context<Root>) -> AnyElement {
    let Some(r) = root.register() else {
        return div().into_any_element();
    };
    body(20., 32.)
        .child(div().flex().child(month_bar(t, r.month, cx)))
        .child(account_box(&r, t))
        .child(register_box(&r, t))
        .into_any_element()
}

fn account_box(r: &Register, t: &Theme) -> Div {
    let top = r
        .history
        .iter()
        .map(|h| f(h.inflow.max(h.outflow)))
        .fold(1., f32::max);
    let last = r.history.len().saturating_sub(1);
    let label = r
        .account
        .split_once(':')
        .map_or(r.account.as_str(), |(_, rest)| rest)
        .to_owned();
    pbox(t, label)
        .child(huge(t, &money(r.balance), 44., t.ink, 4., 14., 1.1))
        .child(row(text("  opening", t.dim), money(r.opening)))
        .child(row(
            line([run("+", t.green), run(" in", t.dim)]),
            text(money(r.inflow), t.green),
        ))
        .child(row(
            line([run("−", t.red), run(" out", t.dim)]),
            money(-r.outflow),
        ))
        .child(
            sum_rule(t).child(row(
                "= balance",
                line([bold(money(r.balance), t.blue)]),
            )),
        )
        .child(
            div()
                .mt(px(16.))
                .flex()
                .flex_row()
                .items_end()
                .gap(px(14.))
                .h(px(70.))
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
                                .h(relative(f(h.inflow) / top))
                                .bg(t.green.opacity(o)),
                        )
                        .child(
                            div()
                                .flex_1()
                                .h(relative(f(h.outflow) / top))
                                .bg(t.red.opacity(o)),
                        )
                })),
        )
        .child(div().mt(px(4.)).child(col_labels(
            r.history.iter().map(|h| SharedString::from(short(h.month))),
            t.mut_,
            11.,
            14.,
        )))
}

fn register_box(r: &Register, t: &Theme) -> Div {
    let count = r.entries.len();
    pbox(t, "register").children(r.entries.iter().enumerate().map(|(k, e)| {
        let flagged = e.flag == '!';
        let who = if e.payee.is_empty() {
            e.narration.clone()
        } else {
            e.payee.clone()
        };
        div()
            .flex()
            .flex_col()
            .py(px(8.))
            .when(k + 1 < count, |d| d.border_b_1().border_color(t.line))
            .when(flagged, |d| d.bg(t.hl).mx(px(-8.)).px(px(8.)))
            .child(row(
                line([
                    run(format!("{:02}-{:02}", e.date.1, e.date.2), t.cyan),
                    run(" ", t.ink),
                    bold(
                        e.flag.to_string(),
                        if flagged { t.yellow } else { t.green },
                    ),
                    run(format!(" {who}"), t.ink),
                ]),
                text(
                    signed(e.amount),
                    if e.amount > Decimal::ZERO {
                        t.green
                    } else {
                        t.ink
                    },
                ),
            ))
            .child(
                row(e.other.clone(), e.balance.map(money).unwrap_or_default())
                    .text_size(px(11.))
                    .text_color(t.mut_),
            )
    }))
}

fn reports(root: &mut Root, t: &Theme) -> AnyElement {
    let Some(r) = root.reports() else {
        return div().into_any_element();
    };
    let fire = &r.view.fire;
    let progress = fire.progress.unwrap_or_default();
    let p = f(progress).clamp(0., 1.);
    let lean = fire
        .lean_number
        .filter(|_| fire.fire_number > Decimal::ZERO)
        .map(|l| f(l / fire.fire_number));
    let scenarios = r.scenarios();
    let middle = scenarios.len() / 2;
    let independence = pbox(t, "independence")
        .child(row(
            text(format!("25 × {}", money(fire.annual_spend)), t.dim),
            text("4% rule", t.dim),
        ))
        .child(huge(t, &whole(fire.fire_number), 46., t.ink, 8., 14., 1.1))
        .child(
            div()
                .relative()
                .child(pbar(
                    t,
                    vec![
                        (p, Fill::Solid(t.blue)),
                        (1. - p, Fill::Hatch(t.line2)),
                    ],
                    12.,
                ))
                .children(lean.map(|l| {
                    div()
                        .absolute()
                        .left(relative(l))
                        .top(px(-4.))
                        .bottom(px(-4.))
                        .w(px(2.))
                        .bg(t.ink2)
                })),
        )
        .child(
            row(
                line([
                    run(format!("{} ", whole(fire.net_worth)), t.mut_),
                    run(percent(progress, 1), t.blue),
                ]),
                fire.lean_number
                    .map(|n| {
                        format!("lean {}k", whole(n / Decimal::from(1000)))
                    })
                    .unwrap_or_default(),
            )
            .mt(px(4.))
            .text_size(px(11.))
            .text_color(t.mut_),
        )
        .child(dash(t, 22., 16.).children(scenarios.iter().enumerate().map(
            |(i, s)| {
                let mid = i == middle;
                row(
                    text(
                        format!("{}%", s.rate_pct),
                        if mid { t.ink } else { t.dim },
                    ),
                    line([
                        run(format!("{:<6}", s.reach), t.ink).weight(if mid {
                            FontWeight::BOLD
                        } else {
                            FontWeight::NORMAL
                        }),
                        run(
                            format!(
                                " {}",
                                s.around
                                    .map(|m| m.to_string())
                                    .unwrap_or_default()
                            ),
                            t.cyan,
                        ),
                    ]),
                )
            },
        )));

    let m = r.moved_recent(12);
    let first = r.view.growth.window.map(|w| short(w.0)).unwrap_or("");
    let last = m.months.last().map(|p| short(p.0)).unwrap_or("");
    let moved = pbox(t, "net worth moved")
        .child(row(
            text(format!("  opening · {first}"), t.dim),
            money(m.opening),
        ))
        .child(row(
            line([run("+", t.blue), run(" saved", t.dim)]),
            text(money(m.saved), t.blue),
        ))
        .child(row(
            line([run("+", t.purple), run(" markets", t.dim)]),
            text(money(m.markets), t.purple),
        ))
        .child(sum_rule(t).child(row(
            format!("= {last}"),
            line([bold(money(m.closing), t.ink)]),
        )))
        .child(
            div()
                .mt(px(16.))
                .child(super::reports::diverging(t, &m.months, 90., 6.)),
        )
        .child(
            div().mt(px(4.)).child(col_labels(
                m.months
                    .iter()
                    .map(|p| SharedString::from(&short(p.0)[..1])),
                t.mut_,
                11.,
                6.,
            )),
        );

    let payees = &r.view.payees;
    let goes = pbox(t, "where the money goes")
        .children(payees.items.iter().take(5).map(|i| {
            let sh = f(i.share).clamp(0., 1.);
            div()
                .flex()
                .flex_col()
                .gap(px(4.))
                .py(px(6.))
                .child(row(
                    i.name.clone(),
                    line([
                        run(whole(i.spent), t.ink),
                        run(format!(" {}", share(i.share)), t.mut_),
                    ]),
                ))
                .child(pbar(
                    t,
                    vec![
                        (sh, Fill::Solid(t.blue)),
                        (1. - sh, Fill::Hatch(t.line2)),
                    ],
                    12.,
                ))
        }))
        .child(
            sum_rule(t).child(row(
                format!("= {} payees", payees.items.len()),
                div()
                    .font_weight(FontWeight::BOLD)
                    .child(money(payees.total)),
            )),
        );

    let (flagged, uncategorized) = r.doubt();
    let rests = pbox(t, "rests on")
        .child(row(
            line([bold("!", t.yellow), run(" unconfirmed", t.ink)]),
            money(flagged),
        ))
        .child(row(
            line([bold("?", t.yellow), run(" uncategorized", t.ink)]),
            money(uncategorized),
        ));

    body(24., 32.)
        .child(independence)
        .child(moved)
        .child(goes)
        .child(rests)
        .into_any_element()
}

fn investments(
    root: &mut Root,
    t: &Theme,
    cx: &mut Context<Root>,
) -> AnyElement {
    let Some(i) = root.investments() else {
        return div().into_any_element();
    };
    let portfolio = pbox(t, "portfolio")
        .child(huge(t, &whole(i.value), 56., t.purple, 4., 16., 1.05))
        .children(i.positions.iter().map(|p| {
            row(
                text(
                    format!("  {} × {}", units(p.units), money(p.price)),
                    t.dim,
                ),
                money(p.value),
            )
        }))
        .child(row(text("− cost", t.dim), money(-i.basis)))
        .child(
            sum_rule(t)
                .child(row("= gain", line([bold(signed(i.gain), t.green)]))),
        );

    let p = &i.performance;
    // Every point keeps its place, priced or not.
    let values: Vec<Option<Decimal>> =
        p.points.iter().map(|x| x.value).collect();
    let min = values.iter().flatten().copied().min().unwrap_or_default();
    let max = values.iter().flatten().copied().max().unwrap_or_default();
    let floor = f(min) * 0.9;
    let top = (f(max) - floor).max(1.);
    let last = values.len().saturating_sub(1);
    let ranges = [
        (Range::Months(1), "1M"),
        (Range::Months(6), "6M"),
        (Range::Ytd, "YTD"),
        (Range::All, "ALL"),
    ];
    let selector = div()
        .flex()
        .flex_row()
        .border_1()
        .border_color(t.line2)
        .mb(px(16.))
        .children(ranges.into_iter().enumerate().map(|(k, (range, label))| {
            let on = range == i.range;
            div()
                .id(SharedString::from(format!("p-range-{label}")))
                .flex_1()
                .flex()
                .justify_center()
                .py(px(5.))
                .when(k > 0, |d| d.border_l_1().border_color(t.line2))
                .when(on, |d| d.bg(t.sel))
                .text_color(if on { t.ink } else { t.dim })
                .on_click(
                    cx.listener(move |r, _, _, cx| r.set_range(range, cx)),
                )
                .child(label)
        }));
    let performance = pbox(t, "performance")
        .child(selector)
        .child(cols(
            values
                .iter()
                .enumerate()
                .map(|(k, v)| match v {
                    Some(v) => (
                        ((f(*v) - floor) / top).max(0.01),
                        Fill::Solid(if k == last { t.purple } else { t.line2 }),
                    ),
                    None => (0., Fill::Solid(t.line2)),
                })
                .collect(),
            90.,
            6.,
        ))
        .child(div().mt(px(4.)).child(col_labels(
            {
                let days: Vec<_> = p.points.iter().map(|x| x.date).collect();
                crate::model::investments::month_labels(&days)
                    .into_iter()
                    .enumerate()
                    .map(move |(k, l)| {
                        // The phone has room for an initial, and a dot
                        // for today.
                        if k == last {
                            SharedString::from("·")
                        } else {
                            SharedString::from(
                                l.chars()
                                    .next()
                                    .map(String::from)
                                    .unwrap_or_default(),
                            )
                        }
                    })
            },
            t.mut_,
            11.,
            6.,
        )))
        .child(
            dash(t, 22., 16.)
                .child(row(
                    text("  opening", t.dim),
                    p.opening.map(money).unwrap_or_default(),
                ))
                .child(row(
                    text("+ flows", t.dim),
                    p.net_flows.map(money).unwrap_or_default(),
                ))
                .child(row(
                    text("+ gain", t.dim),
                    text(p.gain.map(money).unwrap_or_default(), t.green),
                ))
                .child(sum_rule(t).child(row(
                    "= closing",
                    line([bold(
                        p.closing.map(money).unwrap_or_default(),
                        t.purple,
                    )]),
                ))),
        );
    let holdings = pbox(t, "holdings").children(i.positions.iter().map(|h| {
        let s = f(h.share).clamp(0., 1.);
        div()
            .flex()
            .flex_col()
            .gap(px(6.))
            .child(row(
                line([
                    bold(h.currency.clone(), t.purple),
                    run(format!(" × {}", units(h.units)), t.ink),
                ]),
                money(h.value),
            ))
            .child(pbar(
                t,
                vec![
                    (s, Fill::Solid(t.purple)),
                    (1. - s, Fill::Hatch(t.line2)),
                ],
                12.,
            ))
            .child(
                row(
                    format!(
                        "{}{}",
                        money(h.price),
                        h.price_date
                            .map(|d| format!(" · {:02}-{:02}", d.1, d.2))
                            .unwrap_or_default()
                    ),
                    text(h.gain.map(signed).unwrap_or_default(), t.green),
                )
                .text_size(px(11.))
                .text_color(t.mut_),
            )
    }));
    body(24., 32.)
        .child(portfolio)
        .child(performance)
        .child(holdings)
        .into_any_element()
}

fn liabilities(
    root: &mut Root,
    t: &Theme,
    cx: &mut Context<Root>,
) -> AnyElement {
    let Some(l) = root.liabilities() else {
        return div().into_any_element();
    };
    let (on_loans, on_cards) = l.owed_parts();
    let paid: Decimal = l
        .open_debts()
        .iter()
        .filter(|d| d.kind == bean_core::reports::DebtKind::Installment)
        .map(|d| d.principal_paid.min(d.peak))
        .sum();
    let owed = pbox(t, "owed")
        .child(huge(t, &whole(l.view.owed), 56., t.red, 4., 16., 1.05))
        .child(pbar(
            t,
            vec![
                (f(paid), Fill::Solid(t.blue)),
                (f(l.view.owed), Fill::Hatch(t.red)),
            ],
            12.,
        ))
        .child(
            div()
                .mt(px(14.))
                .child(row(text("  on loans", t.dim), money(on_loans)))
                .child(row(text("+ on cards", t.dim), money(on_cards)))
                .child(sum_rule(t).child(row(
                    "= owed",
                    line([bold(money(l.view.owed), t.red)]),
                ))),
        )
        .children(l.view.debt_free.map(|m| {
            row(text("debt-free", t.dim), text(m.to_string(), t.cyan))
                .mt(px(14.))
        }))
        .children(l.covered().map(|c| {
            row(
                text("covered by free cash", t.dim),
                text(format!("{}×", fixed(c, 1)), t.green),
            )
        }));

    let mut out = body(24., 32.).child(owed);
    for (k, d) in l.open_debts().into_iter().enumerate() {
        let extra = root.extra.get(&d.account).copied().unwrap_or_default();
        let projection = l.projection(d, extra);
        let recent = l.history(d, 24);
        let top = recent
            .iter()
            .filter_map(|p| p.owed)
            .chain([d.owed])
            .map(f)
            .fold(1., f32::max);
        let items: Vec<(f32, Fill)> = recent
            .iter()
            .map(|p| (f(p.owed.unwrap_or_default()) / top, Fill::Solid(t.red)))
            .chain(projection.iter().map(|v| {
                ((f(*v) / top).max(0.006), Fill::Stripes(t.red, 1.5, 4.))
            }))
            .collect();
        let payoff = l.payoff(d, extra);
        let first = recent
            .first()
            .map(|p| format!("{:02}-{:02}", p.month.month, p.month.year % 100));
        let label = d
            .account
            .split_once(':')
            .map_or(d.account.as_str(), |(_, rest)| rest)
            .to_owned();
        let account = d.account.clone();
        let slider = super::liabilities::slider_for(
            t,
            100 + k,
            f(extra) / super::liabilities::EXTRA_MAX as f32,
            cx,
            account,
        );
        out = out.child(
            pbox(t, label)
                .child(row(
                    div().font_weight(FontWeight::BOLD).child(d.label.clone()),
                    text(
                        d.rate.map(|r| percent(r, 2)).unwrap_or_default(),
                        t.yellow,
                    ),
                ))
                .child(div().mt(px(16.)).child(cols(items, 80., 1.)))
                .child(
                    row(
                        first.unwrap_or_default(),
                        payoff
                            .map(|(m, _)| {
                                format!("{:02}-{:02}", m.month, m.year % 100)
                            })
                            .unwrap_or_default(),
                    )
                    .relative()
                    .mt(px(4.))
                    .text_size(px(11.))
                    .text_color(t.mut_)
                    .child(
                        div()
                            .absolute()
                            .left_0()
                            .right_0()
                            .flex()
                            .justify_center()
                            .text_color(t.blue)
                            .child("today"),
                    ),
                )
                .child(
                    dash(t, 22., 16.)
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(10.))
                        .child(text("extra", t.dim))
                        .child(div().flex_1().child(slider))
                        .child(format!("+{}", whole(extra))),
                )
                .child(
                    row(
                        text("→ payoff", t.dim),
                        text(
                            payoff.map(|p| p.0.to_string()).unwrap_or_default(),
                            t.cyan,
                        ),
                    )
                    .mt(px(6.)),
                )
                .child(
                    dash(t, 22., 16.).flex().flex_col().gap(px(4.)).children(
                        d.payments.iter().take(4).map(|p| {
                            div()
                                .flex()
                                .flex_row()
                                .items_center()
                                .gap(px(10.))
                                .child(
                                    div()
                                        .w(px(6. * 7.8))
                                        .flex_none()
                                        .text_color(t.cyan)
                                        .child(format!(
                                            "{:02}-{:02}",
                                            p.date.1, p.date.2
                                        )),
                                )
                                .child(div().flex_1().child(pbar(
                                    t,
                                    vec![
                                        (f(p.principal), Fill::Solid(t.blue)),
                                        (f(p.interest), Fill::Solid(t.red)),
                                    ],
                                    12.,
                                )))
                                .child(
                                    div()
                                        .w(px(7. * 7.8))
                                        .flex_none()
                                        .flex()
                                        .justify_end()
                                        .child(money(p.total)),
                                )
                        }),
                    ),
                ),
        );
    }
    out.into_any_element()
}

/// The search screen takes the whole phone.
pub fn search(
    query: &str,
    selected: usize,
    hits: &[Hit],
    t: &Theme,
    cx: &mut Context<Root>,
) -> AnyElement {
    let terms: Vec<String> = query
        .split_whitespace()
        .map(|s| s.trim_start_matches(['#', '^']).to_lowercase())
        .collect();
    let total: Decimal = hits.iter().map(|h| h.amount).sum();
    let selected = selected.min(hits.len().saturating_sub(1));
    div()
        .size_full()
        .flex()
        .flex_col()
        .bg(t.bg)
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(10.))
                .h(px(56.))
                .px(px(16.))
                .bg(t.bar)
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
                        .pl(px(2.))
                        .text_size(px(16.))
                        .child(query.to_owned())
                        .child(div().w(px(2.)).h(px(20.)).bg(t.ink)),
                )
                .child(
                    div()
                        .id("p-search-close")
                        .py(px(6.))
                        .px(px(10.))
                        .border_1()
                        .border_color(t.line2)
                        .text_color(t.dim)
                        .on_click(cx.listener(|r, _, _, cx| r.close_search(cx)))
                        .child("close"),
                ),
        )
        .child(
            div()
                .flex()
                .flex_row()
                .gap(px(8.))
                .py(px(10.))
                .px(px(16.))
                .overflow_hidden()
                .text_size(px(12.))
                .text_color(t.mut_)
                .border_b_1()
                .border_color(t.line)
                .children(
                    ["payee:", "account:", "#tag", "^link", "2026-08"].map(
                        |c| {
                            div()
                                .flex_none()
                                .py(px(2.))
                                .px(px(8.))
                                .bg(t.hl)
                                .child(c)
                        },
                    ),
                ),
        )
        .child(
            row(format!("{} txns", hits.len()), money(total))
                .py(px(10.))
                .px(px(16.))
                .text_color(t.dim),
        )
        .child(div().flex().flex_col().children(
            hits.iter().take(50).enumerate().map(|(k, h)| {
                let on = k == selected;
                let who = if h.payee.is_empty() {
                    &h.narration
                } else {
                    &h.payee
                };
                div()
                    .id(("p-hit", k))
                    .relative()
                    .flex()
                    .flex_col()
                    .py(px(8.))
                    .px(px(16.))
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
                    .on_click(cx.listener(move |r, _, _, cx| r.pick_hit(k, cx)))
                    .child(row(
                        div()
                            .flex()
                            .flex_row()
                            .child(text(
                                format!("{:02}-{:02} ", h.date.1, h.date.2),
                                t.cyan,
                            ))
                            .child(text(format!("{} ", h.flag), t.green))
                            .child(marked_at(
                                who, &terms, t.ink, t, SIZE, LINE,
                            )),
                        text(signed(h.amount), t.ink),
                    ))
                    .child(div().text_size(px(11.)).child(marked_at(
                        &h.category,
                        &terms,
                        t.mut_,
                        t,
                        11.,
                        LINE,
                    )))
            }),
        ))
        .into_any_element()
}
