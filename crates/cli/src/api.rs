//! The read-only JSON API over a loaded [`Ledger`].
//!
//! Three endpoints drive the whole UI: `/api/summary` (ledger shape and
//! month range), `/api/month/{month}` (the monthly budget page), and
//! `/api/category/{account}/{month}` (the inspector drill-down).

use std::collections::HashMap;
use std::sync::{Arc, PoisonError, RwLock, RwLockReadGuard, RwLockWriteGuard};
use std::time::{SystemTime, UNIX_EPOCH};

use axum::extract::{Path, Query, State};
use axum::http::{StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use bean_core::model::{Day, Ledger, MonthKey, Txn};
use bean_core::query::{AccountRow, CategoryRow, Group};
use bean_core::reports::FireScenario;
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use serde_json::{Map, Value, json};

use crate::ui::UiSource;

/// One reading of the ledger from disk. Reloading builds a whole new
/// snapshot and swaps it in, so a request answers entirely from the
/// ledger it started with even if the files change underneath it.
pub struct Snapshot {
    pub ledger: Ledger,
    pub parse_ms: u64,
    /// Counts loads, starting at zero for the one the server booted with.
    /// The UI watches it to know when what it is showing went stale.
    pub revision: u64,
}

/// What the API answers from right now.
struct Current {
    snapshot: Arc<Snapshot>,
    /// Why the last reload attempt failed, if it did — the snapshot above
    /// is still the last ledger that loaded cleanly.
    error: Option<String>,
}

/// Everything a request handler needs. The ledger inside can be replaced
/// while the server runs; everything else is fixed at startup.
pub struct AppState {
    current: RwLock<Current>,
    /// Fixed "today" for deterministic tests; `None` means the real clock.
    pub today_override: Option<Day>,
    pub ui: UiSource,
}

impl AppState {
    pub fn new(ledger: Ledger, parse_ms: u64) -> Self {
        Self {
            current: RwLock::new(Current {
                snapshot: Arc::new(Snapshot {
                    ledger,
                    parse_ms,
                    revision: 0,
                }),
                error: None,
            }),
            today_override: None,
            ui: UiSource::Embedded,
        }
    }

    pub fn with_today(mut self, today: Day) -> Self {
        self.today_override = Some(today);
        self
    }

    pub fn with_ui(mut self, ui: UiSource) -> Self {
        self.ui = ui;
        self
    }

    pub fn today(&self) -> Day {
        self.today_override.unwrap_or_else(today_utc)
    }

    /// The ledger to answer one request from.
    pub fn snapshot(&self) -> Arc<Snapshot> {
        self.read().snapshot.clone()
    }

    pub fn reload_error(&self) -> Option<String> {
        self.read().error.clone()
    }

    /// Answer from this ledger from now on, and forget any earlier
    /// failure.
    pub fn install(&self, ledger: Ledger, parse_ms: u64) -> Arc<Snapshot> {
        let mut current = self.write();
        let snapshot = Arc::new(Snapshot {
            ledger,
            parse_ms,
            revision: current.snapshot.revision + 1,
        });
        current.snapshot = Arc::clone(&snapshot);
        current.error = None;
        snapshot
    }

    /// Keep answering from the ledger we have, and remember why the last
    /// attempt to replace it did not work.
    pub fn reload_failed(&self, message: String) {
        self.write().error = Some(message);
    }

    // A panic can only reach these while an `Arc` is being cloned or a
    // field assigned, so a poisoned lock still holds a usable ledger.
    fn read(&self) -> RwLockReadGuard<'_, Current> {
        self.current.read().unwrap_or_else(PoisonError::into_inner)
    }

    fn write(&self) -> RwLockWriteGuard<'_, Current> {
        self.current.write().unwrap_or_else(PoisonError::into_inner)
    }
}

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/api/summary", get(summary))
        .route("/api/month/{month}", get(month_view))
        .route("/api/category/{account}/{month}", get(category_view))
        .route("/api/reports", get(reports))
        .fallback(fallback)
        .with_state(state)
}

type ApiError = (StatusCode, Json<Value>);

fn err(status: StatusCode, message: &str) -> ApiError {
    (status, Json(json!({ "error": message })))
}

/// Unmatched routes: API misses stay JSON, everything else is the UI.
async fn fallback(State(state): State<Arc<AppState>>, uri: Uri) -> Response {
    let path = uri.path();
    if path == "/api" || path.starts_with("/api/") {
        return err(StatusCode::NOT_FOUND, "not found").into_response();
    }
    state.ui.respond(path)
}

async fn summary(State(state): State<Arc<AppState>>) -> Json<Value> {
    let snapshot = state.snapshot();
    let ledger = &snapshot.ledger;
    let today = state.today();
    let months: Vec<String> = ledger
        .months_range(today)
        .iter()
        .map(|m| m.to_string())
        .collect();
    let root = ledger
        .files
        .first()
        .and_then(|p| p.file_name())
        .map(|n| n.to_string_lossy().into_owned());
    Json(json!({
        "title": ledger.title,
        "root": root,
        "files": ledger.files.len(),
        "directives": ledger.directives,
        "parse_ms": snapshot.parse_ms,
        "operating_currencies": ledger.operating_currencies,
        "months": months,
        "today": format_day(today),
        "default_month": ledger.default_month(today).to_string(),
        // The server follows the files on disk: this counts the times it
        // has reloaded, and holds the reason it last could not.
        "revision": snapshot.revision,
        "reload_error": state.reload_error(),
    }))
}

async fn month_view(
    State(state): State<Arc<AppState>>,
    Path(month): Path<String>,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Json<Value>, ApiError> {
    let month = parse_month(&month)?;
    let snapshot = state.snapshot();
    let (basis, cur) = params(&snapshot, &query)?;
    let view = snapshot.ledger.month_view(month, basis, &cur);

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
    let snapshot = state.snapshot();
    let (basis, cur) = params(&snapshot, &query)?;
    let Some(view) =
        snapshot.ledger.category_view(&account, month, basis, &cur)
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
        .map(|t| txn_json(&snapshot.ledger, t, &view.account, &cur, at))
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

async fn reports(
    State(state): State<Arc<AppState>>,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Json<Value>, ApiError> {
    let snapshot = state.snapshot();
    let (basis, cur) = params(&snapshot, &query)?;
    let view = snapshot.ledger.reports_view(state.today(), basis, &cur);

    let net_worth: Vec<Value> = view
        .net_worth
        .iter()
        .map(|p| {
            json!({
                "month": p.month.to_string(),
                "assets": num(p.assets),
                "cash": num(p.cash),
                "holdings": num(p.holdings),
                "liabilities": num(p.liabilities),
                "net": num(p.net),
            })
        })
        .collect();
    let cashflow: Vec<Value> = view
        .cashflow
        .iter()
        .map(|p| {
            json!({
                "month": p.month.to_string(),
                "income": num(p.income),
                "expenses": num(p.expenses),
                "net": num(p.net),
            })
        })
        .collect();
    let fire = &view.fire;
    let scenarios = scenarios_json(&fire.scenarios);
    let coast = scenarios_json(&fire.coast);
    let steps: Vec<Value> = fire
        .steps
        .iter()
        .map(|s| {
            json!({
                "extra": num(s.extra),
                "months": s.months.map_or(Value::Null, |m| json!(m)),
            })
        })
        .collect();
    let growth: Vec<Value> = view
        .growth
        .points
        .iter()
        .map(|p| {
            json!({
                "month": p.month.to_string(),
                "delta": num(p.delta),
                "saved": num(p.saved),
                "equity": num(p.equity),
                "market": num(p.market),
            })
        })
        .collect();
    // `monthly` is indexed by `year.months` below and always as long,
    // zeroes and all, so the two can be drawn against one axis.
    let year_groups: Vec<Value> = view
        .year
        .groups
        .iter()
        .map(|g| {
            let monthly: Vec<Value> =
                g.monthly.iter().copied().map(num).collect();
            json!({
                "name": g.name,
                "total": num(g.total),
                "prior": opt_num(g.prior),
                "typical": opt_num(g.typical),
                "monthly": monthly,
            })
        })
        .collect();
    let year_months: Vec<Value> = view
        .year
        .months
        .iter()
        .map(|m| {
            json!({
                "month": m.month.to_string(),
                "total": num(m.total),
                "prior": opt_num(m.prior),
            })
        })
        .collect();
    let sources: Vec<Value> = view
        .income
        .sources
        .iter()
        .map(|s| {
            json!({
                "name": s.name,
                "total": num(s.total),
                "share": num(s.share),
                "passive": num(s.passive),
            })
        })
        .collect();
    let projects: Vec<Value> = view
        .projects
        .items
        .iter()
        .map(|p| {
            json!({
                "name": p.name,
                "sigil": p.kind.sigil().to_string(),
                "spent": num(p.spent),
                "income": num(p.income),
                "net": num(p.net),
                "count": p.count,
                "categories": p.categories,
                "first": format_day(p.first),
                "last": format_day(p.last),
                "months": p.months,
            })
        })
        .collect();
    let movers: Vec<Value> = view
        .movers
        .items
        .iter()
        .map(|m| {
            json!({
                "account": m.account,
                "label": m.label,
                "group": m.group,
                "recent": num(m.recent),
                "prior": num(m.prior),
                "delta": num(m.delta),
                "ratio": ratio_json(m.ratio),
            })
        })
        .collect();
    let season: Vec<Value> = view
        .season
        .months
        .iter()
        .map(|p| {
            json!({
                "month": p.month,
                "median": num(p.median),
                // A share of a year at cents precision: 6.25% of it
                // and 6% of it are different months.
                "share": ratio_json(Some(p.share)),
                "samples": p.samples,
                "actual": opt_num(p.actual),
            })
        })
        .collect();
    let payees: Vec<Value> = view
        .payees
        .items
        .iter()
        .map(|p| {
            json!({
                "name": p.name,
                "spent": num(p.spent),
                "count": p.count,
                "average": num(p.average),
                "share": ratio_json(Some(p.share)),
                "categories": p.categories,
                "months": p.months,
                "first": format_day(p.first),
                "last": format_day(p.last),
            })
        })
        .collect();
    let stale: Vec<Value> = view
        .trust
        .stale
        .iter()
        .map(|s| {
            json!({
                "commodity": s.commodity,
                "last": format_day(s.last),
                "days": s.days,
                "value": num(s.value),
            })
        })
        .collect();
    let flagged: Vec<Value> = view
        .trust
        .flagged
        .recent
        .iter()
        .map(|t| {
            json!({
                "date": format_day(t.date),
                "payee": t.payee,
                "narration": t.narration,
                "amount": num(t.amount),
            })
        })
        .collect();

    Ok(Json(json!({
        "month": view.month.to_string(),
        "net_worth": net_worth,
        "cashflow": cashflow,
        "fire": {
            "window": window_json(fire.window),
            "monthly_spend": num(fire.monthly_spend),
            "annual_spend": num(fire.annual_spend),
            "fire_number": num(fire.fire_number),
            "net_worth": num(fire.net_worth),
            "progress": ratio_json(fire.progress),
            "monthly_savings": num(fire.monthly_savings),
            "swr_monthly": num(fire.swr_monthly),
            "scenarios": scenarios,
            "coast": coast,
            "steps": steps,
            "lean_number": opt_num(fire.lean_number),
            "lean_progress": ratio_json(fire.lean_progress),
        },
        "runway": {
            "liquid": num(view.runway.liquid),
            "months": opt_num(view.runway.months),
            "lean_months": opt_num(view.runway.lean_months),
        },
        "growth": {
            "window": window_json(view.growth.window),
            "saved": num(view.growth.saved),
            "equity": num(view.growth.equity),
            "market": num(view.growth.market),
            "delta": num(view.growth.delta),
            "implied_return": ratio_json(view.growth.implied_return),
            "unpriced": view.growth.unpriced,
            "points": growth,
        },
        // Only the fixed nut, not the charges behind it: the page
        // reads it inside the lean FIRE target and the lean runway,
        // and never as a report of its own.
        "recurring": {
            "window": window_json(view.recurring.window),
            "monthly_fixed": num(view.recurring.monthly_fixed),
            "annual_fixed": num(view.recurring.annual_fixed),
            "coverage": ratio_json(view.recurring.coverage),
        },
        "unpriced": view.unpriced,
        "year": {
            "window": window_json(view.year.window),
            "prior_window": window_json(view.year.prior_window),
            "months": year_months,
            "typical": opt_num(view.year.typical),
            "groups": year_groups,
        },
        "income": {
            "window": window_json(view.income.window),
            "total": num(view.income.total),
            "sources": sources,
            "effective_sources": opt_num(view.income.effective_sources),
            "passive": num(view.income.passive),
            "passive_share": ratio_json(view.income.passive_share),
            "passive_cover": ratio_json(view.income.passive_cover),
            "declared": view.income.declared,
            "inferred": view.income.inferred,
        },
        "projects": {
            "items": projects,
            "singletons": view.projects.singletons,
            "markers": view.projects.markers,
        },
        "movers": {
            "recent": window_json(view.movers.recent),
            "prior": window_json(view.movers.prior),
            "recent_total": num(view.movers.recent_total),
            "prior_total": num(view.movers.prior_total),
            "items": movers,
        },
        "season": {
            "years": view.season.years.map_or(Value::Null, |(from, to)| {
                json!([from, to])
            }),
            "months": season,
            "typical": num(view.season.typical),
            "year": view.season.year,
            "elapsed": view.season.elapsed,
            "ytd": num(view.season.ytd),
            "pace": ratio_json(view.season.pace),
            "projected": opt_num(view.season.projected),
        },
        "payees": {
            "window": window_json(view.payees.window),
            "items": payees,
            "total": num(view.payees.total),
            "others": view.payees.others,
            "others_spent": num(view.payees.others_spent),
            "anonymous": num(view.payees.anonymous),
            "anonymous_count": view.payees.anonymous_count,
        },
        "trust": {
            "window": window_json(view.trust.window),
            "stale": stale,
            "flagged": {
                "total": view.trust.flagged.total,
                "window": view.trust.flagged.window,
                "amount": num(view.trust.flagged.amount),
                "recent": flagged,
            },
            "uncategorized": {
                "total": num(view.trust.uncategorized.total),
                "share": ratio_json(view.trust.uncategorized.share),
                "accounts": view.trust.uncategorized.accounts,
            },
            "warnings": view.trust.warnings,
        },
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
    snapshot: &Snapshot,
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
        None => snapshot
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
fn scenarios_json(scenarios: &[FireScenario]) -> Vec<Value> {
    scenarios
        .iter()
        .map(|s| {
            json!({
                "rate": s.rate,
                "months": s.months.map_or(Value::Null, |m| json!(m)),
            })
        })
        .collect()
}

/// A trailing window as `[from, to]`, or null when there is none.
fn window_json(window: Option<(MonthKey, MonthKey)>) -> Value {
    window.map_or(Value::Null, |(from, to)| {
        json!([from.to_string(), to.to_string()])
    })
}

fn num(value: Decimal) -> Value {
    json!(value.round_dp(2).to_f64())
}

fn opt_num(value: Option<Decimal>) -> Value {
    value.map_or(Value::Null, num)
}

/// A share as JSON.
///
/// Four decimal places is a hundredth of a percent: enough for a share
/// you can see on a bar, and far too coarse for one you cannot. Every
/// share smaller than that would arrive as the same number, so a line
/// carrying two of them would read as though it said one thing twice.
/// Below a percent the digits the display needs to tell them apart
/// survive the trip.
fn ratio_json(ratio: Option<Decimal>) -> Value {
    ratio.map_or(Value::Null, |r| {
        let dp = if r.abs() < Decimal::new(1, 2) { 8 } else { 4 };
        json!(r.round_dp(dp).to_f64())
    })
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
    use serde_json::json;

    use super::*;

    #[test]
    fn civil_from_days_matches_known_dates() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(20_686), (2026, 8, 21));
        // A leap day.
        assert_eq!(civil_from_days(19_782), (2024, 2, 29));
    }

    #[test]
    fn ratio_json_keeps_small_shares_apart() {
        // Two fabricated sub-percent shares that coarse rounding must
        // keep distinct.
        let a = ratio_json(Some(Decimal::new(123, 6)));
        let b = ratio_json(Some(Decimal::new(456, 6)));
        assert_eq!(a, json!(0.000123));
        assert_ne!(a, b);
    }

    #[test]
    fn ratio_json_rounds_a_visible_share_to_a_hundredth_of_a_percent() {
        let r = ratio_json(Some(Decimal::new(123_456_789, 9)));
        assert_eq!(r, json!(0.1235));
        assert_eq!(ratio_json(None), Value::Null);
    }
}
