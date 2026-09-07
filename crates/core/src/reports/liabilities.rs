//! What you owe, what it costs, and when it ends.
//!
//! Every liability with something on it gets a card: how much, at what
//! rate, paid how much and when, and — for a loan or a card being
//! carried — the month the last payment lands if the current one keeps
//! landing. None of it needs a number the ledger does not already
//! hold. The rate is what the interest legs say against the balance
//! they were charged on; the payment is what has been paid lately; the
//! due day is where the payments fall; a card is paid in full when the
//! payment cleared everything older than a statement.
//!
//! What the postings cannot say, the open directive may: a `rate:`
//! before any interest is charged, a `due:` day before two payments
//! have landed, a card's `limit:`, and the `collateral:` a loan is
//! secured on. When the ledger states one of those, it wins over the
//! guess.
//!
//! Signs are the reader's, not the ledger's: a liability holds a
//! negative balance, and this module reports it as a positive amount
//! owed. A charge raises it, a payment lowers it.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use rust_decimal::Decimal;

use crate::model::{
    AccountInfo, AccountKind, Day, Ledger, MonthKey, add_sum, days_between,
};
use crate::query::{cents, median};
use crate::reports::fire::SCENARIO_RATES;

/// Interest observations the rate is read from: the newest few, so a
/// loan whose rate was reset reads at the new one.
const RATE_SAMPLES: usize = 6;
/// Payments the current payment is the median of. Three, so one lump
/// does not become the plan.
const PAYMENT_SAMPLES: usize = 3;
/// Payments whose days are read for the due day.
const DUE_SAMPLES: usize = 6;
/// Payments needed before their day means anything.
const DUE_PAYMENTS: usize = 2;
/// Payments the view lists per debt, newest first.
const LISTED_PAYMENTS: usize = 24;
/// Charges younger than this when a card is paid belong to the cycle
/// being paid into, not to the balance carried past it.
const STATEMENT_DAYS: i64 = 31;
/// How long after the usual day a payment can land before the month
/// counts as missed.
const GRACE_DAYS: u8 = 3;
/// How many months a payer can go quiet before the habit is over and
/// a month without a payment is not a missed one.
const HABIT_MONTHS: u32 = 2;
/// Month-ends that have to rise, one after another, for a card to be
/// growing.
const GROWING_MONTHS: usize = 3;
/// Payments this far ahead are listed as coming up.
const UPCOMING_DAYS: i64 = 31;
/// The projection gives up here, as the independence one does.
const MAX_MONTHS: u32 = 1200;
/// Complete months a carried card's pace is read over: what its
/// payments really take off once the new charges are counted.
const PACE_MONTHS: usize = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DebtKind {
    /// A card: charged and paid, with a cycle rather than a term.
    Revolving,
    /// A loan: drawn once and paid down.
    Installment,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum NoticeKind {
    /// A regular payer whose day has passed this month without one.
    Missed,
    /// A card that ended higher three months running while carrying a
    /// balance past its statements.
    Growing,
    /// A liability in credit: it owes you.
    Overpaid,
}

/// Something the figures show but do not say.
#[derive(Debug, Clone)]
pub struct Notice {
    pub kind: NoticeKind,
    pub account: String,
    pub label: String,
    /// The money the notice is about, when there is some.
    pub amount: Option<Decimal>,
    /// The day of the month a missed payment usually lands on.
    pub day: Option<u8>,
}

/// One payment, split the way the transaction split it.
#[derive(Debug, Clone)]
pub struct Payment {
    pub date: Day,
    pub total: Decimal,
    /// What came off the balance.
    pub principal: Decimal,
    /// What went to the lender's interest account in the same
    /// transaction; zero when the payment had no interest leg.
    pub interest: Decimal,
}

/// What was owed at a month's end. `None` past a hole in the prices.
#[derive(Debug, Clone)]
pub struct DebtPoint {
    pub month: MonthKey,
    pub owed: Option<Decimal>,
}

/// What was owed after one transaction — the tooth of a card's saw,
/// the step of a loan's stair.
#[derive(Debug, Clone)]
pub struct TrailPoint {
    pub date: Day,
    pub owed: Option<Decimal>,
    /// The change this point made; `None` on the point that opens the
    /// trail.
    pub delta: Option<Decimal>,
}

/// A loan paid down at a fixed payment.
#[derive(Debug, Clone, Copy)]
pub struct Amortization {
    /// Payments until nothing is owed.
    pub months: u32,
    /// Interest paid along the way.
    pub interest: Decimal,
}

/// Where the current payment lands the loan.
#[derive(Debug, Clone)]
pub struct Payoff {
    pub months: u32,
    /// The month the last payment falls in.
    pub month: MonthKey,
    /// Interest still to be paid between now and then.
    pub interest: Decimal,
}

/// A card's month, and whether it is being paid off or carried.
#[derive(Debug, Clone)]
pub struct Cycle {
    /// Put on the card this month.
    pub charges: Decimal,
    /// Paid off it this month.
    pub payments: Decimal,
    /// What the latest payment left behind that was older than a
    /// statement — the balance being revolved. `None` without a recent
    /// payment to read it from.
    pub carried: Option<Decimal>,
    /// `carried` is zero: the statement was paid in full.
    pub in_full: Option<bool>,
}

/// A carried card's month: what went on it, what came off it, and
/// what the bank added.
#[derive(Debug, Clone)]
pub struct TreadmillMonth {
    pub month: MonthKey,
    /// Put on the card, the interest left out.
    pub charges: Decimal,
    /// Paid off it.
    pub payments: Decimal,
    /// Charged by the lender.
    pub interest: Decimal,
}

/// A carried card measured with the new charges counted, which the
/// payoff leaves out: the payments against what keeps landing on it,
/// and where that pace really ends.
#[derive(Debug, Clone)]
pub struct Treadmill {
    /// The complete months of the trailing window, oldest first.
    pub months: Vec<TreadmillMonth>,
    /// How many of the newest of them the pace is read over.
    pub pace: u32,
    /// Put on the card over those months.
    pub charged: Decimal,
    /// Paid off it over those months.
    pub paid: Decimal,
    /// What a month really takes off: paid less charged, per month.
    /// Negative when the card is growing.
    pub net: Decimal,
    /// Where that pace lands, when it lands anywhere.
    pub payoff: Option<Payoff>,
}

/// One category's share of what a carried card holds.
#[derive(Debug, Clone)]
pub struct MakeupRow {
    /// The account the charges went to; empty when nothing says.
    pub account: String,
    pub label: String,
    /// Still on the card from this category, its interest included.
    pub owed: Decimal,
    /// The charges themselves, still unpaid.
    pub charged: Decimal,
    /// Interest the open charges have drawn so far, paid or not.
    pub interest: Decimal,
    /// The day of the oldest open charge.
    pub since: Day,
    /// Open charges.
    pub count: usize,
}

/// What a carried balance is made of: the charges the payments have
/// not reached yet, by where they went.
#[derive(Debug, Clone)]
pub struct Makeup {
    /// Biggest first.
    pub rows: Vec<MakeupRow>,
    pub total: Decimal,
}

/// The asset a loan is secured on, as the open directive named it.
#[derive(Debug, Clone)]
pub struct Collateral {
    pub account: String,
    pub label: String,
    /// What it is worth at the month's end, converted; `None` when it
    /// cannot be priced or the account is not in the ledger.
    pub value: Option<Decimal>,
}

/// One foreign-currency part of what is owed.
#[derive(Debug, Clone)]
pub struct Foreign {
    pub code: String,
    /// Owed in that currency: positive, the reader's sign.
    pub amount: Decimal,
    /// The same in the display currency, when there is a price.
    pub converted: Option<Decimal>,
}

/// A debt that is over: paid to nothing, and either a loan (which has
/// nowhere else to go) or closed. Kept as a receipt.
#[derive(Debug, Clone)]
pub struct Beaten {
    pub account: String,
    pub label: String,
    /// The most that was ever owed.
    pub peak: Decimal,
    pub principal_paid: Decimal,
    pub interest_paid: Decimal,
    /// The first day anything was owed.
    pub first: Day,
    /// The day of the last payment.
    pub last: Day,
}

#[derive(Debug, Clone)]
pub struct Debt {
    pub account: String,
    pub label: String,
    pub kind: DebtKind,
    /// Owed now, in the display currency. Negative when in credit.
    pub owed: Decimal,
    /// Owed now per commodity, as the ledger signs it.
    pub balances: Vec<(String, Decimal)>,
    /// The parts of `balances` not in the display currency, converted.
    pub foreign: Vec<Foreign>,
    /// `limit:` from the open directive, in the display currency.
    pub limit: Option<Decimal>,
    /// `owed` over `limit`.
    pub utilisation: Option<Decimal>,
    /// `collateral:` from the open directive, looked up.
    pub collateral: Option<Collateral>,
    /// The most that was ever owed.
    pub peak: Decimal,
    /// How much of the peak has been paid down; loans only.
    pub progress: Option<Decimal>,
    /// Nominal annual rate: `rate:` from the open directive, else read
    /// off the interest legs.
    pub rate: Option<Decimal>,
    /// What a payment has been lately.
    pub payment: Option<Decimal>,
    /// The day of the month payments land on: `due:` from the open
    /// directive, else where the payments fall.
    pub due_day: Option<u8>,
    /// When the next one is expected, while something is owed.
    pub next_due: Option<Day>,
    pub principal_paid: Decimal,
    pub interest_paid: Decimal,
    /// Newest first, capped.
    pub payments: Vec<Payment>,
    /// Owed at every month's end over the ledger's range.
    pub history: Vec<DebtPoint>,
    /// Owed after every transaction over the trailing `basis` months,
    /// opened by where it stood going in.
    pub trail: Vec<TrailPoint>,
    /// Where the current payment lands, with nothing new put on: loans,
    /// and cards carrying a balance.
    pub payoff: Option<Payoff>,
    /// Cards only.
    pub cycle: Option<Cycle>,
    /// Cards carrying a balance only: the payments against the charges.
    pub treadmill: Option<Treadmill>,
    /// Cards carrying a balance only: what the balance is made of.
    pub makeup: Option<Makeup>,
}

/// Cash on hand against what the cards are holding.
#[derive(Debug, Clone)]
pub struct Cover {
    /// Liquid assets, the runway's definition.
    pub cash: Decimal,
    /// Owed on revolving debt.
    pub owed: Decimal,
    pub covered: bool,
    /// What paying every card off today would leave.
    pub after: Decimal,
}

/// What there is to pay on top of the usual payments, worked out from
/// the ledger rather than asked for.
#[derive(Debug, Clone)]
pub struct Extra {
    /// Liquid cash at the month's end, the cover's figure.
    pub cash: Decimal,
    /// The payments coming up within a month.
    pub due: Decimal,
    /// A typical month of spending paid in cash rather than on a card.
    pub spend: Decimal,
    /// A month of the fixed nut, kept back.
    pub buffer: Decimal,
    /// Cash less the three: what could go on the debts this month.
    /// Never below zero.
    pub now: Decimal,
    /// What a typical month leaves over after everything, the debt
    /// payments included: the median over the window. Never below
    /// zero.
    pub monthly: Decimal,
}

#[derive(Debug, Clone)]
pub struct Upcoming {
    pub account: String,
    pub label: String,
    pub date: Day,
    pub amount: Decimal,
}

#[derive(Debug, Clone)]
pub struct LiabilitiesView {
    pub month: MonthKey,
    /// Everything owed, net of any account in credit.
    pub owed: Decimal,
    pub installment: Decimal,
    pub revolving: Decimal,
    /// Interest paid this month.
    pub interest_month: Decimal,
    /// Interest paid over the trailing year.
    pub interest_year: Decimal,
    /// Interest received, the same two ways.
    pub earned_month: Decimal,
    pub earned_year: Decimal,
    /// The trailing year the `_year` figures cover.
    pub window: Option<(MonthKey, MonthKey)>,
    /// What a year at these rates and balances costs.
    pub cost_year: Decimal,
    /// `cost_year` over `owed`.
    pub blended_rate: Option<Decimal>,
    /// The month the last debt being paid down is gone, when every
    /// one of them has a month: loans, and cards carrying a balance.
    pub debt_free: Option<MonthKey>,
    /// The yearly return paying a debt down is weighed against: the
    /// middle of the independence page's scenarios, so the two pages
    /// agree.
    pub assumed_return: Decimal,
    pub cover: Cover,
    /// What could be paid on top of the usual payments.
    pub extra: Extra,
    /// Payments expected in the next month, soonest first.
    pub upcoming: Vec<Upcoming>,
    pub notices: Vec<Notice>,
    /// Biggest first.
    pub debts: Vec<Debt>,
    /// Debts that are over, most recently beaten first.
    pub beaten: Vec<Beaten>,
    pub unpriced: Vec<String>,
}

/// Owed after one transaction.
struct Step {
    date: Day,
    owed: Option<Decimal>,
    delta: Option<Decimal>,
}

/// Everything read off one account's postings in one pass.
#[derive(Default)]
struct Walk {
    balances: Vec<(String, Decimal)>,
    steps: Vec<Step>,
    /// Each payment with what was owed once it landed.
    payments: Vec<(Payment, Option<Decimal>)>,
    /// Every rise in what is owed, converted.
    charges: Vec<(Day, Decimal)>,
    /// Interest against the balance it was charged on, annualised.
    rates: Vec<Decimal>,
    /// Interest paid, converted, by date.
    interest: Vec<(Day, Decimal)>,
    /// Owed at the end of every month with activity.
    ends: Vec<(MonthKey, Option<Decimal>)>,
    peak: Decimal,
    /// Something went on it after it was opened.
    recharged: bool,
}

impl Ledger {
    pub fn liabilities_view(
        &self,
        today: Day,
        basis: u32,
        cur: &str,
    ) -> LiabilitiesView {
        if let std::borrow::Cow::Owned(ledger) = self.as_of(today) {
            return ledger.liabilities_view(today, basis, cur);
        }
        let current = self.default_month(today);
        let window = self.window(current, 12);
        let in_window = |month: MonthKey| {
            window.is_some_and(|(from, to)| month >= from && month <= to)
        };
        let mut unpriced = BTreeSet::new();
        let mut debts = Vec::new();
        let mut beaten = Vec::new();
        let mut interest_month = Decimal::ZERO;
        let mut interest_year = Decimal::ZERO;
        let mut principal_paid: BTreeMap<MonthKey, Decimal> = BTreeMap::new();
        for info in self.accounts() {
            if !info.account.starts_with("Liabilities:")
                || info.kind == AccountKind::Hidden
            {
                continue;
            }
            let walk = self.walk_debt(&info.account, current, cur);
            for (payment, _) in &walk.payments {
                let month = MonthKey::new(payment.date.0, payment.date.1);
                *principal_paid.entry(month).or_default() += payment.principal;
            }
            for (date, amount) in &walk.interest {
                let month = MonthKey::new(date.0, date.1);
                if month == current {
                    interest_month += amount;
                }
                if in_window(month) {
                    interest_year += amount;
                }
            }
            if let Some(receipt) = beaten_of(info, &walk) {
                beaten.push(receipt);
            } else if let Some(debt) = self.debt_of(
                info,
                &walk,
                today,
                current,
                basis,
                cur,
                &mut unpriced,
            ) {
                debts.push(debt);
            }
        }
        debts.sort_by(|a, b| {
            b.owed.cmp(&a.owed).then_with(|| a.account.cmp(&b.account))
        });
        beaten.sort_by(|a, b| {
            b.last.cmp(&a.last).then_with(|| a.account.cmp(&b.account))
        });

        let (earned_month, earned_year) =
            self.interest_earned(current, window, cur);
        let owed: Decimal = debts.iter().map(|d| d.owed).sum();
        let sum_kind = |kind: DebtKind| -> Decimal {
            debts
                .iter()
                .filter(|d| d.kind == kind)
                .map(|d| d.owed)
                .sum()
        };
        let cost: Decimal = debts
            .iter()
            .filter(|d| d.owed > Decimal::ZERO)
            .map(|d| d.owed * d.rate.unwrap_or_default())
            .sum();
        let paying_down: Vec<&Debt> =
            debts.iter().filter(|d| paying_down(d)).collect();
        let debt_free = (!paying_down.is_empty())
            .then(|| {
                paying_down
                    .iter()
                    .map(|d| d.payoff.as_ref().map(|p| p.month))
                    .collect::<Option<Vec<_>>>()
            })
            .flatten()
            .and_then(|months| months.into_iter().max());
        let cash = self.liquid_cash(current, cur, &mut unpriced);
        let revolving = sum_kind(DebtKind::Revolving);
        let upcoming = upcoming(&debts, today);
        let notices = notices(&debts, today, current);
        let extra = self.extra_of(
            &upcoming,
            &principal_paid,
            cash,
            current,
            basis,
            cur,
        );

        LiabilitiesView {
            month: current,
            owed,
            installment: sum_kind(DebtKind::Installment),
            revolving,
            interest_month: cents(interest_month),
            interest_year: cents(interest_year),
            earned_month,
            earned_year,
            window,
            cost_year: cents(cost),
            blended_rate: (owed > Decimal::ZERO)
                .then(|| (cost / owed).round_dp(4)),
            debt_free,
            assumed_return: Decimal::try_from(SCENARIO_RATES[1])
                .map(|r| r.round_dp(4))
                .unwrap_or_default(),
            cover: Cover {
                cash,
                owed: revolving,
                covered: cash >= revolving,
                after: cash - revolving,
            },
            extra,
            upcoming,
            notices,
            debts,
            beaten,
            unpriced: unpriced.into_iter().collect(),
        }
    }

    /// One pass over the account's transactions through `current`,
    /// keeping the running balance and everything the view reads from
    /// how it moved.
    fn walk_debt(&self, account: &str, current: MonthKey, cur: &str) -> Walk {
        let mut w = Walk::default();
        let mut first = true;
        for (month, _) in self.months_of(account) {
            if month > current {
                break;
            }
            let at = month.end_of_month();
            for txn in self.txns(account, month) {
                let mut own: Vec<(String, Decimal)> = Vec::new();
                let mut interest: Vec<(String, Decimal)> = Vec::new();
                let mut from_assets = false;
                for posting in &txn.postings {
                    if posting.account == account {
                        for (v, c) in &posting.amounts {
                            add_sum(&mut own, c, *v);
                        }
                    } else if is_interest_expense(&posting.account) {
                        for (v, c) in &posting.amounts {
                            add_sum(&mut interest, c, *v);
                        }
                    } else if posting.account.starts_with("Assets:") {
                        from_assets = true;
                    }
                }
                // The rate: this interest against what it was charged
                // on, in the currency it was charged in.
                for (c, i) in &interest {
                    let before = -amount_in(&w.balances, c);
                    if *i > Decimal::ZERO && before > Decimal::ZERO {
                        w.rates.push(*i / before * Decimal::from(12));
                    }
                }
                let interest_conv = self.convert_all(&interest, cur, at);
                let delta = self.txn_delta(txn, account, cur, at).map(|d| -d);
                for (c, v) in &own {
                    add_sum(&mut w.balances, c, *v);
                }
                let owed = self.owed_of(&w.balances, cur, at);
                if let Some(o) = owed {
                    w.peak = w.peak.max(o);
                }
                let raised = own.iter().any(|(_, v)| *v < Decimal::ZERO);
                let lowered = own.iter().any(|(_, v)| *v > Decimal::ZERO);
                if raised && !first && interest.is_empty() {
                    w.recharged = true;
                }
                if let Some(d) = delta.filter(|d| raised && *d > Decimal::ZERO)
                {
                    w.charges.push((txn.date, d));
                }
                let is_payment = lowered && !raised && from_assets;
                if let Some(principal) =
                    delta.filter(|_| is_payment).map(|d| -d)
                {
                    let interest = interest_conv.unwrap_or_default();
                    let payment = Payment {
                        date: txn.date,
                        total: principal + interest,
                        principal,
                        interest,
                    };
                    w.payments.push((payment, owed));
                }
                if let Some(i) = interest_conv.filter(|i| *i > Decimal::ZERO) {
                    w.interest.push((txn.date, i));
                }
                w.steps.push(Step {
                    date: txn.date,
                    owed,
                    delta,
                });
                first = false;
            }
            w.ends.push((month, self.owed_of(&w.balances, cur, at)));
        }
        w
    }

    /// The card for one account, or nothing when there is nothing on
    /// it and nothing happened to it lately.
    #[allow(clippy::too_many_arguments)]
    fn debt_of(
        &self,
        info: &AccountInfo,
        w: &Walk,
        today: Day,
        current: MonthKey,
        basis: u32,
        cur: &str,
        unpriced: &mut BTreeSet<String>,
    ) -> Option<Debt> {
        let at = current.end_of_month();
        let from = current.minus(basis.max(1) - 1);
        let window_start: Day = (from.year, from.month, 1);
        let active = w.steps.iter().any(|s| s.date >= window_start);
        let held = w.balances.iter().any(|(_, v)| !v.is_zero());
        if !active && !held {
            return None;
        }
        let owed = -self.convertible(&w.balances, cur, at, unpriced);
        let kind = debt_kind(&info.account, w.recharged);

        let mut balances: Vec<(String, Decimal)> = w
            .balances
            .iter()
            .filter(|(_, v)| !v.is_zero())
            .cloned()
            .collect();
        self.sort_amounts(&mut balances, cur);

        let foreign = balances
            .iter()
            .filter(|(c, _)| c != cur)
            .map(|(c, v)| Foreign {
                code: c.clone(),
                amount: -v,
                converted: self.convert(-v, c, cur, at).map(cents),
            })
            .collect();

        let mut rates: Vec<Decimal> =
            w.rates.iter().rev().take(RATE_SAMPLES).copied().collect();
        let rate = info
            .debt
            .rate
            .or_else(|| (!rates.is_empty()).then(|| median(&mut rates)))
            .map(|r| r.round_dp(4));

        let payments: Vec<&Payment> =
            w.payments.iter().map(|(p, _)| p).collect();
        let mut recent: Vec<Decimal> = payments
            .iter()
            .rev()
            .take(PAYMENT_SAMPLES)
            .map(|p| p.total)
            .collect();
        let payment = (!recent.is_empty()).then(|| cents(median(&mut recent)));
        let mut days: Vec<Decimal> = payments
            .iter()
            .rev()
            .take(DUE_SAMPLES)
            .map(|p| Decimal::from(p.date.2))
            .collect();
        let due_day = info.debt.due.or_else(|| {
            (payments.len() >= DUE_PAYMENTS)
                .then(|| {
                    median(&mut days).round().to_string().parse::<u8>().ok()
                })
                .flatten()
        });
        let paid_this_month = payments
            .iter()
            .any(|p| MonthKey::new(p.date.0, p.date.1) == current);
        let next_due = due_day
            .filter(|_| owed > Decimal::ZERO)
            .map(|day| next_due(day, paid_this_month, today, current));

        let cycle =
            (kind == DebtKind::Revolving).then(|| self.cycle_of(w, current));
        let carried = cycle
            .as_ref()
            .and_then(|c| c.carried)
            .is_some_and(|c| c > Decimal::ZERO);
        let payoff = (owed > Decimal::ZERO
            && (kind == DebtKind::Installment || carried))
            .then(|| {
                let payment = payment.filter(|p| *p > Decimal::ZERO)?;
                let plan = amortize(owed, rate.unwrap_or_default(), payment)?;
                Some(Payoff {
                    months: plan.months,
                    month: (0..plan.months).fold(current, |m, _| m.next()),
                    interest: plan.interest,
                })
            })
            .flatten();
        let treadmill = (owed > Decimal::ZERO && carried)
            .then(|| {
                treadmill_of(w, current, basis, owed, rate.unwrap_or_default())
            })
            .flatten();
        let makeup = (owed > Decimal::ZERO && carried)
            .then(|| self.makeup_of(&info.account, current, cur))
            .flatten();
        let peak = cents(w.peak);
        let limit = info.debt.limit.and_then(|limit| {
            let own = match info.currencies.as_slice() {
                [only] => only.as_str(),
                _ => cur,
            };
            self.convert(limit, own, cur, at).map(cents)
        });
        let utilisation = limit
            .filter(|l| *l > Decimal::ZERO)
            .map(|l| (owed.max(Decimal::ZERO) / l).round_dp(4));
        let collateral = info.debt.collateral.as_ref().map(|name| {
            let held = self.account(name);
            Collateral {
                account: name.clone(),
                label: held.map_or_else(
                    || name.rsplit(':').next().unwrap_or(name).to_string(),
                    |a| a.label.clone(),
                ),
                value: held.and_then(|a| {
                    let balances = self.balance_at(&a.account, current);
                    self.owed_of(&balances, cur, at).map(|v| -v)
                }),
            }
        });
        let progress = (kind == DebtKind::Installment && peak > Decimal::ZERO)
            .then(|| {
                ((peak - owed) / peak)
                    .round_dp(4)
                    .clamp(Decimal::ZERO, Decimal::ONE)
            });

        Some(Debt {
            account: info.account.clone(),
            label: info.label.clone(),
            kind,
            owed,
            balances,
            foreign,
            limit,
            utilisation,
            collateral,
            peak,
            progress,
            rate,
            payment,
            due_day,
            next_due,
            principal_paid: cents(payments.iter().map(|p| p.principal).sum()),
            interest_paid: cents(w.interest.iter().map(|(_, i)| *i).sum()),
            payments: payments
                .iter()
                .rev()
                .take(LISTED_PAYMENTS)
                .map(|p| (*p).clone())
                .collect(),
            history: history(w, self.months_range(today)),
            trail: trail(w, from),
            payoff,
            cycle,
            treadmill,
            makeup,
        })
    }

    /// This month's traffic on a card, and what its last payment left
    /// behind that was older than a statement.
    fn cycle_of(&self, w: &Walk, current: MonthKey) -> Cycle {
        let month_of = |d: Day| MonthKey::new(d.0, d.1);
        let charges = w
            .charges
            .iter()
            .filter(|(d, _)| month_of(*d) == current)
            .map(|(_, v)| *v)
            .sum();
        let payments = w
            .payments
            .iter()
            .filter(|(p, _)| month_of(p.date) == current)
            .map(|(p, _)| p.total)
            .sum();
        let carried = w
            .payments
            .last()
            .filter(|(p, _)| month_of(p.date) >= current.minus(HABIT_MONTHS))
            .and_then(|(p, after)| {
                let fresh: Decimal = w
                    .charges
                    .iter()
                    .filter(|(d, _)| {
                        let age = days_between(*d, p.date);
                        (0..STATEMENT_DAYS).contains(&age)
                    })
                    .map(|(_, v)| *v)
                    .sum();
                Some(cents((*after)? - fresh).max(Decimal::ZERO))
            });
        Cycle {
            charges: cents(charges),
            payments: cents(payments),
            carried,
            in_full: carried.map(|c| c.is_zero()),
        }
    }

    /// Budget-kind assets at the month's end: the runway's liquid.
    fn liquid_cash(
        &self,
        current: MonthKey,
        cur: &str,
        unpriced: &mut BTreeSet<String>,
    ) -> Decimal {
        let at = current.end_of_month();
        let mut cash = Decimal::ZERO;
        for info in self.accounts() {
            if !info.account.starts_with("Assets:")
                || info.kind != AccountKind::Budget
            {
                continue;
            }
            let balances = self.balance_at(&info.account, current);
            cash += self.convertible(&balances, cur, at, unpriced);
        }
        cents(cash)
    }

    /// What a carried card holds, charge by charge. Payments clear the
    /// oldest charges first, as a statement does; interest lands on
    /// every open charge in proportion to what is left of it, and is
    /// paid before the charge itself. A refund is a payment: it comes
    /// off the oldest charge, not the one it refunds. Each currency on
    /// the card keeps its own queue, converted only at the end.
    fn makeup_of(
        &self,
        account: &str,
        current: MonthKey,
        cur: &str,
    ) -> Option<Makeup> {
        let mut queues: BTreeMap<String, VecDeque<Lot>> = BTreeMap::new();
        for (month, _) in self.months_of(account) {
            if month > current {
                break;
            }
            for txn in self.txns(account, month) {
                let mut own: Vec<(String, Decimal)> = Vec::new();
                let mut others: Vec<(&str, Decimal, &str)> = Vec::new();
                let mut interest = false;
                for posting in &txn.postings {
                    if posting.account == account {
                        for (v, c) in &posting.amounts {
                            add_sum(&mut own, c, *v);
                        }
                        continue;
                    }
                    interest |= is_interest_expense(&posting.account);
                    for (v, c) in &posting.amounts {
                        others.push((posting.account.as_str(), *v, c.as_str()));
                    }
                }
                for (c, v) in &own {
                    let queue = queues.entry(c.clone()).or_default();
                    if *v < Decimal::ZERO {
                        let rise = -*v;
                        if interest && !queue.is_empty() {
                            spread(queue, rise);
                        } else {
                            for (to, share) in
                                self.charge_split(&others, rise, c, txn.date)
                            {
                                queue.push_back(Lot {
                                    date: txn.date,
                                    account: to,
                                    principal: share,
                                    interest: Decimal::ZERO,
                                    accrued: Decimal::ZERO,
                                });
                            }
                        }
                    } else if *v > Decimal::ZERO {
                        consume(queue, *v);
                    }
                }
            }
        }

        let at = current.end_of_month();
        let mut rows: BTreeMap<String, MakeupRow> = BTreeMap::new();
        for (c, queue) in &queues {
            for lot in queue {
                let Some((principal, interest, accrued)) = (|| {
                    Some((
                        cents(self.convert(lot.principal, c, cur, at)?),
                        cents(self.convert(lot.interest, c, cur, at)?),
                        cents(self.convert(lot.accrued, c, cur, at)?),
                    ))
                })() else {
                    continue;
                };
                let row =
                    rows.entry(lot.account.clone()).or_insert_with(|| {
                        MakeupRow {
                            account: lot.account.clone(),
                            label: self.category_label(&lot.account),
                            owed: Decimal::ZERO,
                            charged: Decimal::ZERO,
                            interest: Decimal::ZERO,
                            since: lot.date,
                            count: 0,
                        }
                    });
                row.owed += principal + interest;
                row.charged += principal;
                row.interest += accrued;
                row.since = row.since.min(lot.date);
                row.count += 1;
            }
        }
        let mut rows: Vec<MakeupRow> = rows.into_values().collect();
        rows.sort_by(|a, b| {
            b.owed.cmp(&a.owed).then_with(|| a.account.cmp(&b.account))
        });
        let total = cents(rows.iter().map(|r| r.owed).sum());
        (!rows.is_empty()).then_some(Makeup { rows, total })
    }

    /// Where a charge went: the transaction's other postings that were
    /// debited, each taking its share of the rise. Nothing debited, or
    /// nothing priceable, and the charge goes unexplained.
    fn charge_split(
        &self,
        others: &[(&str, Decimal, &str)],
        rise: Decimal,
        currency: &str,
        at: Day,
    ) -> Vec<(String, Decimal)> {
        let weights: Vec<(&str, Decimal)> = others
            .iter()
            .filter(|(_, v, _)| *v > Decimal::ZERO)
            .filter_map(|(to, v, c)| {
                Some((*to, self.convert(*v, c, currency, at)?))
            })
            .collect();
        let total: Decimal = weights.iter().map(|(_, w)| *w).sum();
        if total <= Decimal::ZERO {
            return vec![(String::new(), rise)];
        }
        let mut shares = Vec::with_capacity(weights.len());
        let mut given = Decimal::ZERO;
        for (i, (to, weight)) in weights.iter().enumerate() {
            let share = if i + 1 == weights.len() {
                rise - given
            } else {
                cents(rise * *weight / total)
            };
            given += share;
            shares.push((to.to_string(), share));
        }
        shares
    }

    /// The name a category is shown under.
    fn category_label(&self, account: &str) -> String {
        if account.is_empty() {
            return "Other".to_string();
        }
        self.account(account).map_or_else(
            || account.rsplit(':').next().unwrap_or(account).to_string(),
            |a| a.label.clone(),
        )
    }

    /// What could go on the debts beyond the usual payments, this
    /// month and every month.
    fn extra_of(
        &self,
        upcoming: &[Upcoming],
        principal_paid: &BTreeMap<MonthKey, Decimal>,
        cash: Decimal,
        current: MonthKey,
        basis: u32,
        cur: &str,
    ) -> Extra {
        let due = cents(upcoming.iter().map(|u| u.amount).sum());
        let window = self.window(current, basis);
        let months: Vec<MonthKey> = window
            .map(|(from, to)| {
                std::iter::successors(Some(from), |m| Some(m.next()))
                    .take_while(|m| *m <= to)
                    .collect()
            })
            .unwrap_or_default();
        let cash_spend = self.cash_spend(&months, cur);
        let mut spent: Vec<Decimal> = cash_spend
            .values()
            .copied()
            .filter(|v| *v > Decimal::ZERO)
            .collect();
        let spend = if spent.is_empty() {
            Decimal::ZERO
        } else {
            cents(median(&mut spent))
        };
        let buffer = self
            .recurring_view(current, cur, Decimal::ZERO)
            .monthly_fixed;
        let now = cents(cash - due - spend - buffer).max(Decimal::ZERO);
        let mut left: Vec<Decimal> = months
            .iter()
            .map(|m| {
                self.net_flow(*m, cur)
                    - principal_paid.get(m).copied().unwrap_or_default()
            })
            .collect();
        let monthly = if left.is_empty() {
            Decimal::ZERO
        } else {
            cents(median(&mut left)).max(Decimal::ZERO)
        };
        Extra {
            cash,
            due,
            spend,
            buffer,
            now,
            monthly,
        }
    }

    /// Spending paid straight from cash in each of `months`: the
    /// expenses in transactions that touch a budget account and no
    /// liability. What goes on a card is paid when the card is.
    fn cash_spend(
        &self,
        months: &[MonthKey],
        cur: &str,
    ) -> BTreeMap<MonthKey, Decimal> {
        let mut spend: BTreeMap<MonthKey, Decimal> =
            months.iter().map(|m| (*m, Decimal::ZERO)).collect();
        let (Some(first), Some(last)) = (months.first(), months.last()) else {
            return spend;
        };
        for txn in &self.txns {
            let month = MonthKey::new(txn.date.0, txn.date.1);
            if month < *first || month > *last {
                continue;
            }
            let on_credit = txn
                .postings
                .iter()
                .any(|p| p.account.starts_with("Liabilities:"));
            let from_cash = txn.postings.iter().any(|p| {
                p.account.starts_with("Assets:")
                    && self
                        .account(&p.account)
                        .is_some_and(|a| a.kind == AccountKind::Budget)
            });
            if on_credit || !from_cash {
                continue;
            }
            let at = month.end_of_month();
            let expenses: Decimal = txn
                .postings
                .iter()
                .filter(|p| p.account.starts_with("Expenses:"))
                .flat_map(|p| p.amounts.iter())
                .filter_map(|(v, c)| self.convert(*v, c, cur, at))
                .sum();
            *spend.entry(month).or_default() += expenses;
        }
        for v in spend.values_mut() {
            *v = cents(*v);
        }
        spend
    }

    /// Income less expenses in one month, converted at its end. Both
    /// sit on the same side of the ledger's sign: income is negative,
    /// spending positive, so what is left is the negative of their sum.
    fn net_flow(&self, month: MonthKey, cur: &str) -> Decimal {
        let at = month.end_of_month();
        let mut unpriced = BTreeSet::new();
        let mut net = Decimal::ZERO;
        for info in self.accounts() {
            if !info.account.starts_with("Income:")
                && !info.account.starts_with("Expenses:")
            {
                continue;
            }
            let sums = self.sums(&info.account, month);
            net -= self.convertible(sums, cur, at, &mut unpriced);
        }
        cents(net)
    }

    /// Interest received this month and over the window, from the
    /// income accounts that say so.
    fn interest_earned(
        &self,
        current: MonthKey,
        window: Option<(MonthKey, MonthKey)>,
        cur: &str,
    ) -> (Decimal, Decimal) {
        let mut month_total = Decimal::ZERO;
        let mut year_total = Decimal::ZERO;
        let mut unpriced = BTreeSet::new();
        for info in self.accounts() {
            if !info.account.starts_with("Income:")
                || !names_interest(&info.account)
            {
                continue;
            }
            for (month, sums) in self.months_of(&info.account) {
                if month > current {
                    break;
                }
                let earned = -self.convertible(
                    sums,
                    cur,
                    month.end_of_month(),
                    &mut unpriced,
                );
                if month == current {
                    month_total += earned;
                }
                if window.is_some_and(|(from, to)| month >= from && month <= to)
                {
                    year_total += earned;
                }
            }
        }
        (cents(month_total), cents(year_total))
    }

    /// What is owed given a balance list: the negative of its converted
    /// sum, or `None` when any nonzero part has no price.
    fn owed_of(
        &self,
        balances: &[(String, Decimal)],
        cur: &str,
        at: Day,
    ) -> Option<Decimal> {
        let mut total = Decimal::ZERO;
        for (c, v) in balances {
            if v.is_zero() {
                continue;
            }
            total += self.convert(*v, c, cur, at)?;
        }
        Some(cents(-total))
    }

    fn convert_all(
        &self,
        amounts: &[(String, Decimal)],
        cur: &str,
        at: Day,
    ) -> Option<Decimal> {
        let mut total = Decimal::ZERO;
        for (c, v) in amounts {
            total += self.convert(*v, c, cur, at)?;
        }
        Some(cents(total))
    }
}

/// A loan paid down at `payment` a month, interest accruing monthly at
/// a twelfth of `rate` and rounded to cents as a lender would. `None`
/// when the payment never gets ahead of the interest.
pub fn amortize(
    owed: Decimal,
    rate: Decimal,
    payment: Decimal,
) -> Option<Amortization> {
    if owed <= Decimal::ZERO {
        return Some(Amortization {
            months: 0,
            interest: Decimal::ZERO,
        });
    }
    if payment <= Decimal::ZERO {
        return None;
    }
    let monthly = rate / Decimal::from(12);
    let mut balance = owed;
    let mut interest = Decimal::ZERO;
    for months in 1..=MAX_MONTHS {
        let accrued = cents(balance * monthly);
        if payment <= accrued {
            return None;
        }
        interest += accrued;
        balance += accrued - payment;
        if balance <= Decimal::ZERO {
            return Some(Amortization { months, interest });
        }
    }
    None
}

/// Card or loan. The name says which when it can; otherwise a debt
/// that took new charges after it was opened is a card.
pub fn debt_kind(account: &str, recharged: bool) -> DebtKind {
    const REVOLVING: &[&str] =
        &["card", "credit", "visa", "mastercard", "amex", "overdraft"];
    const INSTALLMENT: &[&str] = &[
        "loan", "mortgage", "financ", "student", "auto", "car", "lease",
    ];
    let name = account.to_ascii_lowercase();
    for segment in name.split(':').skip(1) {
        if REVOLVING.iter().any(|k| segment.contains(k)) {
            return DebtKind::Revolving;
        }
        if INSTALLMENT.iter().any(|k| segment.contains(k)) {
            return DebtKind::Installment;
        }
    }
    if recharged {
        DebtKind::Revolving
    } else {
        DebtKind::Installment
    }
}

fn names_interest(account: &str) -> bool {
    account
        .split(':')
        .skip(1)
        .any(|s| s.to_ascii_lowercase().contains("interest"))
}

fn is_interest_expense(account: &str) -> bool {
    account.starts_with("Expenses:") && names_interest(account)
}

/// One charge on a card, as much of it as is still unpaid.
struct Lot {
    date: Day,
    /// Where the charge went.
    account: String,
    /// The charge itself, still unpaid.
    principal: Decimal,
    /// Interest on it, still unpaid.
    interest: Decimal,
    /// Interest on it so far, paid or not.
    accrued: Decimal,
}

impl Lot {
    fn left(&self) -> Decimal {
        self.principal + self.interest
    }
}

/// Land an interest charge on every open lot in proportion to what is
/// left of it, the last lot taking the rounding.
fn spread(queue: &mut VecDeque<Lot>, interest: Decimal) {
    let total: Decimal = queue.iter().map(Lot::left).sum();
    if total <= Decimal::ZERO {
        return;
    }
    let last = queue.len() - 1;
    let mut given = Decimal::ZERO;
    for (i, lot) in queue.iter_mut().enumerate() {
        let share = if i == last {
            interest - given
        } else {
            cents(interest * lot.left() / total)
        };
        given += share;
        lot.interest += share;
        lot.accrued += share;
    }
}

/// Pay the oldest lots first, each one's interest before its charge.
fn consume(queue: &mut VecDeque<Lot>, mut amount: Decimal) {
    while amount > Decimal::ZERO {
        let Some(lot) = queue.front_mut() else {
            return;
        };
        let interest = amount.min(lot.interest);
        lot.interest -= interest;
        amount -= interest;
        let principal = amount.min(lot.principal);
        lot.principal -= principal;
        amount -= principal;
        if lot.left() <= Decimal::ZERO {
            queue.pop_front();
        }
    }
}

/// The complete months of the trailing window on a carried card, and
/// the pace its newest few set.
fn treadmill_of(
    w: &Walk,
    current: MonthKey,
    basis: u32,
    owed: Decimal,
    rate: Decimal,
) -> Option<Treadmill> {
    let month_of = |d: Day| MonthKey::new(d.0, d.1);
    let first = month_of(w.steps.first()?.date);
    let from = current.minus(basis.max(1)).max(first);
    let to = current.prev();
    let sum_in = |xs: &[(Day, Decimal)], month: MonthKey| -> Decimal {
        xs.iter()
            .filter(|(d, _)| month_of(*d) == month)
            .map(|(_, v)| *v)
            .sum()
    };
    let months: Vec<TreadmillMonth> =
        std::iter::successors(Some(from), |m| Some(m.next()))
            .take_while(|m| *m <= to)
            .map(|month| {
                let interest = sum_in(&w.interest, month);
                let charges =
                    (sum_in(&w.charges, month) - interest).max(Decimal::ZERO);
                let payments = w
                    .payments
                    .iter()
                    .filter(|(p, _)| month_of(p.date) == month)
                    .map(|(p, _)| p.total)
                    .sum();
                TreadmillMonth {
                    month,
                    charges: cents(charges),
                    payments: cents(payments),
                    interest: cents(interest),
                }
            })
            .collect();
    if months.is_empty() {
        return None;
    }
    let recent: Vec<&TreadmillMonth> =
        months.iter().rev().take(PACE_MONTHS).collect();
    let pace = recent.len();
    let charged: Decimal = recent.iter().map(|m| m.charges).sum();
    let paid: Decimal = recent.iter().map(|m| m.payments).sum();
    let net = cents((paid - charged) / Decimal::from(pace as u32));
    let payoff = (net > Decimal::ZERO)
        .then(|| amortize(owed, rate, net))
        .flatten()
        .map(|plan| Payoff {
            months: plan.months,
            month: (0..plan.months).fold(current, |m, _| m.next()),
            interest: plan.interest,
        });
    Some(Treadmill {
        months,
        pace: pace as u32,
        charged: cents(charged),
        paid: cents(paid),
        net,
        payoff,
    })
}

fn amount_in(balances: &[(String, Decimal)], currency: &str) -> Decimal {
    balances
        .iter()
        .find(|(c, _)| c == currency)
        .map(|(_, v)| *v)
        .unwrap_or_default()
}

/// The next time the usual day comes round: this month if it has not
/// yet and nothing has been paid, otherwise next month.
fn next_due(
    day: u8,
    paid_this_month: bool,
    today: Day,
    current: MonthKey,
) -> Day {
    let month = if paid_this_month || day < today.2 {
        current.next()
    } else {
        current
    };
    (month.year, month.month, day.min(month.days_in_month()))
}

/// Month-end balances over the whole range, carried forward through
/// the months nothing happened in.
fn history(w: &Walk, months: Vec<MonthKey>) -> Vec<DebtPoint> {
    let mut ends = w.ends.iter().peekable();
    let mut owed = Some(Decimal::ZERO);
    months
        .into_iter()
        .map(|month| {
            while let Some((m, o)) = ends.peek() {
                if *m > month {
                    break;
                }
                owed = *o;
                ends.next();
            }
            DebtPoint { month, owed }
        })
        .collect()
}

/// Every step since `from`, opened by where the balance stood at the
/// end of the month before.
fn trail(w: &Walk, from: MonthKey) -> Vec<TrailPoint> {
    let start: Day = (from.year, from.month, 1);
    let opening = w
        .ends
        .iter()
        .take_while(|(m, _)| *m < from)
        .last()
        .map_or(Some(Decimal::ZERO), |(_, o)| *o);
    std::iter::once(TrailPoint {
        date: start,
        owed: opening,
        delta: None,
    })
    .chain(
        w.steps
            .iter()
            .filter(|s| s.date >= start)
            .map(|s| TrailPoint {
                date: s.date,
                owed: s.owed,
                delta: s.delta,
            }),
    )
    .collect()
}

/// Owing, and on the way down at a payment: a loan, or a card carrying
/// a balance. What `debt_free` waits for.
fn paying_down(d: &Debt) -> bool {
    let carried = d
        .cycle
        .as_ref()
        .and_then(|c| c.carried)
        .is_some_and(|c| c > Decimal::ZERO);
    d.owed > Decimal::ZERO && (d.kind == DebtKind::Installment || carried)
}

/// A debt that is over, if this one is: nothing owed in any currency,
/// something once was, and either it is a loan or the account has
/// been closed. A card at zero that is still open is just between
/// statements.
fn beaten_of(info: &AccountInfo, w: &Walk) -> Option<Beaten> {
    let held = w.balances.iter().any(|(_, v)| !v.is_zero());
    let kind = debt_kind(&info.account, w.recharged);
    let over = kind == DebtKind::Installment || info.closed.is_some();
    if held || w.peak <= Decimal::ZERO || !over {
        return None;
    }
    let first = w.steps.first()?.date;
    let last = w.payments.last().map(|(p, _)| p.date)?;
    Some(Beaten {
        account: info.account.clone(),
        label: info.label.clone(),
        peak: cents(w.peak),
        principal_paid: cents(
            w.payments.iter().map(|(p, _)| p.principal).sum(),
        ),
        interest_paid: cents(w.interest.iter().map(|(_, i)| *i).sum()),
        first,
        last,
    })
}

fn upcoming(debts: &[Debt], today: Day) -> Vec<Upcoming> {
    let mut due: Vec<Upcoming> = debts
        .iter()
        .filter_map(|d| {
            let date = d.next_due?;
            // A card cleared every statement pays what is on it; a
            // loan, or a card being carried, pays its payment.
            let in_full = d.cycle.as_ref().and_then(|c| c.in_full);
            let amount = if in_full == Some(true) {
                d.owed
            } else {
                d.payment?
            };
            let ahead = days_between(today, date);
            ((0..=UPCOMING_DAYS).contains(&ahead)).then(|| Upcoming {
                account: d.account.clone(),
                label: d.label.clone(),
                date,
                amount,
            })
        })
        .collect();
    due.sort_by(|a, b| a.date.cmp(&b.date).then_with(|| a.label.cmp(&b.label)));
    due
}

fn notices(debts: &[Debt], today: Day, current: MonthKey) -> Vec<Notice> {
    let month_of = |d: Day| MonthKey::new(d.0, d.1);
    let mut found = Vec::new();
    for d in debts {
        let notice = |kind, amount, day| Notice {
            kind,
            account: d.account.clone(),
            label: d.label.clone(),
            amount,
            day,
        };
        if let Some(day) = d.due_day.filter(|_| d.owed > Decimal::ZERO) {
            let habit = d.payments.first().is_some_and(|p| {
                month_of(p.date) >= current.minus(HABIT_MONTHS)
            });
            let paid = d.payments.iter().any(|p| month_of(p.date) == current);
            let late = month_of(today) == current
                && today.2 > day.saturating_add(GRACE_DAYS);
            if habit && !paid && late {
                found.push(notice(NoticeKind::Missed, d.payment, Some(day)));
            }
        }
        let carried = d
            .cycle
            .as_ref()
            .and_then(|c| c.carried)
            .filter(|c| *c > Decimal::ZERO && rising(&d.history, current));
        if let Some(carried) = carried {
            found.push(notice(NoticeKind::Growing, Some(carried), None));
        }
        if d.owed < Decimal::ZERO {
            found.push(notice(NoticeKind::Overpaid, Some(-d.owed), None));
        }
    }
    found.sort_by_key(|n| n.kind);
    found
}

/// The last few complete months each ended higher than the one before.
fn rising(history: &[DebtPoint], current: MonthKey) -> bool {
    let complete: Vec<Decimal> = history
        .iter()
        .filter(|p| p.month < current)
        .map(|p| p.owed)
        .collect::<Option<Vec<_>>>()
        .unwrap_or_default();
    if complete.len() <= GROWING_MONTHS {
        return false;
    }
    complete[complete.len() - GROWING_MONTHS - 1..]
        .windows(2)
        .all(|w| w[1] > w[0])
}
