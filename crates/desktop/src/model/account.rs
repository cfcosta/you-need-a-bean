//! One account's register for a month: how it opened, what came in and
//! went out, and every entry with the balance it left.

use bean_core::model::{AccountKind, Day, Ledger, MonthKey};
use rust_decimal::Decimal;

#[derive(Clone, Debug)]
pub struct Flow {
    pub month: MonthKey,
    pub inflow: Decimal,
    pub outflow: Decimal,
    pub balance: Option<Decimal>,
}

#[derive(Clone, Debug)]
pub struct Entry {
    pub date: Day,
    pub flag: char,
    pub payee: String,
    pub narration: String,
    /// The first posting that is not this account: where the money went
    /// or came from.
    pub other: String,
    pub amount: Decimal,
    pub balance: Option<Decimal>,
}

#[derive(Clone, Debug)]
pub struct Register {
    pub account: String,
    pub label: String,
    pub kind: &'static str,
    pub month: MonthKey,
    pub opening: Decimal,
    pub inflow: Decimal,
    pub ins: usize,
    pub outflow: Decimal,
    pub outs: usize,
    pub balance: Decimal,
    pub history: Vec<Flow>,
    pub entries: Vec<Entry>,
}

impl Register {
    pub fn build(
        ledger: &Ledger,
        today: Day,
        account: &str,
        month: MonthKey,
        basis: u32,
        cur: &str,
    ) -> Option<Self> {
        let l = ledger.as_of(today);
        let v = l.account_view(account, month, basis, cur)?;
        let entries: Vec<Entry> = v
            .entries
            .iter()
            .map(|e| Entry {
                date: e.txn.date,
                flag: e.txn.flag,
                payee: e.txn.payee.clone().unwrap_or_default(),
                narration: e.txn.narration.clone().unwrap_or_default(),
                other: e
                    .txn
                    .postings
                    .iter()
                    .find(|p| p.account != account)
                    .map(|p| p.account.clone())
                    .unwrap_or_default(),
                amount: e.delta.unwrap_or_default(),
                balance: e.balance,
            })
            .collect();
        let opening = v.opening.unwrap_or_default();
        Some(Self {
            account: v.account.clone(),
            label: v.label.clone(),
            kind: match v.kind {
                AccountKind::Budget => "budget",
                AccountKind::Tracking => "tracking",
                AccountKind::Hidden => "hidden",
            },
            month,
            opening,
            ins: entries.iter().filter(|e| e.amount > Decimal::ZERO).count(),
            outs: entries.iter().filter(|e| e.amount < Decimal::ZERO).count(),
            inflow: v.inflow,
            outflow: v.outflow,
            balance: v.converted.unwrap_or(opening + v.inflow - v.outflow),
            history: v
                .history
                .iter()
                .map(|h| Flow {
                    month: h.month,
                    inflow: h.inflow,
                    outflow: h.outflow,
                    balance: h.balance,
                })
                .collect(),
            entries,
        })
    }
}
