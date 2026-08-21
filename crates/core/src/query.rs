//! The budget queries the API serves: trailing averages, status, the
//! monthly table, sidebar balances, and per-category drill-downs.
//!
//! Everything here is a pure read over the indexes in [`Ledger`]; the
//! HTTP layer only shapes these results into JSON.

use rust_decimal::Decimal;

use crate::model::{
    AccountInfo, AccountKind, Day, Ledger, MonthKey, Txn, add_sum,
};

/// Spend vs the trailing average, with the mockup's thresholds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Good,
    Warn,
    Over,
}

impl Status {
    pub fn as_str(self) -> &'static str {
        match self {
            Status::Good => "good",
            Status::Warn => "warn",
            Status::Over => "over",
        }
    }
}

#[derive(Debug, Clone)]
pub struct CategoryRow {
    pub account: String,
    pub label: String,
    /// This month's spend in the display currency (convertible part only).
    pub spent: Decimal,
    /// Trailing average ("typical") over the window months that saw
    /// spending; `None` when there is no window or none of them did.
    pub avg: Option<Decimal>,
    /// spent / avg; `None` when there is no positive average.
    pub ratio: Option<Decimal>,
    pub status: Option<Status>,
    /// Native per-currency spend, display currency first.
    pub split: Vec<(String, Decimal)>,
}

#[derive(Debug, Clone)]
pub struct Group {
    pub name: String,
    pub spent: Decimal,
    pub avg: Option<Decimal>,
    pub categories: Vec<CategoryRow>,
}

#[derive(Debug, Clone)]
pub struct AccountRow {
    pub account: String,
    pub label: String,
    /// Cumulative balance through the end of the month, per commodity.
    pub balances: Vec<(String, Decimal)>,
    /// Sum of the convertible balances; `None` when nothing converts.
    pub converted: Option<Decimal>,
}

#[derive(Debug, Clone)]
pub struct MonthView {
    pub month: MonthKey,
    pub income: Decimal,
    pub spent: Decimal,
    pub typical: Option<Decimal>,
    pub groups: Vec<Group>,
    pub budget_accounts: Vec<AccountRow>,
    pub tracking_accounts: Vec<AccountRow>,
}

#[derive(Debug, Clone)]
pub struct HistoryPoint {
    pub month: MonthKey,
    pub spent: Decimal,
}

#[derive(Debug)]
pub struct CategoryView<'a> {
    pub account: String,
    pub label: String,
    pub spent: Decimal,
    pub avg: Option<Decimal>,
    pub ratio: Option<Decimal>,
    pub status: Option<Status>,
    /// The averaging window, when one exists.
    pub window: Option<(MonthKey, MonthKey)>,
    /// The six months ending at the selected one.
    pub history: Vec<HistoryPoint>,
    pub split: Vec<(String, Decimal)>,
    pub txns: Vec<&'a Txn>,
}

impl Ledger {
    /// This month's posting sum converted to `cur` at month end.
    /// Unconvertible commodities are left out (they stay in the split).
    pub fn spent_converted(
        &self,
        account: &str,
        month: MonthKey,
        cur: &str,
    ) -> Decimal {
        let at = month.end_of_month();
        self.sums(account, month)
            .iter()
            .filter_map(|(c, v)| self.convert(*v, c, cur, at))
            .sum()
    }

    /// Trailing window `[month − basis, month − 1]`, clamped to the
    /// ledger's first activity month. `None` when nothing precedes `month`.
    pub fn window(
        &self,
        month: MonthKey,
        basis: u32,
    ) -> Option<(MonthKey, MonthKey)> {
        let first = self.first_txn_month?;
        let from = month.minus(basis).max(first);
        let to = month.prev();
        (from <= to && to < month).then_some((from, to))
    }

    /// Mean monthly spend over the window, skipping months without any
    /// spend so sporadic categories aren't diluted toward zero; each
    /// month converts at its own end date. `None` when there is no
    /// window or no month in it saw spending.
    pub fn average(
        &self,
        account: &str,
        month: MonthKey,
        basis: u32,
        cur: &str,
    ) -> Option<Decimal> {
        let (from, to) = self.window(month, basis)?;
        let mut total = Decimal::ZERO;
        let mut count = 0u32;
        let mut m = from;
        while m <= to {
            let spent = self.spent_converted(account, m, cur);
            if !spent.is_zero() {
                total += spent;
                count += 1;
            }
            m = m.next();
        }
        (count > 0).then(|| total / Decimal::from(count))
    }

    /// The whole monthly page: stat tiles, grouped category table, and
    /// the budget/tracking sidebar.
    pub fn month_view(
        &self,
        month: MonthKey,
        basis: u32,
        cur: &str,
    ) -> MonthView {
        let mut income = Decimal::ZERO;
        let mut groups: Vec<Group> = Vec::new();
        let mut budget_accounts = Vec::new();
        let mut tracking_accounts = Vec::new();

        for info in self.accounts() {
            if info.account.starts_with("Income:") {
                income -= self.spent_converted(&info.account, month, cur);
                continue;
            }
            if let Some(group_name) = &info.group {
                if info.kind == AccountKind::Hidden
                    || self.months_of(&info.account).next().is_none()
                {
                    continue;
                }
                let row = self.category_row(info, month, basis, cur);
                match groups.iter_mut().find(|g| &g.name == group_name) {
                    Some(group) => group.categories.push(row),
                    None => groups.push(Group {
                        name: group_name.clone(),
                        spent: Decimal::ZERO,
                        avg: None,
                        categories: vec![row],
                    }),
                }
                continue;
            }
            if info.account.starts_with("Assets:")
                || info.account.starts_with("Liabilities:")
            {
                let row = self.account_row(info, month, cur);
                // A sidebar full of zero rows is noise: closed accounts
                // and accounts with no activity yet stay hidden.
                if row.balances.iter().all(|(_, v)| v.is_zero()) {
                    continue;
                }
                match info.kind {
                    AccountKind::Budget => budget_accounts.push(row),
                    AccountKind::Tracking => tracking_accounts.push(row),
                    AccountKind::Hidden => {}
                }
            }
        }

        groups.sort_by(|a, b| a.name.cmp(&b.name));
        for group in &mut groups {
            group.categories.sort_by(|a, b| a.label.cmp(&b.label));
            group.spent = group.categories.iter().map(|c| c.spent).sum();
            group.avg = sum_present(group.categories.iter().map(|c| c.avg));
        }

        MonthView {
            month,
            income: cents(income),
            spent: groups.iter().map(|g| g.spent).sum(),
            typical: sum_present(groups.iter().map(|g| g.avg)),
            groups,
            budget_accounts,
            tracking_accounts,
        }
    }

    /// The category drill-down for the inspector panel.
    pub fn category_view(
        &self,
        account: &str,
        month: MonthKey,
        basis: u32,
        cur: &str,
    ) -> Option<CategoryView<'_>> {
        let info = self.account(account)?;
        let spent = cents(self.spent_converted(account, month, cur));
        let avg = self.average(account, month, basis, cur).map(cents);
        let (ratio, status) = ratio_status(spent, avg);
        let history = (0..6)
            .rev()
            .map(|back| {
                let m = month.minus(back);
                HistoryPoint {
                    month: m,
                    spent: cents(self.spent_converted(account, m, cur)),
                }
            })
            .collect();
        Some(CategoryView {
            account: info.account.clone(),
            label: info.label.clone(),
            spent,
            avg,
            ratio,
            status,
            window: self.window(month, basis),
            history,
            split: self.split(account, month, cur),
            txns: self.txns(account, month),
        })
    }

    /// All months the UI can navigate: first transaction month through
    /// the later of the last transaction month and today.
    pub fn months_range(&self, today: Day) -> Vec<MonthKey> {
        let today = MonthKey::new(today.0, today.1);
        let first = self.first_txn_month.unwrap_or(today);
        let last = self.last_txn_month.unwrap_or(today).max(today);
        let mut months = Vec::new();
        let mut m = first;
        while m <= last {
            months.push(m);
            m = m.next();
        }
        months
    }

    /// Today's month clamped into the navigable range.
    pub fn default_month(&self, today: Day) -> MonthKey {
        let today = MonthKey::new(today.0, today.1);
        let first = self.first_txn_month.unwrap_or(today);
        let last = self.last_txn_month.unwrap_or(today).max(today);
        today.clamp(first, last)
    }

    fn category_row(
        &self,
        info: &AccountInfo,
        month: MonthKey,
        basis: u32,
        cur: &str,
    ) -> CategoryRow {
        let spent = cents(self.spent_converted(&info.account, month, cur));
        let avg = self.average(&info.account, month, basis, cur).map(cents);
        let (ratio, status) = ratio_status(spent, avg);
        CategoryRow {
            account: info.account.clone(),
            label: info.label.clone(),
            spent,
            avg,
            ratio,
            status,
            split: self.split(&info.account, month, cur),
        }
    }

    fn account_row(
        &self,
        info: &AccountInfo,
        month: MonthKey,
        cur: &str,
    ) -> AccountRow {
        let mut balances: Vec<(String, Decimal)> = Vec::new();
        for (_, sums) in self
            .months_of(&info.account)
            .take_while(|(m, _)| *m <= month)
        {
            for (c, v) in sums {
                add_sum(&mut balances, c, *v);
            }
        }
        self.sort_amounts(&mut balances, cur);
        let at = month.end_of_month();
        let parts: Vec<Decimal> = balances
            .iter()
            .filter_map(|(c, v)| self.convert(*v, c, cur, at))
            .collect();
        AccountRow {
            account: info.account.clone(),
            label: info.label.clone(),
            balances,
            converted: (!parts.is_empty())
                .then(|| cents(parts.iter().copied().sum())),
        }
    }

    /// Native per-currency spend for the month, display-currency first.
    fn split(
        &self,
        account: &str,
        month: MonthKey,
        cur: &str,
    ) -> Vec<(String, Decimal)> {
        let mut split = self.sums(account, month).to_vec();
        self.sort_amounts(&mut split, cur);
        split
    }

    /// Display currency first, then operating currencies in declaration
    /// order, then everything else alphabetically.
    fn sort_amounts(&self, amounts: &mut [(String, Decimal)], cur: &str) {
        amounts.sort_by_key(|(c, _)| {
            (
                c != cur,
                self.operating_currencies
                    .iter()
                    .position(|op| op == c)
                    .unwrap_or(usize::MAX),
                c.clone(),
            )
        });
    }
}

/// Quantize a display value to cents. Applied at the leaves (category
/// spend/avg, account conversions, income) so every aggregate is a sum
/// of already-rounded values and what the UI shows always adds up.
pub(crate) fn cents(value: Decimal) -> Decimal {
    value.round_dp(2)
}

fn ratio_status(
    spent: Decimal,
    avg: Option<Decimal>,
) -> (Option<Decimal>, Option<Status>) {
    let Some(avg) = avg else { return (None, None) };
    if avg <= Decimal::ZERO {
        return (None, None);
    }
    let ratio = spent / avg;
    let status = if ratio > Decimal::ONE {
        Status::Over
    } else if ratio > Decimal::new(85, 2) {
        Status::Warn
    } else {
        Status::Good
    };
    (Some(ratio), Some(status))
}

/// Sum of the `Some` values; `None` when every input is `None`.
fn sum_present(
    values: impl Iterator<Item = Option<Decimal>>,
) -> Option<Decimal> {
    values
        .flatten()
        .fold(None, |acc, v| Some(acc.unwrap_or_default() + v))
}
