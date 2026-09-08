//! The reports page: net worth and cashflow over the ledger's whole
//! range, the FIRE projection built on the 4% rule, and the breakdowns
//! that say where the money actually went.
//!
//! Like [`crate::query`], everything here is a pure read over
//! [`Ledger`]; money is quantized to cents at the leaves so displayed
//! sums add up.

mod fire;
mod growth;
mod income;
mod investments;
mod liabilities;
mod movers;
mod payees;
mod projects;
mod recurring;
mod season;
mod trust;
mod year;

pub use fire::{
    FireScenario, FireView, RunwayView, SCENARIO_RATES, SavingsStep,
    months_to_fire,
};
pub use growth::{GrowthPoint, GrowthView};
pub use income::{IncomeSource, IncomeView};
pub use investments::{AssetClass, InvestmentsView, Position};
pub use liabilities::{
    Amortization, Beaten, Collateral, Cover, Cycle, Debt, DebtKind, DebtPoint,
    Extra, Foreign, LiabilitiesView, Makeup, MakeupRow, Notice, NoticeKind,
    Payment, Payoff, TrailPoint, Treadmill, TreadmillMonth, Upcoming, amortize,
    debt_kind,
};
pub use movers::{Mover, MoversView};
pub use payees::{Payee, PayeesView};
pub use projects::{Project, ProjectsView, Topic};
pub use recurring::{Cadence, PriceChange, Recurring, RecurringView};
pub use season::{SeasonPoint, SeasonView};
pub use trust::{Flagged, FlaggedTxn, StalePrice, TrustView, Uncategorized};
pub use year::{YearGroup, YearMonth, YearView};

use std::collections::{BTreeMap, BTreeSet};

use rust_decimal::Decimal;

use crate::model::{AccountKind, Day, Ledger, MonthKey, add_sum};
use crate::query::cents;

#[derive(Debug, Clone, Copy)]
pub struct NetWorthPoint {
    pub month: MonthKey,
    /// Convertible asset balances at month end.
    pub assets: Decimal,
    /// The budget-kind slice of `assets`: cash and its equivalents.
    pub cash: Decimal,
    /// The rest of `assets` — accounts holding commodities, and any
    /// hidden from the budget page.
    pub holdings: Decimal,
    /// Convertible liability balances at month end (negative when owed).
    pub liabilities: Decimal,
    pub net: Decimal,
}

#[derive(Debug, Clone, Copy)]
pub struct CashflowPoint {
    pub month: MonthKey,
    pub income: Decimal,
    pub expenses: Decimal,
    pub net: Decimal,
}

#[derive(Debug, Clone)]
pub struct ReportsView {
    /// The month the FIRE numbers are anchored to (today, clamped).
    pub month: MonthKey,
    pub net_worth: Vec<NetWorthPoint>,
    pub cashflow: Vec<CashflowPoint>,
    pub fire: FireView,
    pub runway: RunwayView,
    pub growth: GrowthView,
    pub recurring: RecurringView,
    /// Commodities held or moved that nothing prices in the display
    /// currency, sorted. Every amount in one of these is missing from
    /// the figures above — see [`GrowthView::unpriced`] for why that
    /// matters most to the growth split.
    pub unpriced: Vec<String>,
    /// Spend per expense group over the trailing year, against the
    /// year before it.
    pub year: YearView,
    /// The categories whose last quarter moved most against the one
    /// before — the year card's ranking asked as a question about
    /// change rather than size.
    pub movers: MoversView,
    /// What each tag and link cost, over how long.
    pub projects: ProjectsView,
    /// Where income comes from, and how much of it is passive.
    pub income: IncomeView,
    /// Every commodity position, what it is worth, and what it cost
    /// where the ledger recorded enough to say.
    pub investments: InvestmentsView,
    /// The shape a year of spending has, and where this one is going.
    pub season: SeasonView,
    /// Who the year's money went to, ranked.
    pub payees: PayeesView,
    /// The reasons to doubt everything above.
    pub trust: TrustView,
}

/// Per-month currency deltas for one root of the ledger.
type Deltas = BTreeMap<MonthKey, Vec<(String, Decimal)>>;

/// The balance-sheet and flow deltas, bucketed by root, in one pass.
#[derive(Default)]
struct Roots {
    assets: Deltas,
    /// The subset of assets in budget-kind accounts: cash and its
    /// equivalents, which leaves out accounts holding commodities.
    liquid: Deltas,
    liabilities: Deltas,
    income: Deltas,
    expenses: Deltas,
    /// Capital arriving from outside the ledger: opening balances and
    /// the like. Net worth moves that these explain were never earned.
    equity: Deltas,
}

impl Ledger {
    /// Net worth and cashflow for every navigable month, plus the FIRE
    /// projection anchored to today's month.
    pub fn reports_view(
        &self,
        today: Day,
        basis: u32,
        cur: &str,
    ) -> ReportsView {
        if let std::borrow::Cow::Owned(ledger) = self.as_of(today) {
            return ledger.reports_view(today, basis, cur);
        }
        let months = self.months_range(today);
        let current = self.default_month(today);
        let roots = self.roots();
        // Anything that cannot be valued in `cur` is dropped from every
        // figure below. Collect the commodities as we go so the page can
        // say so instead of quietly reporting a smaller world.
        let mut unpriced = BTreeSet::new();

        let mut asset_bal: Vec<(String, Decimal)> = Vec::new();
        let mut liquid_bal: Vec<(String, Decimal)> = Vec::new();
        let mut liab_bal: Vec<(String, Decimal)> = Vec::new();
        let mut net_worth = Vec::with_capacity(months.len());
        let mut cashflow = Vec::with_capacity(months.len());
        let mut equity = Vec::with_capacity(months.len());
        for &month in &months {
            apply(&mut asset_bal, roots.assets.get(&month));
            apply(&mut liquid_bal, roots.liquid.get(&month));
            apply(&mut liab_bal, roots.liabilities.get(&month));
            let at = month.end_of_month();
            let mut into = |amounts: &[(String, Decimal)]| {
                cents(self.convertible(amounts, cur, at, &mut unpriced))
            };
            let a = into(&asset_bal);
            let c = into(&liquid_bal);
            let l = into(&liab_bal);
            net_worth.push(NetWorthPoint {
                month,
                assets: a,
                cash: c,
                holdings: a - c,
                liabilities: l,
                net: a + l,
            });

            // Income postings are negative in beancount.
            let inc = roots.income.get(&month).map_or(Decimal::ZERO, |f| {
                -self.convertible(f, cur, at, &mut unpriced)
            });
            let exp = roots.expenses.get(&month).map_or(Decimal::ZERO, |f| {
                self.convertible(f, cur, at, &mut unpriced)
            });
            let (inc, exp) = (cents(inc), cents(exp));
            cashflow.push(CashflowPoint {
                month,
                income: inc,
                expenses: exp,
                net: inc - exp,
            });

            // Equity postings are negative when capital comes in, the
            // same convention income follows.
            equity.push((
                month,
                cents(roots.equity.get(&month).map_or(Decimal::ZERO, |f| {
                    -self.convertible(f, cur, at, &mut unpriced)
                })),
            ));
        }

        let mut fire = fire::fire_view(
            self.window(current, basis),
            net_at(&net_worth, current),
            &cashflow,
        );
        // What the charges that come back cost every month, which is
        // both the lean FIRE target and the runway you have if you cut
        // everything discretionary.
        let recurring = self.recurring_view(current, cur, fire.monthly_spend);
        let monthly_fixed = recurring.monthly_fixed;
        fire.with_fixed(monthly_fixed);
        let runway = fire::runway_view(
            self.liquid_cash(current, cur, &mut unpriced),
            fire.monthly_spend,
            monthly_fixed,
        );
        // A year of history behind the growth split, matching the year
        // card's window without sharing its meaning.
        let growth_window = self.window(current, 12);
        let projects = self.projects_view(cur, &mut unpriced);
        // Every conversion the view needs has run by now, so the set is
        // complete. The growth split is the report the gaps hurt most:
        // an amount dropped from one side of a transaction and kept on
        // the other lands in the residual as a market move that never
        // happened.
        let unpriced: Vec<String> = unpriced.into_iter().collect();
        let growth = growth::growth_view(
            &net_worth,
            &cashflow,
            &equity,
            growth_window,
            unpriced.clone(),
        );
        let season = season::season_view(&cashflow, current);

        ReportsView {
            month: current,
            net_worth,
            cashflow,
            fire,
            runway,
            growth,
            recurring,
            unpriced,
            year: self.year_view(current, cur),
            movers: self.movers_view(current, cur),
            projects,
            income: self.income_view(current, cur),
            investments: self.investments_view(today, cur),
            season,
            payees: self.payees_view(current, cur),
            trust: self.trust_view(today, current, cur),
        }
    }

    /// One pass over the indexes: per-month currency deltas for the
    /// balance-sheet roots, per-month flows for the flow roots.
    fn roots(&self) -> Roots {
        let mut roots = Roots::default();
        for info in self.accounts() {
            let is_asset = info.account.starts_with("Assets:");
            let maps: [Option<&mut Deltas>; 2] = if is_asset {
                [
                    Some(&mut roots.assets),
                    (info.kind == AccountKind::Budget)
                        .then_some(&mut roots.liquid),
                ]
            } else if info.account.starts_with("Liabilities:") {
                [Some(&mut roots.liabilities), None]
            } else if info.account.starts_with("Income:") {
                [Some(&mut roots.income), None]
            } else if info.account.starts_with("Expenses:") {
                [Some(&mut roots.expenses), None]
            } else if info.account.starts_with("Equity:") {
                [Some(&mut roots.equity), None]
            } else {
                continue;
            };
            for map in maps.into_iter().flatten() {
                for (month, sums) in self.months_of(&info.account) {
                    let entry = map.entry(month).or_default();
                    for (c, v) in sums {
                        add_sum(entry, c, *v);
                    }
                }
            }
        }
        roots
    }

    /// Sum of the convertible parts of a per-currency balance list,
    /// naming the commodities it had to leave out.
    pub(crate) fn convertible(
        &self,
        amounts: &[(String, Decimal)],
        cur: &str,
        at: Day,
        unpriced: &mut BTreeSet<String>,
    ) -> Decimal {
        amounts
            .iter()
            .filter_map(|(c, v)| {
                let converted = self.convert(*v, c, cur, at);
                // A zero balance in a commodity nobody prices is not a
                // gap worth reporting — nothing is missing from the sum.
                if converted.is_none() && !v.is_zero() {
                    unpriced.insert(c.clone());
                }
                converted
            })
            .sum()
    }
}

fn net_at(series: &[NetWorthPoint], month: MonthKey) -> Decimal {
    series
        .iter()
        .find(|p| p.month == month)
        .map(|p| p.net)
        .unwrap_or_default()
}

fn apply(
    balance: &mut Vec<(String, Decimal)>,
    delta: Option<&Vec<(String, Decimal)>>,
) {
    for (c, v) in delta.into_iter().flatten() {
        add_sum(balance, c, *v);
    }
}
