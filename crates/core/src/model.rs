//! Turns a [`LoadedLedger`](crate::loader::LoadedLedger) into the indexed,
//! immutable model every query reads from: labeled accounts, per-month
//! posting sums, transaction listings, and price-based currency conversion.

use std::collections::{BTreeMap, HashMap};
use std::fmt;
use std::path::PathBuf;

use beancount_parser::metadata::Value;
use beancount_parser::{DirectiveContent, PostingPrice};
use rust_decimal::Decimal;

use crate::loader::{Document, LoadedLedger};

/// A concrete calendar date as `(year, month, day)`.
pub type Day = (u16, u8, u8);

/// Whole days from `from` to `to`, negative when `to` came first.
///
/// Days since a fixed epoch by way of Howard Hinnant's civil-date
/// algorithm: shift the year to start in March so the leap day lands at
/// the end and never has to be special-cased.
pub fn days_between(from: Day, to: Day) -> i64 {
    fn serial((y, m, d): Day) -> i64 {
        let (y, m, d) = (i64::from(y), i64::from(m), i64::from(d));
        let y = if m <= 2 { y - 1 } else { y };
        let era = y.div_euclid(400);
        let year_of_era = y - era * 400;
        let day_of_year =
            (153 * (m + if m > 2 { -3 } else { 9 }) + 2) / 5 + d - 1;
        let day_of_era = year_of_era * 365 + year_of_era / 4
            - year_of_era / 100
            + day_of_year;
        era * 146_097 + day_of_era
    }
    serial(to) - serial(from)
}

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
    pub purpose: AccountPurpose,
    pub account: String,
    /// Pretty name: `name:` metadata on the open directive, else derived.
    pub label: String,
    /// Second segment for `Expenses:*` accounts, `None` otherwise.
    pub group: Option<String>,
    pub kind: AccountKind,
    /// What `income:` on the open directive said: `Some(true)` for
    /// `passive`, `Some(false)` for `active`, `None` when the ledger
    /// is silent and the name is all there is to go on.
    pub passive: Option<bool>,
    /// The currencies the open directive constrained the account to,
    /// sorted; empty when it named none or the account was never opened.
    pub currencies: Vec<String>,
    /// The day of the `close` directive, when there is one.
    pub closed: Option<Day>,
    /// What the open directive said about the account as a debt.
    pub debt: DebtMeta,
}

/// Financial meaning is independent of sidebar visibility.
#[derive(Debug, Clone, serde::Serialize)]
pub struct AccountPurpose {
    pub scope: String,
    pub liquidity: String,
    pub declared: bool,
    /// Reserved amount in the ledger's first operating currency.
    pub reserve: Decimal,
    pub goal: Option<Decimal>,
    pub goal_date: Option<String>,
    /// External source coverage, explicitly declared by the ledger owner.
    pub updated: Option<String>,
}
impl AccountPurpose {
    pub fn liquid(&self) -> bool {
        self.liquidity == "cash"
    }
}

/// What an `open` directive says about a debt that its postings cannot:
/// the rate before any interest is charged, the day a payment is due
/// before two have landed, the limit on a card, and what a loan is
/// secured on. Every field is optional and every field is a fact the
/// ledger's author wrote down, so a page can trust it over a guess.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DebtMeta {
    /// `rate:` as a yearly fraction: `5.25` or `"5.25%"` both read as
    /// 0.0525.
    pub rate: Option<Decimal>,
    /// `due:` the day of the month a payment is due, 1 through 31.
    pub due: Option<u8>,
    /// `limit:` in the account's own currency.
    pub limit: Option<Decimal>,
    /// `collateral:` the asset account the debt is secured on.
    pub collateral: Option<String>,
}

/// The metadata one `open` directive carried, kept until the accounts
/// are labelled.
#[derive(Default)]
struct OpenMeta {
    name: Option<String>,
    ynab: Option<String>,
    income: Option<String>,
    currencies: Vec<String>,
    debt: DebtMeta,
    purpose: Option<AccountPurpose>,
}

/// What a posting's `{...}` lot annotation named, resolved to a total.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PostingCost {
    /// Cost of the whole posting, signed the way its units are. A
    /// per-unit `{100.00 USD}` on 20 units and a total `{{2000.00 USD}}`
    /// are the same lot written two ways, and both land here as 2000.00.
    pub total: Decimal,
    pub currency: String,
}

#[derive(Debug, Clone)]
pub struct TxnPosting {
    pub account: String,
    /// Resolved amounts; an elided posting carries the balancing residual,
    /// which can span several currencies.
    pub amounts: Vec<(Decimal, String)>,
    /// The lot this posting was acquired or released at, when it named
    /// one. `{}` names no cost — it defers to whatever lot is open —
    /// and reads the same here as no annotation at all.
    pub cost: Option<PostingCost>,
}

/// What a `commodity` directive declared about a ticker.
#[derive(Debug, Clone)]
pub struct Commodity {
    pub currency: String,
    /// `name:` metadata: "Example Index Fund" rather than MOCK.
    pub name: Option<String>,
    /// `asset-class:` metadata. Taken as the ledger writes it, never
    /// inferred from the ticker.
    pub asset_class: Option<String>,
    /// `quote-currency:` metadata.
    pub quote_currency: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Txn {
    pub source: Option<crate::loader::SourceLocation>,
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
#[derive(Debug, Clone)]
pub struct Ledger {
    pub scope: Option<String>,
    pub title: Option<String>,
    pub operating_currencies: Vec<String>,
    pub files: Vec<PathBuf>,
    pub warnings: Vec<String>,
    pub audit: crate::validation::Audit,
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
    /// Every commodity a `commodity` directive declared, by ticker.
    commodities: BTreeMap<String, Commodity>,
    /// Every `document` directive, in the order the files declared them.
    documents: Vec<Document>,
    /// account → day → indexes into `documents`
    document_index: HashMap<String, BTreeMap<Day, Vec<usize>>>,
}

impl Ledger {
    pub fn build(loaded: LoadedLedger) -> Self {
        let mut ledger = Builder::default().build(&loaded);
        ledger.audit = crate::validation::validate(&loaded, &ledger);
        ledger
            .warnings
            .extend(ledger.audit.issues.iter().map(|i| i.message.clone()));
        ledger
    }

    /// A reporting scope keeps original audit evidence and source references.
    pub fn scoped(&self, scope: &str) -> std::borrow::Cow<'_, Self> {
        if scope == "all" {
            return std::borrow::Cow::Borrowed(self);
        }
        let mut l = self.clone();
        l.scope = Some(scope.to_string());
        l.accounts.retain(|_, a| a.purpose.scope == scope);
        l.monthly.retain(|a, _| l.accounts.contains_key(a));
        l.txn_index.retain(|a, _| l.accounts.contains_key(a));
        // Keep transaction indices stable for existing account drilldowns.
        for t in &mut l.txns {
            t.postings.retain(|p| l.accounts.contains_key(&p.account));
        }
        std::borrow::Cow::Owned(l)
    }

    /// A dated reading: future postings and future quotes never become cash today.
    /// The usual case borrows the existing indexes without copying the ledger.
    pub fn as_of(&self, day: Day) -> std::borrow::Cow<'_, Self> {
        if !self.txns.iter().any(|t| t.date > day)
            && !self
                .prices
                .values()
                .any(|p| p.last().is_some_and(|(d, _)| *d > day))
        {
            return std::borrow::Cow::Borrowed(self);
        }
        let mut l = self.clone();
        l.txns.retain(|t| t.date <= day);
        for points in l.prices.values_mut() {
            points.retain(|(d, _)| *d <= day);
        }
        l.monthly.clear();
        l.txn_index.clear();
        l.first_txn_month = None;
        l.last_txn_month = None;
        for (i, t) in l.txns.iter().enumerate() {
            let month = MonthKey::new(t.date.0, t.date.1);
            l.first_txn_month =
                Some(l.first_txn_month.map_or(month, |m| m.min(month)));
            l.last_txn_month =
                Some(l.last_txn_month.map_or(month, |m| m.max(month)));
            for p in &t.postings {
                let sums = l
                    .monthly
                    .entry(p.account.clone())
                    .or_default()
                    .entry(month)
                    .or_default();
                for (v, c) in &p.amounts {
                    add_sum(sums, c, *v);
                }
                let ids = l
                    .txn_index
                    .entry(p.account.clone())
                    .or_default()
                    .entry(month)
                    .or_default();
                if ids.last() != Some(&i) {
                    ids.push(i);
                }
            }
        }
        for months in l.txn_index.values_mut() {
            for ids in months.values_mut() {
                ids.sort_by_key(|i| l.txns[*i].date);
            }
        }
        std::borrow::Cow::Owned(l)
    }

    /// One document by the id [`Ledger::documents_on`] hands out.
    pub fn document(&self, id: usize) -> Option<&Document> {
        self.documents.get(id)
    }

    /// How many documents the ledger declared, for diagnostics.
    pub fn document_count(&self) -> usize {
        self.documents.len()
    }

    /// Documents attached to `account` on `date`, in declaration order.
    pub fn documents_on(&self, account: &str, date: Day) -> &[usize] {
        self.document_index
            .get(account)
            .and_then(|days| days.get(&date))
            .map_or([].as_slice(), Vec::as_slice)
    }

    /// The paperwork one transaction produced.
    ///
    /// Beancount attaches a document to an account and a day, never to a
    /// transaction, so this is as close to a link as the data comes: every
    /// document on an account the transaction posts to, dated the same day.
    /// Matching only the account being inspected would show half of what a
    /// single purchase generated — the invoice hangs off the expense, the
    /// statement off the card that paid it — so this walks every posting,
    /// in the order they were written.
    pub fn documents_of(&self, txn: &Txn) -> Vec<usize> {
        let mut ids: Vec<usize> = Vec::new();
        for posting in &txn.postings {
            for &id in self.documents_on(&posting.account, txn.date) {
                if !ids.contains(&id) {
                    ids.push(id);
                }
            }
        }
        ids
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

    /// The date of the price [`Self::convert`] would use, following the
    /// same chain. A pivot is only as fresh as its stalest half, so it
    /// reports the older of the two legs. `None` when `from == to`, or
    /// when no chain exists at all — nothing priced is a different
    /// problem from priced long ago.
    pub fn priced_at(&self, from: &str, to: &str, at: Day) -> Option<Day> {
        if from == to {
            return None;
        }
        if let Some(day) = self.leg_at(from, to, at) {
            return Some(day);
        }
        for pivot in &self.operating_currencies {
            if pivot == from || pivot == to {
                continue;
            }
            if let (Some(a), Some(b)) =
                (self.leg_at(from, pivot, at), self.leg_at(pivot, to, at))
            {
                return Some(a.min(b));
            }
        }
        None
    }

    /// What the ledger declared about one commodity, if it declared it.
    /// A commodity nobody wrote a directive for is still perfectly
    /// tradeable — it just has no name and no class to go on.
    pub fn commodity(&self, currency: &str) -> Option<&Commodity> {
        self.commodities.get(currency)
    }

    /// Every declared commodity, by ticker.
    pub fn commodities(&self) -> impl Iterator<Item = &Commodity> {
        self.commodities.values()
    }

    /// Direct (multiply) or inverse (divide, so exact) rate for one leg.
    fn leg(&self, from: &str, to: &str, at: Day) -> Option<Leg> {
        if let Some(rate) = self.rate_at(from, to, at) {
            return Some(Leg::Mul(rate));
        }
        self.rate_at(to, from, at).map(Leg::Div)
    }

    /// The date behind [`Self::leg`], picked the same way round.
    fn leg_at(&self, from: &str, to: &str, at: Day) -> Option<Day> {
        self.point_at(from, to, at)
            .or_else(|| self.point_at(to, from, at))
            .map(|(day, _)| day)
    }

    fn rate_at(&self, from: &str, to: &str, at: Day) -> Option<Decimal> {
        self.point_at(from, to, at).map(|(_, rate)| rate)
    }

    /// The newest price point on or before `at` for one direction.
    fn point_at(
        &self,
        from: &str,
        to: &str,
        at: Day,
    ) -> Option<(Day, Decimal)> {
        let series = self.prices.get(&(from.to_string(), to.to_string()))?;
        let idx = series.partition_point(|(date, _)| *date <= at);
        (idx > 0).then(|| series[idx - 1])
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
    /// account → what its open directive said.
    opens: HashMap<String, OpenMeta>,
    /// account → the day it was closed.
    closes: HashMap<String, Day>,
    txns: Vec<Txn>,
    monthly: HashMap<String, BTreeMap<MonthKey, CurrencySums>>,
    txn_index: HashMap<String, BTreeMap<MonthKey, Vec<usize>>>,
    prices: HashMap<(String, String), PriceSeries>,
    commodities: BTreeMap<String, Commodity>,
    first_month: Option<MonthKey>,
    last_month: Option<MonthKey>,
}

impl Builder {
    fn build(mut self, loaded: &LoadedLedger) -> Ledger {
        let title = loaded.title().map(str::to_string);
        let operating: Vec<String> = loaded
            .operating_currencies()
            .into_iter()
            .map(str::to_string)
            .collect();

        for (index, directive) in loaded.directives.iter().enumerate() {
            let date = (
                directive.date.year,
                directive.date.month,
                directive.date.day,
            );
            match &directive.content {
                DirectiveContent::Transaction(txn) => {
                    self.add_txn(date, txn, &directive.metadata);
                    self.txns.last_mut().unwrap().source =
                        loaded.origins.get(index).cloned();
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
                DirectiveContent::Commodity(currency) => {
                    self.commodities.insert(
                        currency.to_string(),
                        Commodity {
                            currency: currency.to_string(),
                            name: meta_string(&directive.metadata, "name"),
                            asset_class: meta_string(
                                &directive.metadata,
                                "asset-class",
                            ),
                            quote_currency: meta_string(
                                &directive.metadata,
                                "quote-currency",
                            ),
                        },
                    );
                }
                DirectiveContent::Open(open) => {
                    let mut currencies: Vec<String> =
                        open.currencies.iter().map(|c| c.to_string()).collect();
                    currencies.sort();
                    let meta = &directive.metadata;
                    self.opens.insert(
                        open.account.to_string(),
                        OpenMeta {
                            name: meta_string(meta, "name"),
                            ynab: meta_string(meta, "ynab"),
                            income: meta_string(meta, "income"),
                            currencies,
                            purpose: Some(AccountPurpose {
                                scope: meta_string(meta, "scope")
                                    .unwrap_or_else(|| {
                                        inferred_scope(open.account.as_str())
                                    }),
                                liquidity: meta_string(meta, "liquidity")
                                    .unwrap_or_else(|| {
                                        inferred_liquidity(
                                            open.account.as_str(),
                                        )
                                    }),
                                declared: meta_string(meta, "liquidity")
                                    .is_some(),
                                reserve: meta_number(meta, "reserve")
                                    .unwrap_or_default()
                                    .max(Decimal::ZERO),
                                goal: meta_number(meta, "goal"),
                                goal_date: meta_string(meta, "goal-date"),
                                updated: meta_string(meta, "updated"),
                            }),
                            debt: DebtMeta {
                                rate: meta_number(meta, "rate")
                                    .map(|r| r / Decimal::from(100)),
                                due: meta_number(meta, "due")
                                    .and_then(|d| u8::try_from(d).ok())
                                    .filter(|d| (1..=31).contains(d)),
                                limit: meta_number(meta, "limit"),
                                collateral: meta_string(meta, "collateral"),
                            },
                        },
                    );
                }
                DirectiveContent::Close(close) => {
                    self.closes.insert(close.account.to_string(), date);
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

        let mut document_index: HashMap<String, BTreeMap<Day, Vec<usize>>> =
            HashMap::new();
        for (id, doc) in loaded.documents.iter().enumerate() {
            document_index
                .entry(doc.account.clone())
                .or_default()
                .entry(doc.date)
                .or_default()
                .push(id);
        }

        Ledger {
            scope: None,
            title,
            operating_currencies: operating,
            files: loaded.files.clone(),
            warnings: loaded.warnings.clone(),
            audit: Default::default(),
            directives: loaded.directives.len(),
            first_txn_month: self.first_month,
            last_txn_month: self.last_month,
            txns: self.txns,
            accounts,
            monthly: self.monthly,
            txn_index: self.txn_index,
            prices: self.prices,
            commodities: self.commodities,
            documents: loaded.documents.clone(),
            document_index,
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
                        cost: lot_cost(amount, posting).map(
                            |(total, currency)| PostingCost { total, currency },
                        ),
                    });
                }
                None => {
                    elided.get_or_insert(postings.len());
                    postings.push(TxnPosting {
                        account,
                        amounts: Vec::new(),
                        cost: None,
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
            source: None,
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
            let open = self.opens.get(name);
            let meta_name = open.and_then(|o| o.name.as_deref());
            let meta_ynab = open.and_then(|o| o.ynab.as_deref());
            // `income: passive` on the open directive is the ledger
            // saying so outright; anything else is a guess made
            // elsewhere from the account's name.
            let passive = match open.and_then(|o| o.income.as_deref()) {
                Some("passive") => Some(true),
                Some("active") => Some(false),
                _ => None,
            };
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
                    purpose: open
                        .and_then(|o| o.purpose.clone())
                        .unwrap_or_else(|| AccountPurpose {
                            scope: inferred_scope(name),
                            liquidity: inferred_liquidity(name),
                            declared: false,
                            reserve: Decimal::ZERO,
                            goal: None,
                            goal_date: None,
                            updated: None,
                        }),
                    account: name.clone(),
                    label,
                    group,
                    kind,
                    passive,
                    currencies: open
                        .map(|o| o.currencies.clone())
                        .unwrap_or_default(),
                    closed: self.closes.get(name).copied(),
                    debt: open.map(|o| o.debt.clone()).unwrap_or_default(),
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
/// The total cost a posting's `{...}` names, if it names one. A
/// per-unit cost is multiplied out; a `{{total}}` takes the units' sign,
/// so a lot leaves at the magnitude it arrived at.
fn lot_cost(
    amount: &beancount_parser::Amount<Decimal>,
    posting: &beancount_parser::Posting<Decimal>,
) -> Option<(Decimal, String)> {
    let basis = posting.cost.as_ref()?.amount.as_ref()?;
    let value = if posting.cost.as_ref()?.total {
        if amount.value.is_sign_negative() {
            -basis.value
        } else {
            basis.value
        }
    } else {
        amount.value * basis.value
    };
    Some((value, basis.currency.to_string()))
}

pub(crate) fn weight(
    amount: &beancount_parser::Amount<Decimal>,
    posting: &beancount_parser::Posting<Decimal>,
) -> (Decimal, String) {
    // When both a cost and a price are present, the cost wins (beancount
    // semantics); an empty cost (`{}`) falls through to the price.
    if let Some(cost) = lot_cost(amount, posting) {
        return cost;
    }
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

/// A numeric metadata value, whether it was written as a number or as a
/// string: `5.25`, `"5.25"` and `"5.25%"` all read as 5.25.
fn meta_number(
    metadata: &beancount_parser::metadata::Map<Decimal>,
    wanted: &str,
) -> Option<Decimal> {
    metadata.iter().find_map(|(key, value)| {
        if key.to_string() != wanted {
            return None;
        }
        match value {
            Value::Number(n) => Some(*n),
            Value::String(s) => {
                s.trim().trim_end_matches('%').trim().parse().ok()
            }
            _ => None,
        }
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

fn inferred_scope(name: &str) -> String {
    if name.split(':').any(|s| s.eq_ignore_ascii_case("business")) {
        "business"
    } else {
        "personal"
    }
    .into()
}
fn inferred_liquidity(name: &str) -> String {
    let parts: Vec<String> = name.split(':').map(str::to_lowercase).collect();
    if parts.iter().any(|s| {
        ["home", "house", "property", "realestate", "car", "vehicle"]
            .contains(&s.as_str())
    }) {
        "illiquid"
    } else if parts.iter().any(|s| {
        ["receivable", "receivables", "reimbursements"].contains(&s.as_str())
    }) {
        "receivable"
    } else if parts.iter().any(|s| {
        ["retirement", "pension", "restricted", "401k", "ira"]
            .contains(&s.as_str())
    }) {
        "restricted"
    } else {
        "cash"
    }
    .into()
}
