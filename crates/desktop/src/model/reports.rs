//! Page 3: the longer view — independence, what moved net worth,
//! cashflow, where the money went and what the figures rest on.

use bean_core::{
    model::{Day, Ledger, MonthKey},
    reports::ReportsView,
};
use rust_decimal::Decimal;

use super::add_months;

/// `101` → `8y 5m`; under a year, just the months.
pub fn duration(months: u32) -> String {
    let (y, m) = (months / 12, months % 12);
    if y == 0 {
        format!("{m}m")
    } else {
        format!("{y}y {m}m")
    }
}

#[derive(Clone, Debug)]
pub struct Scenario {
    pub rate_pct: u32,
    pub months: Option<u32>,
    pub reach: String,
    pub around: Option<MonthKey>,
    pub coast: String,
}

#[derive(Clone, Debug)]
pub struct Moved {
    pub opening: Decimal,
    pub saved: Decimal,
    pub markets: Decimal,
    pub closing: Decimal,
    /// Each full month: what was saved and what the markets added.
    pub months: Vec<(MonthKey, Decimal, Decimal)>,
    /// The month still under way, if anything has moved in it.
    pub so_far: Option<(MonthKey, Decimal)>,
    pub implied_return: Option<Decimal>,
}

#[derive(Clone, Debug)]
pub struct Cashflow {
    pub month: MonthKey,
    pub income: Decimal,
    pub expenses: Decimal,
    /// Kept ÷ income; `None` when nothing came in.
    pub rate: Option<Decimal>,
}

#[derive(Clone, Debug)]
pub struct Reports {
    pub view: ReportsView,
    pub current: MonthKey,
    pub basis: u32,
}

impl Reports {
    pub fn build(ledger: &Ledger, today: Day, basis: u32, cur: &str) -> Self {
        Self {
            view: ledger.reports_view(today, basis, cur),
            current: MonthKey::new(today.0, today.1),
            basis,
        }
    }

    pub fn scenarios(&self) -> Vec<Scenario> {
        let fire = &self.view.fire;
        fire.scenarios
            .iter()
            .enumerate()
            .map(|(i, s)| Scenario {
                rate_pct: (s.rate * 100.).round() as u32,
                months: s.months,
                reach: s.months.map(duration).unwrap_or_else(|| "—".into()),
                around: s.months.map(|m| add_months(self.current, m)),
                coast: fire
                    .coast
                    .get(i)
                    .and_then(|c| c.months)
                    .map(duration)
                    .unwrap_or_else(|| "—".into()),
            })
            .collect()
    }

    pub fn moved(&self) -> Moved {
        let g = &self.view.growth;
        let months = g
            .points
            .iter()
            .filter(|p| p.equity.is_zero() && p.month < self.current)
            .map(|p| (p.month, p.saved, p.market))
            .collect();
        let so_far = g
            .points
            .iter()
            .find(|p| p.month == self.current && !p.delta.is_zero())
            .map(|p| (p.month, p.delta));
        Moved {
            opening: g.equity,
            saved: g.saved,
            markets: g.market,
            closing: g.equity + g.saved + g.market,
            months,
            so_far,
            implied_return: g.implied_return,
        }
    }

    pub fn cashflow(&self) -> Vec<Cashflow> {
        self.view
            .cashflow
            .iter()
            .skip_while(|c| c.income.is_zero() && c.expenses.is_zero())
            .map(|c| Cashflow {
                month: c.month,
                income: c.income,
                expenses: c.expenses,
                rate: (c.income > Decimal::ZERO)
                    .then(|| (c.income - c.expenses) / c.income),
            })
            .collect()
    }

    /// What the figures rest on: flagged spending, and spending with no
    /// category.
    pub fn doubt(&self) -> (Decimal, Decimal) {
        let t = &self.view.trust;
        (t.flagged.amount, t.uncategorized.total)
    }

    /// The first month in the reports' window, for labels like `dec–sep`.
    pub fn window(&self) -> Option<(MonthKey, MonthKey)> {
        self.view.payees.window
    }
}
