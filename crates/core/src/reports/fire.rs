//! Financial independence: the 4% rule, how long the stash lasts
//! without income, and what saving more would buy.

use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;

use crate::model::MonthKey;
use crate::query::cents;

use super::CashflowPoint;

/// Annual real-return scenarios for the time-to-FIRE projection.
pub const SCENARIO_RATES: [f64; 3] = [0.03, 0.05, 0.07];

/// Extra monthly savings to price, as multiples of what is saved today.
const SAVINGS_STEPS: [u32; 2] = [25, 50];

#[derive(Debug, Clone, Copy)]
pub struct FireScenario {
    /// Assumed annual real return.
    pub rate: f64,
    /// Months until the stash reaches the FIRE number; `None` when it
    /// stays short for 100 years.
    pub months: Option<u32>,
}

/// What saving `extra` more each month does to the middle scenario.
#[derive(Debug, Clone, Copy)]
pub struct SavingsStep {
    pub extra: Decimal,
    pub months: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct FireView {
    /// The averaging window behind the spend and savings numbers.
    pub window: Option<(MonthKey, MonthKey)>,
    pub monthly_spend: Decimal,
    pub annual_spend: Decimal,
    /// 25× annual spend: the stash a 4% withdrawal rate needs.
    pub fire_number: Decimal,
    pub net_worth: Decimal,
    /// net worth / FIRE number; `None` without a positive target.
    pub progress: Option<Decimal>,
    pub monthly_savings: Decimal,
    /// What the current stash already sustains: 4% of net worth / 12.
    pub swr_monthly: Decimal,
    pub scenarios: Vec<FireScenario>,
    /// The same scenarios with nothing further saved — coast FIRE.
    pub coast: Vec<FireScenario>,
    /// Time to target at the middle rate when saving more per month.
    pub steps: Vec<SavingsStep>,
    /// 25× a year of fixed costs only, and progress against it. `None`
    /// until the recurring pass finds something. See [`super::recurring`].
    pub lean_number: Option<Decimal>,
    pub lean_progress: Option<Decimal>,
}

/// How long the liquid stash covers spending with no income at all.
#[derive(Debug, Clone)]
pub struct RunwayView {
    /// Balances in budget-kind asset accounts: cash and its equivalents,
    /// which leaves out the accounts holding commodities.
    pub liquid: Decimal,
    /// liquid / monthly spend; `None` without a positive spend history.
    pub months: Option<Decimal>,
    /// The same against fixed costs alone — how long the lights stay on
    /// after cutting everything discretionary.
    pub lean_months: Option<Decimal>,
}

/// First month where `net_worth`, compounded monthly at `annual_rate`
/// with `monthly_savings` added each month, reaches `target`. `None`
/// when it stays short for 100 years.
pub fn months_to_fire(
    net_worth: f64,
    monthly_savings: f64,
    target: f64,
    annual_rate: f64,
) -> Option<u32> {
    let monthly_rate = (1.0 + annual_rate).powf(1.0 / 12.0) - 1.0;
    let mut balance = net_worth;
    for month in 0..=1200 {
        if balance >= target {
            return Some(month);
        }
        balance = balance * (1.0 + monthly_rate) + monthly_savings;
    }
    None
}

pub(super) fn fire_view(
    window: Option<(MonthKey, MonthKey)>,
    net_worth: Decimal,
    cashflow: &[CashflowPoint],
) -> FireView {
    let (monthly_spend, monthly_savings) = match window {
        Some((from, to)) => {
            let pts: Vec<&CashflowPoint> = cashflow
                .iter()
                .filter(|p| p.month >= from && p.month <= to)
                .collect();
            let count = Decimal::from(pts.len().max(1));
            (
                cents(pts.iter().map(|p| p.expenses).sum::<Decimal>() / count),
                cents(pts.iter().map(|p| p.net).sum::<Decimal>() / count),
            )
        }
        None => (Decimal::ZERO, Decimal::ZERO),
    };
    let annual_spend = monthly_spend * Decimal::from(12);
    let fire_number = annual_spend * Decimal::from(25);
    let progress = share(net_worth, fire_number);
    let swr_monthly = cents(net_worth * Decimal::new(4, 2) / Decimal::from(12));

    let to_fire = |savings: Decimal, rate: f64| {
        months_to_fire(
            net_worth.to_f64().unwrap_or(0.0),
            savings.to_f64().unwrap_or(0.0),
            fire_number.to_f64().unwrap_or(f64::MAX),
            rate,
        )
    };
    let (scenarios, coast, steps) = if fire_number > Decimal::ZERO {
        let scenarios = SCENARIO_RATES
            .iter()
            .map(|&rate| FireScenario {
                rate,
                months: to_fire(monthly_savings, rate),
            })
            .collect();
        // Coasting is the same projection with the contributions turned
        // off: what the stash gets to on its own from here.
        let coast = SCENARIO_RATES
            .iter()
            .map(|&rate| FireScenario {
                rate,
                months: to_fire(Decimal::ZERO, rate),
            })
            .collect();
        // Priced against the middle scenario, and only worth showing
        // while there is a savings habit to add to.
        let middle = SCENARIO_RATES[SCENARIO_RATES.len() / 2];
        let steps = if monthly_savings > Decimal::ZERO {
            SAVINGS_STEPS
                .iter()
                .map(|&pct| {
                    let extra = cents(
                        monthly_savings * Decimal::from(pct)
                            / Decimal::from(100),
                    );
                    SavingsStep {
                        extra,
                        months: to_fire(monthly_savings + extra, middle),
                    }
                })
                .collect()
        } else {
            Vec::new()
        };
        (scenarios, coast, steps)
    } else {
        (Vec::new(), Vec::new(), Vec::new())
    };

    FireView {
        window,
        monthly_spend,
        annual_spend,
        fire_number,
        net_worth,
        progress,
        monthly_savings,
        swr_monthly,
        scenarios,
        coast,
        steps,
        lean_number: None,
        lean_progress: None,
    }
}

impl FireView {
    /// Fold in the fixed-cost target once the recurring pass has priced
    /// the monthly nut. A stash 25× a year of fixed costs covers the
    /// bills forever, which lands long before it covers the whole
    /// lifestyle.
    pub(super) fn with_fixed(&mut self, monthly_fixed: Decimal) {
        if monthly_fixed <= Decimal::ZERO {
            return;
        }
        let lean = cents(monthly_fixed * Decimal::from(12) * Decimal::from(25));
        self.lean_progress = share(self.net_worth, lean);
        self.lean_number = Some(lean);
    }
}

pub(super) fn runway_view(
    liquid: Decimal,
    monthly_spend: Decimal,
    monthly_fixed: Decimal,
) -> RunwayView {
    let months = |spend: Decimal| {
        (spend > Decimal::ZERO).then(|| (liquid / spend).round_dp(1))
    };
    RunwayView {
        liquid,
        months: months(monthly_spend),
        lean_months: months(monthly_fixed),
    }
}

/// `part / whole` at four decimals; `None` without a positive whole.
pub(super) fn share(part: Decimal, whole: Decimal) -> Option<Decimal> {
    (whole > Decimal::ZERO).then(|| (part / whole).round_dp(4))
}
