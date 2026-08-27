//! The shape of a year's spending, and where this one is heading.
//!
//! Spending is not flat. December costs what December costs, and a
//! month read against the trailing average always looks like a
//! surprise when the surprise arrives every year. The median across
//! years strips the level out and leaves the shape, which is the only
//! honest thing to hold a month up against — and, scaled to what this
//! year has actually spent so far, the only honest way to guess where
//! it lands.

use rust_decimal::Decimal;

use crate::model::MonthKey;
use crate::query::{cents, median};

use super::CashflowPoint;

/// How many whole calendar years feed the medians, most recent first.
///
/// A synthetic long-history ledger demonstrates that very old price
/// levels distort the current pace. Five recent complete years provide
/// enough samples for a median without mixing in distant spending
/// levels.
const SEASON_YEARS: u16 = 5;

/// One calendar month, across every year the ledger covers.
#[derive(Debug, Clone, Copy)]
pub struct SeasonPoint {
    /// 1 through 12.
    pub month: u8,
    /// Median spend in this calendar month over the sample years.
    pub median: Decimal,
    /// `median` over the twelve medians added up: the shape alone,
    /// with the level divided out.
    pub share: Decimal,
    /// How many years fed the median. One is not a median, and the
    /// page says so.
    pub samples: usize,
    /// What this year actually spent, once the month is over.
    pub actual: Option<Decimal>,
}

#[derive(Debug, Clone)]
pub struct SeasonView {
    /// Calendar years behind the medians, inclusive — at most
    /// [`SEASON_YEARS`] of them. `None` when the ledger doesn't cover
    /// all twelve months of one, which is when there is no shape to
    /// speak of.
    pub years: Option<(u16, u16)>,
    /// Twelve points in calendar order, empty when `years` is `None`.
    pub months: Vec<SeasonPoint>,
    /// What a median year costs: the twelve medians added up.
    pub typical: Decimal,
    /// The year being projected.
    pub year: u16,
    /// Months of `year` that are over, and what they cost.
    pub elapsed: u8,
    pub ytd: Decimal,
    /// `ytd` over what those same months usually cost. Above one is a
    /// year running hot, and it is a comparison against the same
    /// months, so a January-heavy ledger doesn't read as a rise every
    /// spring.
    pub pace: Option<Decimal>,
    /// `ytd` plus the rest of the year at this year's pace. `None`
    /// before the first month of the year is over, when there is
    /// nothing to take a pace from.
    pub projected: Option<Decimal>,
}

/// The seasonal shape from every whole prior year, and where the
/// current one lands if it keeps to that shape at its own level.
///
/// Prior years only: the year being projected can't also be the
/// baseline it is measured against, and only the last [`SEASON_YEARS`]
/// of them. The ledger's first month is left out too, since a ledger
/// that starts mid-month reports a cheap one.
pub(super) fn season_view(
    cashflow: &[CashflowPoint],
    current: MonthKey,
) -> SeasonView {
    let year = current.year;
    let earliest = year.saturating_sub(SEASON_YEARS);
    let mut samples: [Vec<Decimal>; 12] = Default::default();
    let mut actual: [Option<Decimal>; 12] = [None; 12];
    let mut span: Option<(u16, u16)> = None;

    for point in cashflow.iter().skip(1) {
        let slot = usize::from(point.month.month - 1);
        if (earliest..year).contains(&point.month.year) {
            samples[slot].push(point.expenses);
            span = Some(match span {
                Some((from, to)) => {
                    (from.min(point.month.year), to.max(point.month.year))
                }
                None => (point.month.year, point.month.year),
            });
        } else if point.month.year == year && point.month < current {
            actual[slot] = Some(point.expenses);
        }
    }

    // A shape needs a whole year. Anything less and the missing months
    // read as months that cost nothing.
    let years = samples
        .iter()
        .all(|s| !s.is_empty())
        .then_some(span)
        .flatten();
    let elapsed = actual.iter().filter(|a| a.is_some()).count() as u8;
    let ytd = cents(actual.iter().flatten().sum::<Decimal>());

    let Some(years) = years else {
        return SeasonView {
            years: None,
            months: Vec::new(),
            typical: Decimal::ZERO,
            year,
            elapsed,
            ytd,
            pace: None,
            projected: None,
        };
    };

    let medians: Vec<Decimal> =
        samples.iter_mut().map(|s| cents(median(s))).collect();
    let typical: Decimal = medians.iter().sum();

    // What the months already behind us usually cost, and what the
    // rest of the year usually costs.
    let (mut baseline, mut ahead) = (Decimal::ZERO, Decimal::ZERO);
    for (slot, &m) in medians.iter().enumerate() {
        if actual[slot].is_some() {
            baseline += m;
        } else {
            ahead += m;
        }
    }
    let pace = (baseline > Decimal::ZERO).then(|| (ytd / baseline).round_dp(4));

    SeasonView {
        months: medians
            .iter()
            .enumerate()
            .map(|(slot, &m)| SeasonPoint {
                month: slot as u8 + 1,
                median: m,
                share: if typical > Decimal::ZERO {
                    (m / typical).round_dp(4)
                } else {
                    Decimal::ZERO
                },
                samples: samples[slot].len(),
                actual: actual[slot],
            })
            .collect(),
        years: Some(years),
        typical,
        year,
        elapsed,
        ytd,
        pace,
        projected: pace.map(|p| cents(ytd + ahead * p)),
    }
}
