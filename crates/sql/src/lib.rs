//! The ledger as an in-memory DuckDB database, for the SQL console.
//!
//! Everything here reads the ledger after it has been built: elided
//! postings already carry their residuals and lots are resolved, so the
//! tables hold the same numbers every page shows. Each reading of the
//! ledger gets a fresh database; nothing is ever updated in place.

use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use bean_core::model::{Day, Ledger};
use duckdb::{
    Connection, InterruptHandle, ToSql,
    arrow::datatypes::DataType,
    types::{Value, ValueRef},
};
use rust_decimal::Decimal;

/// Every amount is stored at one fixed scale: eighteen places is what
/// a token like ETH is written to, and leaves twenty digits of whole
/// units for the largest sum.
const SCALE: u32 = 18;
const DECIMAL: &str = "DECIMAL(38,18)";

/// One value of an answer, in the terms the console draws it in.
#[derive(Debug, Clone, PartialEq)]
pub enum Cell {
    Null,
    Bool(bool),
    Int(i64),
    Decimal(Decimal),
    Float(f64),
    Text(String),
    Date(Day),
    /// Anything else, as DuckDB spells it.
    Other(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Column {
    pub name: String,
    /// DuckDB's name for the type: `VARCHAR`, `DECIMAL(38,18)`.
    pub ty: String,
}

/// What a query answered.
#[derive(Debug, Clone)]
pub struct Answer {
    pub columns: Vec<Column>,
    /// At most the cap's worth of rows, in order.
    pub rows: Vec<Vec<Cell>>,
    /// How many rows the query produced, kept or not.
    pub total: usize,
    pub elapsed: Duration,
}

/// Why a query failed, as DuckDB put it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Failure {
    /// The error's class without the word "Error": `Catalog`, `Parser`.
    pub kind: String,
    /// The message after the class, line pointer and all.
    pub message: String,
}

impl Failure {
    fn from(error: duckdb::Error) -> Self {
        let text = match &error {
            duckdb::Error::DuckDBFailure(_, Some(message)) => message.clone(),
            other => other.to_string(),
        };
        match text.split_once(" Error: ") {
            Some((kind, message)) if !kind.contains(char::is_whitespace) => {
                Failure {
                    kind: kind.to_owned(),
                    message: message.to_owned(),
                }
            }
            _ => Failure {
                kind: "Error".into(),
                message: text,
            },
        }
    }
}

/// A table or view, for the console's schema list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Relation {
    pub name: String,
    pub view: bool,
    /// How many rows a table holds; views are not counted.
    pub rows: Option<u64>,
    pub columns: Vec<Column>,
}

pub struct Database {
    conn: Connection,
}

/// The tables in the order the console lists them, with their columns.
const TABLES: &[(&str, &str)] = &[
    (
        "postings",
        "txn_id INTEGER, posting INTEGER, date DATE, flag VARCHAR, \
         payee VARCHAR, narration VARCHAR, account VARCHAR, kind VARCHAR, \
         amount DECIMAL(38,18), currency VARCHAR, \
         cost DECIMAL(38,18), cost_currency VARCHAR, \
         price DECIMAL(38,18), price_currency VARCHAR",
    ),
    (
        "txns",
        "txn_id INTEGER, date DATE, flag VARCHAR, payee VARCHAR, \
         narration VARCHAR, file VARCHAR, line INTEGER",
    ),
    ("tags", "txn_id INTEGER, tag VARCHAR"),
    ("links", "txn_id INTEGER, link VARCHAR"),
    ("meta", "txn_id INTEGER, key VARCHAR, value VARCHAR"),
    (
        "prices",
        "date DATE, base VARCHAR, quote VARCHAR, rate DECIMAL(38,18)",
    ),
    (
        "accounts",
        "account VARCHAR, label VARCHAR, kind VARCHAR, closed DATE, \
         liquidity VARCHAR, reserve DECIMAL(38,18), goal DECIMAL(38,18), \
         rate DECIMAL(38,18), due INTEGER, credit_limit DECIMAL(38,18), \
         collateral VARCHAR",
    ),
    (
        "commodities",
        "currency VARCHAR, name VARCHAR, asset_class VARCHAR, \
         quote_currency VARCHAR",
    ),
];

const VIEWS: &[&str] = &["converted", "monthly", "balances"];

fn views(currency: &str) -> String {
    let c = currency.replace('\'', "''");
    format!(
        "CREATE VIEW converted AS
         SELECT p.*,
                CASE WHEN p.currency = '{c}' THEN p.amount
                     -- Two scale-18 factors would need 36 places; eight
                     -- for the amount and ten for the rate stay in range.
                     ELSE CAST(CAST(p.amount AS DECIMAL(38,8))
                               * CAST(coalesce(d.rate, 1 / i.rate)
                                      AS DECIMAL(38,10))
                               AS {DECIMAL})
                END AS value
         FROM postings p
         ASOF LEFT JOIN prices d
           ON d.base = p.currency AND d.quote = '{c}' AND p.date >= d.date
         ASOF LEFT JOIN prices i
           ON i.quote = p.currency AND i.base = '{c}' AND p.date >= i.date;
         CREATE VIEW monthly AS
         SELECT CAST(date_trunc('month', date) AS DATE) AS month,
                account, currency, sum(amount) AS amount
         FROM postings GROUP BY ALL;
         CREATE VIEW balances AS
         SELECT txn_id, posting, date, payee, narration, account, currency,
                amount,
                sum(amount) OVER (
                  PARTITION BY account, currency
                  ORDER BY date, txn_id, posting
                  ROWS UNBOUNDED PRECEDING) AS balance
         FROM postings;"
    )
}

/// Days since 1970-01-01, which is how DuckDB stores a `DATE`.
fn epoch_days(d: Day) -> i32 {
    let (y, m, dd) = (i64::from(d.0), i64::from(d.1), i64::from(d.2));
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + dd - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    (era * 146_097 + doe - 719_468) as i32
}

fn civil(days: i32) -> Day {
    let z = i64::from(days) + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let dd = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    (y as u16, m as u8, dd as u8)
}

fn date(d: Day) -> Value {
    Value::Date32(epoch_days(d))
}

/// `v` as an integer count of 10⁻¹⁸ units. Done in i128 because
/// `Decimal::rescale` would round away digits a 96-bit mantissa can't
/// hold, and an amount of 80 billion rupiah is not exotic.
fn scaled(v: Decimal) -> i128 {
    let scale = v.scale();
    if scale <= SCALE {
        v.mantissa() * 10i128.pow(SCALE - scale)
    } else {
        v.round_dp(SCALE).mantissa()
    }
}

fn dec(v: Decimal) -> Value {
    duckdb::types::Decimal::new(38, SCALE as u8, scaled(v))
        .map(Value::Decimal)
        .unwrap_or(Value::Null)
}

fn text(s: impl Into<String>) -> Value {
    Value::Text(s.into())
}

fn opt<T>(v: Option<T>, f: impl FnOnce(T) -> Value) -> Value {
    v.map_or(Value::Null, f)
}

fn root(account: &str) -> &str {
    account.split(':').next().unwrap_or(account)
}

impl Database {
    /// The ledger, loaded into a fresh in-memory database. Converted
    /// values are in the ledger's first operating currency.
    pub fn load(ledger: &Ledger) -> Result<Self, Failure> {
        let conn = Connection::open_in_memory().map_err(Failure::from)?;
        let mut ddl = String::new();
        for (name, columns) in TABLES {
            ddl.push_str(&format!("CREATE TABLE {name} ({columns});\n"));
        }
        conn.execute_batch(&ddl).map_err(Failure::from)?;
        Self::fill(&conn, ledger).map_err(Failure::from)?;
        let currency = ledger
            .operating_currencies
            .first()
            .map_or("USD", String::as_str);
        conn.execute_batch(&views(currency))
            .map_err(Failure::from)?;
        Ok(Self { conn })
    }

    fn fill(conn: &Connection, ledger: &Ledger) -> duckdb::Result<()> {
        let mut postings = conn.appender("postings")?;
        let mut txns = conn.appender("txns")?;
        let mut tags = conn.appender("tags")?;
        let mut links = conn.appender("links")?;
        let mut meta = conn.appender("meta")?;
        for (id, t) in ledger.txns.iter().enumerate() {
            let id = Value::Int(id as i32);
            let flag = text(t.flag.to_string());
            let payee = opt(t.payee.clone(), text);
            let narration = opt(t.narration.clone(), text);
            let (file, line) = match &t.source {
                Some(s) => (
                    text(
                        s.path
                            .file_name()
                            .map(|f| f.to_string_lossy().into_owned())
                            .unwrap_or_default(),
                    ),
                    Value::Int(s.line as i32),
                ),
                None => (Value::Null, Value::Null),
            };
            txns.append_row([
                &id as &dyn ToSql,
                &date(t.date),
                &flag,
                &payee,
                &narration,
                &file,
                &line,
            ])?;
            for tag in &t.tags {
                tags.append_row([&id as &dyn ToSql, &text(tag.as_str())])?;
            }
            for link in &t.links {
                links.append_row([&id as &dyn ToSql, &text(link.as_str())])?;
            }
            for (k, v) in &t.meta {
                meta.append_row([
                    &id as &dyn ToSql,
                    &text(k.as_str()),
                    &text(v.as_str()),
                ])?;
            }
            for (k, p) in t.postings.iter().enumerate() {
                let account = text(p.account.as_str());
                let kind = text(root(&p.account));
                let cost = opt(p.cost.as_ref(), |c| dec(c.total));
                let cost_cur =
                    opt(p.cost.as_ref(), |c| text(c.currency.as_str()));
                let price = opt(p.price.as_ref(), |c| dec(c.total));
                let price_cur =
                    opt(p.price.as_ref(), |c| text(c.currency.as_str()));
                for (amount, currency) in &p.amounts {
                    postings.append_row([
                        &id as &dyn ToSql,
                        &Value::Int(k as i32),
                        &date(t.date),
                        &flag,
                        &payee,
                        &narration,
                        &account,
                        &kind,
                        &dec(*amount),
                        &text(currency.as_str()),
                        &cost,
                        &cost_cur,
                        &price,
                        &price_cur,
                    ])?;
                }
            }
        }
        drop((postings, txns, tags, links, meta));

        let mut prices = conn.appender("prices")?;
        for (day, base, quote, rate) in ledger.prices() {
            prices.append_row([
                &date(day) as &dyn ToSql,
                &text(base),
                &text(quote),
                &dec(rate),
            ])?;
        }
        drop(prices);

        let mut accounts = conn.appender("accounts")?;
        for a in ledger.accounts() {
            let debt = &a.debt;
            accounts.append_row([
                &text(a.account.as_str()) as &dyn ToSql,
                &text(a.label.as_str()),
                &text(root(&a.account)),
                &opt(a.closed, date),
                &text(a.purpose.liquidity.as_str()),
                &dec(a.purpose.reserve),
                &opt(a.purpose.goal, dec),
                &opt(debt.rate, dec),
                &opt(debt.due, |d| Value::Int(i32::from(d))),
                &opt(debt.limit, dec),
                &opt(debt.collateral.clone(), text),
            ])?;
        }
        drop(accounts);

        let mut commodities = conn.appender("commodities")?;
        for c in ledger.commodities() {
            commodities.append_row([
                &text(c.currency.as_str()) as &dyn ToSql,
                &opt(c.name.clone(), text),
                &opt(c.asset_class.clone(), text),
                &opt(c.quote_currency.clone(), text),
            ])?;
        }
        Ok(())
    }

    /// Run one statement, keeping the first `cap` rows of its answer.
    pub fn run(&self, sql: &str, cap: usize) -> Result<Answer, Failure> {
        let started = Instant::now();
        let mut stmt = self.conn.prepare(sql).map_err(Failure::from)?;
        let mut rows = stmt.query([]).map_err(Failure::from)?;
        let mut kept = Vec::new();
        let mut total = 0;
        let mut width = None;
        while let Some(row) = rows.next().map_err(Failure::from)? {
            let n = *width.get_or_insert_with(|| row.as_ref().column_count());
            if kept.len() < cap {
                kept.push(
                    (0..n)
                        .map(|i| row.get_ref(i).map(cell).unwrap_or(Cell::Null))
                        .collect(),
                );
            }
            total += 1;
        }
        let columns = match rows.as_ref() {
            Some(stmt) => (0..stmt.column_count())
                .map(|i| Column {
                    name: stmt
                        .column_name(i)
                        .map_or_else(|_| String::new(), Clone::clone),
                    ty: type_name(&stmt.column_type(i)),
                })
                .collect(),
            None => Vec::new(),
        };
        Ok(Answer {
            columns,
            rows: kept,
            total,
            elapsed: started.elapsed(),
        })
    }

    /// The tables, then the views, each with its columns.
    pub fn schema(&self) -> Result<Vec<Relation>, Failure> {
        let mut out = Vec::new();
        for (name, view) in TABLES
            .iter()
            .map(|(n, _)| (*n, false))
            .chain(VIEWS.iter().map(|n| (*n, true)))
        {
            let described =
                self.run(&format!("DESCRIBE {name}"), usize::MAX)?;
            let columns = described
                .rows
                .iter()
                .map(|r| Column {
                    name: text_of(&r[0]),
                    ty: text_of(&r[1]),
                })
                .collect();
            let rows = if view {
                None
            } else {
                match self
                    .run(&format!("SELECT count(*) FROM {name}"), 1)?
                    .rows
                    .first()
                    .and_then(|r| r.first())
                {
                    Some(Cell::Int(n)) => Some(*n as u64),
                    _ => None,
                }
            };
            out.push(Relation {
                name: name.to_owned(),
                view,
                rows,
                columns,
            });
        }
        Ok(out)
    }

    /// A handle that stops whatever query is running, from any thread.
    pub fn interrupt_handle(&self) -> Arc<InterruptHandle> {
        self.conn.interrupt_handle()
    }
}

fn text_of(c: &Cell) -> String {
    match c {
        Cell::Text(s) | Cell::Other(s) => s.clone(),
        other => format!("{other:?}"),
    }
}

fn cell(v: ValueRef<'_>) -> Cell {
    match v {
        ValueRef::Null => Cell::Null,
        ValueRef::Boolean(b) => Cell::Bool(b),
        ValueRef::TinyInt(i) => Cell::Int(i.into()),
        ValueRef::SmallInt(i) => Cell::Int(i.into()),
        ValueRef::Int(i) => Cell::Int(i.into()),
        ValueRef::BigInt(i) => Cell::Int(i),
        ValueRef::UTinyInt(i) => Cell::Int(i.into()),
        ValueRef::USmallInt(i) => Cell::Int(i.into()),
        ValueRef::UInt(i) => Cell::Int(i.into()),
        ValueRef::UBigInt(i) => i64::try_from(i)
            .map_or_else(|_| Cell::Other(i.to_string()), Cell::Int),
        ValueRef::HugeInt(i) => i64::try_from(i)
            .map_or_else(|_| Cell::Other(i.to_string()), Cell::Int),
        ValueRef::Float(f) => Cell::Float(f.into()),
        ValueRef::Double(f) => Cell::Float(f),
        ValueRef::Decimal(d) => {
            Decimal::try_from_i128_with_scale(d.value(), u32::from(d.scale()))
                .map(|v| Cell::Decimal(v.normalize_to(u32::from(d.scale()))))
                .unwrap_or_else(|_| Cell::Other(d.to_string()))
        }
        ValueRef::Text(bytes) => {
            Cell::Text(String::from_utf8_lossy(bytes).into_owned())
        }
        ValueRef::Date32(days) => Cell::Date(civil(days)),
        other => match Value::from(other) {
            Value::Text(s) => Cell::Other(s),
            v => Cell::Other(format!("{v:?}")),
        },
    }
}

/// Decimals come back at the column's scale (`22.0000000000`); a figure
/// reads at the fewest places that keep its value, but never fewer than
/// two when it has a fraction at all.
trait Normalize {
    fn normalize_to(self, scale: u32) -> Self;
}

impl Normalize for Decimal {
    fn normalize_to(self, _scale: u32) -> Self {
        let n = self.normalize();
        if n.scale() > 0 && n.scale() < 2 {
            let mut n = n;
            n.rescale(2);
            n
        } else {
            n
        }
    }
}

/// DuckDB's own name for a column's type.
fn type_name(t: &DataType) -> String {
    match t {
        DataType::Null => "NULL".into(),
        DataType::Boolean => "BOOLEAN".into(),
        DataType::Int8 => "TINYINT".into(),
        DataType::Int16 => "SMALLINT".into(),
        DataType::Int32 => "INTEGER".into(),
        DataType::Int64 => "BIGINT".into(),
        DataType::UInt8 => "UTINYINT".into(),
        DataType::UInt16 => "USMALLINT".into(),
        DataType::UInt32 => "UINTEGER".into(),
        DataType::UInt64 => "UBIGINT".into(),
        DataType::Float32 => "FLOAT".into(),
        DataType::Float64 => "DOUBLE".into(),
        DataType::Utf8 | DataType::LargeUtf8 | DataType::Utf8View => {
            "VARCHAR".into()
        }
        DataType::Date32 | DataType::Date64 => "DATE".into(),
        DataType::Decimal128(p, s) => format!("DECIMAL({p},{s})"),
        DataType::Timestamp(..) => "TIMESTAMP".into(),
        DataType::Interval(_) => "INTERVAL".into(),
        DataType::List(f) | DataType::LargeList(f) => {
            format!("{}[]", type_name(f.data_type()))
        }
        other => format!("{other:?}").to_uppercase(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates_survive_the_round_trip_through_epoch_days() {
        for d in [(1970, 1, 1), (2026, 2, 28), (2028, 2, 29), (1999, 12, 31)] {
            assert_eq!(civil(epoch_days(d)), d);
        }
        assert_eq!(epoch_days((1970, 1, 2)), 1);
    }

    #[test]
    fn large_amounts_keep_every_digit_at_the_fixed_scale() {
        let big: Decimal = "120000000000.25".parse().unwrap();
        assert_eq!(scaled(big), 120_000_000_000_250_000_000_000_000_000);
        let fine: Decimal = "0.0000000000000000004".parse().unwrap();
        assert_eq!(scaled(fine), 0, "rounded at the eighteenth place");
    }

    #[test]
    fn a_figure_keeps_two_places_when_it_has_a_fraction() {
        let d = |s: &str| s.parse::<Decimal>().unwrap();
        assert_eq!(d("22.0000000000").normalize_to(18).to_string(), "22");
        assert_eq!(d("4.5000000000").normalize_to(18).to_string(), "4.50");
        assert_eq!(d("1.1250000000").normalize_to(18).to_string(), "1.125");
    }
}
