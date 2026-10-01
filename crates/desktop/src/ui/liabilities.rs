//! Page 5, `Liabilities.dc.html`.

use std::{cell::Cell, rc::Rc};

use bean_core::{
    model::MonthKey,
    reports::{Debt, DebtKind},
};
use gpui::{
    AnyElement, Bounds, Context, Div, FontWeight, Hsla, InteractiveElement,
    IntoElement, MouseButton, MouseDownEvent, MouseMoveEvent, ParentElement,
    Pixels, SharedString, Styled, canvas, div, prelude::*, px, relative,
};
use rust_decimal::{Decimal, prelude::ToPrimitive};

use super::{
    budget::wrap,
    kit::{
        Fill, bar, bold, boxed, cols, dash, line, row, run, slim, sum_rule,
        tracked,
    },
};
use crate::{
    fmt::{fixed, money, percent, whole},
    model::liabilities::Liabilities,
    root::Root,
    theme::Theme,
};

const CH: f32 = 8.4;
const CH12: f32 = 12. * 0.6;
/// The most the slider adds to a payment, and its step.
pub const EXTRA_MAX: u32 = 2000;
const EXTRA_STEP: u32 = 25;

fn f(v: Decimal) -> f32 {
    v.to_f32().unwrap_or(0.)
}

fn text(s: impl Into<SharedString>, color: Hsla) -> Div {
    div().text_color(color).whitespace_nowrap().child(s.into())
}

fn short(m: MonthKey) -> &'static str {
    super::budget::MONTHS[usize::from(m.month) - 1]
}

pub fn page(
    root: &mut Root,
    t: &Theme,
    width: f32,
    cx: &mut Context<Root>,
) -> AnyElement {
    let Some(l) = root.liabilities() else {
        return div().into_any_element();
    };
    let wide = width >= 1060.;
    let mut grid = div()
        .grid()
        .grid_cols(12)
        .gap_x(px(40.))
        .gap_y(px(44.))
        .items_start()
        .child(owed(&l, t).col_span(if wide { 7 } else { 12 }))
        .child(cover(&l, t).col_span(if wide { 5 } else { 12 }));
    for (k, debt) in l.view.debts.iter().enumerate() {
        let extra = root.extra.get(&debt.account).copied().unwrap_or_default();
        grid = grid.child(loan(k, &l, debt, extra, t, cx).col_span(12));
    }
    wrap(width, grid)
}

fn owed(l: &Liabilities, t: &Theme) -> Div {
    let loans = l
        .view
        .debts
        .iter()
        .filter(|d| d.kind == DebtKind::Installment)
        .count();
    let cards = l.view.debts.len() - loans;
    let peak: Decimal = l.view.debts.iter().map(|d| d.peak).sum();
    let paid: Decimal = l.view.debts.iter().map(|d| d.principal_paid).sum();
    let mut b = boxed(t, "owed")
        .child(row(
            text(
                format!(
                    "{loans} loan{} · {cards} card{}",
                    if loans == 1 { "" } else { "s" },
                    if cards == 1 { "" } else { "s" }
                ),
                t.dim,
            ),
            text("USD", t.dim),
        ))
        .child(div().mt(px(14.)).mb(px(22.)).h(px(92.)).child(tracked(
            &[
                ("$", t.mut_, FontWeight::NORMAL),
                (&whole(l.view.owed), t.red, FontWeight::SEMIBOLD),
            ],
            92.,
            -0.04,
        )))
        .child(bar(
            t,
            vec![
                (f(paid), Fill::Solid(t.blue)),
                (f(l.view.owed), Fill::Hatch(t.red)),
            ],
        ))
        .child(
            div()
                .mt(px(18.))
                .child(row(text("  peak", t.dim), money(peak)))
                .child(row(
                    line([run("−", t.blue), run(" principal paid", t.dim)]),
                    text(money(-paid), t.blue),
                ))
                .child(sum_rule(t).child(row(
                    "= owed",
                    line([bold(money(l.view.owed), t.red)]),
                ))),
        );
    let mut facts = dash(t, 22., 16.).flex().flex_col().gap(px(6.));
    facts = facts.child(row(
        text("costs / mo", t.dim),
        line([run(money(l.cost_month()), t.ink)].into_iter().chain(
            l.view.blended_rate.map(|r| {
                run(
                    format!(
                        " {}/yr @ {}",
                        money(l.view.cost_year),
                        percent(r, 2)
                    ),
                    t.mut_,
                )
            }),
        )),
    ));
    if let Some(free) = l.view.debt_free {
        let months = (i32::from(free.year) - i32::from(l.current.year)) * 12
            + i32::from(free.month)
            - i32::from(l.current.month);
        facts = facts.child(row(
            text("debt-free", t.dim),
            line([
                run(free.to_string(), t.cyan),
                run(format!(" {months} mo at today's payments"), t.mut_),
            ]),
        ));
    }
    b = b.child(facts);
    b
}

fn cover(l: &Liabilities, t: &Theme) -> Div {
    let v = &l.view;
    let window = v
        .window
        .map(|(a, b)| format!(" · {}–{}", short(a), short(b)))
        .unwrap_or_default();
    let next = v.upcoming.first();
    boxed(t, "cover")
        .child(row(
            text("  free cash", t.dim),
            text(money(v.cover.cash), t.blue),
        ))
        .child(row(
            line([run("÷", t.red), run(" owed", t.dim)]),
            money(v.owed),
        ))
        .child(
            sum_rule(t).child(row(
                "= covered",
                line([bold(
                    l.covered()
                        .map(|c| format!("{}×", fixed(c, 1)))
                        .unwrap_or_else(|| "—".into()),
                    t.green,
                )]),
            )),
        )
        .child(
            dash(t, 22., 16.)
                .flex()
                .flex_col()
                .gap(px(6.))
                .child(row(text("spare today", t.dim), money(v.extra.now)))
                .child(row(text("spare / mo", t.dim), money(v.extra.monthly))),
        )
        .child(
            dash(t, 22., 16.)
                .flex()
                .flex_col()
                .gap(px(6.))
                .child(row(
                    text(format!("interest paid{window}"), t.dim),
                    text(money(v.interest_year), t.red),
                ))
                .child(row(
                    text("interest earned", t.dim),
                    text(money(v.earned_year), t.mut_),
                )),
        )
        .child(
            dash(t, 22., 16.)
                .flex()
                .flex_col()
                .gap(px(6.))
                .children(next.map(|u| {
                    row(
                        text("next", t.dim),
                        line([
                            run(
                                format!("{:02}-{:02}", u.date.1, u.date.2),
                                t.cyan,
                            ),
                            run(" \"", t.mut_),
                            run(u.label.clone(), t.ink),
                            run("\" ", t.mut_),
                            run(money(-u.amount), t.ink),
                        ]),
                    )
                }))
                .child(
                    div().flex().child(
                        div()
                            .mt(px(8.))
                            .py(px(6.))
                            .px(px(12.))
                            .border_1()
                            .border_dashed()
                            .border_color(t.line2)
                            .text_color(t.dim)
                            .child("↓ due dates .ics"),
                    ),
                ),
        )
}

fn loan(
    k: usize,
    l: &Liabilities,
    d: &Debt,
    extra: Decimal,
    t: &Theme,
    cx: &mut Context<Root>,
) -> Div {
    let projection = l.projection(d, extra);
    let history: Vec<Decimal> = d
        .history
        .iter()
        .map(|p| p.owed.unwrap_or_default())
        .collect();
    let top = f(d.peak).max(1.);
    let items: Vec<(f32, Fill)> =
        history
            .iter()
            .map(|v| (f(*v) / top, Fill::Solid(t.red)))
            .chain(projection.iter().map(|v| {
                ((f(*v) / top).max(0.006), Fill::Stripes(t.red, 1.5, 4.))
            }))
            .collect();
    let total = items.len().max(1) as f32;
    let today_at = history.len() as f32 / total;
    let first = d.history.first().map(|p| p.month);
    let next_year = MonthKey::new(l.current.year + 2, 1);
    let year_at = |m: MonthKey| {
        let i = (i32::from(m.year) - i32::from(l.current.year)) * 12
            + i32::from(m.month)
            - i32::from(l.current.month);
        (history.len() as f32 + i as f32 - 1.) / total
    };
    let payoff = l.payoff(d, extra);
    let marks = div()
        .relative()
        .h(px(22.))
        .mt(px(6.))
        .text_size(px(12.))
        .text_color(t.mut_)
        .whitespace_nowrap()
        .children(first.map(|m| {
            div()
                .absolute()
                .left_0()
                .child(format!("{m}  {}", whole(d.peak)))
        }))
        .child(
            div()
                .absolute()
                .left(relative(today_at))
                .text_color(t.blue)
                .child(format!("{:02}-01  {}", l.current.month, money(d.owed))),
        )
        .when(year_at(next_year) < 0.9, |m| {
            m.child(
                div()
                    .absolute()
                    .left(relative(year_at(next_year)))
                    .child(next_year.year.to_string()),
            )
        })
        .children(payoff.map(|(when, _)| {
            div()
                .absolute()
                .right_0()
                .text_color(t.cyan)
                .child(format!("{when}  0"))
        }));

    // The canvas lines these 14px pills up with the 22px name on their
    // baselines, which drops them three pixels and the row with them.
    let pill = |s: String, color: Hsla| {
        div()
            .mt(px(3.))
            .px(px(8.))
            .bg(t.hl)
            .text_color(color)
            .child(s)
    };
    let head = div()
        .flex()
        .flex_row()
        .flex_wrap()
        .items_start()
        .gap(px(14.))
        .child(
            div()
                .text_size(px(22.))
                .font_weight(FontWeight::BOLD)
                .child(d.label.clone()),
        )
        .children(d.rate.map(|r| pill(percent(r, 2), t.yellow)))
        .child(pill(
            match (d.kind, d.due_day) {
                (DebtKind::Installment, Some(day)) => {
                    format!("installment · due {day}")
                }
                (DebtKind::Installment, None) => "installment".into(),
                (_, Some(day)) => format!("card · due {day}"),
                _ => "card".into(),
            },
            t.dim,
        ))
        .child(div().flex_1())
        .child(
            line([
                run("paid ", t.dim),
                run(
                    d.progress
                        .map(|p| percent(p, 1))
                        .unwrap_or_else(|| "—".into()),
                    t.blue,
                ),
            ])
            .mt(px(3.)),
        );

    let chart = div()
        .relative()
        .mt(px(24.))
        .child(cols(items, 150., 2.))
        .child(
            div()
                .absolute()
                .left(relative(today_at))
                .ml(px(-1.))
                .top(px(-8.))
                .bottom_0()
                .w(px(2.))
                .bg(t.blue),
        );

    let account = d.account.clone();
    let slider = slider(
        t,
        k,
        f(extra) / EXTRA_MAX as f32,
        cx,
        move |root, share, cx| {
            let steps =
                (share * (EXTRA_MAX / EXTRA_STEP) as f32).round() as u32;
            root.set_extra(
                account.clone(),
                Decimal::from(steps * EXTRA_STEP),
                cx,
            );
        },
    );

    let pay_cols = |ch: f32, cells: [AnyElement; 5]| {
        let [date, bar, principal, interest, total] = cells;
        let right = |w: f32, e: AnyElement| {
            div().w(px(w)).flex_none().flex().justify_end().child(e)
        };
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(14.))
            .child(div().w(px(6. * ch)).flex_none().child(date))
            .child(div().flex_1().min_w(px(0.)).child(bar))
            .child(right(9. * ch, principal))
            .child(right(8. * ch, interest))
            .child(right(9. * ch, total))
    };
    let txt = |s: &str| {
        div()
            .child(SharedString::from(s.to_owned()))
            .into_any_element()
    };
    let pay_head = pay_cols(
        CH12,
        [
            txt("paid"),
            txt(""),
            txt("principal"),
            txt("interest"),
            txt("total"),
        ],
    )
    .text_size(px(12.))
    .text_color(t.mut_)
    .pb(px(6.));
    let payments = d.payments.iter().take(6).map(|p| {
        pay_cols(
            CH,
            [
                text(format!("{:02}-{:02}", p.date.1, p.date.2), t.cyan)
                    .into_any_element(),
                slim(
                    t,
                    vec![
                        (f(p.principal), Fill::Solid(t.blue)),
                        (f(p.interest), Fill::Solid(t.red)),
                    ],
                )
                .into_any_element(),
                text(money(p.principal), t.blue).into_any_element(),
                text(money(p.interest), t.red).into_any_element(),
                text(money(p.total), t.ink).into_any_element(),
            ],
        )
        .py(px(2.))
    });

    boxed(t, d.account.clone())
        .child(head)
        .child(chart)
        .child(marks)
        .child(
            dash(t, 22., 16.)
                .flex()
                .flex_row()
                .items_center()
                .gap(px(16.))
                .child(text("extra / mo", t.dim))
                .child(div().flex_1().child(slider))
                .child(
                    div()
                        .font_weight(FontWeight::BOLD)
                        .child(format!("+{}", money(extra))),
                ),
        )
        .child(
            row(
                text("→ payoff", t.dim),
                line(match payoff {
                    Some((when, interest)) => vec![
                        run(when.to_string(), t.cyan),
                        run("   interest to come ", t.ink),
                        run(money(interest), t.red),
                    ],
                    None => vec![run("never at this payment", t.red)],
                }),
            )
            .mt(px(8.)),
        )
        .when(!d.payments.is_empty(), |b| {
            b.child(dash(t, 22., 16.).child(pay_head).children(payments))
        })
}

/// A range input, drawn the way Chromium draws one in the canvas: a thin
/// track, filled to the thumb, and a round thumb. Drag or click to set.
fn slider(
    t: &Theme,
    k: usize,
    share: f32,
    cx: &mut Context<Root>,
    set: impl Fn(&mut Root, f32, &mut Context<Root>) + Clone + 'static,
) -> impl IntoElement {
    let bounds: Rc<Cell<Option<Bounds<Pixels>>>> = Rc::new(Cell::new(None));
    let share = share.clamp(0., 1.);
    let (track, edge, fill, thumb) =
        (t.range_track, t.range_edge, t.blue, t.blue);
    let at = |b: &Rc<Cell<Option<Bounds<Pixels>>>>, x: Pixels| {
        b.get().map(|b| {
            let w = f32::from(b.size.width).max(1.);
            (f32::from(x - b.origin.x) / w).clamp(0., 1.)
        })
    };
    let (b1, b2, b3) = (bounds.clone(), bounds.clone(), bounds.clone());
    let (s1, s2) = (set.clone(), set);
    div()
        .id(("slider", k))
        .h(px(22.))
        .cursor_pointer()
        .child(
            canvas(
                move |b, _, _| b1.set(Some(b)),
                move |b, _, window, _| {
                    let w = f32::from(b.size.width);
                    let cy = b.origin.y + b.size.height / 2.;
                    let thumb_x = b.origin.x + px(8. + (w - 16.) * share);
                    // An 8px track with a 1px edge, filled up to the thumb.
                    let rail = Bounds::new(
                        gpui::point(b.origin.x, cy - px(4.)),
                        gpui::size(b.size.width, px(8.)),
                    );
                    window.paint_quad(
                        gpui::fill(rail, track)
                            .corner_radii(px(4.))
                            .border_widths(px(1.))
                            .border_color(edge),
                    );
                    window.paint_quad(
                        gpui::fill(
                            Bounds::new(
                                gpui::point(b.origin.x, cy - px(4.)),
                                gpui::size(thumb_x - b.origin.x, px(8.)),
                            ),
                            fill,
                        )
                        .corner_radii(px(4.)),
                    );
                    window.paint_quad(
                        gpui::fill(
                            Bounds::new(
                                gpui::point(thumb_x - px(8.), cy - px(8.)),
                                gpui::size(px(16.), px(16.)),
                            ),
                            thumb,
                        )
                        .corner_radii(px(8.)),
                    );
                },
            )
            .size_full(),
        )
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |root, e: &MouseDownEvent, _, cx| {
                if let Some(s) = at(&b2, e.position.x) {
                    s1(root, s, cx);
                }
            }),
        )
        .on_mouse_move(cx.listener(move |root, e: &MouseMoveEvent, _, cx| {
            if e.pressed_button == Some(MouseButton::Left)
                && let Some(s) = at(&b3, e.position.x)
            {
                s2(root, s, cx);
            }
        }))
}
