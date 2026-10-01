//! Period price performance of non-operating commodities, across all asset
//! accounts. Cash, income, taxes and fees are outside this boundary. Unit
//! additions (including rewards) are flows, not price gains. Quote fallbacks
//! make this an estimate when execution prices are absent.
use crate::model::{Day, Ledger, MonthKey, days_between};
use crate::query::cents;
use rust_decimal::Decimal;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone)]
pub struct PerformancePoint {
    pub date: Day,
    pub value: Option<Decimal>,
    pub net_flows: Option<Decimal>,
    pub gain: Option<Decimal>,
}
#[derive(Debug, Clone)]
pub struct PerformanceHolding {
    pub currency: String,
    pub label: String,
    pub opening: Option<Decimal>,
    pub closing: Option<Decimal>,
    pub net_flows: Option<Decimal>,
    pub gain: Option<Decimal>,
    pub ret: Option<Decimal>,
}
#[derive(Debug, Clone)]
pub struct InvestmentPerformance {
    pub start: Day,
    pub end: Day,
    pub opening: Option<Decimal>,
    pub closing: Option<Decimal>,
    pub net_flows: Option<Decimal>,
    pub gain: Option<Decimal>,
    /// Modified Dietz estimate, end-of-day flow weights. Not annualized.
    pub ret: Option<Decimal>,
    pub holdings: Vec<PerformanceHolding>,
    pub points: Vec<PerformancePoint>,
    pub issues: Vec<String>,
    pub warnings: Vec<String>,
}
struct Holding {
    units: Decimal,
    opening: Option<Decimal>,
    flows: Option<Decimal>,
    weighted: Decimal,
    active: bool,
}
impl Default for Holding {
    fn default() -> Self {
        Self {
            units: Decimal::ZERO,
            opening: Some(Decimal::ZERO),
            flows: Some(Decimal::ZERO),
            weighted: Decimal::ZERO,
            active: false,
        }
    }
}
fn gain(
    opening: Option<Decimal>,
    closing: Option<Decimal>,
    flows: Option<Decimal>,
) -> Option<Decimal> {
    Some(closing? - opening? - flows?)
}
fn estimate(
    gain: Option<Decimal>,
    opening: Option<Decimal>,
    weighted: Decimal,
    days: i64,
) -> Option<Decimal> {
    let capital = opening? + weighted;
    if days <= 0 || capital <= Decimal::ZERO {
        return None;
    }
    gain?.checked_div(capital)
}
impl Ledger {
    /// `start` is the opening end-of-day valuation. Flows occur in (start,end].
    pub fn investment_performance(
        &self,
        start: Day,
        end: Day,
        cur: &str,
    ) -> InvestmentPerformance {
        let mut issues = BTreeSet::new();
        let mut warnings = BTreeSet::new();
        let mut holdings: BTreeMap<String, Holding> = BTreeMap::new();
        let mut order: Vec<_> =
            self.txns.iter().filter(|t| t.date <= end).collect();
        order.sort_by_key(|t| t.date);
        let eligible = |account: &str, currency: &str| {
            account.starts_with("Assets:")
                && currency != cur
                && !self.operating_currencies.iter().any(|c| c == currency)
        };
        let mut dates = BTreeSet::from([start, end]);
        let mut month = MonthKey::new(start.0, start.1);
        while month.end_of_month() < end {
            if month.end_of_month() > start {
                dates.insert(month.end_of_month());
            }
            month = month.next();
        }
        for t in &order {
            if t.date > start
                && t.postings.iter().any(|p| {
                    p.amounts.iter().any(|(_, c)| eligible(&p.account, c))
                })
            {
                dates.insert(t.date);
            }
        }
        let days = days_between(start, end);
        let mut cursor = 0;
        let mut points = Vec::new();
        let mut opening = Some(Decimal::ZERO);
        for date in dates {
            while cursor < order.len() && order[cursor].date <= date {
                let txn = order[cursor];
                cursor += 1;
                let mut changes: BTreeMap<&str, (Decimal, Option<Decimal>)> =
                    BTreeMap::new();
                for p in &txn.postings {
                    for (units, currency) in &p.amounts {
                        if !eligible(&p.account, currency) || units.is_zero() {
                            continue;
                        }
                        let h = holdings.entry(currency.clone()).or_default();
                        h.units += units;
                        let change = changes
                            .entry(currency)
                            .or_insert((Decimal::ZERO, Some(Decimal::ZERO)));
                        change.0 += units;
                        if txn.date <= start {
                            continue;
                        }
                        h.active = true;
                        let annotation = p.price.as_ref().or_else(|| {
                            if *units > Decimal::ZERO {
                                p.cost.as_ref()
                            } else {
                                None
                            }
                        });
                        let flow = if p.amounts.len() == 1
                            && let Some(a) = annotation
                        {
                            self.convert(a.total, &a.currency, cur, txn.date)
                        } else {
                            self.convert(*units, currency, cur, txn.date)
                        };
                        change.1 = change.1.zip(flow).map(|(a, b)| a + b);
                    }
                }
                for (currency, (units, flow)) in changes {
                    let h = holdings.get_mut(currency).unwrap();
                    if h.units < Decimal::ZERO && txn.date > start {
                        issues.insert(format!(
                            "{currency}: negative inventory is unsupported"
                        ));
                        h.flows = None;
                    }
                    // Net-zero movement across asset accounts is an internal transfer.
                    if txn.date <= start || units.is_zero() {
                        continue;
                    }
                    if flow.is_none() {
                        issues.insert(format!("{currency}: missing flow conversion on {}-{:02}-{:02}", txn.date.0,txn.date.1,txn.date.2));
                    }
                    h.flows = h.flows.zip(flow).map(|(a, b)| a + b);
                    if let Some(flow) = flow
                        && days > 0
                    {
                        h.weighted += flow
                            * Decimal::from(days_between(txn.date, end))
                            / Decimal::from(days);
                    }
                }
            }
            let mut value = Some(Decimal::ZERO);
            for (currency, h) in &mut holdings {
                let v = self.performance_value(
                    h.units,
                    currency,
                    cur,
                    date,
                    &mut issues,
                    &mut warnings,
                );
                if date == start {
                    h.opening = v;
                    h.active |= !h.units.is_zero();
                }
                value = value.zip(v).map(|(a, b)| a + b);
            }
            if date == start {
                opening = value;
            }
            let flows: Option<Decimal> =
                holdings.values().map(|h| h.flows).sum();
            points.push(PerformancePoint {
                date,
                value: value.map(cents),
                net_flows: flows.map(cents),
                gain: gain(opening, value, flows).map(cents),
            });
        }
        let rows: Vec<_> = holdings
            .iter()
            .filter(|(_, h)| h.active)
            .map(|(currency, h)| {
                let closing = self.performance_value(
                    h.units,
                    currency,
                    cur,
                    end,
                    &mut issues,
                    &mut warnings,
                );
                let gain = gain(h.opening, closing, h.flows);
                PerformanceHolding {
                    currency: currency.clone(),
                    label: self
                        .commodity(currency)
                        .and_then(|c| c.name.clone())
                        .unwrap_or_else(|| currency.clone()),
                    opening: h.opening.map(cents),
                    closing: closing.map(cents),
                    net_flows: h.flows.map(cents),
                    gain: gain.map(cents),
                    ret: estimate(gain, h.opening, h.weighted, days),
                }
            })
            .collect();
        let closing: Option<Decimal> = rows.iter().map(|h| h.closing).sum();
        let flows: Option<Decimal> = holdings.values().map(|h| h.flows).sum();
        let g = gain(opening, closing, flows).filter(|_| issues.is_empty());
        let ret = estimate(
            g,
            opening,
            holdings.values().map(|h| h.weighted).sum(),
            days,
        );
        InvestmentPerformance {
            start,
            end,
            opening: opening.map(cents),
            closing,
            net_flows: flows.map(cents),
            gain: g.map(cents),
            ret,
            holdings: rows,
            points,
            issues: issues.into_iter().collect(),
            warnings: warnings.into_iter().collect(),
        }
    }
    fn performance_value(
        &self,
        units: Decimal,
        currency: &str,
        cur: &str,
        date: Day,
        issues: &mut BTreeSet<String>,
        warnings: &mut BTreeSet<String>,
    ) -> Option<Decimal> {
        if units.is_zero() {
            return Some(Decimal::ZERO);
        }
        if units < Decimal::ZERO {
            issues.insert(format!(
                "{currency}: negative inventory is unsupported"
            ));
            return None;
        }
        let value = self.convert(units, currency, cur, date);
        if value.is_none() {
            issues.insert(format!(
                "{currency}: missing valuation on {}-{:02}-{:02}",
                date.0, date.1, date.2
            ));
        }
        if self
            .priced_at(currency, cur, date)
            .is_some_and(|d| days_between(d, date) >= 45)
        {
            warnings.insert(format!(
                "{currency}: a valuation uses a quote at least 45 days old"
            ));
        }
        value
    }
}
