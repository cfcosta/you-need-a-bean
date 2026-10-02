//! The SQL console's model: the editor's buffer, how a line of SQL is
//! coloured, and how an answer is laid out for the page.

use bean_desktop::model::console::{
    Console, Editor, Layout, Tok, highlight, layout, write_cell,
};
use bean_sql::{Answer, Cell, Column};
use rust_decimal::Decimal;

fn d(s: &str) -> Decimal {
    s.parse().unwrap()
}

#[test]
fn typing_inserts_at_the_cursor_and_newlines_keep_the_indent() {
    let mut e = Editor::new("");
    e.insert("SELECT a,");
    e.newline();
    e.insert("  b");
    e.newline();
    e.insert("c");
    assert_eq!(e.text(), "SELECT a,\n  b\n  c");
    assert_eq!(e.cursor(), (2, 3));
}

#[test]
fn backspace_joins_lines_and_delete_pulls_the_next_one_up() {
    let mut e = Editor::new("ab\ncd");
    e.move_to(1, 0);
    e.backspace();
    assert_eq!(e.text(), "abcd");
    assert_eq!(e.cursor(), (0, 2));
    e.end();
    e.delete();
    assert_eq!(e.text(), "abcd", "nothing after the end");
    e.home();
    e.delete();
    assert_eq!(e.text(), "bcd");
}

#[test]
fn arrows_move_by_character_and_line_and_clamp_to_short_lines() {
    let mut e = Editor::new("long line\nab\nanother!!");
    e.move_to(0, 8);
    e.down();
    assert_eq!(e.cursor(), (1, 2), "clamped to the short line");
    e.down();
    assert_eq!(e.cursor(), (2, 8), "back to the remembered column");
    e.right();
    e.right();
    assert_eq!(e.cursor(), (2, 9), "the end stops it");
    e.home();
    e.left();
    assert_eq!(e.cursor(), (1, 2), "left from the start goes up a line");
    e.up();
    e.up();
    assert_eq!(e.cursor(), (0, 2));
}

#[test]
fn a_pasted_block_lands_as_lines() {
    let mut e = Editor::new("X");
    e.move_to(0, 0);
    e.insert("SELECT 1\nFROM t\n");
    assert_eq!(e.text(), "SELECT 1\nFROM t\nX");
    assert_eq!(e.cursor(), (2, 0));
}

#[test]
fn multibyte_text_moves_by_character() {
    let mut e = Editor::new("'café'");
    e.end();
    e.left();
    e.backspace();
    assert_eq!(e.text(), "'caf'");
}

#[test]
fn a_line_is_coloured_by_what_each_word_is() {
    let toks: Vec<(String, Tok)> = highlight(
        "SELECT sum(value) AS spent FROM converted WHERE x >= '2026' -- note",
    )
    .into_iter()
    .filter(|(_, t)| *t != Tok::Space)
    .collect();
    let kinds: Vec<Tok> = toks.iter().map(|(_, t)| *t).collect();
    assert_eq!(
        kinds,
        [
            Tok::Keyword,
            Tok::Function,
            Tok::Punct,
            Tok::Ident,
            Tok::Punct,
            Tok::Keyword,
            Tok::Ident,
            Tok::Keyword,
            Tok::Relation,
            Tok::Keyword,
            Tok::Ident,
            Tok::Op,
            Tok::Str,
            Tok::Comment,
        ]
    );
    let joined: String =
        highlight("a  b").into_iter().map(|(s, _)| s).collect();
    assert_eq!(joined, "a  b", "nothing is lost between the tokens");
}

#[test]
fn cells_are_written_the_way_the_ledger_writes_them() {
    assert_eq!(write_cell(&Cell::Decimal(d("1234.5"))), "1,234.50");
    assert_eq!(write_cell(&Cell::Decimal(d("-62"))), "−62.00");
    assert_eq!(
        write_cell(&Cell::Decimal(d("0.0055371108"))),
        "0.0055371108"
    );
    assert_eq!(write_cell(&Cell::Int(80)), "80");
    assert_eq!(write_cell(&Cell::Date((2026, 9, 1))), "2026-09-01");
    assert_eq!(write_cell(&Cell::Null), "NULL");
    assert_eq!(write_cell(&Cell::Bool(true)), "true");
}

fn answer(columns: &[(&str, &str)], rows: Vec<Vec<Cell>>) -> Answer {
    Answer {
        columns: columns
            .iter()
            .map(|(n, t)| Column {
                name: (*n).into(),
                ty: (*t).into(),
            })
            .collect(),
        total: rows.len(),
        rows,
        elapsed: std::time::Duration::from_micros(2400),
    }
}

#[test]
fn one_non_negative_figure_column_gets_bars_and_a_total() {
    let a = answer(
        &[
            ("payee", "VARCHAR"),
            ("txns", "BIGINT"),
            ("spent", "DECIMAL(38,18)"),
        ],
        vec![
            vec![
                Cell::Text("A".into()),
                Cell::Int(9),
                Cell::Decimal(d("100")),
            ],
            vec![Cell::Text("B".into()), Cell::Int(3), Cell::Decimal(d("25"))],
        ],
    );
    let l: Layout = layout(&a);
    assert_eq!(l.bars, Some(2));
    assert_eq!(l.shares, [1.0, 0.25]);
    assert_eq!(
        l.totals,
        [None, Some(Cell::Int(12)), Some(Cell::Decimal(d("125")))]
    );
    assert_eq!(l.numeric, [false, true, true]);
}

#[test]
fn several_figure_columns_or_a_negative_one_get_no_bars() {
    let a = answer(
        &[("amount", "DECIMAL(38,18)"), ("balance", "DECIMAL(38,18)")],
        vec![vec![Cell::Decimal(d("-5")), Cell::Decimal(d("10"))]],
    );
    let l = layout(&a);
    assert_eq!(l.bars, None);
    assert!(
        l.totals.iter().all(Option::is_none),
        "a running balance is not summed"
    );
    let neg = answer(
        &[("spent", "DECIMAL(38,18)")],
        vec![vec![Cell::Decimal(d("-1"))]],
    );
    assert_eq!(layout(&neg).bars, None);
}

#[test]
fn the_console_opens_the_first_saved_query_and_names_edits() {
    let mut c = Console::new(&[("food-by-month".into(), "SELECT 1".into())]);
    assert_eq!(c.editor.text(), "SELECT 1");
    assert_eq!(c.title(), ("food-by-month".to_string(), false));
    c.editor.insert("0");
    c.touch();
    assert_eq!(c.title(), ("food-by-month".to_string(), true));
    let empty = Console::new(&[]);
    assert_eq!(empty.title(), ("scratch".to_string(), false));
}

#[test]
fn a_missing_table_comes_with_duckdbs_suggestion() {
    let f = bean_sql::Failure {
        kind: "Catalog".into(),
        message: "Table with name posting does not exist!\nDid you mean \"postings\"?\n\nLINE 2: FROM posting\n             ^".into(),
    };
    assert_eq!(
        bean_desktop::model::console::suggestion(&f),
        Some(("posting".into(), "postings".into()))
    );
    let mut e = Editor::new("SELECT posting_id\nFROM posting\nLIMIT 20;");
    assert!(e.replace_word("posting", "postings"));
    assert_eq!(e.text(), "SELECT posting_id\nFROM postings\nLIMIT 20;");
}

#[test]
fn a_finished_run_goes_to_the_top_of_the_history() {
    let mut c = Console::new(&[("top".into(), "SELECT 1".into())]);
    c.running = true;
    c.finish(
        "14:02".into(),
        "SELECT 1".into(),
        Ok(answer(&[("x", "INTEGER")], vec![vec![Cell::Int(1)]])),
    );
    c.finish(
        "14:03".into(),
        "SELECT 1".into(),
        Err(bean_sql::Failure {
            kind: "Parser".into(),
            message: "".into(),
        }),
    );
    assert!(!c.running);
    assert_eq!(c.history[0].at, "14:03");
    assert_eq!(c.history[0].outcome, Err("Parser".into()));
    assert_eq!(c.history[1].outcome, Ok(1));
    assert_eq!(c.history[1].name, "top");
}

#[test]
fn an_empty_editor_still_has_a_line_to_type_on() {
    let mut e = Editor::default();
    assert_eq!(e.lines().len(), 1);
    e.insert("x");
    e.backspace();
    e.backspace();
    e.newline();
    assert_eq!(e.text(), "\n");
    let c = Console::new(&[]);
    assert_eq!(
        c.editor.lines().len(),
        1,
        "no saved queries, one empty line"
    );
}

mod table {
    use super::*;
    use bean_desktop::model::console::{Style, prepare};

    fn food() -> Answer {
        answer(
            &[
                ("month", "DATE"),
                ("account", "VARCHAR"),
                ("payee", "VARCHAR"),
                ("spent", "DECIMAL(38,18)"),
            ],
            vec![
                vec![
                    Cell::Date((2026, 6, 1)),
                    Cell::Text("Expenses:Food:Groceries".into()),
                    Cell::Text("Green Basket".into()),
                    Cell::Decimal(d("441")),
                ],
                vec![
                    Cell::Date((2026, 6, 1)),
                    Cell::Text("Expenses:Food:Coffee".into()),
                    Cell::Null,
                    Cell::Decimal(d("60")),
                ],
            ],
        )
    }

    #[test]
    fn cells_are_written_once_with_how_to_draw_them() {
        let t = prepare(&food());
        assert_eq!(t.rows.len(), 2);
        assert_eq!(t.rows[0][0].text, "2026-06-01");
        assert_eq!(t.rows[0][0].style, Style::Date);
        assert_eq!(t.rows[1][0].style, Style::Ditto, "the same day again");
        assert_eq!(t.rows[0][1].style, Style::Account);
        assert_eq!(t.rows[0][2].style, Style::Quoted);
        assert_eq!(t.rows[1][2].style, Style::Null);
        assert_eq!(t.rows[0][3].text, "441.00");
        assert_eq!(t.columns[3].ty, "dec");
    }

    #[test]
    fn columns_are_as_wide_as_their_widest_cell_up_to_a_limit() {
        let t = prepare(&food());
        // `account text` is 12 wide; the longest account is 23.
        assert_eq!(t.widths[1], 23.);
        // A quoted payee counts its quotes.
        assert_eq!(t.widths[2], 14.);
        assert_eq!(t.flexible, Some(1), "the widest text column flexes");
        let long = answer(
            &[("note", "VARCHAR"), ("n", "INTEGER")],
            vec![vec![Cell::Text("x".repeat(90)), Cell::Int(1)]],
        );
        assert_eq!(prepare(&long).widths[0], 40.);
    }

    #[test]
    fn gains_are_marked_where_a_column_holds_both_signs() {
        let a = answer(
            &[("amount", "DECIMAL(38,18)"), ("cost", "DECIMAL(38,18)")],
            vec![
                vec![Cell::Decimal(d("5")), Cell::Decimal(d("5"))],
                vec![Cell::Decimal(d("-5")), Cell::Decimal(d("6"))],
            ],
        );
        let t = prepare(&a);
        assert_eq!(t.rows[0][0].style, Style::Gain);
        assert_eq!(t.rows[0][0].text, "+5.00");
        assert_eq!(t.rows[0][1].style, Style::Plain, "never negative, no plus");
    }

    #[test]
    fn the_table_is_at_least_as_wide_as_its_columns() {
        let t = prepare(&food());
        // fixed columns + the flexible one's floor + the gaps, in ch.
        let fixed: f32 = t
            .widths
            .iter()
            .enumerate()
            .filter(|(i, _)| Some(*i) != t.flexible)
            .map(|(_, w)| w)
            .sum();
        assert!(t.min_chars() >= fixed + 20.);
    }
}
