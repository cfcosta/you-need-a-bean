//! Accounting checks are distinct from parsing and external reconciliation.
//! Unsupported transformations make the ledger incomplete, even when readable.
use crate::{
    loader::{LoadedLedger, SourceLocation},
    model::{Day, Ledger, weight},
};
use beancount_parser::DirectiveContent;
use rust_decimal::Decimal;
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize)]
pub struct Issue {
    pub code: String,
    pub message: String,
    pub account: Option<String>,
    pub source: Option<SourceLocation>,
}
#[derive(Debug, Clone, Serialize)]
pub struct Reconciliation {
    pub account: String,
    pub date: Day,
    pub currency: String,
    pub passed: bool,
    pub source: Option<SourceLocation>,
}
#[derive(Debug, Clone, Default, Serialize)]
pub struct Audit {
    pub issues: Vec<Issue>,
    pub balances: Vec<Reconciliation>,
}

pub(crate) fn validate(loaded: &LoadedLedger, ledger: &Ledger) -> Audit {
    let mut audit = Audit::default();
    for (name, source) in &loaded.plugins {
        audit.issues.push(Issue {
            code: "unsupported".into(),
            message: format!("Unsupported plugin {name}; its transformations have not been applied"),
            account: None, source: Some(source.clone()),
        });
    }
    for (i, d) in loaded.directives.iter().enumerate() {
        let source = loaded.origins.get(i).cloned();
        let date = (d.date.year, d.date.month, d.date.day);
        let mut issue =
            |code: &str, message: String, account: Option<String>| {
                audit.issues.push(Issue {
                    code: code.into(),
                    message,
                    account,
                    source: source.clone(),
                });
            };
        match &d.content {
            DirectiveContent::Open(open) => {
                for (key, value) in &d.metadata {
                    let key = key.to_string();
                    let text = value.as_string();
                    let number = value.as_number().copied().or_else(|| {
                        text.and_then(|s| s.parse::<Decimal>().ok())
                    });
                    let valid = match key.as_str() {
                        "scope" => text.is_some_and(|v| {
                            ["personal", "business"].contains(&v)
                        }),
                        "liquidity" => text.is_some_and(|v| {
                            [
                                "cash",
                                "investment",
                                "illiquid",
                                "receivable",
                                "restricted",
                            ]
                            .contains(&v)
                        }),
                        "reserve" => number.is_some_and(|n| n >= Decimal::ZERO),
                        "goal" => number.is_some_and(|n| n > Decimal::ZERO),
                        "updated" | "goal-date" => {
                            text.and_then(crate::home::parse_date).is_some()
                        }
                        _ => true,
                    };
                    if !valid {
                        issue(
                            "metadata",
                            format!(
                                "Invalid {key} metadata on {}; review the account declaration",
                                open.account
                            ),
                            Some(open.account.to_string()),
                        );
                    }
                }
            }

            DirectiveContent::Pad(p) => issue(
                "unsupported",
                format!(
                    "Unsupported pad for {}; expand padding into transactions before relying on these balances",
                    p.account
                ),
                Some(p.account.to_string()),
            ),
            DirectiveContent::Balance(b) => {
                let name = b.account.to_string();
                let currency = b.amount.currency.to_string();
                // Assertions describe the start of the day and include children.
                let actual: Decimal = ledger
                    .txns
                    .iter()
                    .filter(|t| t.date < date)
                    .flat_map(|t| &t.postings)
                    .filter(|p| {
                        p.account == name
                            || p.account.starts_with(&format!("{name}:"))
                    })
                    .flat_map(|p| &p.amounts)
                    .filter(|(_, c)| c == &currency)
                    .map(|(v, _)| *v)
                    .sum();
                let tolerance =
                    b.tolerance.unwrap_or_else(|| precision(b.amount.value));
                let difference = b.amount.value - actual;
                let passed = difference.abs() <= tolerance;
                if !passed {
                    issue(
                        "balance",
                        format!(
                            "Balance mismatch on {name}: expected {} {currency}, calculated {actual}; difference {difference}",
                            b.amount.value
                        ),
                        Some(name.clone()),
                    );
                }
                audit.balances.push(Reconciliation {
                    account: name,
                    date,
                    currency,
                    passed,
                    source,
                });
            }
            DirectiveContent::Transaction(t) => {
                let omitted =
                    t.postings.iter().filter(|p| p.amount.is_none()).count();
                let mut residual = BTreeMap::<String, Decimal>::new();
                let mut tolerances = BTreeMap::<String, Decimal>::new();
                let mut unresolved = false;
                for p in &t.postings {
                    if p.cost.as_ref().is_some_and(|c| c.amount.is_none())
                        && p.price.is_none()
                    {
                        unresolved = true;
                    }
                    if let Some(a) = &p.amount {
                        let (v, c) = weight(a, p);
                        *residual.entry(c.clone()).or_default() += v;
                        let tol = tolerances.entry(c).or_default();
                        *tol = (*tol).max(precision(a.value));
                    }
                }
                if omitted > 1 {
                    issue("ambiguous", "Transaction has multiple omitted amounts; interpolation is ambiguous".into(), None);
                }
                if unresolved {
                    issue("unsupported", "Unresolved lot booking; expand empty cost annotations before relying on these totals".into(), None);
                }
                if omitted == 0 && !unresolved {
                    for (c, v) in residual {
                        if v.abs()
                            > tolerances.get(&c).copied().unwrap_or_default()
                        {
                            issue(
                                "unbalanced",
                                format!("Transaction is unbalanced by {v} {c}"),
                                None,
                            );
                        }
                    }
                }
            }
            _ => {}
        }
    }
    audit.balances.sort_by_key(|b| b.date);
    audit
}

fn precision(value: Decimal) -> Decimal {
    // Half a unit at MAX_SCALE is smaller than any representable residual.
    // Require exact agreement there instead of constructing scale 29 or
    // rounding the tolerance up and accepting a real one-unit mismatch.
    if value.scale() == 0 || value.scale() == Decimal::MAX_SCALE {
        Decimal::ZERO
    } else {
        Decimal::new(5, value.scale() + 1)
    }
}
