//! The ⌘K prompt: every term must appear somewhere in a transaction —
//! date, flag, payee, narration, tags, links, accounts, amounts or
//! metadata — case-insensitive, newest first.

use bean_core::model::{Day, Ledger};
use rust_decimal::Decimal;

#[derive(Clone, Debug)]
pub struct Hit {
    pub date: Day,
    pub flag: char,
    pub payee: String,
    pub narration: String,
    /// The leg that says what it was: the first that is not an asset or
    /// a liability.
    pub category: String,
    /// The asset or liability the money moved through.
    pub bank: String,
    /// What moved through `bank`; negative when money left it.
    pub amount: Decimal,
    /// Every leg as the ledger writes it: account, amount, currency.
    pub postings: Vec<(String, Decimal, String)>,
    pub file: Option<String>,
    pub line: Option<usize>,
    /// Dated after today: a scheduled transaction.
    pub scheduled: bool,
}

fn is_balance(account: &str) -> bool {
    account.starts_with("Assets:") || account.starts_with("Liabilities:")
}

pub fn search(ledger: &Ledger, today: Day, query: &str) -> Vec<Hit> {
    let terms: Vec<String> = query
        .split_whitespace()
        .map(|t| t.trim_start_matches(['#', '^']).to_lowercase())
        .filter(|t| !t.is_empty())
        .collect();
    if terms.is_empty() {
        return vec![];
    }
    let mut hits: Vec<Hit> = ledger
        .txns
        .iter()
        .filter(|t| {
            let mut text = format!(
                "{} {} {} {} {} {}",
                bean_core::home::date(t.date),
                t.flag,
                t.payee.as_deref().unwrap_or(""),
                t.narration.as_deref().unwrap_or(""),
                t.tags.join(" "),
                t.links.join(" ")
            );
            for p in &t.postings {
                text.push(' ');
                text.push_str(&p.account);
                for (amount, currency) in &p.amounts {
                    text.push_str(&format!(" {amount} {currency}"));
                }
            }
            for (k, v) in &t.meta {
                text.push_str(&format!(" {k} {v}"));
            }
            let text = text.to_lowercase();
            terms.iter().all(|term| text.contains(term.as_str()))
        })
        .map(|t| {
            let bank = t
                .postings
                .iter()
                .find(|p| is_balance(&p.account))
                .or_else(|| t.postings.first());
            let category = t
                .postings
                .iter()
                .find(|p| !is_balance(&p.account))
                .or_else(|| {
                    t.postings
                        .iter()
                        .find(|p| Some(&p.account) != bank.map(|b| &b.account))
                })
                .map(|p| p.account.clone())
                .unwrap_or_default();
            Hit {
                date: t.date,
                flag: t.flag,
                payee: t.payee.clone().unwrap_or_default(),
                narration: t.narration.clone().unwrap_or_default(),
                category,
                bank: bank.map(|p| p.account.clone()).unwrap_or_default(),
                amount: bank
                    .map(|p| p.amounts.iter().map(|a| a.0).sum())
                    .unwrap_or_default(),
                postings: t
                    .postings
                    .iter()
                    .flat_map(|p| {
                        p.amounts
                            .iter()
                            .map(|(v, c)| (p.account.clone(), *v, c.clone()))
                    })
                    .collect(),
                file: t.source.as_ref().and_then(|s| {
                    s.path.file_name().map(|f| f.to_string_lossy().into_owned())
                }),
                line: t.source.as_ref().map(|s| s.line as usize),
                scheduled: t.date > today,
            }
        })
        .collect();
    hits.sort_by_key(|h| std::cmp::Reverse(h.date));
    hits
}
