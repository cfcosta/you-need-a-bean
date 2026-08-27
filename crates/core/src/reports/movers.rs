//! The categories that moved: the last quarter's spend against the
//! quarter before it.
//!
//! Ranked by money, not by percentage. A category doubling from 5 to 10
//! is a 100% rise and answers nothing; one climbing 2,000 to 2,400 is
//! 20% and is the entire reason the year looks worse. Falls rank
//! alongside rises, because a quarter that came in under is worth the
//! same sentence as one that came in over.

use rust_decimal::Decimal;

use crate::model::{Ledger, MonthKey};
use crate::query::cents;

/// Months per period. A quarter is long enough that one late invoice
/// can't invent a trend, and short enough that what it finds is still
/// worth acting on.
const PERIOD_MONTHS: u32 = 3;

/// The most movers to name. Past this the list stops being an answer
/// and becomes the category table again.
const LIMIT: usize = 8;

/// A category whose spend moved between two adjacent quarters.
#[derive(Debug, Clone)]
pub struct Mover {
    pub account: String,
    pub label: String,
    /// The expense group it belongs to, for grouping in the UI.
    pub group: Option<String>,
    pub recent: Decimal,
    pub prior: Decimal,
    /// `recent - prior`; positive means more was spent.
    pub delta: Decimal,
    /// `delta / prior`. `None` when nothing was spent before — a
    /// category that appeared, which no percentage describes.
    pub ratio: Option<Decimal>,
}

#[derive(Debug, Clone)]
pub struct MoversView {
    /// The two quarters compared, recent first. Both `None` unless the
    /// ledger covers six whole months before the current one.
    pub recent: Option<(MonthKey, MonthKey)>,
    pub prior: Option<(MonthKey, MonthKey)>,
    /// Total categorized spend in each, so a single move can be read
    /// against the quarter it happened in.
    pub recent_total: Decimal,
    pub prior_total: Decimal,
    /// Biggest movers first, rises and falls together.
    pub items: Vec<Mover>,
}

impl Ledger {
    pub(super) fn movers_view(
        &self,
        current: MonthKey,
        cur: &str,
    ) -> MoversView {
        let Some((recent, prior)) = self.periods(current, PERIOD_MONTHS) else {
            return MoversView {
                recent: None,
                prior: None,
                recent_total: Decimal::ZERO,
                prior_total: Decimal::ZERO,
                items: Vec::new(),
            };
        };

        let mut items = Vec::new();
        let (mut recent_total, mut prior_total) =
            (Decimal::ZERO, Decimal::ZERO);
        for info in self.accounts() {
            if info.group.is_none() {
                continue;
            }
            let r = cents(self.range_spend(&info.account, recent, cur));
            let p = cents(self.range_spend(&info.account, prior, cur));
            recent_total += r;
            prior_total += p;
            items.push(Mover {
                account: info.account.clone(),
                label: info.label.clone(),
                group: info.group.clone(),
                recent: r,
                prior: p,
                delta: r - p,
                ratio: (p > Decimal::ZERO).then(|| ((r - p) / p).round_dp(4)),
            });
        }

        // A move has to be worth a sentence. One percent of the bigger
        // quarter keeps rounding and one-off cents out of a list whose
        // only job is to say what changed, and it scales with the
        // ledger instead of assuming a currency.
        let floor = recent_total.max(prior_total) * Decimal::new(1, 2);
        items.retain(|m| !m.delta.is_zero() && m.delta.abs() >= floor);
        items.sort_by(|a, b| {
            b.delta
                .abs()
                .cmp(&a.delta.abs())
                .then(a.account.cmp(&b.account))
        });
        items.truncate(LIMIT);

        MoversView {
            recent: Some(recent),
            prior: Some(prior),
            recent_total,
            prior_total,
            items,
        }
    }
}
