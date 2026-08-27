//! Where the year went, and whether it went there last year too.
//!
//! The breakdown on its own ranks but never compares, which answers
//! "what do I spend on?" and leaves "why is it more than it was?"
//! sitting right next to it. Each group carries the same twelve months
//! a year earlier so the two questions share a card.

use std::collections::BTreeMap;

use rust_decimal::Decimal;

use crate::model::{Ledger, MonthKey};
use crate::query::cents;

/// One expense group's share of the trailing year.
#[derive(Debug, Clone)]
pub struct YearGroup {
    pub name: String,
    pub total: Decimal,
    /// The same group over the twelve months before. `None` when the
    /// ledger doesn't reach back far enough to hold a whole prior year.
    pub prior: Option<Decimal>,
}

#[derive(Debug, Clone)]
pub struct YearView {
    /// The trailing twelve months, clamped to the ledger's first
    /// activity.
    pub window: Option<(MonthKey, MonthKey)>,
    /// The twelve months before that, and only when the ledger holds
    /// all of them — see [`Ledger::periods`].
    pub prior_window: Option<(MonthKey, MonthKey)>,
    /// Converted spend per expense group over `window`, biggest first.
    /// A group is kept when either year spent something on it, so a
    /// category that stopped entirely still shows up as the fall it is.
    pub groups: Vec<YearGroup>,
}

impl Ledger {
    pub(super) fn year_view(&self, current: MonthKey, cur: &str) -> YearView {
        let window = self.window(current, 12);
        let prior_window = self.periods(current, 12).map(|(_, prior)| prior);

        // Hidden accounts included, the same every-account stance the
        // flows above take.
        let mut totals: BTreeMap<&str, (Decimal, Decimal)> = BTreeMap::new();
        for info in self.accounts() {
            let Some(group) = &info.group else { continue };
            let entry = totals.entry(group).or_default();
            if let Some(window) = window {
                entry.0 += self.range_spend(&info.account, window, cur);
            }
            if let Some(prior) = prior_window {
                entry.1 += self.range_spend(&info.account, prior, cur);
            }
        }

        let mut groups: Vec<YearGroup> = totals
            .into_iter()
            .map(|(name, (total, prior))| YearGroup {
                name: name.to_string(),
                total: cents(total),
                prior: prior_window.map(|_| cents(prior)),
            })
            .filter(|g| {
                g.total > Decimal::ZERO
                    || g.prior.is_some_and(|p| p > Decimal::ZERO)
            })
            .collect();
        groups.sort_by(|a, b| b.total.cmp(&a.total).then(a.name.cmp(&b.name)));

        YearView {
            window,
            prior_window,
            groups,
        }
    }
}
