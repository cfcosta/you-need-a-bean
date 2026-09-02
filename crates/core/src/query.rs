//! The budget queries the API serves: trailing averages, status, the
//! monthly table, sidebar balances, and per-category drill-downs.
//!
//! Everything here is a pure read over the indexes in [`Ledger`]; the
//! HTTP layer only shapes these results into JSON.

use std::collections::HashMap;

use rust_decimal::Decimal;

use crate::model::{
    AccountInfo, AccountKind, Day, Ledger, MonthKey, Txn, add_sum,
};

/// Spend vs the typical month, with the mockup's thresholds.
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
    /// The typical month: the median of the window months that saw
    /// real payments; `None` when there is no window or none of them
    /// did.
    pub avg: Option<Decimal>,
    /// spent / avg; `None` when there is no positive typical.
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
    /// What a normal whole month costs: the median of the window
    /// months' total spend, months with real payments only.
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

/// One month of an account's register: what came in, what went out,
/// and what it was left holding.
#[derive(Debug, Clone)]
pub struct AccountPoint {
    pub month: MonthKey,
    pub inflow: Decimal,
    pub outflow: Decimal,
    /// Closing balance converted at that month's end; `None` when
    /// something it holds has no price.
    pub balance: Option<Decimal>,
}

/// One line of the register.
#[derive(Debug)]
pub struct AccountEntry<'a> {
    pub txn: &'a Txn,
    /// What this transaction did to the account, converted. `None`
    /// when a leg of it has no price in the display currency.
    pub delta: Option<Decimal>,
    /// The balance it left behind. `None` from the first entry whose
    /// delta could not be converted onwards — a running total with a
    /// hole in it is not a balance.
    pub balance: Option<Decimal>,
}

/// An asset or liability account over one month: the statement view,
/// where money arriving and money leaving are the two things worth
/// seeing and the balance is what they add up to.
#[derive(Debug)]
pub struct AccountView<'a> {
    pub account: String,
    pub label: String,
    pub kind: AccountKind,
    /// The balance the month opened on: the end of the month before.
    pub opening: Option<Decimal>,
    /// What it holds at the end of this one, per commodity.
    pub balances: Vec<(String, Decimal)>,
    /// The same, converted; `None` when nothing converts.
    pub converted: Option<Decimal>,
    /// This month's arrivals and departures, converted.
    pub inflow: Decimal,
    pub outflow: Decimal,
    /// `basis` months ending at the selected one.
    pub history: Vec<AccountPoint>,
    pub entries: Vec<AccountEntry<'a>>,
    /// Commodities this month moved that have no price in the display
    /// currency, and so are missing from every figure above.
    pub unpriced: Vec<String>,
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

    /// The typical month: the median of the window months with real
    /// payments, each converted at its own end date. Months where
    /// nothing was paid — zero or a net refund — don't count at all,
    /// and a median can't be dragged by a one-off emergency. `None`
    /// when there is no window or no month in it saw payments.
    pub fn typical(
        &self,
        account: &str,
        month: MonthKey,
        basis: u32,
        cur: &str,
    ) -> Option<Decimal> {
        self.median_active(month, basis, |m| {
            self.spent_converted(account, m, cur)
        })
    }

    /// The median of `spend_in` across the window months where it was
    /// actually paid (> 0) — the one definition of "typical" shared by
    /// the category rows and the whole-month tile.
    fn median_active(
        &self,
        month: MonthKey,
        basis: u32,
        spend_in: impl Fn(MonthKey) -> Decimal,
    ) -> Option<Decimal> {
        let (from, to) = self.window(month, basis)?;
        let mut spends = Vec::new();
        let mut m = from;
        while m <= to {
            let spent = spend_in(m);
            if spent > Decimal::ZERO {
                spends.push(spent);
            }
            m = m.next();
        }
        (!spends.is_empty()).then(|| median(&mut spends))
    }

    /// Total converted spend over an inclusive month range, each month
    /// valued at its own end date.
    pub(crate) fn range_spend(
        &self,
        account: &str,
        (from, to): (MonthKey, MonthKey),
        cur: &str,
    ) -> Decimal {
        let mut total = Decimal::ZERO;
        let mut m = from;
        while m <= to {
            total += self.spent_converted(account, m, cur);
            m = m.next();
        }
        total
    }

    /// Total converted spend over the trailing twelve months — the
    /// sort weight behind the table and the raw material of the
    /// reports page's year breakdown. Amortizing what the year really
    /// cost, empty months and refunds included, ranks a big quarterly
    /// bill above daily noise. `None` when the month has no window.
    pub(crate) fn year_spend(
        &self,
        account: &str,
        month: MonthKey,
        cur: &str,
    ) -> Option<Decimal> {
        Some(self.range_spend(account, self.window(month, 12)?, cur))
    }

    /// Two adjacent windows of `months` each ending the month before
    /// `month`: the recent one, then the one before it.
    ///
    /// `None` unless the ledger covers both in full. Unlike
    /// [`Self::window`] this refuses to clamp, because the whole point
    /// of the pair is the comparison: measuring a year against the four
    /// months that happen to precede it reports a collapse that is only
    /// the edge of the data.
    pub(crate) fn periods(
        &self,
        month: MonthKey,
        months: u32,
    ) -> Option<((MonthKey, MonthKey), (MonthKey, MonthKey))> {
        let start = month.minus(months * 2);
        if months == 0 || start < self.first_txn_month? {
            return None;
        }
        let mid = month.minus(months);
        Some(((mid, month.prev()), (start, mid.prev())))
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

        // Biggest yearly cost first, so the table leads with the
        // categories that really spend the money. The order runs on
        // the trailing-year total — not the displayed basis — so
        // switching the target window never reshuffles the table, and
        // a quarterly tax bill outranks a monthly coffee even though
        // its typical month reads lower. Rows the last year knows
        // nothing about sink to the bottom, ordered by this month's
        // spend, then name. `None < Some` makes the descending Option
        // compare do exactly that.
        let weight_of: HashMap<String, Option<Decimal>> = groups
            .iter()
            .flat_map(|g| &g.categories)
            .map(|c| {
                (c.account.clone(), self.year_spend(&c.account, month, cur))
            })
            .collect();
        for group in &mut groups {
            group.categories.sort_by(|a, b| {
                weight_of[&b.account]
                    .cmp(&weight_of[&a.account])
                    .then_with(|| b.spent.cmp(&a.spent))
                    .then_with(|| a.label.cmp(&b.label))
            });
            group.spent = group.categories.iter().map(|c| c.spent).sum();
            group.avg = sum_present(group.categories.iter().map(|c| c.avg));
        }
        let weight = |g: &Group| {
            sum_present(g.categories.iter().map(|c| weight_of[&c.account]))
        };
        groups.sort_by(|a, b| {
            weight(b)
                .cmp(&weight(a))
                .then_with(|| b.spent.cmp(&a.spent))
                .then_with(|| a.name.cmp(&b.name))
        });

        // The headline typical is the median real month — the median
        // of the window months' total spend — not the sum of the
        // category medians, which would bill every sporadic category
        // every month and read far above any month that ever happened.
        let typical = self
            .median_active(month, basis, |m| {
                groups
                    .iter()
                    .flat_map(|g| &g.categories)
                    .map(|c| self.spent_converted(&c.account, m, cur))
                    .sum()
            })
            .map(cents);

        MonthView {
            month,
            income: cents(income),
            spent: groups.iter().map(|g| g.spent).sum(),
            typical,
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
        let avg = self.typical(account, month, basis, cur).map(cents);
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

    /// The register for one account: the month's transactions with a
    /// running balance, the flows on either side of it, and enough
    /// history to see the shape.
    pub fn account_view(
        &self,
        account: &str,
        month: MonthKey,
        basis: u32,
        cur: &str,
    ) -> Option<AccountView<'_>> {
        let info = self.account(account)?;
        let at = month.end_of_month();

        let mut balances = self.balance_at(account, month);
        self.sort_amounts(&mut balances, cur);
        let unpriced: Vec<String> = self
            .sums(account, month)
            .iter()
            .filter(|(c, _)| self.convert(Decimal::ONE, c, cur, at).is_none())
            .map(|(c, _)| c.clone())
            .collect();

        let opening = self.balance_converted(account, month.prev(), cur);
        let mut running = opening;
        let mut inflow = Decimal::ZERO;
        let mut outflow = Decimal::ZERO;
        let mut entries = Vec::new();
        for txn in self.txns(account, month) {
            let delta = self.txn_delta(txn, account, cur, at);
            match delta {
                Some(d) if d > Decimal::ZERO => inflow += d,
                Some(d) if d < Decimal::ZERO => outflow -= d,
                _ => {}
            }
            running = running.zip(delta).map(|(r, d)| r + d);
            entries.push(AccountEntry {
                txn,
                delta,
                balance: running,
            });
        }

        let history = (0..basis)
            .rev()
            .map(|back| {
                let m = month.minus(back);
                let (inflow, outflow) = self.flows(account, m, cur);
                AccountPoint {
                    month: m,
                    inflow,
                    outflow,
                    balance: self.balance_converted(account, m, cur),
                }
            })
            .collect();

        Some(AccountView {
            account: info.account.clone(),
            label: info.label.clone(),
            kind: info.kind,
            opening,
            converted: self.balance_converted(account, month, cur),
            balances,
            inflow,
            outflow,
            history,
            entries,
            unpriced,
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
        let avg = self.typical(&info.account, month, basis, cur).map(cents);
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
        let mut balances = self.balance_at(&info.account, month);
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

    /// Cumulative balance through the end of `month`, per commodity,
    /// in whatever order the postings arrived in.
    pub(crate) fn balance_at(
        &self,
        account: &str,
        month: MonthKey,
    ) -> Vec<(String, Decimal)> {
        let mut balances: Vec<(String, Decimal)> = Vec::new();
        for (_, sums) in
            self.months_of(account).take_while(|(m, _)| *m <= month)
        {
            for (c, v) in sums {
                add_sum(&mut balances, c, *v);
            }
        }
        balances
    }

    /// The same converted at the month's end. An account that holds
    /// nothing yet is at zero — the `None` is for a balance no price
    /// can put a number on, not for an empty one.
    fn balance_converted(
        &self,
        account: &str,
        month: MonthKey,
        cur: &str,
    ) -> Option<Decimal> {
        let at = month.end_of_month();
        let balances = self.balance_at(account, month);
        if balances.is_empty() {
            return Some(Decimal::ZERO);
        }
        let parts: Vec<Decimal> = balances
            .iter()
            .filter_map(|(c, v)| self.convert(*v, c, cur, at))
            .collect();
        (!parts.is_empty()).then(|| cents(parts.iter().copied().sum()))
    }

    /// What one transaction did to one account, converted at the
    /// month's end. `None` when any leg of it has no price: a partial
    /// total would be a wrong one.
    pub(crate) fn txn_delta(
        &self,
        txn: &Txn,
        account: &str,
        cur: &str,
        at: Day,
    ) -> Option<Decimal> {
        let mut total = Decimal::ZERO;
        for posting in &txn.postings {
            if posting.account != account {
                continue;
            }
            for (value, currency) in &posting.amounts {
                total += self.convert(*value, currency, cur, at)?;
            }
        }
        Some(cents(total))
    }

    /// A month's arrivals and departures, counted per transaction so
    /// they add up to the rows the register shows.
    fn flows(
        &self,
        account: &str,
        month: MonthKey,
        cur: &str,
    ) -> (Decimal, Decimal) {
        let at = month.end_of_month();
        let mut inflow = Decimal::ZERO;
        let mut outflow = Decimal::ZERO;
        for txn in self.txns(account, month) {
            match self.txn_delta(txn, account, cur, at) {
                Some(d) if d > Decimal::ZERO => inflow += d,
                Some(d) if d < Decimal::ZERO => outflow -= d,
                _ => {}
            }
        }
        (inflow, outflow)
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
    pub(crate) fn sort_amounts(
        &self,
        amounts: &mut [(String, Decimal)],
        cur: &str,
    ) {
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

/// Median of a nonempty slice; sorts it in place.
pub(crate) fn median(xs: &mut [Decimal]) -> Decimal {
    xs.sort_unstable();
    let mid = xs.len() / 2;
    if xs.len() % 2 == 1 {
        xs[mid]
    } else {
        (xs[mid - 1] + xs[mid]) / Decimal::from(2)
    }
}

/// Sum of the `Some` values; `None` when every input is `None`.
fn sum_present(
    values: impl Iterator<Item = Option<Decimal>>,
) -> Option<Decimal> {
    values
        .flatten()
        .fold(None, |acc, v| Some(acc.unwrap_or_default() + v))
}
