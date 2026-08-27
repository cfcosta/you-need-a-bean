//! Where the money comes from, and how much of it arrives without you.
//!
//! The page shows income as one number, which hides the two things
//! worth knowing about it: whether it all comes from one place, and
//! how much of it would keep arriving if you stopped working. The
//! second is the whole premise of Coast and Barista FIRE — you don't
//! need the full number if part of the bill already pays itself.

use std::collections::BTreeMap;

use rust_decimal::Decimal;

use crate::model::{Ledger, MonthKey};
use crate::query::cents;

/// Segments that read as money arriving without work. Matched against
/// every segment after `Income:`, case-insensitively, and only when
/// the ledger hasn't said outright with `income:` metadata.
///
/// A guess, and named as one on the page — `Income:Cashback` is a
/// rebate that stops when the spending does, and a referral commission
/// isn't a dividend, so neither is here.
const PASSIVE_SEGMENTS: &[&str] = &[
    "capital",
    "capitalgains",
    "coupon",
    "distribution",
    "distributions",
    "dividend",
    "dividends",
    "gains",
    "interest",
    "investment",
    "investments",
    "rent",
    "rental",
    "rewards",
    "royalties",
    "royalty",
    "staking",
    "yield",
];

/// One `Income:*` group over the window.
#[derive(Debug, Clone)]
pub struct IncomeSource {
    /// The second segment, or the account itself when it has none.
    pub name: String,
    pub total: Decimal,
    /// `total` over all income in the window.
    pub share: Decimal,
    /// The part of `total` from accounts that count as passive. A
    /// group can be mixed — staking rewards and a referral commission
    /// both live under `Income:Crypto`.
    pub passive: Decimal,
}

#[derive(Debug, Clone)]
pub struct IncomeView {
    pub window: Option<(MonthKey, MonthKey)>,
    pub total: Decimal,
    /// Biggest first.
    pub sources: Vec<IncomeSource>,
    /// `1 / Σ share²` — how many equally-sized sources this spread is
    /// worth. Two jobs paying the same is 2.0; one job and a rounding
    /// error is barely above 1.0.
    pub effective_sources: Option<Decimal>,
    pub passive: Decimal,
    /// `passive / total`.
    pub passive_share: Option<Decimal>,
    /// `passive` over the same window's spend: the share of the bill
    /// that already pays itself.
    pub passive_cover: Option<Decimal>,
    /// Accounts the ledger marked `income: passive` outright.
    pub declared: usize,
    /// Accounts taken as passive from their name alone.
    pub inferred: usize,
}

impl Ledger {
    pub(super) fn income_view(
        &self,
        current: MonthKey,
        cur: &str,
    ) -> IncomeView {
        let window = self.window(current, 12);
        let (mut declared, mut inferred) = (0, 0);
        let mut totals: BTreeMap<&str, (Decimal, Decimal)> = BTreeMap::new();
        let mut spend = Decimal::ZERO;

        for info in self.accounts() {
            let Some(window) = window else { break };
            if info.account.starts_with("Expenses:") {
                spend += self.range_spend(&info.account, window, cur);
                continue;
            }
            let Some(rest) = info.account.strip_prefix("Income:") else {
                continue;
            };
            // Income postings are negative in beancount, so what came
            // in is the negation of what the spend helper reports.
            let total = -self.range_spend(&info.account, window, cur);
            if total <= Decimal::ZERO {
                continue;
            }
            let passive = match info.passive {
                Some(passive) => {
                    if passive {
                        declared += 1;
                    }
                    passive
                }
                None => {
                    let guess = rest.split(':').any(|segment| {
                        PASSIVE_SEGMENTS
                            .contains(&segment.to_ascii_lowercase().as_str())
                    });
                    if guess {
                        inferred += 1;
                    }
                    guess
                }
            };
            let group = rest.split(':').next().unwrap_or(rest);
            let entry = totals.entry(group).or_default();
            entry.0 += total;
            if passive {
                entry.1 += total;
            }
        }

        let mut sources: Vec<IncomeSource> = totals
            .into_iter()
            .map(|(name, (total, passive))| IncomeSource {
                name: name.to_string(),
                total: cents(total),
                share: Decimal::ZERO,
                passive: cents(passive),
            })
            .collect();
        sources.sort_by(|a, b| b.total.cmp(&a.total).then(a.name.cmp(&b.name)));

        let total: Decimal = sources.iter().map(|s| s.total).sum();
        let passive: Decimal = sources.iter().map(|s| s.passive).sum();
        let mut concentration = Decimal::ZERO;
        if total > Decimal::ZERO {
            for source in &mut sources {
                source.share = (source.total / total).round_dp(4);
                concentration += source.share * source.share;
            }
        }
        let spend = cents(spend);

        IncomeView {
            window,
            total,
            sources,
            effective_sources: (concentration > Decimal::ZERO)
                .then(|| (Decimal::ONE / concentration).round_dp(2)),
            passive,
            // Not rounded here, unlike the per-source shares. Passive
            // income starts as a rounding error and grows out of one,
            // so a hundredth of a percent is the whole of it for years
            // — and these two are read side by side, where rounding
            // both to nothing makes them look like the same number
            // said twice. The serializer decides what survives.
            passive_share: (total > Decimal::ZERO).then(|| passive / total),
            passive_cover: (spend > Decimal::ZERO).then(|| passive / spend),
            declared,
            inferred,
        }
    }
}
