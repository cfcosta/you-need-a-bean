//! What the portfolio is actually made of.
//!
//! The net worth chart stacks holdings into one band and the growth
//! card splits a move into what you saved and what the market did.
//! Neither says *which* holding, and that is the only question this
//! answers: per position, what it is worth, what share of the whole
//! that is, and — where the ledger recorded enough to say — what it
//! cost.
//!
//! Cost is the part that has to be handled carefully. Beancount records
//! a lot with `{...}`, and a ledger that buys shares through a broker
//! has one on every acquisition. A ledger that accumulates a commodity
//! some other way — staking rewards, commission rebates, airdrops,
//! swaps booked through an equity adjustment — has one on none of them,
//! because none of those were a purchase and there was never a price
//! paid to record. Treating that absence as a cost of zero reports a
//! gain of +100% on the entire holding, which is not a rounding
//! problem but a fabrication. So a position reports a basis only when
//! every unit of it arrived carrying one, and the view says how much of
//! the portfolio that covers, in the same spirit as the unpriced
//! commodities the rest of the page already refuses to guess at.
//!
//! Where a basis exists it is an average, not a lot-by-lot match: a
//! disposal written `{}` means "out of whichever lot", which is exactly
//! the statement average cost makes. It is an accounting figure and not
//! a tax basis — jurisdictions have their own rules about what an
//! acquisition cost is, and this does not implement any of them.

use std::collections::BTreeMap;

use rust_decimal::Decimal;

use crate::model::{Day, Ledger, MonthKey};
use crate::query::cents;

/// A position worth less than this share of the portfolio is folded
/// into one line. Eight rows of rounding error make a table that has to
/// be read past rather than read, and the leftovers of a closed
/// position are not a holding anyone is deciding anything about. The
/// floor is a share rather than an amount so it scales with the ledger
/// instead of assuming a currency, the same reason the movers card
/// measures its cutoff against the quarter it came from.
const DUST_SHARE: u64 = 1000;

/// A unit that arrived without a cost only withholds the position's
/// basis if there is meaningfully any of it left. Selling a tranche
/// reduces the uncosted pool proportionally, and proportional means
/// division, which leaves a remainder in the last digits — this is the
/// floor under that remainder, not a tolerance for missing data.
const COSTED: u64 = 1_000_000;

/// One commodity held, wherever it is held.
#[derive(Debug, Clone)]
pub struct Position {
    pub currency: String,
    /// `name:` from the commodity directive, else the ticker itself.
    pub label: String,
    /// `asset-class:` from the commodity directive. Taken as the ledger
    /// writes it; a commodity nobody declared has none.
    pub class: Option<String>,
    pub units: Decimal,
    /// Price of one unit in the report currency, at the anchor date.
    pub price: Decimal,
    pub value: Decimal,
    /// `value` over the whole portfolio, dust included, so the shares
    /// are against everything held and not against what is listed.
    pub share: Decimal,
    /// What the units cost, averaged, or `None` when any part of the
    /// holding arrived without a lot to average.
    pub basis: Option<Decimal>,
    /// `value - basis`.
    pub gain: Option<Decimal>,
    /// `gain / basis`.
    pub ret: Option<Decimal>,
    /// Accounts it sits in. A holding moved between brokers is one
    /// position with two, not two positions.
    pub accounts: usize,
    /// Postings that touched it. A three-posting stock and a
    /// thousand-posting token are different kinds of thing.
    pub postings: usize,
    pub first: Day,
    pub last: Day,
}

/// What the ledger calls a kind of holding, and how much is in it.
#[derive(Debug, Clone)]
pub struct AssetClass {
    /// `None` for holdings no commodity directive classified.
    pub name: Option<String>,
    pub value: Decimal,
    pub share: Decimal,
    pub positions: usize,
}

#[derive(Debug, Clone)]
pub struct InvestmentsView {
    /// Largest first, dust folded out.
    pub items: Vec<Position>,
    /// Largest first, unclassified last.
    pub classes: Vec<AssetClass>,
    /// Every position, dust included.
    pub total: Decimal,
    /// Positions folded out of `items`, and what they came to together.
    pub dust: usize,
    pub dust_value: Decimal,
    /// Cost of the positions that recorded one.
    pub basis: Decimal,
    /// What those same positions are worth now, so the gain below is
    /// never read against a total it does not cover.
    pub based_value: Decimal,
    pub gain: Decimal,
    /// `gain / basis`.
    pub ret: Option<Decimal>,
    /// `based_value / total` — how much of the portfolio the gain
    /// speaks for.
    pub coverage: Option<Decimal>,
    /// Held, valued, and with nothing to compare against.
    pub unbased: Decimal,
    pub unbased_count: usize,
    /// `1 / Σ share²` — how many equally-sized holdings this spread is
    /// worth, the same measure the income card puts on sources.
    pub effective: Option<Decimal>,
    /// Commodities held that nothing prices in the report currency.
    /// They are missing from every figure above.
    pub unpriced: Vec<String>,
}

/// What one commodity has accumulated in one account, as the
/// transactions are walked.
///
/// Keyed by account and not by commodity alone, because that is where
/// beancount keeps an inventory: a sale posted against one broker can
/// only be filled out of the lots that broker is holding. Averaging a
/// position across every account it has ever sat in charges a disposal
/// at one broker against the shares bought at another, which for a
/// holding that moved brokers mid-life is a basis nobody ever paid.
#[derive(Clone, Default)]
struct Pool {
    units: Decimal,
    /// Cost of the units that arrived with one.
    basis: Decimal,
    /// Units in the pool that arrived without one.
    uncosted: Decimal,
}

/// The parts of a position that are the same wherever it is held.
#[derive(Default)]
struct Trace {
    postings: usize,
    span: Option<(Day, Day)>,
}

impl Pool {
    /// Units arriving. A lot the ledger converted into the report
    /// currency joins the basis; anything else joins the pool that has
    /// none, and takes the position's whole basis down with it.
    fn acquire(&mut self, units: Decimal, cost: Option<Decimal>) {
        self.units += units;
        match cost {
            Some(cost) => self.basis += cost,
            None => self.uncosted += units,
        }
    }

    /// Units leaving, at the pool's average — which is what a disposal
    /// written `{}` asks for. Both pools shrink by the same fraction,
    /// so a tranche that arrived uncosted and has since been sold stops
    /// withholding the basis of the units that are left. A disposal
    /// larger than the pool empties it and keeps going negative rather
    /// than being clipped to what was there: the units are the balance
    /// sheet's, and quietly dropping the overhang would leave every
    /// later acquisition stacked on top of a sale that never landed.
    fn release(&mut self, units: Decimal) {
        let gone = units.abs();
        if self.units > Decimal::ZERO {
            let kept = (self.units - gone).max(Decimal::ZERO) / self.units;
            self.basis *= kept;
            self.uncosted *= kept;
        }
        self.units -= gone;
    }
}

impl Ledger {
    /// Every commodity position on the balance sheet at `today`.
    pub(super) fn investments_view(
        &self,
        today: Day,
        cur: &str,
    ) -> InvestmentsView {
        let mut pools: BTreeMap<(&str, &str), Pool> = BTreeMap::new();
        let mut traces: BTreeMap<&str, Trace> = BTreeMap::new();

        let through = MonthKey::new(today.0, today.1);
        let mut order: Vec<usize> = (0..self.txns.len()).collect();
        order.sort_by_key(|&i| self.txns[i].date);
        for &i in &order {
            let txn = &self.txns[i];
            if MonthKey::new(txn.date.0, txn.date.1) > through {
                break;
            }
            for posting in &txn.postings {
                if !posting.account.starts_with("Assets:") {
                    continue;
                }
                // A lot belongs to the amount it was written beside. An
                // elided posting carrying a residual has no lot, and
                // several currencies to spread one across anyway.
                let sole = posting.amounts.len() == 1;
                for (units, currency) in &posting.amounts {
                    let currency = currency.as_str();
                    if currency == cur
                        || self
                            .operating_currencies
                            .iter()
                            .any(|c| c == currency)
                        || units.is_zero()
                    {
                        continue;
                    }
                    let trace = traces.entry(currency).or_default();
                    trace.postings += 1;
                    trace.span = Some(match trace.span {
                        Some((first, last)) => {
                            (first.min(txn.date), last.max(txn.date))
                        }
                        None => (txn.date, txn.date),
                    });
                    let pool = pools
                        .entry((posting.account.as_str(), currency))
                        .or_default();
                    if units.is_sign_negative() {
                        pool.release(*units);
                    } else {
                        let cost = sole
                            .then_some(posting.cost.as_ref())
                            .flatten()
                            .and_then(|cost| {
                                self.convert(
                                    cost.total,
                                    &cost.currency,
                                    cur,
                                    txn.date,
                                )
                            });
                        pool.acquire(*units, cost);
                    }
                }
            }
        }

        // Units come off the same per-month index net worth reads, so a
        // position can never disagree with the band it sits in on the
        // chart. The walk above is only how the cost got here.
        let mut held: BTreeMap<String, Decimal> = BTreeMap::new();
        let mut sites: BTreeMap<String, usize> = BTreeMap::new();
        for info in self.accounts() {
            if !info.account.starts_with("Assets:") {
                continue;
            }
            let mut balances: BTreeMap<&str, Decimal> = BTreeMap::new();
            for (month, sums) in self.months_of(&info.account) {
                if month > through {
                    break;
                }
                for (currency, value) in sums {
                    *balances.entry(currency.as_str()).or_default() += value;
                }
            }
            for (currency, value) in balances {
                if value.is_zero() {
                    continue;
                }
                *held.entry(currency.to_string()).or_default() += value;
                // A holding is somewhere now; the brokers it passed
                // through on the way are history, not places it sits.
                *sites.entry(currency.to_string()).or_default() += 1;
            }
        }

        let mut unpriced = Vec::new();
        let mut items: Vec<Position> = Vec::new();
        for (currency, units) in &held {
            if *units <= Decimal::ZERO
                || currency == cur
                || self.operating_currencies.iter().any(|c| c == currency)
            {
                continue;
            }
            let units = *units;
            // Every account's inventory of it, summed only once each
            // has been averaged against its own disposals.
            let (basis_paid, uncosted) =
                pools.iter().filter(|((_, c), _)| *c == currency).fold(
                    (Decimal::ZERO, Decimal::ZERO),
                    |(b, u), (_, pool)| (b + pool.basis, u + pool.uncosted),
                );
            let trace = traces.get(currency.as_str());
            let Some(value) =
                self.convert(units, currency, cur, today).map(cents)
            else {
                unpriced.push(currency.clone());
                continue;
            };
            let price = self
                .convert(Decimal::ONE, currency, cur, today)
                .unwrap_or_default();
            let declared = self.commodity(currency);
            // A tranche that arrived without a lot is only a tranche
            // while it is still held; what is left of it after the
            // proportional sales is arithmetic residue.
            let costed = uncosted <= units / Decimal::from(COSTED);
            let basis = costed.then(|| cents(basis_paid));
            let (first, last) =
                trace.and_then(|t| t.span).unwrap_or((today, today));
            items.push(Position {
                currency: currency.clone(),
                label: declared
                    .and_then(|c| c.name.clone())
                    .unwrap_or_else(|| currency.clone()),
                class: declared.and_then(|c| c.asset_class.clone()),
                units,
                price,
                value,
                share: Decimal::ZERO,
                basis,
                gain: basis.map(|basis| value - basis),
                ret: basis
                    .filter(|b| *b > Decimal::ZERO)
                    .map(|basis| ((value - basis) / basis).round_dp(4)),
                accounts: sites.get(currency).copied().unwrap_or_default(),
                postings: trace.map_or(0, |t| t.postings),
                first,
                last,
            });
        }

        let total: Decimal = items.iter().map(|p| p.value).sum();
        let mut concentration = Decimal::ZERO;
        if total > Decimal::ZERO {
            for item in &mut items {
                item.share = (item.value / total).round_dp(4);
                concentration += item.share * item.share;
            }
        }

        let basis: Decimal = items.iter().filter_map(|p| p.basis).sum();
        let based_value: Decimal = items
            .iter()
            .filter(|p| p.basis.is_some())
            .map(|p| p.value)
            .sum();
        let unbased: Decimal = items
            .iter()
            .filter(|p| p.basis.is_none())
            .map(|p| p.value)
            .sum();
        let unbased_count = items.iter().filter(|p| p.basis.is_none()).count();

        let mut by_class: BTreeMap<Option<String>, (Decimal, usize)> =
            BTreeMap::new();
        for item in &items {
            let entry = by_class.entry(item.class.clone()).or_default();
            entry.0 += item.value;
            entry.1 += 1;
        }
        let mut classes: Vec<AssetClass> = by_class
            .into_iter()
            .map(|(name, (value, positions))| AssetClass {
                name,
                value,
                share: if total > Decimal::ZERO {
                    (value / total).round_dp(4)
                } else {
                    Decimal::ZERO
                },
                positions,
            })
            .collect();
        // Unclassified last on a tie: it is the absence of an answer,
        // not one of the answers.
        classes.sort_by(|a, b| {
            b.value
                .cmp(&a.value)
                .then(a.name.is_none().cmp(&b.name.is_none()))
                .then(a.name.cmp(&b.name))
        });

        items.sort_by(|a, b| {
            b.value.cmp(&a.value).then(a.currency.cmp(&b.currency))
        });
        let floor = total / Decimal::from(DUST_SHARE);
        let kept = items.iter().take_while(|p| p.value >= floor).count();
        let tail = items.split_off(kept);

        InvestmentsView {
            items,
            classes,
            total,
            dust: tail.len(),
            dust_value: tail.iter().map(|p| p.value).sum(),
            basis,
            based_value,
            gain: based_value - basis,
            ret: (basis > Decimal::ZERO)
                .then(|| ((based_value - basis) / basis).round_dp(4)),
            coverage: (total > Decimal::ZERO)
                .then(|| (based_value / total).round_dp(4)),
            unbased,
            unbased_count,
            effective: (concentration > Decimal::ZERO)
                .then(|| (Decimal::ONE / concentration).round_dp(2)),
            unpriced,
        }
    }
}
