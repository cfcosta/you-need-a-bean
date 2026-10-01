//! Page 1: where the money stands today.

use bean_core::model::{Day, Ledger, MonthKey};
use rust_decimal::{Decimal, prelude::ToPrimitive};
use serde_json::Value;

use super::{add_days, add_months, day, dec, text};

#[derive(Clone, Debug)]
pub struct Runway {
    pub months: Decimal,
    pub burn: Decimal,
    /// The month the free cash is spent at `burn`.
    pub until: MonthKey,
}

#[derive(Clone, Debug)]
pub struct Holding {
    pub currency: String,
    pub units: Decimal,
    pub value: Decimal,
    pub ret: Option<Decimal>,
}

#[derive(Clone, Debug)]
pub struct Forecast {
    pub days: u32,
    /// The balance at the end of each day, today first, `days + 1` long.
    pub daily: Vec<Decimal>,
    pub end: Decimal,
    pub low: Decimal,
}

#[derive(Clone, Debug)]
pub struct Event {
    pub date: Day,
    pub label: String,
    pub account: String,
    pub amount: Decimal,
    /// Scheduled (`*`) rather than estimated (`~`).
    pub scheduled: bool,
    /// The forecast balance once this event has landed.
    pub balance: Decimal,
}

#[derive(Clone, Debug)]
pub struct Goal {
    pub label: String,
    pub funded: Decimal,
    pub target: Decimal,
    pub date: Day,
}

impl Goal {
    pub fn ratio(&self) -> Decimal {
        if self.target.is_zero() {
            Decimal::ZERO
        } else {
            self.funded / self.target
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReviewKind {
    Flagged,
    Category,
    Other,
}

#[derive(Clone, Debug)]
pub struct Review {
    pub label: String,
    pub kind: ReviewKind,
}

#[derive(Clone, Debug)]
pub struct Overview {
    pub today: Day,
    pub held: Decimal,
    pub reserved: Decimal,
    pub free: Decimal,
    pub out_30: Decimal,
    pub runway: Option<Runway>,
    pub assets: Decimal,
    pub liabilities: Decimal,
    pub net_worth: Decimal,
    pub history: Vec<(MonthKey, Decimal)>,
    pub saved_month: Decimal,
    pub savings_rate: Option<Decimal>,
    pub holdings: Vec<Holding>,
    /// Progress towards the 4% number, and the number.
    pub independence: Option<(Decimal, Decimal)>,
    /// 30, 60 and 90 days ahead.
    pub forecast: Vec<Forecast>,
    pub events: Vec<Event>,
    pub recurring: usize,
    pub goals: Vec<Goal>,
    pub review: Vec<Review>,
    /// Accounts with fresh source coverage, of how many.
    pub coverage: (u64, u64),
}

impl Overview {
    pub fn build(ledger: &Ledger, today: Day, cur: &str) -> Self {
        let home = ledger.home_view(today, cur);
        let reports = ledger.reports_view(today, 6, cur);

        let runway = home["runway"]["months"].as_f64().map(|_| {
            let months = dec(&home["runway"]["months"]);
            let whole = months.trunc().to_u32().unwrap_or(0);
            Runway {
                months,
                burn: dec(&home["runway"]["monthly_spend"]),
                until: add_months(MonthKey::new(today.0, today.1), whole),
            }
        });

        let history = home["history"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|p| {
                Some((MonthKey::parse(p["month"].as_str()?)?, dec(&p["net"])))
            })
            .collect();

        let forecast: Vec<Forecast> = ["30", "60", "90"]
            .iter()
            .map(|h| forecast(&home["forecast"][*h], today, h.parse().unwrap()))
            .collect();

        let points = &home["forecast"]["90"]["points"];
        let events = home["events"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|e| {
                let date = day(&e["date"])?;
                Some(Event {
                    date,
                    label: text(&e["label"]),
                    account: text(&e["account"]),
                    amount: dec(&e["amount"]),
                    scheduled: e["kind"] == "scheduled",
                    balance: balance_on(points, date),
                })
            })
            .collect();

        let goals = home["goals"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|g| {
                Some(Goal {
                    label: text(&g["label"]),
                    funded: dec(&g["funded"]),
                    target: dec(&g["target"]),
                    date: day(&g["date"])?,
                })
            })
            .collect();

        let review = home["attention"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|a| a["kind"] != "coverage")
            .map(|a| Review {
                label: text(&a["label"]),
                kind: match a["kind"].as_str() {
                    Some("flagged") => ReviewKind::Flagged,
                    Some("category") => ReviewKind::Category,
                    _ => ReviewKind::Other,
                },
            })
            .collect();

        let fire = &reports.fire;
        let spend = fire.monthly_spend;
        let saved = fire.monthly_savings;
        let savings_rate =
            (saved + spend > Decimal::ZERO).then(|| saved / (saved + spend));

        Self {
            today,
            held: dec(&home["cash"]),
            reserved: dec(&home["reserved"]),
            free: dec(&home["available"]),
            out_30: dec(&home["upcoming_out"]),
            runway,
            assets: dec(&home["assets"]),
            liabilities: dec(&home["owed"]),
            net_worth: dec(&home["net_worth"]),
            history,
            saved_month: saved,
            savings_rate,
            holdings: reports
                .investments
                .items
                .iter()
                .map(|p| Holding {
                    currency: p.currency.clone(),
                    units: p.units,
                    value: p.value,
                    ret: p.ret,
                })
                .collect(),
            independence: fire.progress.map(|p| (p, fire.fire_number)),
            forecast,
            events,
            recurring: home["recurring"].as_array().map_or(0, Vec::len),
            goals,
            review,
            coverage: (
                home["coverage"]["current"].as_u64().unwrap_or(0),
                home["coverage"]["total"].as_u64().unwrap_or(0),
            ),
        }
    }
}

/// The step balance after everything dated on or before `date`.
fn balance_on(points: &Value, date: Day) -> Decimal {
    points
        .as_array()
        .into_iter()
        .flatten()
        .take_while(|p| day(&p["date"]).is_some_and(|d| d <= date))
        .last()
        .map(|p| dec(&p["balance"]))
        .unwrap_or_default()
}

fn forecast(h: &Value, today: Day, days: u32) -> Forecast {
    let daily = (0..=i64::from(days))
        .map(|i| balance_on(&h["points"], add_days(today, i)))
        .collect();
    Forecast {
        days,
        daily,
        end: dec(&h["balance"]),
        low: dec(&h["low"]),
    }
}
