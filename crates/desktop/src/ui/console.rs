//! `Query.dc.html`: the SQL console. The schema, saved queries and the
//! runs so far down the left; the editor and its answer on the right.

use bean_sql::{Answer, Cell, Failure};
use gpui::{
    AnyElement, Context, Div, FontWeight, Hsla, InteractiveElement,
    IntoElement, MouseButton, MouseDownEvent, ParentElement, SharedString,
    StatefulInteractiveElement, Styled, canvas, div, prelude::*, px,
};

use super::{
    budget::wrap,
    kit::{Fill, boxed_right, dash, slim, sum_rule},
};
use crate::{
    model::console::{Layout, Tok, highlight, layout, suggestion, write_cell},
    root::{Field, Root},
    theme::Theme,
};

/// A glyph's advance at 14px, 13px and 12px.
const CH: f32 = 8.4;
const CH_PHONE: f32 = 7.2;
/// The rail's width: 34 characters.
const RAIL: f32 = 34. * CH;
/// At most this many rows of an answer are drawn.
const DRAWN: usize = 500;

fn tok_color(t: &Theme, tok: Tok) -> Hsla {
    match tok {
        Tok::Keyword => t.purple,
        Tok::Function => t.blue,
        Tok::Relation => t.teal,
        Tok::Str => t.green,
        Tok::Number => t.orange,
        Tok::Op => t.cyan,
        Tok::Punct => t.dim,
        Tok::Comment => t.mut_,
        Tok::Space | Tok::Ident => t.ink,
    }
}

/// `DECIMAL(38,18)` → `dec`: the type the way the rail writes it.
fn short_type(ty: &str) -> String {
    let base = ty.split('(').next().unwrap_or(ty);
    match base {
        "DECIMAL" => "dec".into(),
        "VARCHAR" => "text".into(),
        "INTEGER" | "BIGINT" | "SMALLINT" | "TINYINT" | "HUGEINT" => {
            "int".into()
        }
        "DOUBLE" | "FLOAT" => "float".into(),
        other => other.to_lowercase(),
    }
}

/// A row that runs to the box's edges, lit when chosen.
fn rail_row(t: &Theme, id: SharedString, on: bool) -> gpui::Stateful<Div> {
    div()
        .id(id)
        .relative()
        .flex()
        .flex_row()
        .justify_between()
        .gap(px(12.))
        .mx(px(-25.))
        .px(px(25.))
        .py(px(2.))
        .whitespace_nowrap()
        .cursor_pointer()
        .when(on, |d| {
            d.bg(t.sel).text_color(t.ink).child(
                div()
                    .absolute()
                    .left_0()
                    .top_0()
                    .bottom_0()
                    .w(px(2.))
                    .bg(t.blue),
            )
        })
        .when(!on, |d| d.hover(|s| s.bg(t.hl)))
}

fn tables(root: &Root, t: &Theme, cx: &mut Context<Root>) -> Div {
    let c = &root.console;
    let n_tables = c.schema.iter().filter(|r| !r.view).count();
    let n_views = c.schema.len() - n_tables;
    let mut b = boxed_right(t, "tables", format!("{n_tables} + {n_views}"));
    for r in c.schema.iter().filter(|r| !r.view) {
        let open = c.open_table.as_deref() == Some(r.name.as_str());
        let name = r.name.clone();
        b = b.child(
            rail_row(t, format!("table-{}", r.name).into(), false)
                .on_click(cx.listener(move |root, _, _, cx| {
                    root.toggle_table(&name, cx)
                }))
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .child(div().text_color(t.mut_).child(if open {
                            "▾ "
                        } else {
                            "▸ "
                        }))
                        .child(div().text_color(t.teal).child(r.name.clone())),
                )
                .child(
                    div()
                        .text_color(t.dim)
                        .child(crate::fmt::whole(r.rows.unwrap_or(0).into())),
                ),
        );
        if open {
            for col in &r.columns {
                let name = col.name.clone();
                b = b.child(
                    rail_row(
                        t,
                        format!("column-{}-{}", r.name, col.name).into(),
                        false,
                    )
                    .text_size(px(13.))
                    .on_click(cx.listener(move |root, _, _, cx| {
                        root.insert_name(&name, cx)
                    }))
                    .child(format!("    {}", col.name))
                    .child(div().text_color(t.mut_).child(short_type(&col.ty))),
                );
            }
        }
    }
    let mut views = dash(t, 14., 10.);
    for r in c.schema.iter().filter(|r| r.view) {
        let name = r.name.clone();
        views =
            views.child(
                rail_row(t, format!("view-{}", r.name).into(), false)
                    .on_click(cx.listener(move |root, _, _, cx| {
                        root.insert_name(&name, cx)
                    }))
                    .child(
                        div().flex().flex_row().child("  ").child(
                            div().text_color(t.teal).child(r.name.clone()),
                        ),
                    )
                    .child(div().text_color(t.mut_).child("view")),
            );
    }
    b.child(views)
}

fn saved(root: &Root, t: &Theme, cx: &mut Context<Root>) -> Option<Div> {
    let data = root.data.as_ref()?;
    let queries = &data.ledger.queries;
    if queries.is_empty() {
        return None;
    }
    let file = queries[0]
        .source
        .as_ref()
        .and_then(|s| s.path.file_name())
        .map(|f| f.to_string_lossy().into_owned())
        .unwrap_or_default();
    let mut b = boxed_right(t, "saved", file);
    for (k, q) in queries.iter().enumerate() {
        let on = root.console.origin.as_deref() == Some(q.name.as_str());
        b = b.child(
            rail_row(t, ("saved", k).into_element_id(), on)
                .on_click(cx.listener(move |r, _, _, cx| r.open_saved(k, cx)))
                .child(div().overflow_hidden().child(q.name.clone()))
                .child(
                    div().flex_none().text_color(t.mut_).child(
                        q.source
                            .as_ref()
                            .map(|s| format!(":{}", s.line))
                            .unwrap_or_default(),
                    ),
                ),
        );
    }
    Some(b)
}

trait IntoId {
    fn into_element_id(self) -> SharedString;
}

impl IntoId for (&str, usize) {
    fn into_element_id(self) -> SharedString {
        format!("{}-{}", self.0, self.1).into()
    }
}

fn history(root: &Root, t: &Theme, cx: &mut Context<Root>) -> Option<Div> {
    let runs = &root.console.history;
    if runs.is_empty() {
        return None;
    }
    let mut b = boxed_right(t, "history", "today");
    for (k, run) in runs.iter().take(12).enumerate() {
        let (mark, mark_color, count, count_color) = match &run.outcome {
            Ok(n) => ("✓", t.green, n.to_string(), t.dim),
            Err(_) => ("✕", t.red, "err".to_string(), t.red),
        };
        b = b.child(
            rail_row(t, ("run", k).into_element_id(), k == 0)
                .on_click(cx.listener(move |r, _, _, cx| r.open_run(k, cx)))
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .min_w(px(0.))
                        .overflow_hidden()
                        .child(div().text_color(t.cyan).child(run.at.clone()))
                        .child(" ")
                        .child(div().text_color(mark_color).child(mark))
                        .child(" ")
                        .child(div().child(run.name.clone())),
                )
                .child(div().flex_none().text_color(count_color).child(count)),
        );
    }
    Some(b)
}

/// The underlined word in a query DuckDB could not find.
fn wrong_word(root: &Root) -> Option<String> {
    match &root.console.result {
        Some(Err(f)) => suggestion(f).map(|(wrong, _)| wrong),
        _ => None,
    }
}

/// The editor's lines: a gutter of line numbers, the SQL in colour, and
/// the cursor as a block while the editor has the keys.
fn code(
    root: &Root,
    t: &Theme,
    ch: f32,
    line_h: f32,
    gutter: usize,
    cx: &mut Context<Root>,
) -> gpui::Stateful<Div> {
    let e = &root.console.editor;
    let (crow, ccol) = e.cursor();
    let focused = root.typing == Some(Field::Sql);
    let wrong = wrong_word(root);
    let origin = root.code_origin.clone();
    let gap = if gutter > 2 { 16. } else { 10. };
    let mut lines = div()
        .id("code")
        .relative()
        .flex()
        .flex_col()
        .cursor_text()
        .child(
            canvas(
                move |bounds, _, _| {
                    origin.set((bounds.origin.x.into(), bounds.origin.y.into()))
                },
                |_, _, _, _| {},
            )
            .absolute()
            .inset_0(),
        )
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |r, ev: &MouseDownEvent, _, cx| {
                let (ox, oy) = r.code_origin.get();
                let x =
                    f32::from(ev.position.x) - ox - gutter as f32 * ch - gap;
                let y = f32::from(ev.position.y) - oy;
                let row = (y / line_h).floor().max(0.) as usize;
                let col = (x / ch).round().max(0.) as usize;
                r.click_code(row, col, cx);
            }),
        );
    for (i, text) in e.lines().iter().enumerate() {
        let current = i == crow;
        let mut body = div().flex().flex_row().flex_1().min_w(px(0.));
        let mut at = 0usize;
        for (piece, tok) in highlight(text) {
            let n = piece.chars().count();
            let color = tok_color(t, tok);
            let wavy = wrong.as_deref() == Some(piece.as_str());
            let cursor_here = focused && current && ccol >= at && ccol < at + n;
            if cursor_here {
                let k = ccol - at;
                let before: String = piece.chars().take(k).collect();
                let under: String = piece.chars().skip(k).take(1).collect();
                let after: String = piece.chars().skip(k + 1).collect();
                body = body
                    .child(div().text_color(color).child(before))
                    .child(div().bg(t.blue).text_color(t.bg).child(under))
                    .child(div().text_color(color).child(after));
            } else {
                body = body.child(
                    div()
                        .text_color(color)
                        .when(wavy, |d| {
                            d.text_decoration_1()
                                .text_decoration_wavy()
                                .text_decoration_color(t.red)
                        })
                        .child(piece),
                );
            }
            at += n;
        }
        if focused && current && ccol >= at {
            body = body.child(
                div().w(px(ch)).h(px(line_h - 4.)).mt(px(2.)).bg(t.blue),
            );
        }
        lines = lines.child(
            div()
                .flex()
                .flex_row()
                .gap(px(gap))
                .whitespace_nowrap()
                .child(
                    div()
                        .w(px(gutter as f32 * ch))
                        .flex_none()
                        .flex()
                        .justify_end()
                        .text_color(t.mut_)
                        .when(current, |d| d.bg(t.hl))
                        .child((i + 1).to_string()),
                )
                .child(body.when(current, |d| d.bg(t.hl))),
        );
    }
    lines
}

fn button(t: &Theme, id: &'static str, primary: bool) -> gpui::Stateful<Div> {
    div()
        .id(id)
        .flex()
        .flex_row()
        .items_center()
        .gap(px(10.))
        .h(px(30.))
        .px(px(12.))
        .border_1()
        .cursor_pointer()
        .when(primary, |d| {
            d.bg(t.blue)
                .border_color(t.blue)
                .text_color(t.on_block)
                .font_weight(FontWeight::BOLD)
        })
        .when(!primary, |d| {
            d.border_color(t.line2)
                .text_color(t.ink2)
                .hover(|s| s.bg(t.hl))
        })
}

fn key(text: &'static str, color: Hsla) -> Div {
    div()
        .text_color(color)
        .font_weight(FontWeight::NORMAL)
        .child(text)
}

fn title(root: &Root, t: &Theme) -> Div {
    let (name, edited) = root.console.title();
    let scratch = root.console.origin.is_none();
    let color = if edited || scratch { t.yellow } else { t.ink };
    div()
        .flex()
        .flex_row()
        .child(div().text_color(t.dim).child("query · "))
        .child(div().text_color(color).child(name))
        .when(edited, |d| d.child(div().text_color(t.yellow).child(" ●")))
}

fn editor(root: &Root, t: &Theme, cx: &mut Context<Root>) -> Div {
    let c = &root.console;
    let status = if c.running {
        div().text_color(t.dim).child("running…")
    } else {
        match (&c.result, c.history.first()) {
            (Some(Ok(_)), Some(run)) => div()
                .flex()
                .flex_row()
                .text_color(t.dim)
                .child(div().text_color(t.green).child("✓"))
                .child(format!(" ran {}", run.at)),
            (Some(Err(f)), _) => {
                div().text_color(t.red).child(if f.kind == "Error" {
                    "✕ error".to_string()
                } else {
                    format!("✕ {} error", f.kind.to_lowercase())
                })
            }
            _ => div(),
        }
    };
    let run = if c.running {
        button(t, "stop", false)
            .on_click(cx.listener(|r, _, _, _| r.cancel_query()))
            .child("■ stop")
            .child(key("esc", t.mut_))
    } else {
        button(t, "run", true)
            .on_click(cx.listener(|r, _, _, cx| r.run_query(cx)))
            .child("▶ run")
            .child(key("⌃enter", t.on_block.opacity(0.65)))
    };
    boxed_right(t, "", "duckdb · memory")
        .child(
            // The legend, drawn here rather than by `boxed` so the name
            // can take its own colour.
            div()
                .absolute()
                .top(px(-10.))
                .left(px(17.))
                .px(px(8.))
                .bg(t.bg)
                .child(title(root, t)),
        )
        .child(code(root, t, CH, 22., 3, cx))
        .child(
            dash(t, 22., 16.)
                .flex()
                .flex_row()
                .items_center()
                .gap(px(10.))
                .child(run)
                .child(div().flex_1())
                .child(status),
        )
}

/// How wide each column of an answer is drawn, in characters; the
/// widest text column takes what is left.
fn widths(a: &Answer, l: &Layout) -> (Vec<f32>, Option<usize>) {
    let mut w: Vec<f32> = a
        .columns
        .iter()
        .map(|c| (c.name.chars().count() + 1 + short_type(&c.ty).len()) as f32)
        .collect();
    for row in a.rows.iter().take(DRAWN) {
        for (i, cell) in row.iter().enumerate() {
            let n = write_cell(cell).chars().count() as f32
                + if matches!(cell, Cell::Text(_)) {
                    2.
                } else {
                    0.
                };
            w[i] = w[i].max(n);
        }
    }
    let flexible = (0..w.len())
        .filter(|i| !l.numeric[*i])
        .filter(|i| a.columns[*i].ty == "VARCHAR")
        .max_by(|a, b| w[*a].total_cmp(&w[*b]));
    (w.into_iter().map(|n| n.min(40.)).collect(), flexible)
}

fn looks_like_account(s: &str) -> bool {
    s.contains(':') && s.chars().next().is_some_and(char::is_uppercase)
}

/// One answer cell, coloured by what it holds.
fn cell_view(t: &Theme, column: &str, c: &Cell, signed: bool) -> Div {
    match c {
        Cell::Null => div().text_color(t.mut_).child("NULL"),
        Cell::Date(_) => div().text_color(t.cyan).child(write_cell(c)),
        Cell::Text(s) if column == "payee" || column == "narration" => div()
            .flex()
            .flex_row()
            .child(div().text_color(t.mut_).child("\""))
            .child(s.clone())
            .child(div().text_color(t.mut_).child("\"")),
        Cell::Text(s) if looks_like_account(s) => {
            div().text_color(t.dim).child(s.clone())
        }
        Cell::Decimal(d) if signed && d.is_sign_positive() && !d.is_zero() => {
            div()
                .text_color(t.green)
                .child(format!("+{}", write_cell(c)))
        }
        _ => div().child(write_cell(c)),
    }
}

/// Whether a figure column holds both gains and losses, and so wants its
/// gains marked with a plus.
fn mixed(a: &Answer, i: usize) -> bool {
    let has = |neg: bool| {
        a.rows.iter().any(|r| match r.get(i) {
            Some(Cell::Decimal(d)) => {
                !d.is_zero() && d.is_sign_negative() == neg
            }
            _ => false,
        })
    };
    has(true) && has(false)
}

fn answer(a: &Answer, t: &Theme, cx: &mut Context<Root>) -> Div {
    let l = layout(a);
    let (w, flexible) = widths(a, &l);
    let signed: Vec<bool> = (0..a.columns.len()).map(|i| mixed(a, i)).collect();
    let col = |i: usize, d: Div| -> Div {
        let d = if Some(i) == flexible {
            d.flex_1().min_w(px(0.)).overflow_hidden()
        } else {
            d.w(px(w[i] * CH)).flex_none().overflow_hidden()
        };
        if l.numeric[i] {
            d.flex().justify_end()
        } else {
            d
        }
    };
    let bar_cell = |d: Div| d.w(px(16. * CH)).flex_shrink_1().min_w(px(0.));
    let row = || {
        div()
            .flex()
            .flex_row()
            .gap(px(16.))
            .items_center()
            .whitespace_nowrap()
    };

    let mut head = row()
        .text_color(t.ink2)
        .pb(px(6.))
        .mb(px(4.))
        .border_b_1()
        .border_color(t.line2);
    for (i, c) in a.columns.iter().enumerate() {
        head = head.child(col(
            i,
            div().child(
                div()
                    .flex()
                    .flex_row()
                    .items_baseline()
                    .child(c.name.clone())
                    .child(" ")
                    .child(
                        div()
                            .text_size(px(12.))
                            .text_color(t.mut_)
                            .child(short_type(&c.ty)),
                    ),
            ),
        ));
    }
    if l.bars.is_some() {
        head = head.child(bar_cell(div()));
    }

    let mut body = div().flex().flex_col();
    for (k, r) in a.rows.iter().take(DRAWN).enumerate() {
        let mut line = row().py(px(2.)).hover(|s| s.bg(t.hl));
        for (i, c) in r.iter().enumerate() {
            let ditto = i == 0
                && k > 0
                && matches!(c, Cell::Date(_))
                && a.rows[k - 1].first() == Some(c);
            let v = if ditto {
                div().flex().justify_end().text_color(t.mut_).child("″")
            } else {
                cell_view(t, &a.columns[i].name, c, signed[i])
            };
            line = line.child(col(i, div().child(v)));
        }
        if let Some(share) = l.shares.get(k) {
            line = line.child(bar_cell(div().child(slim(
                t,
                vec![
                    (share.max(0.002), Fill::Solid(t.blue)),
                    ((1. - share).max(0.), Fill::Hatch(t.line2)),
                ],
            ))));
        }
        body = body.child(line);
    }

    let shown = a.rows.len().min(DRAWN);
    let count = if a.total > shown {
        format!("= {shown} of {} rows", a.total)
    } else {
        format!("= {} rows", a.total)
    };
    let mut foot = row();
    let mut first = true;
    for (i, total) in l.totals.iter().enumerate() {
        let d = match total {
            _ if first => {
                first = false;
                div().text_color(t.dim).child(count.clone())
            }
            Some(c @ Cell::Int(_)) => {
                div().text_color(t.dim).child(write_cell(c))
            }
            Some(c) => div().font_weight(FontWeight::BOLD).child(write_cell(c)),
            None => div(),
        };
        foot = foot.child(if i == 0 && flexible != Some(0) {
            d.w(px(w[0] * CH)).flex_none()
        } else {
            col(i, d)
        });
    }
    if l.bars.is_some() {
        foot = foot.child(bar_cell(div()));
    }

    let rows_label = if a.total > shown {
        format!("{shown} of {} rows", a.total)
    } else {
        format!("{} rows", a.total)
    };
    let ms = a.elapsed.as_secs_f64() * 1000.;
    boxed_right(t, "result", format!("{rows_label} · {ms:.1} ms"))
        .child(head)
        .child(body)
        .child(sum_rule(t).child(foot))
        .child(
            div().flex().flex_row().justify_end().mt(px(18.)).child(
                button(t, "copy", false)
                    .on_click(cx.listener(|r, _, _, cx| r.copy_answer(cx)))
                    .child("copy"),
            ),
        )
}

fn failure(f: &Failure, t: &Theme, cx: &mut Context<Root>) -> Div {
    let mut text = div().flex().flex_col().text_color(t.ink2);
    for (i, line) in f.message.lines().enumerate() {
        let d = if i == 0 {
            div()
                .flex()
                .flex_row()
                .whitespace_nowrap()
                .child(div().text_color(t.red).child(if f.kind == "Error" {
                    "Error: ".to_string()
                } else {
                    format!("{} Error: ", f.kind)
                }))
                .child(line.to_owned())
        } else if let Some(rest) = line.strip_prefix("LINE ") {
            let (n, code) = rest.split_once(':').unwrap_or((rest, ""));
            div()
                .flex()
                .flex_row()
                .whitespace_nowrap()
                .child(div().text_color(t.mut_).child(format!("LINE {n}:")))
                .child(code.to_owned())
        } else if line.trim() == "^" {
            div()
                .text_color(t.red)
                .whitespace_nowrap()
                .child(line.to_owned())
        } else if line.is_empty() {
            div().h(px(22.))
        } else {
            div().child(line.to_owned())
        };
        text = text.child(d);
    }
    let fix = suggestion(f).map(|(_, right)| {
        dash(t, 22., 16.).flex().flex_row().gap(px(10.)).child(
            button(t, "fix", true)
                .on_click(cx.listener(|r, _, _, cx| r.apply_suggestion(cx)))
                .child(format!("→ {right}")),
        )
    });
    let mut b = div()
        .relative()
        .min_w(px(0.))
        .pt(px(27.))
        .px(px(25.))
        .pb(px(21.))
        .child(div().absolute().inset_0().border_1().border_color(t.red))
        .child(
            div()
                .absolute()
                .top(px(-10.))
                .left(px(17.))
                .px(px(8.))
                .bg(t.bg)
                .text_color(t.red)
                .child("error"),
        )
        .child(
            div()
                .absolute()
                .top(px(-10.))
                .right(px(17.))
                .px(px(8.))
                .bg(t.bg)
                .text_color(t.mut_)
                .child(f.kind.to_lowercase()),
        )
        .child(text);
    if let Some(fix) = fix {
        b = b.child(fix);
    }
    b
}

fn result(root: &Root, t: &Theme, cx: &mut Context<Root>) -> Option<Div> {
    match &root.console.result {
        Some(Ok(a)) => Some(answer(a, t, cx)),
        Some(Err(f)) => Some(failure(f, t, cx)),
        None if root.console.running => Some(
            boxed_right(t, "result", "running…")
                .child(div().text_color(t.mut_).child("…")),
        ),
        None => None,
    }
}

pub fn page(
    root: &mut Root,
    t: &Theme,
    width: f32,
    cx: &mut Context<Root>,
) -> AnyElement {
    let wide = width >= 1060.;
    let rail = div()
        .flex()
        .flex_col()
        .gap(px(44.))
        .when(wide, |d| d.w(px(RAIL)).flex_none())
        .child(tables(root, t, cx))
        .children(saved(root, t, cx))
        .children(history(root, t, cx));
    let main = div()
        .flex()
        .flex_col()
        .gap(px(44.))
        .flex_1()
        .min_w(px(0.))
        .child(editor(root, t, cx))
        .children(result(root, t, cx));
    let body = if wide {
        div()
            .flex()
            .flex_row()
            .items_start()
            .gap(px(40.))
            .child(rail)
            .child(main)
    } else {
        div().flex().flex_col().gap(px(44.)).child(main).child(rail)
    };
    wrap(width, body)
}

/// The phone: saved queries as chips, then the editor and its answer
/// stacked one figure to a row.
pub fn phone(root: &mut Root, t: &Theme, cx: &mut Context<Root>) -> AnyElement {
    let queries = root
        .data
        .as_ref()
        .map(|d| d.ledger.queries.clone())
        .unwrap_or_default();
    let chips = div()
        .id("chips")
        .flex()
        .flex_row()
        .gap(px(8.))
        .px(px(16.))
        .py(px(12.))
        .overflow_x_scroll()
        .border_b_1()
        .border_color(t.line)
        .children(queries.iter().enumerate().map(|(k, q)| {
            let on = root.console.origin.as_deref() == Some(q.name.as_str());
            div()
                .id(("chip", k).into_element_id())
                .flex()
                .items_center()
                .flex_none()
                .h(px(36.))
                .px(px(12.))
                .border_1()
                .border_color(if on { t.blue } else { t.line2 })
                .when(on, |d| d.bg(t.sel).text_color(t.ink))
                .when(!on, |d| d.text_color(t.dim))
                .on_click(cx.listener(move |r, _, _, cx| r.open_saved(k, cx)))
                .child(q.name.clone())
        }));

    let editor = super::phone::pbox(t, "")
        .child(
            div()
                .absolute()
                .top(px(-10.))
                .left(px(13.))
                .px(px(6.))
                .bg(t.bg)
                .child(title(root, t)),
        )
        .child(
            div()
                .id("code-scroll")
                .overflow_x_scroll()
                .text_size(px(12.))
                .line_height(px(20.))
                .child(code(root, t, CH_PHONE, 20., 2, cx)),
        )
        .child(
            div().flex().flex_row().gap(px(8.)).mt(px(16.)).child(
                button(t, "phone-run", !root.console.running)
                    .flex_1()
                    .h(px(44.))
                    .justify_center()
                    .when(root.console.running, |d| {
                        d.on_click(cx.listener(|r, _, _, _| r.cancel_query()))
                            .child("■ stop")
                    })
                    .when(!root.console.running, |d| {
                        d.on_click(cx.listener(|r, _, _, cx| r.run_query(cx)))
                            .child("▶ run")
                    }),
            ),
        );

    let result = match &root.console.result {
        Some(Ok(a)) => Some(phone_answer(a, t)),
        Some(Err(f)) => {
            Some(failure(f, t, cx).px(px(16.)).pt(px(22.)).pb(px(16.)))
        }
        None => None,
    };
    div()
        .flex()
        .flex_col()
        .child(chips)
        .child(
            div()
                .flex()
                .flex_col()
                .pt(px(24.))
                .px(px(16.))
                .pb(px(32.))
                .gap(px(32.))
                .child(editor)
                .children(result),
        )
        .into_any_element()
}

/// An answer on a phone: the words of each row on the left, its last
/// figure on the right, and under it the bar or the other figures.
fn phone_answer(a: &Answer, t: &Theme) -> Div {
    let l = layout(a);
    let last = (0..a.columns.len()).rev().find(|i| l.numeric[*i]);
    let mut b = super::phone::pbox(t, "result").child(
        div()
            .absolute()
            .top(px(-10.))
            .right(px(13.))
            .px(px(6.))
            .bg(t.bg)
            .text_color(t.mut_)
            .child(format!(
                "{} rows · {:.1} ms",
                a.total,
                a.elapsed.as_secs_f64() * 1000.
            )),
    );
    for (k, r) in a.rows.iter().take(DRAWN).enumerate() {
        let mut words = div()
            .flex()
            .flex_row()
            .gap(px(8.))
            .min_w(px(0.))
            .overflow_hidden();
        let mut rest = Vec::new();
        for (i, c) in r.iter().enumerate() {
            if Some(i) == last {
                continue;
            }
            if l.numeric[i] {
                rest.push(write_cell(c));
                continue;
            }
            let d = match c {
                Cell::Date((_, m, d)) => {
                    div().text_color(t.cyan).child(format!("{m:02}-{d:02}"))
                }
                Cell::Text(s) if looks_like_account(s) => {
                    div().text_color(t.dim).child(
                        s.split_once(':')
                            .map_or(s.as_str(), |x| x.1)
                            .to_owned(),
                    )
                }
                other => div().child(write_cell(other)),
            };
            words = words.child(d);
        }
        let figure = last
            .and_then(|i| r.get(i))
            .map(write_cell)
            .unwrap_or_default();
        let mut item = div().flex().flex_col().gap(px(3.)).py(px(4.)).child(
            div()
                .flex()
                .flex_row()
                .justify_between()
                .gap(px(12.))
                .whitespace_nowrap()
                .child(words)
                .child(div().flex_none().child(figure)),
        );
        if let Some(share) = l.shares.get(k) {
            item = item.child(slim(
                t,
                vec![
                    (share.max(0.002), Fill::Solid(t.blue)),
                    ((1. - share).max(0.), Fill::Hatch(t.line2)),
                ],
            ));
        } else if !rest.is_empty() {
            item = item.child(
                div()
                    .flex()
                    .justify_end()
                    .text_size(px(11.))
                    .text_color(t.mut_)
                    .child(rest.join("  ")),
            );
        }
        b = b.child(item);
    }
    let total = last
        .and_then(|i| l.totals.get(i).cloned().flatten())
        .map(|c| write_cell(&c))
        .unwrap_or_default();
    b.child(
        sum_rule(t)
            .flex()
            .flex_row()
            .justify_between()
            .child(div().text_color(t.dim).child(format!("= {} rows", a.total)))
            .child(div().font_weight(FontWeight::BOLD).child(total)),
    )
}
