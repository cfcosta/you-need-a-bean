//! What moved net worth: the part you put there and the part the
//! markets did.
//!
//! Every beancount transaction sums to zero across the five roots, so
//! `Δassets + Δliabilities = −(Δincome + Δexpenses + Δequity)`. In
//! other words the month's change in net worth is exactly what you
//! saved, plus whatever arrived through `Equity:*` (opening balances,
//! transfers in from outside the ledger). Whatever is left over after
//! subtracting both did not come from a transaction at all — it is the
//! prices moving under what you already hold.

use rust_decimal::Decimal;

use crate::model::MonthKey;

use super::{CashflowPoint, NetWorthPoint};

#[derive(Debug, Clone, Copy)]
pub struct GrowthPoint {
    pub month: MonthKey,
    /// Change in net worth over the month.
    pub delta: Decimal,
    /// Income − expenses: the part you saved.
    pub saved: Decimal,
    /// Capital in through `Equity:*` — opening balances, mostly.
    pub equity: Decimal,
    /// The rest: prices and rates moving under what you hold.
    pub market: Decimal,
}

#[derive(Debug, Clone)]
pub struct GrowthView {
    /// The trailing window the totals cover.
    pub window: Option<(MonthKey, MonthKey)>,
    pub saved: Decimal,
    pub equity: Decimal,
    pub market: Decimal,
    pub delta: Decimal,
    /// `market` over the average net worth held across the window — a
    /// rough period return, not annualized. `None` when nothing was
    /// held to earn it, or when [`Self::unpriced`] says the split it
    /// would be computed from is missing pieces.
    pub implied_return: Option<Decimal>,
    /// Commodities the display currency has no price for. While this is
    /// non-empty the residual is not just the market: a transaction
    /// with one leg in an unpriced commodity keeps the other leg, and
    /// the difference lands in `market` as a move nothing caused.
    pub unpriced: Vec<String>,
    pub points: Vec<GrowthPoint>,
}

/// Split each month's net worth move into saving, capital in, and the
/// residual the markets left behind.
pub(super) fn growth_view(
    net_worth: &[NetWorthPoint],
    cashflow: &[CashflowPoint],
    equity: &[(MonthKey, Decimal)],
    window: Option<(MonthKey, MonthKey)>,
    unpriced: Vec<String>,
) -> GrowthView {
    let mut points = Vec::with_capacity(net_worth.len());
    let mut previous = Decimal::ZERO;
    for (i, nw) in net_worth.iter().enumerate() {
        let delta = nw.net - previous;
        previous = nw.net;
        let saved = cashflow.get(i).map_or(Decimal::ZERO, |c| c.net);
        let equity = equity
            .iter()
            .find(|(m, _)| *m == nw.month)
            .map_or(Decimal::ZERO, |(_, v)| *v);
        points.push(GrowthPoint {
            month: nw.month,
            delta,
            saved,
            equity,
            market: delta - saved - equity,
        });
    }

    let in_window =
        |m: MonthKey| window.is_some_and(|(from, to)| m >= from && m <= to);
    let sum = |f: fn(&GrowthPoint) -> Decimal| {
        points.iter().filter(|p| in_window(p.month)).map(f).sum()
    };
    // Average capital employed: the mean of the month-end balances the
    // window covers. Rough — the flows inside a month are not weighted
    // by when they landed — so the return it implies is a scale, not an
    // accounting figure.
    let held: Vec<Decimal> = net_worth
        .iter()
        .filter(|p| in_window(p.month))
        .map(|p| p.net)
        .collect();
    let average = (!held.is_empty())
        .then(|| held.iter().sum::<Decimal>() / Decimal::from(held.len()));
    let market: Decimal = sum(|p| p.market);

    GrowthView {
        window,
        saved: sum(|p| p.saved),
        equity: sum(|p| p.equity),
        market,
        delta: sum(|p| p.delta),
        implied_return: unpriced
            .is_empty()
            .then_some(average)
            .flatten()
            .filter(|a| *a > Decimal::ZERO)
            .map(|a| (market / a).round_dp(4)),
        unpriced,
        points,
    }
}
