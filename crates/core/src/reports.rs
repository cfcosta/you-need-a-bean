//! The reports page: net worth and cashflow over the ledger's whole
//! range, and the FIRE projection built on the 4% rule.
//!
//! Like [`crate::query`], everything here is a pure read over
//! [`Ledger`]; money is quantized to cents at the leaves so displayed
//! sums add up.

use std::collections::BTreeMap;

use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;

use crate::model::{Day, Ledger, MonthKey, add_sum};
use crate::query::cents;

/// Annual real-return scenarios for the time-to-FIRE projection.
pub const SCENARIO_RATES: [f64; 3] = [0.03, 0.05, 0.07];

#[derive(Debug, Clone, Copy)]
pub struct NetWorthPoint {
    pub month: MonthKey,
    /// Convertible asset balances at month end.
    pub assets: Decimal,
    /// Convertible liability balances at month end (negative when owed).
    pub liabilities: Decimal,
    pub net: Decimal,
}

#[derive(Debug, Clone, Copy)]
pub struct CashflowPoint {
    pub month: MonthKey,
    pub income: Decimal,
    pub expenses: Decimal,
    pub net: Decimal,
}

#[derive(Debug, Clone, Copy)]
pub struct FireScenario {
    /// Assumed annual real return.
    pub rate: f64,
    /// Months until the stash reaches the FIRE number; `None` when it
    /// stays short for 100 years.
    pub months: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct FireView {
    /// The averaging window behind the spend and savings numbers.
    pub window: Option<(MonthKey, MonthKey)>,
    pub monthly_spend: Decimal,
    pub annual_spend: Decimal,
    /// 25× annual spend: the stash a 4% withdrawal rate needs.
    pub fire_number: Decimal,
    pub net_worth: Decimal,
    /// net worth / FIRE number; `None` without a positive target.
    pub progress: Option<Decimal>,
    pub monthly_savings: Decimal,
    /// What the current stash already sustains: 4% of net worth / 12.
    pub swr_monthly: Decimal,
    pub scenarios: Vec<FireScenario>,
}

/// One expense group's share of the trailing year.
#[derive(Debug, Clone)]
pub struct YearGroup {
    pub name: String,
    pub total: Decimal,
}

#[derive(Debug, Clone)]
pub struct ReportsView {
    /// The month the FIRE numbers are anchored to (today, clamped).
    pub month: MonthKey,
    pub net_worth: Vec<NetWorthPoint>,
    pub cashflow: Vec<CashflowPoint>,
    pub fire: FireView,
    /// The trailing-twelve-month window behind `year_groups`.
    pub year_window: Option<(MonthKey, MonthKey)>,
    /// Converted spend per expense group over that window, biggest
    /// first. Groups that net to zero or less over the year drop out.
    pub year_groups: Vec<YearGroup>,
}

impl Ledger {
    /// Net worth and cashflow for every navigable month, plus the FIRE
    /// projection anchored to today's month.
    pub fn reports_view(
        &self,
        today: Day,
        basis: u32,
        cur: &str,
    ) -> ReportsView {
        let months = self.months_range(today);
        let current = self.default_month(today);

        // One pass over the indexes: per-month currency deltas for the
        // balance-sheet roots, per-month flows for the flow roots.
        let mut assets = BTreeMap::new();
        let mut liabilities = BTreeMap::new();
        let mut income = BTreeMap::new();
        let mut expenses = BTreeMap::new();
        for info in self.accounts() {
            let map = if info.account.starts_with("Assets:") {
                &mut assets
            } else if info.account.starts_with("Liabilities:") {
                &mut liabilities
            } else if info.account.starts_with("Income:") {
                &mut income
            } else if info.account.starts_with("Expenses:") {
                &mut expenses
            } else {
                continue;
            };
            for (month, sums) in self.months_of(&info.account) {
                let entry: &mut Vec<(String, Decimal)> =
                    map.entry(month).or_default();
                for (c, v) in sums {
                    add_sum(entry, c, *v);
                }
            }
        }

        let mut asset_bal: Vec<(String, Decimal)> = Vec::new();
        let mut liab_bal: Vec<(String, Decimal)> = Vec::new();
        let mut net_worth = Vec::with_capacity(months.len());
        let mut cashflow = Vec::with_capacity(months.len());
        for &month in &months {
            apply(&mut asset_bal, assets.get(&month));
            apply(&mut liab_bal, liabilities.get(&month));
            let at = month.end_of_month();
            let a = cents(self.convertible(&asset_bal, cur, at));
            let l = cents(self.convertible(&liab_bal, cur, at));
            net_worth.push(NetWorthPoint {
                month,
                assets: a,
                liabilities: l,
                net: a + l,
            });

            // Income postings are negative in beancount.
            let inc = income
                .get(&month)
                .map_or(Decimal::ZERO, |f| -self.convertible(f, cur, at));
            let exp = expenses
                .get(&month)
                .map_or(Decimal::ZERO, |f| self.convertible(f, cur, at));
            let (inc, exp) = (cents(inc), cents(exp));
            cashflow.push(CashflowPoint {
                month,
                income: inc,
                expenses: exp,
                net: inc - exp,
            });
        }

        let fire = fire_view(
            self.window(current, basis),
            net_at(&net_worth, current),
            &cashflow,
        );

        // Where the year went: spend per expense group over the
        // trailing twelve months, hidden accounts included — the same
        // every-account stance as the flows above.
        let year_window = self.window(current, 12);
        let mut totals: BTreeMap<&str, Decimal> = BTreeMap::new();
        for info in self.accounts() {
            let Some(group) = &info.group else { continue };
            if let Some(spend) = self.year_spend(&info.account, current, cur) {
                *totals.entry(group).or_default() += spend;
            }
        }
        let mut year_groups: Vec<YearGroup> = totals
            .into_iter()
            .filter(|&(_, total)| total > Decimal::ZERO)
            .map(|(name, total)| YearGroup {
                name: name.to_string(),
                total: cents(total),
            })
            .collect();
        year_groups
            .sort_by(|a, b| b.total.cmp(&a.total).then(a.name.cmp(&b.name)));

        ReportsView {
            month: current,
            net_worth,
            cashflow,
            fire,
            year_window,
            year_groups,
        }
    }

    /// Sum of the convertible parts of a per-currency balance list.
    fn convertible(
        &self,
        amounts: &[(String, Decimal)],
        cur: &str,
        at: Day,
    ) -> Decimal {
        amounts
            .iter()
            .filter_map(|(c, v)| self.convert(*v, c, cur, at))
            .sum()
    }
}

/// First month where `net_worth`, compounded monthly at `annual_rate`
/// with `monthly_savings` added each month, reaches `target`. `None`
/// when it stays short for 100 years.
pub fn months_to_fire(
    net_worth: f64,
    monthly_savings: f64,
    target: f64,
    annual_rate: f64,
) -> Option<u32> {
    let monthly_rate = (1.0 + annual_rate).powf(1.0 / 12.0) - 1.0;
    let mut balance = net_worth;
    for month in 0..=1200 {
        if balance >= target {
            return Some(month);
        }
        balance = balance * (1.0 + monthly_rate) + monthly_savings;
    }
    None
}

fn fire_view(
    window: Option<(MonthKey, MonthKey)>,
    net_worth: Decimal,
    cashflow: &[CashflowPoint],
) -> FireView {
    let (monthly_spend, monthly_savings) = match window {
        Some((from, to)) => {
            let pts: Vec<&CashflowPoint> = cashflow
                .iter()
                .filter(|p| p.month >= from && p.month <= to)
                .collect();
            let count = Decimal::from(pts.len().max(1));
            (
                cents(pts.iter().map(|p| p.expenses).sum::<Decimal>() / count),
                cents(pts.iter().map(|p| p.net).sum::<Decimal>() / count),
            )
        }
        None => (Decimal::ZERO, Decimal::ZERO),
    };
    let annual_spend = monthly_spend * Decimal::from(12);
    let fire_number = annual_spend * Decimal::from(25);
    let progress = (fire_number > Decimal::ZERO)
        .then(|| (net_worth / fire_number).round_dp(4));
    let swr_monthly = cents(net_worth * Decimal::new(4, 2) / Decimal::from(12));
    let scenarios = if fire_number > Decimal::ZERO {
        SCENARIO_RATES
            .iter()
            .map(|&rate| FireScenario {
                rate,
                months: months_to_fire(
                    net_worth.to_f64().unwrap_or(0.0),
                    monthly_savings.to_f64().unwrap_or(0.0),
                    fire_number.to_f64().unwrap_or(f64::MAX),
                    rate,
                ),
            })
            .collect()
    } else {
        Vec::new()
    };
    FireView {
        window,
        monthly_spend,
        annual_spend,
        fire_number,
        net_worth,
        progress,
        monthly_savings,
        swr_monthly,
        scenarios,
    }
}

fn net_at(series: &[NetWorthPoint], month: MonthKey) -> Decimal {
    series
        .iter()
        .find(|p| p.month == month)
        .map(|p| p.net)
        .unwrap_or_default()
}

fn apply(
    balance: &mut Vec<(String, Decimal)>,
    delta: Option<&Vec<(String, Decimal)>>,
) {
    for (c, v) in delta.into_iter().flatten() {
        add_sum(balance, c, *v);
    }
}
