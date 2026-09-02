//! What you owe, what it costs, and when it ends.
//!
//! Every liability with something on it gets a card: how much, at what
//! rate, paid how much and when, and — for a loan — the month the last
//! payment lands if the current one keeps landing. None of it needs a
//! number the ledger does not already hold. The rate is what the
//! interest legs say against the balance they were charged on; the
//! payment is what has been paid lately; the due day is where the
//! payments fall; a card is paid in full when the payment cleared
//! everything older than a statement.
//!
//! Signs are the reader's, not the ledger's: a liability holds a
//! negative balance, and this module reports it as a positive amount
//! owed. A charge raises it, a payment lowers it.

use std::collections::BTreeSet;

use rust_decimal::Decimal;

use crate::model::{
    AccountInfo, AccountKind, Day, Ledger, MonthKey, add_sum, days_between,
};
use crate::query::{cents, median};

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

#[derive(Debug, Clone)]
pub struct Debt {
    pub account: String,
    pub label: String,
    pub kind: DebtKind,
    /// Owed now, in the display currency. Negative when in credit.
    pub owed: Decimal,
    /// Owed now per commodity, as the ledger signs it.
    pub balances: Vec<(String, Decimal)>,
    /// The most that was ever owed.
    pub peak: Decimal,
    /// How much of the peak has been paid down; loans only.
    pub progress: Option<Decimal>,
    /// Nominal annual rate read off the interest legs.
    pub rate: Option<Decimal>,
    /// What a payment has been lately.
    pub payment: Option<Decimal>,
    /// The day of the month payments land on.
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
    /// Loans only: where the current payment lands.
    pub payoff: Option<Payoff>,
    /// Cards only.
    pub cycle: Option<Cycle>,
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
    /// The month the last loan pays off, when every loan does.
    pub debt_free: Option<MonthKey>,
    pub cover: Cover,
    /// Payments expected in the next month, soonest first.
    pub upcoming: Vec<Upcoming>,
    pub notices: Vec<Notice>,
    /// Biggest first.
    pub debts: Vec<Debt>,
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
        let current = self.default_month(today);
        let window = self.window(current, 12);
        let in_window = |month: MonthKey| {
            window.is_some_and(|(from, to)| month >= from && month <= to)
        };
        let mut unpriced = BTreeSet::new();
        let mut debts = Vec::new();
        let mut interest_month = Decimal::ZERO;
        let mut interest_year = Decimal::ZERO;
        for info in self.accounts() {
            if !info.account.starts_with("Liabilities:")
                || info.kind == AccountKind::Hidden
            {
                continue;
            }
            let walk = self.walk_debt(&info.account, current, cur);
            for (date, amount) in &walk.interest {
                let month = MonthKey::new(date.0, date.1);
                if month == current {
                    interest_month += amount;
                }
                if in_window(month) {
                    interest_year += amount;
                }
            }
            if let Some(debt) = self.debt_of(
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
        let loans: Vec<&Debt> = debts
            .iter()
            .filter(|d| {
                d.kind == DebtKind::Installment && d.owed > Decimal::ZERO
            })
            .collect();
        let debt_free = (!loans.is_empty())
            .then(|| {
                loans
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
            cover: Cover {
                cash,
                owed: revolving,
                covered: cash >= revolving,
                after: cash - revolving,
            },
            upcoming,
            notices,
            debts,
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

        let mut rates: Vec<Decimal> =
            w.rates.iter().rev().take(RATE_SAMPLES).copied().collect();
        let rate = (!rates.is_empty()).then(|| median(&mut rates).round_dp(4));

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
        let due_day = (payments.len() >= DUE_PAYMENTS)
            .then(|| median(&mut days).round().to_string().parse::<u8>().ok())
            .flatten();
        let paid_this_month = payments
            .iter()
            .any(|p| MonthKey::new(p.date.0, p.date.1) == current);
        let next_due = due_day
            .filter(|_| owed > Decimal::ZERO)
            .map(|day| next_due(day, paid_this_month, today, current));

        let payoff = (kind == DebtKind::Installment && owed > Decimal::ZERO)
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
        let cycle =
            (kind == DebtKind::Revolving).then(|| self.cycle_of(w, current));
        let peak = cents(w.peak);
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

fn upcoming(debts: &[Debt], today: Day) -> Vec<Upcoming> {
    let mut due: Vec<Upcoming> = debts
        .iter()
        .filter_map(|d| {
            let date = d.next_due?;
            let amount = d.payment?;
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
