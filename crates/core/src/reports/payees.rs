//! Who the money actually goes to.
//!
//! The year card says the money went to groceries. It does not say it
//! went to one supermarket forty times, which is the version you can
//! act on. Three numbers separate the shapes a merchant can take: what
//! it cost, how often it charged, and what one charge came to. A
//! streaming service is twelve small identical tickets; a flight is
//! one large one; a supermarket is a hundred medium ones, and only the
//! first of those is worth cancelling.
//!
//! Payees are taken as the ledger writes them, trimmed and no further.
//! Guessing that `AMZN Mktp` and `Amazon.com` are one merchant is a
//! guess, and a leaderboard built on guesses ranks the guesses.

use std::collections::{BTreeMap, BTreeSet};

use rust_decimal::Decimal;

use crate::model::{Day, Ledger, MonthKey};
use crate::query::cents;

/// How many payees the leaderboard names. A leaderboard is the top of
/// something; the tail is reported as a count and a sum rather than
/// dropped, so the card never reads as the whole list.
const TOP: usize = 15;

/// One merchant over the trailing year.
#[derive(Debug, Clone)]
pub struct Payee {
    /// As the ledger writes it.
    pub name: String,
    /// Converted spend, refunds posted back to the expense already
    /// netted out.
    pub spent: Decimal,
    /// Transactions that moved expense money. A transfer carrying the
    /// same name is not a charge and is not counted.
    pub count: usize,
    /// `spent / count` — what one charge comes to.
    pub average: Decimal,
    /// `spent` over every expense the window moved.
    pub share: Decimal,
    /// Distinct expense accounts it was filed under.
    pub categories: usize,
    /// Calendar months of the window it charged in at all, out of
    /// twelve. Twelve is a subscription; one is a splurge.
    pub months: usize,
    pub first: Day,
    pub last: Day,
}

#[derive(Debug, Clone)]
pub struct PayeesView {
    /// The trailing year everything here covers.
    pub window: Option<(MonthKey, MonthKey)>,
    /// Costliest first, at most [`TOP`] of them.
    pub items: Vec<Payee>,
    /// Every expense the window's transactions moved, named or not, so
    /// each row's share is against the whole and not against the part
    /// that happens to be listed.
    pub total: Decimal,
    /// Payees past the cut, and what they came to between them.
    /// Kept rather than counted, so the line standing in for them can
    /// open onto the names themselves.
    pub others_items: Vec<Payee>,
    pub others_spent: Decimal,
    /// Spend on transactions that name nobody. Not a gap in the
    /// ledger — a narration is often the whole story — but it is money
    /// this card cannot rank, so it says how much.
    pub anonymous: Decimal,
    pub anonymous_count: usize,
}

/// What one payee accumulates while the window is scanned.
#[derive(Default)]
struct Tally {
    spent: Decimal,
    count: usize,
    accounts: BTreeSet<String>,
    months: BTreeSet<MonthKey>,
    span: Option<(Day, Day)>,
}

impl Ledger {
    pub(super) fn payees_view(
        &self,
        current: MonthKey,
        cur: &str,
    ) -> PayeesView {
        let window = self.window(current, 12);
        let Some((from, to)) = window else {
            return PayeesView {
                window,
                items: Vec::new(),
                total: Decimal::ZERO,
                others_items: Vec::new(),
                others_spent: Decimal::ZERO,
                anonymous: Decimal::ZERO,
                anonymous_count: 0,
            };
        };

        let mut tallies: BTreeMap<&str, Tally> = BTreeMap::new();
        let mut total = Decimal::ZERO;
        let mut anonymous = Decimal::ZERO;
        let mut anonymous_count = 0;

        for txn in &self.txns {
            let month = MonthKey::new(txn.date.0, txn.date.1);
            if month < from || month > to {
                continue;
            }

            // Valued on the transaction's own date: a merchant is a
            // sequence of charges, not a column in a monthly table.
            // Anything unpriced drops out here, and is already named
            // by the cashflow pass, which walks the same postings.
            let mut spent = Decimal::ZERO;
            let mut accounts = Vec::new();
            for posting in &txn.postings {
                if !posting.account.starts_with("Expenses:") {
                    continue;
                }
                for (value, currency) in &posting.amounts {
                    if let Some(converted) =
                        self.convert(*value, currency, cur, txn.date)
                    {
                        spent += converted;
                    }
                }
                accounts.push(posting.account.as_str());
            }
            // Nothing was charged, so nobody charged it: a transfer
            // between accounts you already own, whoever it names.
            if accounts.is_empty() {
                continue;
            }
            total += spent;

            let name = txn
                .payee
                .as_deref()
                .map(str::trim)
                .filter(|name| !name.is_empty());
            let Some(name) = name else {
                anonymous += spent;
                anonymous_count += 1;
                continue;
            };

            let tally = tallies.entry(name).or_default();
            tally.spent += spent;
            tally.count += 1;
            tally
                .accounts
                .extend(accounts.iter().map(|a| a.to_string()));
            tally.months.insert(month);
            tally.span = Some(match tally.span {
                Some((first, last)) => {
                    (first.min(txn.date), last.max(txn.date))
                }
                None => (txn.date, txn.date),
            });
        }

        let total = cents(total);
        let mut items: Vec<Payee> = tallies
            .into_iter()
            .filter_map(|(name, tally)| {
                let (first, last) = tally.span?;
                let spent = cents(tally.spent);
                // A name whose charges all came back cost nothing, and
                // ranking it against the ones that didn't says nothing.
                if spent <= Decimal::ZERO {
                    return None;
                }
                Some(Payee {
                    name: name.to_string(),
                    spent,
                    count: tally.count,
                    average: cents(spent / Decimal::from(tally.count)),
                    share: if total > Decimal::ZERO {
                        (spent / total).round_dp(4)
                    } else {
                        Decimal::ZERO
                    },
                    categories: tally.accounts.len(),
                    months: tally.months.len(),
                    first,
                    last,
                })
            })
            .collect();
        items.sort_by(|a, b| b.spent.cmp(&a.spent).then(a.name.cmp(&b.name)));

        let tail = items.split_off(items.len().min(TOP));
        PayeesView {
            window,
            items,
            total,
            others_spent: tail.iter().map(|p| p.spent).sum(),
            others_items: tail,
            anonymous: cents(anonymous),
            anonymous_count,
        }
    }
}
