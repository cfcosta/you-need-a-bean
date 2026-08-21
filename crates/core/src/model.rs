//! Turns a [`LoadedLedger`](crate::loader::LoadedLedger) into the indexed,
//! immutable model every query reads from: labeled accounts, per-month
//! posting sums, transaction listings, and price-based currency conversion.

use std::collections::{BTreeMap, HashMap};
use std::fmt;
use std::path::PathBuf;

use beancount_parser::metadata::Value;
use beancount_parser::{DirectiveContent, PostingPrice};
use rust_decimal::Decimal;

use crate::loader::LoadedLedger;

/// A concrete calendar date as `(year, month, day)`.
pub type Day = (u16, u8, u8);

/// Price points for one `(from, to)` currency pair, sorted by date.
type PriceSeries = Vec<(Day, Decimal)>;

/// A calendar month, the unit every budget view is keyed on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MonthKey {
    pub year: u16,
    pub month: u8,
}

impl MonthKey {
    pub fn new(year: u16, month: u8) -> Self {
        debug_assert!((1..=12).contains(&month));
        Self { year, month }
    }

    /// Strict `YYYY-MM`.
    pub fn parse(s: &str) -> Option<Self> {
        let (y, m) = s.split_once('-')?;
        if y.len() != 4 || m.len() != 2 {
            return None;
        }
        let year: u16 = y.parse().ok()?;
        let month: u8 = m.parse().ok()?;
        (1..=12).contains(&month).then(|| Self::new(year, month))
    }

    pub fn next(self) -> Self {
        if self.month == 12 {
            Self::new(self.year + 1, 1)
        } else {
            Self::new(self.year, self.month + 1)
        }
    }

    pub fn days_in_month(self) -> u8 {
        match self.month {
            1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
            4 | 6 | 9 | 11 => 30,
            _ => {
                let y = self.year;
                let leap = y.is_multiple_of(4)
                    && (!y.is_multiple_of(100) || y.is_multiple_of(400));
                if leap { 29 } else { 28 }
            }
        }
    }

    /// The last day of the month — the date conversions are valued at.
    pub fn end_of_month(self) -> Day {
        (self.year, self.month, self.days_in_month())
    }

    /// Zero-based month count since year 0, for month arithmetic.
    fn ordinal(self) -> u32 {
        u32::from(self.year) * 12 + u32::from(self.month) - 1
    }

    fn from_ordinal(ordinal: u32) -> Self {
        Self::new((ordinal / 12) as u16, (ordinal % 12 + 1) as u8)
    }

    pub fn minus(self, months: u32) -> Self {
        Self::from_ordinal(self.ordinal().saturating_sub(months))
    }

    pub fn prev(self) -> Self {
        self.minus(1)
    }

    /// Number of months in the inclusive range `self..=to` (0 if empty).
    pub fn months_until(self, to: MonthKey) -> u32 {
        (to.ordinal() + 1).saturating_sub(self.ordinal())
    }
}

impl fmt::Display for MonthKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:04}-{:02}", self.year, self.month)
    }
}

/// How an asset/liability account shows up in the sidebar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccountKind {
    Budget,
    Tracking,
    Hidden,
}

#[derive(Debug, Clone)]
pub struct AccountInfo {
    pub account: String,
    /// Pretty name: `name:` metadata on the open directive, else derived.
    pub label: String,
    /// Second segment for `Expenses:*` accounts, `None` otherwise.
    pub group: Option<String>,
    pub kind: AccountKind,
}

#[derive(Debug, Clone)]
pub struct TxnPosting {
    pub account: String,
    /// Resolved amounts; an elided posting carries the balancing residual,
    /// which can span several currencies.
    pub amounts: Vec<(Decimal, String)>,
}

#[derive(Debug, Clone)]
pub struct Txn {
    pub date: Day,
    pub flag: char,
    pub payee: Option<String>,
    pub narration: Option<String>,
    pub tags: Vec<String>,
    pub links: Vec<String>,
    pub meta: Vec<(String, String)>,
    pub postings: Vec<TxnPosting>,
}

type CurrencySums = Vec<(String, Decimal)>;

/// The fully indexed ledger. Built once at startup, then read-only.
#[derive(Debug)]
pub struct Ledger {
    pub title: Option<String>,
    pub operating_currencies: Vec<String>,
    pub files: Vec<PathBuf>,
    pub warnings: Vec<String>,
    /// How many directives the source files contained, for diagnostics.
    pub directives: usize,
    pub first_txn_month: Option<MonthKey>,
    pub last_txn_month: Option<MonthKey>,
    pub txns: Vec<Txn>,
    accounts: BTreeMap<String, AccountInfo>,
    /// account → month → (currency, posting sum)
    monthly: HashMap<String, BTreeMap<MonthKey, CurrencySums>>,
    /// account → month → indexes into `txns`, sorted by date
    txn_index: HashMap<String, BTreeMap<MonthKey, Vec<usize>>>,
    /// (from, to) → price points sorted by date; "1 from = rate to"
    prices: HashMap<(String, String), PriceSeries>,
}

impl Ledger {
    pub fn build(loaded: LoadedLedger) -> Self {
        Builder::default().build(loaded)
    }

    pub fn account(&self, name: &str) -> Option<&AccountInfo> {
        self.accounts.get(name)
    }

    /// All accounts, sorted by raw account name.
    pub fn accounts(&self) -> impl Iterator<Item = &AccountInfo> {
        self.accounts.values()
    }

    /// Posting sum for one account/month/currency (zero when absent).
    pub fn sum(&self, account: &str, month: MonthKey, cur: &str) -> Decimal {
        self.sums(account, month)
            .iter()
            .find(|(c, _)| c == cur)
            .map(|(_, v)| *v)
            .unwrap_or_default()
    }

    /// All per-currency posting sums for one account/month.
    pub fn sums(&self, account: &str, month: MonthKey) -> &[(String, Decimal)] {
        self.monthly
            .get(account)
            .and_then(|months| months.get(&month))
            .map(Vec::as_slice)
            .unwrap_or_default()
    }

    /// Months (and their sums) for one account, ascending, for cumulative
    /// balances.
    pub fn months_of(
        &self,
        account: &str,
    ) -> impl Iterator<Item = (MonthKey, &[(String, Decimal)])> {
        self.monthly
            .get(account)
            .into_iter()
            .flat_map(|months| months.iter().map(|(m, v)| (*m, v.as_slice())))
    }

    /// Transactions touching `account` in `month`, sorted by date.
    pub fn txns(&self, account: &str, month: MonthKey) -> Vec<&Txn> {
        self.txn_index
            .get(account)
            .and_then(|months| months.get(&month))
            .map(|ids| ids.iter().map(|&i| &self.txns[i]).collect())
            .unwrap_or_default()
    }

    /// Convert `value` from one currency to another using the latest price
    /// on or before `at`: direct, else inverse, else a one-hop pivot
    /// through an operating currency. `None` when no rate chain exists.
    pub fn convert(
        &self,
        value: Decimal,
        from: &str,
        to: &str,
        at: Day,
    ) -> Option<Decimal> {
        if from == to {
            return Some(value);
        }
        if let Some(leg) = self.leg(from, to, at) {
            return Some(leg.apply(value));
        }
        for pivot in &self.operating_currencies {
            if pivot == from || pivot == to {
                continue;
            }
            if let (Some(a), Some(b)) =
                (self.leg(from, pivot, at), self.leg(pivot, to, at))
            {
                return Some(b.apply(a.apply(value)));
            }
        }
        None
    }

    /// Direct (multiply) or inverse (divide, so exact) rate for one leg.
    fn leg(&self, from: &str, to: &str, at: Day) -> Option<Leg> {
        if let Some(rate) = self.rate_at(from, to, at) {
            return Some(Leg::Mul(rate));
        }
        self.rate_at(to, from, at).map(Leg::Div)
    }

    fn rate_at(&self, from: &str, to: &str, at: Day) -> Option<Decimal> {
        let series = self.prices.get(&(from.to_string(), to.to_string()))?;
        let idx = series.partition_point(|(date, _)| *date <= at);
        (idx > 0).then(|| series[idx - 1].1)
    }
}

enum Leg {
    Mul(Decimal),
    Div(Decimal),
}

impl Leg {
    fn apply(&self, value: Decimal) -> Decimal {
        match self {
            Leg::Mul(rate) => value * rate,
            Leg::Div(rate) => value / rate,
        }
    }
}

#[derive(Default)]
struct Builder {
    /// account → (`name:` metadata, `ynab:` metadata) from open directives.
    opens: HashMap<String, (Option<String>, Option<String>)>,
    txns: Vec<Txn>,
    monthly: HashMap<String, BTreeMap<MonthKey, CurrencySums>>,
    txn_index: HashMap<String, BTreeMap<MonthKey, Vec<usize>>>,
    prices: HashMap<(String, String), PriceSeries>,
    first_month: Option<MonthKey>,
    last_month: Option<MonthKey>,
}

impl Builder {
    fn build(mut self, loaded: LoadedLedger) -> Ledger {
        let title = loaded.title().map(str::to_string);
        let operating: Vec<String> = loaded
            .operating_currencies()
            .into_iter()
            .map(str::to_string)
            .collect();

        for directive in &loaded.directives {
            let date = (
                directive.date.year,
                directive.date.month,
                directive.date.day,
            );
            match &directive.content {
                DirectiveContent::Transaction(txn) => {
                    self.add_txn(date, txn, &directive.metadata)
                }
                DirectiveContent::Price(price) => {
                    let key = (
                        price.currency.to_string(),
                        price.amount.currency.to_string(),
                    );
                    if !price.amount.value.is_zero() {
                        self.prices
                            .entry(key)
                            .or_default()
                            .push((date, price.amount.value));
                    }
                }
                DirectiveContent::Open(open) => {
                    let name = meta_string(&directive.metadata, "name");
                    let ynab = meta_string(&directive.metadata, "ynab");
                    self.opens.insert(open.account.to_string(), (name, ynab));
                }
                _ => {}
            }
        }

        for series in self.prices.values_mut() {
            series.sort_by_key(|(date, _)| *date);
        }
        for months in self.txn_index.values_mut() {
            for ids in months.values_mut() {
                ids.sort_by_key(|&i| self.txns[i].date);
            }
        }

        let accounts = self.label_accounts(&operating);

        Ledger {
            title,
            operating_currencies: operating,
            files: loaded.files,
            warnings: loaded.warnings,
            directives: loaded.directives.len(),
            first_txn_month: self.first_month,
            last_txn_month: self.last_month,
            txns: self.txns,
            accounts,
            monthly: self.monthly,
            txn_index: self.txn_index,
            prices: self.prices,
        }
    }

    fn add_txn(
        &mut self,
        date: Day,
        txn: &beancount_parser::Transaction<Decimal>,
        metadata: &beancount_parser::metadata::Map<Decimal>,
    ) {
        let month = MonthKey::new(date.0, date.1);
        self.first_month =
            Some(self.first_month.map_or(month, |m| m.min(month)));
        self.last_month = Some(self.last_month.map_or(month, |m| m.max(month)));

        // Resolve posting amounts: explicit ones as written; the balancing
        // residual (weights summed per currency) goes to the first elided
        // posting, as beancount interpolation would.
        let mut postings = Vec::with_capacity(txn.postings.len());
        let mut residual: Vec<(String, Decimal)> = Vec::new();
        let mut elided: Option<usize> = None;
        for posting in &txn.postings {
            let account = posting.account.to_string();
            match &posting.amount {
                Some(amount) => {
                    let (wv, wc) = weight(amount, posting);
                    add_sum(&mut residual, &wc, wv);
                    postings.push(TxnPosting {
                        account,
                        amounts: vec![(
                            amount.value,
                            amount.currency.to_string(),
                        )],
                    });
                }
                None => {
                    elided.get_or_insert(postings.len());
                    postings.push(TxnPosting {
                        account,
                        amounts: Vec::new(),
                    });
                }
            }
        }
        if let Some(idx) = elided {
            postings[idx].amounts = residual
                .into_iter()
                .filter(|(_, v)| !v.is_zero())
                .map(|(c, v)| (-v, c))
                .collect();
        }

        let txn_id = self.txns.len();
        for posting in &postings {
            let sums = self
                .monthly
                .entry(posting.account.clone())
                .or_default()
                .entry(month)
                .or_default();
            for (value, currency) in &posting.amounts {
                add_sum(sums, currency, *value);
            }
            let ids = self
                .txn_index
                .entry(posting.account.clone())
                .or_default()
                .entry(month)
                .or_default();
            if ids.last() != Some(&txn_id) {
                ids.push(txn_id);
            }
        }

        let mut tags: Vec<String> =
            txn.tags.iter().map(|t| t.as_str().to_string()).collect();
        tags.sort();
        let mut links: Vec<String> =
            txn.links.iter().map(|l| l.as_str().to_string()).collect();
        links.sort();
        let mut meta: Vec<(String, String)> = metadata
            .iter()
            .filter_map(|(key, value)| {
                Some((key.to_string(), value_string(value)?))
            })
            .collect();
        meta.sort();

        self.txns.push(Txn {
            date,
            flag: txn.flag.unwrap_or('*'),
            payee: txn.payee.clone(),
            narration: txn.narration.clone(),
            tags,
            links,
            meta,
            postings,
        });
    }

    /// Build the account registry: everything opened or posted to, with
    /// pretty labels and budget/tracking/hidden classification.
    fn label_accounts(
        &self,
        operating: &[String],
    ) -> BTreeMap<String, AccountInfo> {
        let mut names: Vec<&String> =
            self.opens.keys().chain(self.monthly.keys()).collect();
        names.sort();
        names.dedup();

        let mut accounts = BTreeMap::new();
        for name in names {
            let (meta_name, meta_ynab) = self
                .opens
                .get(name)
                .map(|(n, y)| (n.as_deref(), y.as_deref()))
                .unwrap_or((None, None));
            let segments: Vec<&str> = name.split(':').collect();
            let is_expense = segments[0] == "Expenses";

            let group = (is_expense && segments.len() >= 2)
                .then(|| segments[1].to_string());
            let label = meta_name
                .map(str::to_string)
                .unwrap_or_else(|| fallback_label(&segments, is_expense));
            let kind = match meta_ynab {
                Some("budget") => AccountKind::Budget,
                Some("tracking") => AccountKind::Tracking,
                Some("hidden") => AccountKind::Hidden,
                _ => {
                    if self.holds_non_operating(name, operating) {
                        AccountKind::Tracking
                    } else {
                        AccountKind::Budget
                    }
                }
            };
            accounts.insert(
                name.clone(),
                AccountInfo {
                    account: name.clone(),
                    label,
                    group,
                    kind,
                },
            );
        }
        accounts
    }

    fn holds_non_operating(&self, account: &str, operating: &[String]) -> bool {
        self.monthly.get(account).is_some_and(|months| {
            months.values().any(|sums| {
                sums.iter().any(|(cur, _)| !operating.contains(cur))
            })
        })
    }
}

/// The weight a posting contributes to transaction balancing: the cost or
/// price converts it into the settlement currency.
fn weight(
    amount: &beancount_parser::Amount<Decimal>,
    posting: &beancount_parser::Posting<Decimal>,
) -> (Decimal, String) {
    if let Some(price) = &posting.price {
        return match price {
            PostingPrice::Unit(p) => {
                (amount.value * p.value, p.currency.to_string())
            }
            PostingPrice::Total(p) => {
                let value = if amount.value.is_sign_negative() {
                    -p.value
                } else {
                    p.value
                };
                (value, p.currency.to_string())
            }
        };
    }
    if let Some(cost) = &posting.cost
        && let Some(per_unit) = &cost.amount
    {
        return (amount.value * per_unit.value, per_unit.currency.to_string());
    }
    (amount.value, amount.currency.to_string())
}

pub(crate) fn add_sum(
    sums: &mut Vec<(String, Decimal)>,
    currency: &str,
    value: Decimal,
) {
    match sums.iter_mut().find(|(c, _)| c == currency) {
        Some((_, total)) => *total += value,
        None => sums.push((currency.to_string(), value)),
    }
}

fn fallback_label(segments: &[&str], is_expense: bool) -> String {
    if is_expense {
        if segments.len() > 2 {
            segments[2..].join(" · ")
        } else {
            segments.last().unwrap_or(&"").to_string()
        }
    } else if segments.len() > 2 {
        segments[segments.len() - 2..].join(" ")
    } else {
        segments.last().unwrap_or(&"").to_string()
    }
}

fn meta_string(
    metadata: &beancount_parser::metadata::Map<Decimal>,
    wanted: &str,
) -> Option<String> {
    metadata.iter().find_map(|(key, value)| {
        (key.to_string() == wanted)
            .then(|| value.as_string().map(str::to_string))
            .flatten()
    })
}

fn value_string(value: &Value<Decimal>) -> Option<String> {
    match value {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        Value::Currency(c) => Some(c.to_string()),
        _ => None,
    }
}
