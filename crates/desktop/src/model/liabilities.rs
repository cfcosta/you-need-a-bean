//! Page 5: what is owed, what it costs, and when it is gone.

use bean_core::{
    model::{Day, Ledger, MonthKey},
    reports::{Debt, LiabilitiesView, amortize},
};
use rust_decimal::Decimal;

use super::add_months;

#[derive(Clone, Debug)]
pub struct Liabilities {
    pub view: LiabilitiesView,
    pub current: MonthKey,
}

impl Liabilities {
    pub fn build(ledger: &Ledger, today: Day, basis: u32, cur: &str) -> Self {
        Self {
            view: ledger.liabilities_view(today, basis, cur),
            current: MonthKey::new(today.0, today.1),
        }
    }

    /// Interest a month at today's balances.
    pub fn cost_month(&self) -> Decimal {
        self.view.cost_year / Decimal::from(12)
    }

    /// How many times the free cash would clear everything owed.
    pub fn covered(&self) -> Option<Decimal> {
        (self.view.owed > Decimal::ZERO)
            .then(|| self.view.cover.cash / self.view.owed)
    }

    /// When `debt` is cleared paying `extra` on top of each payment, and
    /// the interest still to pay on the way.
    pub fn payoff(
        &self,
        debt: &Debt,
        extra: Decimal,
    ) -> Option<(MonthKey, Decimal)> {
        let a = amortize(debt.owed, debt.rate?, debt.payment? + extra)?;
        Some((add_months(self.current, a.months), a.interest))
    }

    /// The balance after each coming payment, down to zero.
    pub fn projection(&self, debt: &Debt, extra: Decimal) -> Vec<Decimal> {
        let (Some(rate), Some(payment)) = (debt.rate, debt.payment) else {
            return vec![];
        };
        let payment = payment + extra;
        let monthly = rate / Decimal::from(12);
        let mut balance = debt.owed;
        let mut out = vec![];
        while balance > Decimal::ZERO && out.len() < 600 {
            let accrued = (balance * monthly).round_dp(2);
            if payment <= accrued {
                break;
            }
            balance = (balance + accrued - payment).max(Decimal::ZERO);
            out.push(balance);
        }
        out
    }
}
