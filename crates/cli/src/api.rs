//! The read-only JSON API over a loaded [`Ledger`].
//!
//! Three endpoints drive the whole UI: `/api/summary` (ledger shape and
//! month range), `/api/month/{month}` (the monthly budget page), and
//! `/api/category/{account}/{month}` (the inspector drill-down).

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use bean_core::model::{Day, Ledger, MonthKey, Txn};
use bean_core::query::{AccountRow, CategoryRow, Group};
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use serde_json::{Map, Value, json};

/// Everything a request handler needs, built once at startup.
pub struct AppState {
    pub ledger: Ledger,
    pub parse_ms: u64,
    /// Fixed "today" for deterministic tests; `None` means the real clock.
    pub today_override: Option<Day>,
}

impl AppState {
    pub fn new(ledger: Ledger, parse_ms: u64) -> Self {
        Self {
            ledger,
            parse_ms,
            today_override: None,
        }
    }

    pub fn with_today(mut self, today: Day) -> Self {
        self.today_override = Some(today);
        self
    }

    pub fn today(&self) -> Day {
        self.today_override.unwrap_or_else(today_utc)
    }
}

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/api/summary", get(summary))
        .route("/api/month/{month}", get(month_view))
        .route("/api/category/{account}/{month}", get(category_view))
        .fallback(not_found)
        .with_state(state)
}

type ApiError = (StatusCode, Json<Value>);

fn err(status: StatusCode, message: &str) -> ApiError {
    (status, Json(json!({ "error": message })))
}

async fn not_found() -> ApiError {
    err(StatusCode::NOT_FOUND, "not found")
}

async fn summary(State(state): State<Arc<AppState>>) -> Json<Value> {
    let ledger = &state.ledger;
    let today = state.today();
    let months: Vec<String> = ledger
        .months_range(today)
        .iter()
        .map(|m| m.to_string())
        .collect();
    Json(json!({
        "title": ledger.title,
        "files": ledger.files.len(),
        "directives": ledger.directives,
        "parse_ms": state.parse_ms,
        "operating_currencies": ledger.operating_currencies,
        "months": months,
        "today": format_day(today),
        "default_month": ledger.default_month(today).to_string(),
    }))
}

async fn month_view(
    State(state): State<Arc<AppState>>,
    Path(month): Path<String>,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Json<Value>, ApiError> {
    let month = parse_month(&month)?;
    let (basis, cur) = params(&state, &query)?;
    let view = state.ledger.month_view(month, basis, &cur);

    let today = state.today();
    let today_month = MonthKey::new(today.0, today.1);
    let is_current = month == today_month;
    // How far through the month we are: complete for past months, zero
    // for future ones, so the UI can pace "vs typical" bars.
    let day = if is_current {
        today.2
    } else if month < today_month {
        month.days_in_month()
    } else {
        0
    };

    let budget: Vec<Value> =
        view.budget_accounts.iter().map(account_json).collect();
    let tracking: Vec<Value> =
        view.tracking_accounts.iter().map(account_json).collect();
    Ok(Json(json!({
        "month": month.to_string(),
        "is_current": is_current,
        "day": day,
        "days_in_month": month.days_in_month(),
        "income": num(view.income),
        "spent": num(view.spent),
        "typical": opt_num(view.typical),
        "groups": view.groups.iter().map(group_json).collect::<Vec<_>>(),
        "accounts": { "budget": budget, "tracking": tracking },
    })))
}

async fn category_view(
    State(state): State<Arc<AppState>>,
    Path((account, month)): Path<(String, String)>,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Json<Value>, ApiError> {
    let month = parse_month(&month)?;
    let (basis, cur) = params(&state, &query)?;
    let Some(view) = state.ledger.category_view(&account, month, basis, &cur)
    else {
        return Err(err(StatusCode::NOT_FOUND, "unknown category"));
    };

    let at = month.end_of_month();
    let history: Vec<Value> = view
        .history
        .iter()
        .map(|p| json!({ "month": p.month.to_string(), "spent": num(p.spent) }))
        .collect();
    let txns: Vec<Value> = view
        .txns
        .iter()
        .map(|t| txn_json(&state.ledger, t, &view.account, &cur, at))
        .collect();
    Ok(Json(json!({
        "account": view.account,
        "label": view.label,
        "spent": num(view.spent),
        "avg": opt_num(view.avg),
        "ratio": ratio_json(view.ratio),
        "status": view.status.map_or(Value::Null, |s| json!(s.as_str())),
        "window": view.window.map_or(Value::Null, |(from, to)| {
            json!([from.to_string(), to.to_string()])
        }),
        "history": history,
        "split": amounts_json(&view.split),
        "txns": txns,
    })))
}

fn group_json(group: &Group) -> Value {
    let categories: Vec<Value> =
        group.categories.iter().map(category_json).collect();
    json!({
        "name": group.name,
        "spent": num(group.spent),
        "avg": opt_num(group.avg),
        "categories": categories,
    })
}

fn category_json(row: &CategoryRow) -> Value {
    json!({
        "account": row.account,
        "label": row.label,
        "spent": num(row.spent),
        "avg": opt_num(row.avg),
        "ratio": ratio_json(row.ratio),
        "status": row.status.map_or(Value::Null, |s| json!(s.as_str())),
        "split": amounts_json(&row.split),
    })
}

fn account_json(row: &AccountRow) -> Value {
    json!({
        "account": row.account,
        "label": row.label,
        "balances": amounts_json(&row.balances),
        "converted": opt_num(row.converted),
    })
}

fn txn_json(
    ledger: &Ledger,
    txn: &Txn,
    account: &str,
    cur: &str,
    at: Day,
) -> Value {
    // The transaction's effect on this category, per currency.
    let mut sums: Vec<(String, Decimal)> = Vec::new();
    for posting in &txn.postings {
        if posting.account != account {
            continue;
        }
        for (value, currency) in &posting.amounts {
            match sums.iter_mut().find(|(c, _)| c == currency) {
                Some((_, total)) => *total += *value,
                None => sums.push((currency.clone(), *value)),
            }
        }
    }
    let converted: Vec<Decimal> = sums
        .iter()
        .filter_map(|(c, v)| ledger.convert(*v, c, cur, at))
        .collect();

    let postings: Vec<Value> = txn
        .postings
        .iter()
        .flat_map(|posting| {
            if posting.amounts.is_empty() {
                vec![json!({
                    "account": posting.account,
                    "amount": Value::Null,
                    "currency": Value::Null,
                })]
            } else {
                posting
                    .amounts
                    .iter()
                    .map(|(value, currency)| {
                        json!({
                            "account": posting.account,
                            "amount": num(*value),
                            "currency": currency,
                        })
                    })
                    .collect()
            }
        })
        .collect();

    let mut meta = Map::new();
    for (key, value) in &txn.meta {
        meta.insert(key.clone(), Value::String(value.clone()));
    }

    json!({
        "date": format_day(txn.date),
        "flag": txn.flag.to_string(),
        "payee": txn.payee,
        "narration": txn.narration,
        "tags": txn.tags,
        "links": txn.links,
        "meta": meta,
        "amount": sums.first().map_or(Value::Null, |(_, v)| num(*v)),
        "currency": sums.first().map_or(Value::Null, |(c, _)| json!(c)),
        "converted": converted
            .split_first()
            .map_or(Value::Null, |(first, rest)| {
                num(rest.iter().fold(*first, |acc, v| acc + v))
            }),
        "postings": postings,
    })
}

fn parse_month(s: &str) -> Result<MonthKey, ApiError> {
    MonthKey::parse(s)
        .ok_or_else(|| err(StatusCode::BAD_REQUEST, "month must be YYYY-MM"))
}

fn params(
    state: &AppState,
    query: &HashMap<String, String>,
) -> Result<(u32, String), ApiError> {
    let basis = match query.get("basis").map(String::as_str) {
        None => 6,
        Some("3") => 3,
        Some("6") => 6,
        Some("12") => 12,
        Some(_) => {
            return Err(err(
                StatusCode::BAD_REQUEST,
                "basis must be 3, 6 or 12",
            ));
        }
    };
    let cur = match query.get("cur") {
        None => state
            .ledger
            .operating_currencies
            .first()
            .cloned()
            .unwrap_or_else(|| "USD".to_string()),
        Some(c) if is_commodity(c) => c.clone(),
        Some(_) => {
            return Err(err(
                StatusCode::BAD_REQUEST,
                "cur must be an uppercase commodity code",
            ));
        }
    };
    Ok((basis, cur))
}

/// Beancount commodity: uppercase first letter, then uppercase letters,
/// digits, or `'._-`.
fn is_commodity(s: &str) -> bool {
    let mut chars = s.chars();
    chars.next().is_some_and(|c| c.is_ascii_uppercase())
        && chars.all(|c| {
            c.is_ascii_uppercase() || c.is_ascii_digit() || "'._-".contains(c)
        })
}

/// A per-currency amount list as a `{"CUR": 1.23}` object.
fn amounts_json(amounts: &[(String, Decimal)]) -> Value {
    let mut map = Map::new();
    for (currency, value) in amounts {
        map.insert(currency.clone(), num(*value));
    }
    Value::Object(map)
}

/// Money as JSON: rounded to cents, always a float.
fn num(value: Decimal) -> Value {
    json!(value.round_dp(2).to_f64())
}

fn opt_num(value: Option<Decimal>) -> Value {
    value.map_or(Value::Null, num)
}

fn ratio_json(ratio: Option<Decimal>) -> Value {
    ratio.map_or(Value::Null, |r| json!(r.round_dp(4).to_f64()))
}

fn format_day(day: Day) -> String {
    format!("{:04}-{:02}-{:02}", day.0, day.1, day.2)
}

fn today_utc() -> Day {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    civil_from_days((secs / 86_400) as i64)
}

/// Gregorian date from days since 1970-01-01 (Hinnant's algorithm).
fn civil_from_days(days: i64) -> Day {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u8;
    let month = (if mp < 10 { mp + 3 } else { mp - 9 }) as u8;
    let year = yoe + era * 400 + i64::from(month <= 2);
    (year as u16, month, day)
}

#[cfg(test)]
mod tests {
    use super::civil_from_days;

    #[test]
    fn civil_from_days_matches_known_dates() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(20_686), (2026, 8, 21));
        // A leap day.
        assert_eq!(civil_from_days(19_782), (2024, 2, 29));
    }
}
