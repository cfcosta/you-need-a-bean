//! The financial home: facts today, explicit commitments, and visible uncertainty.
//! All arithmetic stays decimal until the presentation boundary.
use crate::date::{add_days, add_months_clamped, days_between};
use crate::model::{AccountInfo, AccountKind, Day, Ledger, MonthKey, Txn};
use rust_decimal::{Decimal, prelude::ToPrimitive};
use serde_json::{Value, json};
use std::collections::BTreeSet;

pub fn date(d: Day) -> String {
    format!("{:04}-{:02}-{:02}", d.0, d.1, d.2)
}
pub fn parse_date(s: &str) -> Option<Day> {
    if s.len() != 10 || !s.is_ascii() || &s[7..8] != "-" {
        return None;
    }
    let m = MonthKey::parse(&s[..7])?;
    let d = s[8..].parse::<u8>().ok()?;
    (d > 0 && d <= m.days_in_month()).then_some((m.year, m.month, d))
}
fn cash_account(a: &AccountInfo) -> bool {
    a.account.starts_with("Assets:")
        && a.purpose.liquid()
        && (a.kind == AccountKind::Budget || a.purpose.declared)
}
fn num(d: Decimal) -> f64 {
    d.round_dp(2).to_f64().unwrap_or_default()
}
#[derive(Clone)]
struct Event {
    day: Day,
    amount: Decimal,
    label: String,
    account: String,
    kind: &'static str,
}
fn event_value(e: &Event) -> Value {
    json!({"date": date(e.day), "amount": num(e.amount), "label": e.label, "account": e.account, "kind": e.kind})
}

fn cash_delta(
    l: &Ledger,
    txn: &Txn,
    cur: &str,
    prices: &Ledger,
    today: Day,
) -> Option<Decimal> {
    let mut total = Decimal::ZERO;
    for p in &txn.postings {
        if !l.account(&p.account).is_some_and(cash_account) {
            continue;
        }
        for (v, c) in &p.amounts {
            total += prices.convert(*v, c, cur, today)?;
        }
    }
    Some(total)
}

impl Ledger {
    pub fn home_view(&self, today: Day, cur: &str) -> Value {
        let l = self.as_of(today);
        let current = MonthKey::new(today.0, today.1);
        let reports = l.reports_view(today, 6, cur);
        let debts = l.liabilities_view(today, 6, cur);
        let mut accounts = Vec::new();
        let mut goals = Vec::new();
        let mut attention = Vec::new();
        let mut missing = BTreeSet::new();
        let (mut assets, mut owed, mut cash, mut reserved) =
            (Decimal::ZERO, Decimal::ZERO, Decimal::ZERO, Decimal::ZERO);
        let reserve_cur = l
            .operating_currencies
            .first()
            .map(String::as_str)
            .unwrap_or(cur);
        let mut current_accounts = 0;
        for a in l.accounts().filter(|a| {
            a.account.starts_with("Assets:")
                || a.account.starts_with("Liabilities:")
        }) {
            let balances = l.balance_at(&a.account, current);
            if balances.iter().all(|(_, v)| v.is_zero()) && a.closed.is_some() {
                continue;
            }
            let mut value = Decimal::ZERO;
            let mut priced = true;
            for (c, v) in &balances {
                if v.is_zero() {
                    continue;
                }
                if let Some(n) = l.convert(*v, c, cur, today) {
                    value += n;
                } else {
                    missing.insert(c.clone());
                    priced = false;
                }
            }
            if a.account.starts_with("Assets:") {
                assets += value;
            } else {
                owed -= value;
            }
            let is_cash = cash_account(a);
            let reserve = l.convert(a.purpose.reserve, reserve_cur, cur, today);
            if is_cash {
                cash += value;
                reserved += reserve.unwrap_or_default();
                if reserve.is_none() && !a.purpose.reserve.is_zero() {
                    missing.insert(reserve_cur.to_string());
                }
            }
            let assertion = l
                .audit
                .balances
                .iter()
                .rev()
                .find(|b| b.account == a.account && b.date <= today);
            let updated = a
                .purpose
                .updated
                .as_deref()
                .and_then(parse_date)
                .filter(|d| *d <= today);
            let checked = assertion.map(|b| b.date);
            let coverage = updated.or(checked);
            let age = coverage.map(|d| days_between(d, today));
            let fresh = age.is_some_and(|d| d <= 7)
                && assertion.is_none_or(|b| b.passed);
            if fresh {
                current_accounts += 1;
            }
            let last_txn = l
                .txns
                .iter()
                .filter(|t| t.postings.iter().any(|p| p.account == a.account))
                .map(|t| t.date)
                .max();
            accounts.push(json!({"account": a.account, "label": a.label,
                "liquidity": if is_cash { "cash" } else { &a.purpose.liquidity }, "declared": a.purpose.declared,
                "value": priced.then(|| num(value)), "native": balances.iter().map(|(c,v)| json!({"currency":c,"amount":num(*v)})).collect::<Vec<_>>(),
                "cash": is_cash, "reserve": reserve.map(num), "updated": updated.map(date), "checked": checked.map(date),
                "assertion_passed": assertion.map(|b| b.passed), "age": age, "fresh": fresh, "last_transaction": last_txn.map(date)}));
            if !fresh && balances.iter().any(|(_, v)| !v.is_zero()) {
                attention.push(json!({"kind":"coverage", "label": a.label, "detail": match age {
                    Some(n) => format!("Source coverage is {n} days old"), None => "Source coverage has not been declared".into()
                }, "account":a.account, "source":assertion.and_then(|b| b.source.as_ref())}));
            }
            if let Some(target) = a
                .purpose
                .goal
                .filter(|g| *g > Decimal::ZERO)
                .and_then(|g| l.convert(g, reserve_cur, cur, today))
            {
                goals.push(json!({"account":a.account,"label":a.label,"funded":priced.then(||num(value.max(Decimal::ZERO))),"target":num(target),"date":a.purpose.goal_date,"reserved":reserve.map(num)}));
            }
        }
        for i in &l.audit.issues {
            attention.insert(0,json!({"kind":"accounting","label":"Accounting needs review","detail":i.message,"account":i.account,"source":i.source}));
        }
        for c in &missing {
            attention.insert(0,json!({"kind":"price","label":format!("{c} has no price"),"detail":format!("Some money is missing from {cur} totals"),"account":null}));
        }
        for s in &reports.trust.stale {
            attention.push(json!({"kind":"price","label":format!("{} price is {} days old",s.commodity,s.days),"detail":format!("Last valued {}", date(s.last)),"account":null}));
        }
        for t in l.txns.iter().filter(|t| t.flag == '!') {
            attention.push(json!({"kind":"flagged","label":t.payee.as_deref().or(t.narration.as_deref()).unwrap_or("Unconfirmed transaction"),"detail":format!("{} · marked for review",date(t.date)),"account":t.postings.first().map(|p|&p.account),"source":t.source}));
        }
        for a in &reports.trust.uncategorized.accounts {
            attention.push(json!({"kind":"category","label":"Uncategorized spending","detail":a,"account":a}));
        }
        for n in &debts.notices {
            attention.push(json!({"kind":"debt","label":n.label,"detail":"Review this account on the liabilities page","account":n.account}));
        }
        let mut events = Vec::<Event>::new();
        let mut forecast_priced = true;
        for t in self
            .txns
            .iter()
            .filter(|t| t.date > today && days_between(today, t.date) <= 90)
        {
            match cash_delta(self, t, cur, &l, today) {
                Some(v) if !v.is_zero() => {
                    let account = t
                        .postings
                        .iter()
                        .find(|p| {
                            !self.account(&p.account).is_some_and(cash_account)
                        })
                        .or_else(|| t.postings.first())
                        .map(|p| p.account.clone())
                        .unwrap_or_default();
                    events.push(Event {
                        day: t.date,
                        amount: v,
                        label: t
                            .payee
                            .clone()
                            .or(t.narration.clone())
                            .unwrap_or("Scheduled transaction".into()),
                        account,
                        kind: "scheduled",
                    });
                }
                None => forecast_priced = false,
                _ => {}
            }
        }
        let mut recurring = Vec::new();
        for r in &reports.recurring.items {
            recurring.push(json!({"account":r.account,"label":r.payee,"cadence":r.cadence.label(),"amount":num(r.amount),"monthly":num(r.monthly),"active":r.active,"last":date(r.last),"change":r.change.map(|c|json!({"from":num(c.from),"to":num(c.to),"annual":num(c.annual)}))}));
            if !r.active {
                continue;
            }
            // Card charges are paid through the debt schedule, never twice.
            let direct = l.txns.iter().any(|t| {
                t.date == r.last
                    && t.payee.as_deref() == Some(r.payee.as_str())
                    && t.postings.iter().any(|p| p.account == r.account)
                    && !t
                        .postings
                        .iter()
                        .any(|p| p.account.starts_with("Liabilities:"))
                    && t.postings.iter().any(|p| {
                        l.account(&p.account).is_some_and(cash_account)
                    })
            });
            if !direct {
                continue;
            }
            for occurrence in 1..=110 {
                use crate::reports::Cadence;
                let day = match r.cadence {
                    Cadence::Monthly => add_months_clamped(r.last, occurrence),
                    Cadence::Quarterly => {
                        add_months_clamped(r.last, occurrence * 3)
                    }
                    Cadence::Yearly => {
                        add_months_clamped(r.last, occurrence * 12)
                    }
                    Cadence::Weekly => {
                        add_days(r.last, i64::from(occurrence) * 7)
                    }
                    Cadence::Biweekly => {
                        add_days(r.last, i64::from(occurrence) * 14)
                    }
                };
                if days_between(today, day) > 90 {
                    break;
                }
                if day <= today {
                    continue;
                }
                let booked = events.iter().any(|e| {
                    e.kind == "scheduled"
                        && e.account == r.account
                        && e.label == r.payee
                        && days_between(e.day, day).abs() <= 7
                });
                if !booked {
                    events.push(Event {
                        day,
                        amount: -r.amount,
                        label: r.payee.clone(),
                        account: r.account.clone(),
                        kind: "estimated",
                    });
                }
            }
        }
        for u in &debts.upcoming {
            let debt = debts.debts.iter().find(|d| d.account == u.account);
            let installment = debt.is_some_and(|d| {
                d.kind == crate::reports::DebtKind::Installment
            });
            let mut remaining =
                debt.map(|d| d.owed).unwrap_or_default().max(Decimal::ZERO);
            for month in 0..=3 {
                let day = add_months_clamped(u.date, month);
                if day < today || days_between(today, day) > 90 {
                    continue;
                }
                if installment && remaining.is_zero() {
                    break;
                }
                if installment {
                    remaining += (remaining
                        * debt.and_then(|d| d.rate).unwrap_or_default()
                        / Decimal::from(12))
                    .round_dp(2);
                }
                let booked = self.txns.iter().any(|t| {
                    t.date > today
                        && MonthKey::new(t.date.0, t.date.1)
                            == MonthKey::new(day.0, day.1)
                        && t.postings.iter().any(|p| p.account == u.account)
                        && cash_delta(self, t, cur, &l, today)
                            .is_some_and(|v| v < Decimal::ZERO)
                });
                if booked && installment {
                    let paid: Decimal = self
                        .txns
                        .iter()
                        .filter(|t| {
                            t.date > today
                                && MonthKey::new(t.date.0, t.date.1)
                                    == MonthKey::new(day.0, day.1)
                        })
                        .flat_map(|t| &t.postings)
                        .filter(|p| p.account == u.account)
                        .flat_map(|p| &p.amounts)
                        .filter_map(|(v, c)| l.convert(*v, c, cur, today))
                        .filter(|v| *v > Decimal::ZERO)
                        .sum();
                    remaining = (remaining - paid).max(Decimal::ZERO);
                }
                if !booked {
                    let amount = if installment {
                        u.amount.min(remaining)
                    } else {
                        u.amount
                    };
                    if installment {
                        remaining = (remaining - amount).max(Decimal::ZERO);
                    }
                    events.push(Event {
                        day,
                        amount: -amount,
                        label: u.label.clone(),
                        account: u.account.clone(),
                        kind: "estimated",
                    });
                }
            }
        }
        for debt in debts.debts.iter().filter(|d| {
            d.owed > Decimal::ZERO
                && (d.next_due.is_none() || d.payment.is_none())
        }) {
            attention.push(json!({"kind":"debt","label":format!("{} needs a payment schedule",debt.label),"detail":"The forecast cannot account for every payment without a due date and payment history","account":debt.account}));
        }
        if !forecast_priced {
            attention.push(json!({"kind":"price","label":"Scheduled money has no price","detail":"The forecast is paused until future currency amounts can be converted","account":null}));
        }
        events.sort_by(|a, b| a.day.cmp(&b.day).then(a.amount.cmp(&b.amount)));
        let trustworthy = l.audit.issues.is_empty() && missing.is_empty();
        let available = cash - reserved;
        // Use the same cash basis as the overview, including explicitly liquid
        // tracking accounts. Historical prices must also cover the spend basis.
        let runway_status = if !trustworthy || !reports.unpriced.is_empty() {
            "needs_review"
        } else if reports.fire.monthly_spend <= Decimal::ZERO {
            "no_baseline"
        } else {
            "ready"
        };
        let runway = json!({
            "status": runway_status,
            "months": (runway_status == "ready").then(|| num(
                (available.max(Decimal::ZERO) / reports.fire.monthly_spend).round_dp(1)
            )),
            "monthly_spend": reports.unpriced.is_empty().then(|| num(reports.fire.monthly_spend)),
            "window": reports.fire.window.map(|(start, end)| [start.to_string(), end.to_string()]),
        });
        let mut forecast = serde_json::Map::new();
        for horizon in [30, 60, 90] {
            let mut balance = available;
            let mut low = available;
            let mut points =
                vec![json!({"date":date(today),"balance":num(balance)})];
            for e in events
                .iter()
                .filter(|e| days_between(today, e.day) <= horizon)
            {
                balance += e.amount;
                low = low.min(balance);
                points.push(json!({"date":date(e.day),"balance":num(balance)}));
            }
            forecast.insert(horizon.to_string(),json!({"balance":(trustworthy && forecast_priced).then(||num(balance)),"low":(trustworthy && forecast_priced).then(||num(low)),"points":if trustworthy && forecast_priced { points } else { vec![] }}));
        }
        let scheduled_in: Decimal = events
            .iter()
            .filter(|e| {
                e.amount > Decimal::ZERO && days_between(today, e.day) <= 30
            })
            .map(|e| e.amount)
            .sum();
        let scheduled_out: Decimal = events
            .iter()
            .filter(|e| {
                e.amount < Decimal::ZERO && days_between(today, e.day) <= 30
            })
            .map(|e| -e.amount)
            .sum();
        json!({"today":date(today),"currency":cur,"assets":num(assets),"owed":num(owed),"net_worth":num(assets-owed),
            "cash":num(cash),"reserved":num(reserved),"available":trustworthy.then(||num(available)),"accounts":accounts,
            "coverage":{"current":current_accounts,"total":accounts.len()},"goals":goals,"attention":attention,"unpriced":missing,
            "forecast":forecast,"forecast_priced":forecast_priced,"events":events.iter().map(event_value).collect::<Vec<_>>(),
            "upcoming_in":num(scheduled_in),"upcoming_out":num(scheduled_out),"recurring":recurring,
            "monthly_spend":num(reports.fire.monthly_spend),"monthly_saved":num(reports.fire.monthly_savings),"runway":runway,
            "history":reports.net_worth.iter().map(|p|json!({"month":p.month.to_string(),"net":num(p.net)})).collect::<Vec<_>>()})
    }
}
