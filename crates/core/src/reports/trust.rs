//! What the rest of this page isn't saying.
//!
//! Every figure on the reports page is a conversion away from wrong.
//! A commodity nothing prices is missing from the totals outright; one
//! priced two years ago is present at a price two years old, which is
//! worse, because it looks like a number. Add the transactions the
//! ledger itself marks unconfirmed, the spend that landed in a
//! catch-all, and whatever the loader complained about on the way in,
//! and you have the list of reasons to doubt the ones above.
//!
//! Each row is a count and an amount. The count says how much work is
//! outstanding; the amount says how much of the page depends on it.

use std::cmp::Reverse;

use rust_decimal::Decimal;

use crate::model::{Day, Ledger, MonthKey, Txn, add_sum, days_between};
use crate::query::cents;

/// How old a price has to be before the valuation resting on it is
/// worth naming. Six weeks: long enough that a commodity priced once a
/// month never trips it, short enough to catch a quarter of drift.
const STALE_DAYS: i64 = 45;

/// How many flagged transactions the view names. Enough to recognise
/// the batch they came from, not enough to be a transaction list.
const RECENT: usize = 5;

/// Account segments that mean "I haven't decided yet", matched
/// case-insensitively against every segment after `Expenses:`.
///
/// `Misc` is deliberately absent: miscellaneous is a decision, and a
/// ledger that files coffee under it is not incomplete.
const CATCHALL_SEGMENTS: &[&str] =
    &["fixme", "todo", "uncategorized", "unclassified", "unknown"];

/// A commodity still held, valued at a price from a while ago.
#[derive(Debug, Clone)]
pub struct StalePrice {
    pub commodity: String,
    /// The date of the price the current valuation actually used.
    pub last: Day,
    /// How many days before today that was.
    pub days: i64,
    /// What that price is holding up, in the display currency.
    pub value: Decimal,
}

/// One transaction the ledger marks `!`.
#[derive(Debug, Clone)]
pub struct FlaggedTxn {
    pub date: Day,
    pub payee: Option<String>,
    pub narration: Option<String>,
    /// What it moved: the sum of its positive postings, converted.
    pub amount: Decimal,
}

#[derive(Debug, Clone)]
pub struct Flagged {
    /// Every `!` in the ledger, however old.
    pub total: usize,
    /// The ones inside the window, which are the ones the figures on
    /// this page are built from.
    pub window: usize,
    /// What those moved.
    pub amount: Decimal,
    /// Newest first.
    pub recent: Vec<FlaggedTxn>,
}

#[derive(Debug, Clone)]
pub struct Uncategorized {
    /// Window spend that landed in a catch-all account.
    pub total: Decimal,
    /// `total` over all window spend.
    pub share: Option<Decimal>,
    /// The catch-all accounts that actually took money, biggest first.
    pub accounts: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct TrustView {
    /// The trailing year the money figures cover, matching the year
    /// and income cards.
    pub window: Option<(MonthKey, MonthKey)>,
    /// Held commodities whose newest price is older than six weeks,
    /// most money first.
    pub stale: Vec<StalePrice>,
    pub flagged: Flagged,
    pub uncategorized: Uncategorized,
    /// What the loader said while reading the files — includes the
    /// postings the parser dropped, which would otherwise vanish.
    pub warnings: Vec<String>,
}

impl Ledger {
    pub(super) fn trust_view(
        &self,
        today: Day,
        current: MonthKey,
        cur: &str,
    ) -> TrustView {
        let window = self.window(current, 12);
        TrustView {
            window,
            stale: self.stale_prices(today, cur),
            flagged: self.flagged(window, cur),
            uncategorized: self.uncategorized(window, cur),
            warnings: self.warnings.clone(),
        }
    }

    /// Everything still on the balance sheet today, valued at a price
    /// that predates today by more than [`STALE_DAYS`].
    fn stale_prices(&self, today: Day, cur: &str) -> Vec<StalePrice> {
        let through = MonthKey::new(today.0, today.1);
        let mut held: Vec<(String, Decimal)> = Vec::new();
        for info in self.accounts() {
            if !info.account.starts_with("Assets:")
                && !info.account.starts_with("Liabilities:")
            {
                continue;
            }
            for (month, sums) in self.months_of(&info.account) {
                if month > through {
                    break;
                }
                for (currency, value) in sums {
                    add_sum(&mut held, currency, *value);
                }
            }
        }

        let mut stale: Vec<StalePrice> = held
            .iter()
            .filter(|(_, value)| !value.is_zero())
            .filter_map(|(commodity, value)| {
                let last = self.priced_at(commodity, cur, today)?;
                let days = days_between(last, today);
                if days < STALE_DAYS {
                    return None;
                }
                let value = cents(self.convert(*value, commodity, cur, today)?);
                // Dust left over from a closed position: the price may
                // be years old, but nothing on the page is standing on
                // it, and a row that says so is noise.
                if value.is_zero() {
                    return None;
                }
                Some(StalePrice {
                    commodity: commodity.clone(),
                    last,
                    days,
                    value,
                })
            })
            .collect();
        // Ranked by what rests on the price, not by how old it is: a
        // three-year-old price on a dust balance is a curiosity.
        stale.sort_by(|a, b| {
            b.value
                .abs()
                .cmp(&a.value.abs())
                .then(a.commodity.cmp(&b.commodity))
        });
        stale
    }

    fn flagged(
        &self,
        window: Option<(MonthKey, MonthKey)>,
        cur: &str,
    ) -> Flagged {
        let mut flagged: Vec<&Txn> =
            self.txns.iter().filter(|txn| txn.flag == '!').collect();
        let total = flagged.len();

        let mut count = 0;
        let mut amount = Decimal::ZERO;
        if let Some((from, to)) = window {
            for txn in &flagged {
                let month = MonthKey::new(txn.date.0, txn.date.1);
                if month >= from && month <= to {
                    count += 1;
                    amount += self.txn_size(txn, cur);
                }
            }
        }

        flagged.sort_by_key(|txn| Reverse(txn.date));
        flagged.truncate(RECENT);
        Flagged {
            total,
            window: count,
            amount: cents(amount),
            recent: flagged
                .into_iter()
                .map(|txn| FlaggedTxn {
                    date: txn.date,
                    payee: txn.payee.clone(),
                    narration: txn.narration.clone(),
                    amount: self.txn_size(txn, cur),
                })
                .collect(),
        }
    }

    /// What a transaction moved: the sum of its positive postings,
    /// which is the whole of it once beancount's identity holds.
    fn txn_size(&self, txn: &Txn, cur: &str) -> Decimal {
        cents(
            txn.postings
                .iter()
                .flat_map(|posting| &posting.amounts)
                .filter(|(value, _)| *value > Decimal::ZERO)
                .filter_map(|(value, currency)| {
                    self.convert(*value, currency, cur, txn.date)
                })
                .sum(),
        )
    }

    fn uncategorized(
        &self,
        window: Option<(MonthKey, MonthKey)>,
        cur: &str,
    ) -> Uncategorized {
        let mut total = Decimal::ZERO;
        let mut spend = Decimal::ZERO;
        let mut accounts: Vec<(String, Decimal)> = Vec::new();
        if let Some(window) = window {
            for info in self.accounts() {
                let Some(rest) = info.account.strip_prefix("Expenses:") else {
                    continue;
                };
                let spent = self.range_spend(&info.account, window, cur);
                spend += spent;
                if spent > Decimal::ZERO && is_catchall(rest) {
                    total += spent;
                    accounts.push((info.account.clone(), spent));
                }
            }
        }
        accounts.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));

        let (total, spend) = (cents(total), cents(spend));
        Uncategorized {
            total,
            share: (spend > Decimal::ZERO).then(|| (total / spend).round_dp(4)),
            accounts: accounts.into_iter().map(|(name, _)| name).collect(),
        }
    }
}

fn is_catchall(rest: &str) -> bool {
    rest.split(':').any(|segment| {
        CATCHALL_SEGMENTS.contains(&segment.to_ascii_lowercase().as_str())
    })
}
