//! The SQL console: the text being edited, what each word of it is,
//! the runs so far and how an answer is laid out. No gpui here; the
//! page draws what this decides.

use bean_sql::{Answer, Cell, Failure, Relation};
use rust_decimal::Decimal;

use crate::fmt::fixed;

/// A multi-line text buffer with one cursor, counted in characters.
#[derive(Clone, Debug, Default)]
pub struct Editor {
    lines: Vec<String>,
    row: usize,
    col: usize,
    /// The column up and down aim for, kept across short lines.
    goal: Option<usize>,
}

fn chars(s: &str) -> usize {
    s.chars().count()
}

/// The byte offset of character `col` in `s`.
fn byte(s: &str, col: usize) -> usize {
    s.char_indices().nth(col).map_or(s.len(), |(i, _)| i)
}

impl Editor {
    /// `text` with the cursor at its end.
    pub fn new(text: &str) -> Self {
        let lines: Vec<String> = text.split('\n').map(str::to_owned).collect();
        let row = lines.len() - 1;
        let col = chars(&lines[row]);
        Self {
            lines,
            row,
            col,
            goal: None,
        }
    }

    pub fn text(&self) -> String {
        self.lines.join("\n")
    }

    pub fn lines(&self) -> &[String] {
        &self.lines
    }

    /// (line, character) of the cursor, from zero.
    pub fn cursor(&self) -> (usize, usize) {
        (self.row, self.col)
    }

    pub fn move_to(&mut self, row: usize, col: usize) {
        self.row = row.min(self.lines.len() - 1);
        self.col = col.min(chars(&self.lines[self.row]));
        self.goal = None;
    }

    /// Insert `text` at the cursor; its newlines split the line.
    pub fn insert(&mut self, text: &str) {
        let mut parts = text.split('\n');
        let first = parts.next().unwrap_or_default();
        let line = &mut self.lines[self.row];
        let at = byte(line, self.col);
        let tail = line.split_off(at);
        line.push_str(first);
        self.col += chars(first);
        for part in parts {
            self.lines.insert(self.row + 1, part.to_owned());
            self.row += 1;
            self.col = chars(part);
        }
        self.lines[self.row].push_str(&tail);
        self.goal = None;
    }

    /// Break the line at the cursor, carrying its indent onto the next.
    pub fn newline(&mut self) {
        let indent: String = self.lines[self.row]
            .chars()
            .take_while(|c| *c == ' ')
            .collect();
        self.insert(&format!("\n{indent}"));
    }

    pub fn backspace(&mut self) {
        if self.col > 0 {
            let line = &mut self.lines[self.row];
            let at = byte(line, self.col - 1);
            line.remove(at);
            self.col -= 1;
        } else if self.row > 0 {
            let line = self.lines.remove(self.row);
            self.row -= 1;
            self.col = chars(&self.lines[self.row]);
            self.lines[self.row].push_str(&line);
        }
        self.goal = None;
    }

    pub fn delete(&mut self) {
        let len = chars(&self.lines[self.row]);
        if self.col < len {
            let line = &mut self.lines[self.row];
            let at = byte(line, self.col);
            line.remove(at);
        } else if self.row + 1 < self.lines.len() {
            let next = self.lines.remove(self.row + 1);
            self.lines[self.row].push_str(&next);
        }
        self.goal = None;
    }

    pub fn left(&mut self) {
        if self.col > 0 {
            self.col -= 1;
        } else if self.row > 0 {
            self.row -= 1;
            self.col = chars(&self.lines[self.row]);
        }
        self.goal = None;
    }

    pub fn right(&mut self) {
        if self.col < chars(&self.lines[self.row]) {
            self.col += 1;
        } else if self.row + 1 < self.lines.len() {
            self.row += 1;
            self.col = 0;
        }
        self.goal = None;
    }

    fn vertical(&mut self, row: usize) {
        let goal = *self.goal.get_or_insert(self.col);
        self.row = row;
        self.col = goal.min(chars(&self.lines[row]));
    }

    pub fn up(&mut self) {
        if self.row > 0 {
            self.vertical(self.row - 1);
        }
    }

    pub fn down(&mut self) {
        if self.row + 1 < self.lines.len() {
            self.vertical(self.row + 1);
        }
    }

    pub fn home(&mut self) {
        self.col = 0;
        self.goal = None;
    }

    pub fn end(&mut self) {
        self.col = chars(&self.lines[self.row]);
        self.goal = None;
    }

    /// Replace the first whole-word `from` with `to`.
    pub fn replace_word(&mut self, from: &str, to: &str) -> bool {
        for line in &mut self.lines {
            let mut start = 0;
            while let Some(i) = line[start..].find(from) {
                let at = start + i;
                let end = at + from.len();
                let word = |c: char| c.is_alphanumeric() || c == '_';
                let before = line[..at].chars().next_back().is_some_and(word);
                let after = line[end..].chars().next().is_some_and(word);
                if !before && !after {
                    line.replace_range(at..end, to);
                    self.goal = None;
                    self.col = self.col.min(chars(&self.lines[self.row]));
                    return true;
                }
                start = end;
            }
        }
        false
    }
}

/// What a stretch of SQL is, for its colour.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tok {
    Space,
    Keyword,
    Function,
    /// A table or view the console provides.
    Relation,
    Str,
    Number,
    Op,
    Punct,
    Ident,
    Comment,
}

/// The tables and views the database holds, in the order it lists them.
pub const RELATIONS: &[&str] = &[
    "postings",
    "txns",
    "tags",
    "links",
    "meta",
    "prices",
    "accounts",
    "commodities",
    "converted",
    "monthly",
    "balances",
];

const KEYWORDS: &[&str] = &[
    "ALL",
    "AND",
    "AS",
    "ASC",
    "ASOF",
    "BETWEEN",
    "BY",
    "CASE",
    "CAST",
    "COPY",
    "CREATE",
    "DATE",
    "DESC",
    "DESCRIBE",
    "DISTINCT",
    "ELSE",
    "END",
    "EXCEPT",
    "EXISTS",
    "FALSE",
    "FILTER",
    "FROM",
    "FULL",
    "GROUP",
    "HAVING",
    "ILIKE",
    "IN",
    "INNER",
    "INTERSECT",
    "INTERVAL",
    "IS",
    "JOIN",
    "LEFT",
    "LIKE",
    "LIMIT",
    "NOT",
    "NULL",
    "OFFSET",
    "ON",
    "OR",
    "ORDER",
    "OUTER",
    "OVER",
    "PARTITION",
    "PIVOT",
    "QUALIFY",
    "RIGHT",
    "ROWS",
    "SELECT",
    "SUMMARIZE",
    "TABLE",
    "THEN",
    "TO",
    "TRUE",
    "UNION",
    "UNPIVOT",
    "USING",
    "VIEW",
    "WHEN",
    "WHERE",
    "WINDOW",
    "WITH",
];

/// A line of SQL cut into coloured stretches that join back into it.
pub fn highlight(line: &str) -> Vec<(String, Tok)> {
    let cs: Vec<char> = line.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    let take = |from: usize, to: usize| cs[from..to].iter().collect::<String>();
    while i < cs.len() {
        let c = cs[i];
        let start = i;
        let tok = if c.is_whitespace() {
            while i < cs.len() && cs[i].is_whitespace() {
                i += 1;
            }
            Tok::Space
        } else if c == '-' && cs.get(i + 1) == Some(&'-') {
            i = cs.len();
            Tok::Comment
        } else if c == '\'' || c == '"' {
            i += 1;
            while i < cs.len() && cs[i] != c {
                i += 1;
            }
            i = (i + 1).min(cs.len());
            if c == '\'' { Tok::Str } else { Tok::Ident }
        } else if c.is_ascii_digit() {
            while i < cs.len() && (cs[i].is_ascii_digit() || cs[i] == '.') {
                i += 1;
            }
            Tok::Number
        } else if c.is_alphabetic() || c == '_' {
            while i < cs.len() && (cs[i].is_alphanumeric() || cs[i] == '_') {
                i += 1;
            }
            let word = take(start, i);
            let mut next = i;
            while next < cs.len() && cs[next] == ' ' {
                next += 1;
            }
            if KEYWORDS.contains(&word.to_uppercase().as_str()) {
                Tok::Keyword
            } else if cs.get(next) == Some(&'(') {
                Tok::Function
            } else if RELATIONS.contains(&word.as_str()) {
                Tok::Relation
            } else {
                Tok::Ident
            }
        } else if "(),;.[]{}".contains(c) {
            i += 1;
            Tok::Punct
        } else {
            while i < cs.len() && "=<>!*+-/%|:~^&".contains(cs[i]) {
                i += 1;
            }
            if i == start {
                i += 1;
            }
            Tok::Op
        };
        out.push((take(start, i), tok));
    }
    out
}

/// A cell as the page writes it: figures grouped with a true minus and
/// at least cents, dates as the ledger writes them.
pub fn write_cell(c: &Cell) -> String {
    match c {
        Cell::Null => "NULL".into(),
        Cell::Bool(b) => b.to_string(),
        Cell::Int(i) => i.to_string(),
        Cell::Decimal(d) => {
            let d = d.normalize();
            fixed(d, d.scale().max(2))
        }
        Cell::Float(f) => format!("{f}"),
        Cell::Text(s) | Cell::Other(s) => s.clone(),
        Cell::Date((y, m, d)) => format!("{y:04}-{m:02}-{d:02}"),
    }
}

fn is_figure(ty: &str) -> bool {
    ty.starts_with("DECIMAL") || ty == "DOUBLE" || ty == "FLOAT"
}

fn is_count(ty: &str) -> bool {
    matches!(
        ty,
        "TINYINT"
            | "SMALLINT"
            | "INTEGER"
            | "BIGINT"
            | "HUGEINT"
            | "UTINYINT"
            | "USMALLINT"
            | "UINTEGER"
            | "UBIGINT"
    )
}

fn value(c: &Cell) -> Option<Decimal> {
    match c {
        Cell::Int(i) => Some(Decimal::from(*i)),
        Cell::Decimal(d) => Some(*d),
        Cell::Float(f) => Decimal::try_from(*f).ok(),
        _ => None,
    }
}

/// How an answer is drawn: which columns are figures (set right), and
/// whether it is the shape a bar reads well in.
#[derive(Clone, Debug, PartialEq)]
pub struct Layout {
    pub numeric: Vec<bool>,
    /// The one figure column the rows are measured on, when there is
    /// exactly one and nothing in it is below zero.
    pub bars: Option<usize>,
    /// Each row's share of the largest value in the bar column.
    pub shares: Vec<f32>,
    /// Column sums, written under the rows only when there are bars: a
    /// sum of anything else (a running balance) would mean nothing.
    pub totals: Vec<Option<Cell>>,
}

pub fn layout(a: &Answer) -> Layout {
    let numeric: Vec<bool> = a
        .columns
        .iter()
        .map(|c| is_figure(&c.ty) || is_count(&c.ty))
        .collect();
    let figures: Vec<usize> = a
        .columns
        .iter()
        .enumerate()
        .filter(|(_, c)| is_figure(&c.ty))
        .map(|(i, _)| i)
        .collect();
    let bars = match figures.as_slice() {
        [k] => {
            let vals: Vec<Option<Decimal>> =
                a.rows.iter().map(|r| r.get(*k).and_then(value)).collect();
            let fine =
                vals.iter().all(|v| v.is_none_or(|v| v >= Decimal::ZERO));
            let some =
                vals.iter().any(|v| v.is_some_and(|v| v > Decimal::ZERO));
            (fine && some).then_some(*k)
        }
        _ => None,
    };
    let shares = match bars {
        Some(k) => {
            let max = a
                .rows
                .iter()
                .filter_map(|r| r.get(k).and_then(value))
                .max()
                .unwrap_or(Decimal::ONE);
            a.rows
                .iter()
                .map(|r| {
                    let v = r.get(k).and_then(value).unwrap_or_default();
                    f32::try_from(v / max).unwrap_or(0.)
                })
                .collect()
        }
        None => Vec::new(),
    };
    let totals = a
        .columns
        .iter()
        .enumerate()
        .map(|(i, c)| {
            if bars.is_none() || !numeric[i] {
                return None;
            }
            let sum: Decimal =
                a.rows.iter().filter_map(|r| r.get(i).and_then(value)).sum();
            Some(if is_count(&c.ty) {
                i64::try_from(sum).map_or(Cell::Decimal(sum), Cell::Int)
            } else {
                Cell::Decimal(sum)
            })
        })
        .collect();
    Layout {
        numeric,
        bars,
        shares,
        totals,
    }
}

/// One run, for the history list.
#[derive(Clone, Debug)]
pub struct Run {
    /// When it ran: `14:02`.
    pub at: String,
    pub name: String,
    pub sql: String,
    /// Rows answered, or the failure's class.
    pub outcome: Result<usize, String>,
}

#[derive(Clone, Debug, Default)]
pub struct Console {
    pub editor: Editor,
    /// The saved query the editor was loaded from.
    pub origin: Option<String>,
    /// Whether the text changed since it was loaded.
    pub edited: bool,
    /// Newest first.
    pub history: Vec<Run>,
    pub result: Option<Result<Answer, Failure>>,
    pub running: bool,
    /// The table whose columns the schema list shows.
    pub open_table: Option<String>,
    /// The database's tables and views, once it is up.
    pub schema: Vec<Relation>,
}

impl Console {
    /// A console holding the first of `saved` (name, sql), if any.
    pub fn new(saved: &[(String, String)]) -> Self {
        let mut c = Console {
            open_table: Some("postings".into()),
            ..Default::default()
        };
        if let Some((name, sql)) = saved.first() {
            c.load(name, sql);
        }
        c
    }

    pub fn load(&mut self, name: &str, sql: &str) {
        self.editor = Editor::new(sql);
        self.origin = Some(name.to_owned());
        self.edited = false;
    }

    /// Text that came from no saved query.
    pub fn scratch(&mut self, sql: &str) {
        self.editor = Editor::new(sql);
        self.origin = None;
        self.edited = false;
    }

    /// The text changed under the cursor.
    pub fn touch(&mut self) {
        self.edited = true;
    }

    /// The name the editor's legend shows, and whether it has unsaved
    /// edits.
    pub fn title(&self) -> (String, bool) {
        match &self.origin {
            Some(name) => (name.clone(), self.edited),
            None => ("scratch".into(), self.edited),
        }
    }

    /// Note a finished run and show its answer.
    pub fn finish(
        &mut self,
        at: String,
        sql: String,
        outcome: Result<Answer, Failure>,
    ) {
        let (name, _) = self.title();
        self.history.insert(
            0,
            Run {
                at,
                name,
                sql,
                outcome: match &outcome {
                    Ok(a) => Ok(a.total),
                    Err(f) => Err(f.kind.clone()),
                },
            },
        );
        self.history.truncate(50);
        self.result = Some(outcome);
        self.running = false;
    }
}

/// DuckDB's "did you mean" for a missing name: (what was written, what
/// it suggests).
pub fn suggestion(f: &Failure) -> Option<(String, String)> {
    let quoted = |s: &str, after: &str| -> Option<String> {
        let rest = &s[s.find(after)? + after.len()..];
        let rest = rest.trim_start_matches(['"', '\'']);
        let end = rest.find(['"', '\'', ' ', '!', '\n'])?;
        Some(rest[..end].to_owned())
    };
    let wrong = quoted(&f.message, "with name ")?;
    let right = quoted(&f.message, "Did you mean ")?;
    let right = right.rsplit('.').next().unwrap_or(&right).to_owned();
    Some((wrong, right))
}
