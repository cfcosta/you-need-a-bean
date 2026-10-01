//! Page 5: what is owed, what it costs, and when it is gone.

use bean_core::{
    model::{Day, Ledger, MonthKey},
    reports::{Debt, DebtKind, DebtPoint, LiabilitiesView, amortize},
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

    /// What is owed on loans, and what on cards.
    pub fn owed_parts(&self) -> (Decimal, Decimal) {
        let sum = |kind| {
            self.view
                .debts
                .iter()
                .filter(|d| d.kind == kind)
                .map(|d| d.owed)
                .sum()
        };
        (sum(DebtKind::Installment), sum(DebtKind::Revolving))
    }

    /// The debts still carrying a balance, largest first; the paid-off
    /// ones step aside.
    pub fn open_debts(&self) -> Vec<&Debt> {
        let mut open: Vec<&Debt> = self
            .view
            .debts
            .iter()
            .filter(|d| d.owed > Decimal::ZERO)
            .collect();
        open.sort_by_key(|d| std::cmp::Reverse(d.owed));
        open
    }

    /// The debts with nothing left on them.
    pub fn settled(&self) -> Vec<&Debt> {
        self.view
            .debts
            .iter()
            .filter(|d| d.owed <= Decimal::ZERO)
            .collect()
    }

    /// When `debt` is cleared paying `extra` on top of each payment, and
    /// the interest still to pay on the way.
    pub fn payoff(
        &self,
        debt: &Debt,
        extra: Decimal,
    ) -> Option<(MonthKey, Decimal)> {
        // A debt with no rate costs nothing to carry: zero, as the core
        // reads it too.
        let a = amortize(
            debt.owed,
            debt.rate.unwrap_or_default(),
            debt.payment? + extra,
        )?;
        Some((add_months(self.current, a.months), a.interest))
    }

    /// The last `months` of a debt's balance, from when it was first
    /// owed: the months before it existed say nothing.
    pub fn history<'a>(
        &self,
        debt: &'a Debt,
        months: usize,
    ) -> &'a [DebtPoint] {
        let all = &debt.history;
        let from = all
            .iter()
            .position(|p| p.owed.is_some_and(|o| o > Decimal::ZERO))
            .unwrap_or(all.len());
        let all = &all[from..];
        &all[all.len().saturating_sub(months)..]
    }

    /// The balance after each coming payment, down to zero.
    pub fn projection(&self, debt: &Debt, extra: Decimal) -> Vec<Decimal> {
        let Some(payment) = debt.payment else {
            return vec![];
        };
        // No rate: carrying it costs nothing, as the core reads it.
        let rate = debt.rate.unwrap_or_default();
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
