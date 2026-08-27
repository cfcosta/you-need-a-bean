//! The charges that come back: subscriptions, rent, premiums — anything
//! that lands on a regular cadence for an amount you could have named
//! in advance.
//!
//! Two things have to hold before a repeated payee counts. The gaps
//! between charges have to cluster on one cadence, and the amounts have
//! to sit still. A supermarket you visit every month fails the second
//! test, which is the point: a recurring charge is one you can plan
//! around, and a number that moves every time is not one.
//!
//! What falls out is the fixed monthly nut, which the lean FIRE target
//! and the lean runway are both built on, plus the price rises that
//! happened quietly along the way.

use std::collections::BTreeMap;

use rust_decimal::Decimal;

use crate::model::{Day, Ledger, MonthKey, days_between};
use crate::query::cents;

/// How often a charge comes back.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cadence {
    Weekly,
    Biweekly,
    Monthly,
    Quarterly,
    Yearly,
}

impl Cadence {
    /// The cadence whose period the median gap sits closest to, if the
    /// gap is close enough to any of them to be one.
    fn of(days: i64) -> Option<Self> {
        // Bands are wide enough for a billing date that drifts with the
        // month's length, narrow enough not to overlap.
        Some(match days {
            5..=9 => Self::Weekly,
            12..=17 => Self::Biweekly,
            26..=35 => Self::Monthly,
            80..=100 => Self::Quarterly,
            340..=390 => Self::Yearly,
            _ => return None,
        })
    }

    /// Nominal length in days, for normalizing to a month.
    fn days(self) -> Decimal {
        Decimal::from(match self {
            Self::Weekly => 7,
            Self::Biweekly => 14,
            Self::Monthly => 30,
            Self::Quarterly => 91,
            Self::Yearly => 365,
        })
    }

    /// How many of these fit in a month.
    fn per_month(self) -> Decimal {
        match self {
            Self::Monthly => Decimal::ONE,
            Self::Quarterly => Decimal::ONE / Decimal::from(3),
            Self::Yearly => Decimal::ONE / Decimal::from(12),
            // A month is not a whole number of weeks, so these two are
            // the only ones that need the calendar.
            other => Decimal::from(365) / Decimal::from(12) / other.days(),
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Weekly => "weekly",
            Self::Biweekly => "fortnightly",
            Self::Monthly => "monthly",
            Self::Quarterly => "quarterly",
            Self::Yearly => "yearly",
        }
    }
}

/// A charge that went up or down and stayed there.
#[derive(Debug, Clone, Copy)]
pub struct PriceChange {
    pub from: Decimal,
    pub to: Decimal,
    /// What the difference costs over a year at this cadence.
    pub annual: Decimal,
    /// The first charge at the new amount.
    pub since: Day,
}

#[derive(Debug, Clone)]
pub struct Recurring {
    pub account: String,
    /// The account's pretty name.
    pub label: String,
    pub payee: String,
    pub cadence: Cadence,
    /// The most recent charge — what it costs now, not on average.
    pub amount: Decimal,
    /// `amount` spread over a month.
    pub monthly: Decimal,
    pub count: usize,
    pub first: Day,
    pub last: Day,
    /// Whether it is still charging, which is what makes it a cost you
    /// have to cover next month.
    pub active: bool,
    pub change: Option<PriceChange>,
}

#[derive(Debug, Clone, Default)]
pub struct RecurringView {
    /// The span the detection looked at.
    pub window: Option<(MonthKey, MonthKey)>,
    /// What the still-charging series cost each month — the fixed nut.
    ///
    /// This is a floor, not a bill. It counts only what could be
    /// recognized as recurring, so a ledger whose payees are thin
    /// reports a small number here and everything built on it — the
    /// lean FIRE target, the lean runway — inherits that. Read it next
    /// to [`Self::coverage`].
    pub monthly_fixed: Decimal,
    pub annual_fixed: Decimal,
    /// `monthly_fixed` over the monthly spend, so the page can say how
    /// much of the spending the fixed nut actually accounts for.
    pub coverage: Option<Decimal>,
    /// Biggest monthly cost first, with the ones that stopped last.
    pub items: Vec<Recurring>,
}

/// How far back to look for a cadence. Long enough for a yearly charge
/// to show up three times, which is the least that proves a pattern.
const LOOKBACK_MONTHS: u32 = 36;

/// How recently a series must have charged to be worth reporting at
/// all, even as one that stopped.
const RELEVANT_MONTHS: u32 = 12;

/// One charge in a candidate series.
struct Charge {
    date: Day,
    amount: Decimal,
}

impl Ledger {
    /// The charges that repeat on a cadence for a steady amount.
    pub(super) fn recurring_view(
        &self,
        current: MonthKey,
        cur: &str,
        monthly_spend: Decimal,
    ) -> RecurringView {
        let from = current.minus(LOOKBACK_MONTHS);
        let cutoff = current.minus(RELEVANT_MONTHS).end_of_month();
        let today = current.end_of_month();

        // (account, payee) is the series. The same payee billing two
        // different accounts is two charges, which is what you want:
        // they can move apart.
        let mut series: BTreeMap<(&str, &str), Vec<Charge>> = BTreeMap::new();
        for txn in &self.txns {
            let Some(payee) = txn.payee.as_deref().map(str::trim) else {
                continue;
            };
            if payee.is_empty() || MonthKey::new(txn.date.0, txn.date.1) < from
            {
                continue;
            }
            for posting in &txn.postings {
                if !posting.account.starts_with("Expenses:") {
                    continue;
                }
                let amount: Decimal = posting
                    .amounts
                    .iter()
                    .filter_map(|(v, c)| self.convert(*v, c, cur, txn.date))
                    .sum();
                // Refunds and corrections are not charges.
                if amount <= Decimal::ZERO {
                    continue;
                }
                series.entry((&posting.account, payee)).or_default().push(
                    Charge {
                        date: txn.date,
                        amount,
                    },
                );
            }
        }

        let mut items: Vec<Recurring> = series
            .into_iter()
            .filter_map(|((account, payee), mut charges)| {
                charges.sort_by_key(|c| c.date);
                let last = charges.last()?.date;
                if last < cutoff {
                    return None;
                }
                let cadence = cadence_of(&charges)?;
                self.describe(account, payee, cadence, &charges, today)
            })
            .collect();

        // Still-charging first and biggest first inside each, so the
        // top of the list is the top of next month's bill.
        items.sort_by(|a, b| {
            b.active
                .cmp(&a.active)
                .then(b.monthly.cmp(&a.monthly))
                .then(a.payee.cmp(&b.payee))
        });

        let monthly_fixed =
            cents(items.iter().filter(|i| i.active).map(|i| i.monthly).sum());
        RecurringView {
            window: (!items.is_empty()).then_some((from, current)),
            monthly_fixed,
            annual_fixed: cents(monthly_fixed * Decimal::from(12)),
            coverage: (monthly_spend > Decimal::ZERO)
                .then(|| (monthly_fixed / monthly_spend).round_dp(4)),
            items,
        }
    }

    /// Turn a series that passed the cadence test into a reportable
    /// charge, or drop it because the amounts never settled.
    fn describe(
        &self,
        account: &str,
        payee: &str,
        cadence: Cadence,
        charges: &[Charge],
        today: Day,
    ) -> Option<Recurring> {
        let amounts: Vec<Decimal> = charges.iter().map(|c| c.amount).collect();
        let mid = median(&amounts)?;
        if mid <= Decimal::ZERO || spread(&amounts, mid) > Decimal::new(25, 2) {
            return None;
        }
        let (first, last) = (charges.first()?, charges.last()?);
        let amount = cents(last.amount);
        // One and a half cadences of grace: a monthly charge that has
        // not landed in six weeks has stopped.
        let grace = cadence.days() * Decimal::new(15, 1);
        let active = Decimal::from(days_between(last.date, today)) <= grace;

        Some(Recurring {
            account: account.to_string(),
            label: self
                .account(account)
                .map_or_else(|| account.to_string(), |a| a.label.clone()),
            payee: payee.to_string(),
            cadence,
            amount,
            monthly: cents(amount * cadence.per_month()),
            count: charges.len(),
            first: first.date,
            last: last.date,
            active,
            change: price_change(charges, cadence),
        })
    }
}

/// The cadence the gaps agree on, if they agree on one.
fn cadence_of(charges: &[Charge]) -> Option<Cadence> {
    // Two points make a gap, not a pattern.
    if charges.len() < 3 {
        return None;
    }
    let gaps: Vec<i64> = charges
        .windows(2)
        .map(|w| days_between(w[0].date, w[1].date))
        .collect();
    let mut sorted = gaps.clone();
    sorted.sort_unstable();
    let cadence = Cadence::of(sorted[sorted.len() / 2])?;
    // The median alone would accept a series that fires twice a year and
    // once a fortnight; most of the gaps have to agree with it.
    let agreeing = gaps.iter().filter(|&&g| Cadence::of(g) == Some(cadence));
    (agreeing.count() * 3 >= gaps.len() * 2).then_some(cadence)
}

/// Median absolute deviation over the median: how much the amounts move,
/// as a fraction of what they usually are.
fn spread(amounts: &[Decimal], mid: Decimal) -> Decimal {
    let deviations: Vec<Decimal> =
        amounts.iter().map(|a| (*a - mid).abs()).collect();
    median(&deviations).unwrap_or_default() / mid
}

fn median(values: &[Decimal]) -> Option<Decimal> {
    let mut sorted = values.to_vec();
    sorted.sort_unstable();
    sorted.get(sorted.len() / 2).copied()
}

/// A step in the amount that the series then kept, ignoring the noise of
/// a single odd charge.
fn price_change(charges: &[Charge], cadence: Cadence) -> Option<PriceChange> {
    let now = cents(charges.last()?.amount);
    // The oldest amount that is not the current one, read backwards, is
    // what it used to cost.
    let earlier = charges
        .iter()
        .rev()
        .map(|c| cents(c.amount))
        .find(|&a| a != now)?;
    let step = now - earlier;
    // A charge billed in another currency drifts with the exchange rate
    // every month without anyone raising a price, so a rise has to be
    // big enough to be a decision rather than the market.
    if step.abs() < earlier.abs() * Decimal::new(2, 2) {
        return None;
    }
    let since = charges
        .iter()
        .rev()
        .take_while(|c| cents(c.amount) == now)
        .last()?
        .date;
    Some(PriceChange {
        from: earlier,
        to: now,
        annual: cents(step * cadence.per_month() * Decimal::from(12)),
        since,
    })
}
