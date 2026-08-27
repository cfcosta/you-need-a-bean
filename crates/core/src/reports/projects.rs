//! Tags and links as topics: what `#renovation` cost, over how long,
//! across how many categories.
//!
//! Beancount users scope work with tags and links, and nothing on the
//! page ever added one up. What it adds up to depends on the ledger,
//! though — importers write identifiers too. For example, an importer
//! may attach a unique `^import-payment-0001` link to each transaction
//! or use `#transfer-marker` on internal transfers.
//!
//! So this doesn't try to guess which topics are projects. It reports
//! the facts that make each one obvious — how many transactions, how
//! many months, how many categories — and drops only the two shapes
//! that can't be a topic under any reading: a name on one transaction,
//! and a name with no money attached.

use std::collections::{BTreeMap, BTreeSet};

use rust_decimal::Decimal;

use crate::model::{Day, Ledger, MonthKey};
use crate::query::cents;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Topic {
    Tag,
    Link,
}

impl Topic {
    /// The sigil the topic is written with in the ledger.
    pub fn sigil(self) -> char {
        match self {
            Topic::Tag => '#',
            Topic::Link => '^',
        }
    }
}

#[derive(Debug, Clone)]
pub struct Project {
    pub name: String,
    pub kind: Topic,
    /// Converted spend across every `Expenses:*` posting carrying it,
    /// refunds already netted out.
    pub spent: Decimal,
    /// Converted income, positive when money came back: a trip that got
    /// reimbursed did not cost what it charged.
    pub income: Decimal,
    /// `spent - income` — what the topic actually took.
    pub net: Decimal,
    pub count: usize,
    /// Distinct expense accounts touched. A project crosses categories;
    /// a statement tag lands in one or two.
    pub categories: usize,
    pub first: Day,
    pub last: Day,
    /// Calendar months the span covers, inclusive — 1 for a topic that
    /// began and ended inside one month.
    pub months: u32,
}

#[derive(Debug, Clone)]
pub struct ProjectsView {
    /// Qualifying topics, costliest first.
    pub items: Vec<Project>,
    /// Names seen on exactly one transaction. Overwhelmingly an
    /// importer's payment id, which is a reference and not a topic.
    pub singletons: usize,
    /// Names that moved no money at all: bookkeeping marks on
    /// transfers between accounts you already own.
    pub markers: usize,
}

/// What one topic accumulates while the ledger is scanned.
#[derive(Default)]
struct Tally {
    spent: Decimal,
    income: Decimal,
    count: usize,
    accounts: BTreeSet<String>,
    span: Option<(Day, Day)>,
}

impl Ledger {
    pub(super) fn projects_view(
        &self,
        cur: &str,
        unpriced: &mut BTreeSet<String>,
    ) -> ProjectsView {
        let mut tallies: BTreeMap<(Topic, &str), Tally> = BTreeMap::new();
        for txn in &self.txns {
            let names = txn
                .tags
                .iter()
                .map(|t| (Topic::Tag, t.as_str()))
                .chain(txn.links.iter().map(|l| (Topic::Link, l.as_str())));
            let mut names = names.peekable();
            if names.peek().is_none() {
                continue;
            }

            // Value each posting on its own date rather than at a month
            // end: a topic is a span, not a column in a monthly table.
            let (mut spent, mut income) = (Decimal::ZERO, Decimal::ZERO);
            let mut accounts = Vec::new();
            for posting in &txn.postings {
                let expense = posting.account.starts_with("Expenses:");
                if !expense && !posting.account.starts_with("Income:") {
                    continue;
                }
                for (v, c) in &posting.amounts {
                    match self.convert(*v, c, cur, txn.date) {
                        // Income postings are negative in beancount.
                        Some(converted) if expense => spent += converted,
                        Some(converted) => income -= converted,
                        None if !v.is_zero() => {
                            unpriced.insert(c.clone());
                        }
                        None => {}
                    }
                }
                if expense {
                    accounts.push(posting.account.clone());
                }
            }

            for key in names {
                let tally = tallies.entry(key).or_default();
                tally.spent += spent;
                tally.income += income;
                tally.count += 1;
                tally.accounts.extend(accounts.iter().cloned());
                tally.span = Some(match tally.span {
                    Some((first, last)) => {
                        (first.min(txn.date), last.max(txn.date))
                    }
                    None => (txn.date, txn.date),
                });
            }
        }

        let (mut singletons, mut markers) = (0, 0);
        let mut items = Vec::new();
        for ((kind, name), tally) in tallies {
            let Some((first, last)) = tally.span else {
                continue;
            };
            if tally.count < 2 {
                singletons += 1;
                continue;
            }
            let (spent, income) = (cents(tally.spent), cents(tally.income));
            if spent.is_zero() && income.is_zero() {
                markers += 1;
                continue;
            }
            items.push(Project {
                name: name.to_string(),
                kind,
                spent,
                income,
                net: spent - income,
                count: tally.count,
                categories: tally.accounts.len(),
                first,
                last,
                months: month_of(first).months_until(month_of(last)),
            });
        }
        items.sort_by(|a, b| b.net.cmp(&a.net).then(a.name.cmp(&b.name)));

        ProjectsView {
            items,
            singletons,
            markers,
        }
    }
}

fn month_of((year, month, _): Day) -> MonthKey {
    MonthKey::new(year, month)
}
