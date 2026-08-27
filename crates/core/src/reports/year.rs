//! Where the year went, month by month, and whether it went there
//! last year too.
//!
//! A ranked breakdown answers "what do I spend on?" and leaves "why
//! is it more than it was?" sitting right next to it — and a year
//! total answers neither honestly on its own, because 3,000 spent
//! once in November is a trip and 3,000 spread over twelve months is
//! a habit. So every group carries the window's months laid out in
//! order, the median month that says what one of them usually costs,
//! and the same twelve months a year earlier.

use std::collections::BTreeMap;

use rust_decimal::Decimal;

use crate::model::{Ledger, MonthKey};
use crate::query::{cents, median};

/// One month of the trailing year, across every expense group.
#[derive(Debug, Clone, Copy)]
pub struct YearMonth {
    pub month: MonthKey,
    pub total: Decimal,
    /// The same calendar month a year earlier — what turns "November
    /// was huge" into "November is always huge", or doesn't. `None`
    /// without a prior window, which is all-or-nothing; see
    /// [`YearView::prior_window`].
    pub prior: Option<Decimal>,
}

/// One expense group's share of the trailing year.
#[derive(Debug, Clone)]
pub struct YearGroup {
    pub name: String,
    pub total: Decimal,
    /// The same group over the twelve months before. `None` when the
    /// ledger doesn't reach back far enough to hold a whole prior year.
    pub prior: Option<Decimal>,
    /// Spend in each of [`YearView::months`], in that order and always
    /// that long — a group that stopped keeps its zeroes so its cells
    /// stay under their months.
    pub monthly: Vec<Decimal>,
    /// The median of the months this group was actually paid, the same
    /// definition of typical the budget page uses: a skipped month
    /// says nothing about what one costs, and one emergency cannot
    /// drag the middle. `None` when it was never paid.
    pub typical: Option<Decimal>,
}

#[derive(Debug, Clone)]
pub struct YearView {
    /// The trailing twelve months, clamped to the ledger's first
    /// activity.
    pub window: Option<(MonthKey, MonthKey)>,
    /// The twelve months before that, and only when the ledger holds
    /// all of them — see [`Ledger::periods`].
    pub prior_window: Option<(MonthKey, MonthKey)>,
    /// The window's months in order, oldest first: the axis every
    /// group's [`YearGroup::monthly`] is indexed by.
    pub months: Vec<YearMonth>,
    /// What a month of this year usually costs — the median of the
    /// months that saw any spending at all.
    pub typical: Option<Decimal>,
    /// Converted spend per expense group over `window`, biggest first.
    /// A group is kept when either year spent something on it, so a
    /// category that stopped entirely still shows up as the fall it is.
    pub groups: Vec<YearGroup>,
}

/// A group's per-month spend over each window, in window order. The
/// prior side is empty until there is a prior year to fill it.
#[derive(Default)]
struct Tally {
    monthly: Vec<Decimal>,
    prior: Vec<Decimal>,
}

impl Ledger {
    pub(super) fn year_view(&self, current: MonthKey, cur: &str) -> YearView {
        let window = self.window(current, 12);
        let prior_window = self.periods(current, 12).map(|(_, prior)| prior);
        let axis = window.map(months_in).unwrap_or_default();
        let prior_axis = prior_window.map(months_in).unwrap_or_default();

        // Hidden accounts included, the same every-account stance the
        // flows above take.
        let mut totals: BTreeMap<&str, Tally> = BTreeMap::new();
        for info in self.accounts() {
            let Some(group) = &info.group else { continue };
            let tally = totals.entry(group).or_insert_with(|| Tally {
                monthly: vec![Decimal::ZERO; axis.len()],
                prior: vec![Decimal::ZERO; prior_axis.len()],
            });
            let spend = |slots: &mut [Decimal], months: &[MonthKey]| {
                for (slot, &month) in slots.iter_mut().zip(months) {
                    *slot += self.spent_converted(&info.account, month, cur);
                }
            };
            spend(&mut tally.monthly, &axis);
            spend(&mut tally.prior, &prior_axis);
        }

        // Cents at the month, not at the year: the strip a reader adds
        // up by eye is the one the totals have to agree with. The
        // prior year's months are quantized the same way and kept
        // beside each group, since the ribbon needs them month by
        // month and only the group's own year total is worth showing.
        let mut rows: Vec<(YearGroup, Vec<Decimal>)> = totals
            .into_iter()
            .map(|(name, tally)| {
                let monthly = to_cents(tally.monthly);
                let prior_monthly = to_cents(tally.prior);
                let group = YearGroup {
                    name: name.to_string(),
                    total: monthly.iter().sum(),
                    prior: prior_window.map(|_| prior_monthly.iter().sum()),
                    typical: typical_of(&monthly),
                    monthly,
                };
                (group, prior_monthly)
            })
            .filter(|(g, _)| {
                g.total > Decimal::ZERO
                    || g.prior.is_some_and(|p| p > Decimal::ZERO)
            })
            .collect();
        rows.sort_by(|(a, _), (b, _)| {
            b.total.cmp(&a.total).then(a.name.cmp(&b.name))
        });

        // Both axes are twelve long whenever the prior one exists at
        // all, so month `i` really is the same month a year earlier.
        let months: Vec<YearMonth> = axis
            .iter()
            .enumerate()
            .map(|(i, &month)| YearMonth {
                month,
                total: rows.iter().map(|(g, _)| g.monthly[i]).sum(),
                prior: (i < prior_axis.len())
                    .then(|| rows.iter().map(|(_, p)| p[i]).sum()),
            })
            .collect();
        let totals: Vec<Decimal> = months.iter().map(|m| m.total).collect();
        let groups: Vec<YearGroup> = rows.into_iter().map(|(g, _)| g).collect();

        YearView {
            window,
            prior_window,
            typical: typical_of(&totals),
            months,
            groups,
        }
    }
}

/// The months of an inclusive range, oldest first.
fn months_in((from, to): (MonthKey, MonthKey)) -> Vec<MonthKey> {
    let mut months = Vec::new();
    let mut m = from;
    while m <= to {
        months.push(m);
        m = m.next();
    }
    months
}

fn to_cents(raw: Vec<Decimal>) -> Vec<Decimal> {
    raw.into_iter().map(cents).collect()
}

/// The median of the months something was actually paid.
fn typical_of(monthly: &[Decimal]) -> Option<Decimal> {
    let mut paid: Vec<Decimal> = monthly
        .iter()
        .copied()
        .filter(|v| *v > Decimal::ZERO)
        .collect();
    (!paid.is_empty()).then(|| median(&mut paid))
}
