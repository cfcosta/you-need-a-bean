//! Page 2: a month of spending against what is typical.

use bean_core::{
    model::{Day, Ledger, MonthKey},
    query::{AccountRow, Status},
};
use rust_decimal::Decimal;

/// One line of the category tree.
#[derive(Clone, Debug)]
pub struct Line {
    /// 0 for a group (or a lone category standing for its group), 1 for
    /// a category under a group.
    pub depth: u8,
    pub name: String,
    pub label: String,
    /// The account behind a category line; `None` on a group heading.
    pub account: Option<String>,
    pub typical: Option<Decimal>,
    pub spent: Decimal,
    pub ratio: Option<Decimal>,
    pub status: Option<Status>,
}

#[derive(Clone, Debug)]
pub struct Txn {
    pub date: Day,
    pub flag: char,
    pub payee: String,
    pub amount: Decimal,
}

/// The selected category, in more depth.
#[derive(Clone, Debug)]
pub struct Inspector {
    pub account: String,
    pub label: String,
    pub spent: Decimal,
    pub typical: Option<Decimal>,
    pub ratio: Option<Decimal>,
    pub status: Option<Status>,
    pub history: Vec<(MonthKey, Decimal)>,
    pub txns: Vec<Txn>,
}

#[derive(Clone, Debug)]
pub struct Account {
    pub account: String,
    pub label: String,
    pub value: Decimal,
    /// The holding in its own units, where that says more: `420 BEAN`.
    pub native: String,
}

#[derive(Clone, Debug)]
pub struct Budget {
    pub month: MonthKey,
    /// The month is over: nothing more will land in it.
    pub closed: bool,
    pub basis: u32,
    pub income: Decimal,
    pub spent: Decimal,
    pub typical: Option<Decimal>,
    pub lines: Vec<Line>,
    pub inspector: Option<Inspector>,
    pub accounts: Vec<Account>,
    pub budget_total: Decimal,
    pub tracking: Vec<Account>,
}

impl Budget {
    pub fn kept(&self) -> Decimal {
        self.income - self.spent
    }

    pub fn build(
        ledger: &Ledger,
        today: Day,
        month: MonthKey,
        basis: u32,
        cur: &str,
        selected: Option<&str>,
    ) -> Self {
        let l = ledger.as_of(today);
        let view = l.month_view(month, basis, cur);
        let mut lines = Vec::new();
        for g in &view.groups {
            if let [only] = g.categories.as_slice() {
                lines.push(Line {
                    depth: 0,
                    name: only
                        .account
                        .strip_prefix("Expenses:")
                        .unwrap_or(&only.account)
                        .to_owned(),
                    label: only.label.clone(),
                    account: Some(only.account.clone()),
                    typical: only.avg,
                    spent: only.spent,
                    ratio: only.ratio,
                    status: only.status,
                });
                continue;
            }
            let (ratio, status) = ratio_status(g.spent, g.avg);
            lines.push(Line {
                depth: 0,
                name: g.name.clone(),
                label: String::new(),
                account: None,
                typical: g.avg,
                spent: g.spent,
                ratio,
                status,
            });
            lines.extend(g.categories.iter().map(|c| {
                Line {
                    depth: 1,
                    name: c
                        .account
                        .rsplit(':')
                        .next()
                        .unwrap_or(&c.account)
                        .to_owned(),
                    label: c.label.clone(),
                    account: Some(c.account.clone()),
                    typical: c.avg,
                    spent: c.spent,
                    ratio: c.ratio,
                    status: c.status,
                }
            }));
        }

        let inspector = selected
            .and_then(|account| l.category_view(account, month, basis, cur))
            .map(|c| {
                let txns = c
                    .txns
                    .iter()
                    .map(|t| Txn {
                        date: t.date,
                        flag: t.flag,
                        payee: t
                            .payee
                            .clone()
                            .or_else(|| t.narration.clone())
                            .unwrap_or_default(),
                        amount: t
                            .postings
                            .iter()
                            .filter(|p| p.account == c.account)
                            .flat_map(|p| &p.amounts)
                            .map(|(v, ccy)| {
                                l.convert(*v, ccy, cur, month.end_of_month())
                                    .unwrap_or(*v)
                            })
                            .sum(),
                    })
                    .collect();
                Inspector {
                    account: c.account.clone(),
                    label: c.label.clone(),
                    spent: c.spent,
                    typical: c.avg,
                    ratio: c.ratio,
                    status: c.status,
                    history: c
                        .history
                        .iter()
                        .map(|h| (h.month, h.spent))
                        .collect(),
                    txns,
                }
            });

        let account = |a: &AccountRow| Account {
            account: a.account.clone(),
            label: a.label.clone(),
            value: a.converted.unwrap_or_default(),
            native: a
                .balances
                .iter()
                .map(|(c, v)| {
                    format!(
                        "{} {c}",
                        crate::fmt::fixed(*v, if *c == cur { 2 } else { 0 })
                    )
                })
                .collect::<Vec<_>>()
                .join(" "),
        };
        let accounts: Vec<Account> =
            view.budget_accounts.iter().map(account).collect();
        Self {
            month,
            closed: month < MonthKey::new(today.0, today.1),
            basis,
            income: view.income,
            spent: view.spent,
            typical: view.typical,
            budget_total: accounts.iter().map(|a| a.value).sum(),
            accounts,
            tracking: view.tracking_accounts.iter().map(account).collect(),
            lines,
            inspector,
        }
    }
}

/// The same thresholds the core uses for a category: over past 100%,
/// warning past 85%.
pub fn ratio_status(
    spent: Decimal,
    avg: Option<Decimal>,
) -> (Option<Decimal>, Option<Status>) {
    let Some(avg) = avg.filter(|a| *a > Decimal::ZERO) else {
        return (None, None);
    };
    let ratio = spent / avg;
    let status = if ratio > Decimal::ONE {
        Status::Over
    } else if ratio > Decimal::new(85, 2) {
        Status::Warn
    } else {
        Status::Good
    };
    (Some(ratio), Some(status))
}
