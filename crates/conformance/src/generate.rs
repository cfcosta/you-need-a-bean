//! Seeded generation of ledgers, for differential testing and benchmarks.
//!
//! Generation goes through [`crate::ast`], so every generated file comes with
//! the dump it must parse to — no oracle needed beyond the model itself.
//! Everything is driven by [`crate::rng::Rng`], so a seed reproduces a file
//! exactly, on any machine, forever. That matters twice over: a failing
//! property prints a seed that reproduces, and a benchmark measures the same
//! bytes run to run.
//!
//! [`minimize`] shrinks a failing ledger by repeatedly trying smaller
//! candidates, which keeps counterexamples readable without a property-testing
//! framework (and without the Python runtime one would drag in).

use rust_decimal::Decimal;

use crate::ast::{
    Amount, Content, Cost, Date, Directive, Entry, Expr, Flag, Ledger,
    MetaValue, Num, Posting, Price, Txn,
};
use crate::rng::Rng;

/// What kind of ledger to build. Benchmarks use these to separate "the parser
/// got slower at transactions" from "the parser got slower at arithmetic".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shape {
    /// Mostly two-posting transactions with the occasional price, balance and
    /// metadata line. What a real personal ledger looks like.
    Realistic,
    /// Nothing but bare two-posting transactions: the parser's hot path with
    /// every optional feature switched off.
    Plain,
    /// Every optional feature at once — flags, payees, tags, links, costs,
    /// prices, metadata on both directives and postings.
    Rich,
    /// No transactions at all: open/close/balance/pad/price/commodity/event.
    Directives,
    /// Every amount is a nested arithmetic expression.
    Arithmetic,
    /// Short transactions carrying long metadata blocks.
    Metadata,
}

impl Shape {
    pub const ALL: [Shape; 6] = [
        Shape::Realistic,
        Shape::Plain,
        Shape::Rich,
        Shape::Directives,
        Shape::Arithmetic,
        Shape::Metadata,
    ];

    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Shape::Realistic => "realistic",
            Shape::Plain => "plain",
            Shape::Rich => "rich",
            Shape::Directives => "directives",
            Shape::Arithmetic => "arithmetic",
            Shape::Metadata => "metadata",
        }
    }
}

/// How much of what to generate.
#[derive(Debug, Clone, Copy)]
pub struct Config {
    pub shape: Shape,
    /// Number of directives. Blank lines, comments and pushtags are extra.
    pub directives: usize,
}

impl Config {
    #[must_use]
    pub fn new(shape: Shape, directives: usize) -> Self {
        Self { shape, directives }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self::new(Shape::Realistic, 64)
    }
}

const ACCOUNTS: &[&str] = &[
    "Assets:Cash",
    "Assets:Bank:Checking",
    "Assets:Bank:Savings",
    "Assets:Investments:Broker",
    "Liabilities:CreditCard",
    "Liabilities:Loan:Car",
    "Equity:Opening-Balances",
    "Income:Salary",
    "Income:Dividends",
    "Expenses:Groceries",
    "Expenses:Rent",
    "Expenses:Transport:Fuel",
    "Expenses:Fun:Books",
];

const CURRENCIES: &[&str] = &[
    "USD", "EUR", "CHF", "BRL", "GBP", "VTSAX", "BTC", "X-Y", "A.B", "C'D",
];

const PAYEES: &[&str] = &[
    "Whole Foods",
    "Landlord",
    "Employer AG",
    "Café \"Central\"",
    "Bäckerei",
    "北京超市",
];

const NARRATIONS: &[&str] = &[
    "groceries",
    "monthly rent",
    "salary",
    "transfer",
    "reimbursement — travel",
    "",
];

const TAGS: &[&str] =
    &["food", "trip", "trip-2026", "work", "recurring", "tax_2026"];

const LINKS: &[&str] = &["inv-1", "inv-2", "payroll", "receipt-88"];

const META_KEYS: &[&str] =
    &["note", "rate", "source", "statement", "lot", "category"];

const BOOKINGS: &[&str] = &["STRICT", "FIFO", "LIFO", "NONE", "AVERAGE"];

/// Build a ledger from a seed.
#[must_use]
pub fn ledger(seed: u64, config: Config) -> Ledger {
    let mut rng = Rng::new(seed);
    let mut entries = Vec::with_capacity(config.directives * 3);

    entries.push(Entry::Option {
        name: "title".to_string(),
        value: format!("generated ledger {seed}"),
    });
    entries.push(Entry::Option {
        name: "operating_currency".to_string(),
        value: "USD".to_string(),
    });
    entries.push(Entry::Blank);

    let mut pushed: Vec<String> = Vec::new();
    for i in 0..config.directives {
        // Punctuate with the noise a real file carries, so the benchmark is
        // not measuring an unrealistically dense file.
        if config.shape != Shape::Plain && rng.chance(1, 8) {
            entries.push(Entry::Blank);
        }
        if config.shape != Shape::Plain && rng.chance(1, 12) {
            entries.push(Entry::Comment(format!("section {i}")));
        }
        if config.shape == Shape::Rich && rng.chance(1, 16) {
            let tag = (*rng.pick(TAGS)).to_string();
            if !pushed.contains(&tag) {
                entries.push(Entry::PushTag(tag.clone()));
                pushed.push(tag);
            }
        }
        if !pushed.is_empty() && rng.chance(1, 8) {
            let tag = pushed.remove(rng.below(pushed.len()));
            entries.push(Entry::PopTag(tag));
        }
        entries.push(Entry::Directive(directive(&mut rng, config, i)));
    }
    for tag in pushed {
        entries.push(Entry::PopTag(tag));
    }

    Ledger { entries }
}

fn directive(rng: &mut Rng, config: Config, index: usize) -> Directive {
    let date = date(index);
    let content = match config.shape {
        Shape::Plain | Shape::Arithmetic | Shape::Metadata => {
            Content::Transaction(txn(rng, config))
        }
        Shape::Directives => non_transaction(rng),
        Shape::Realistic => {
            if rng.chance(6, 8) {
                Content::Transaction(txn(rng, config))
            } else {
                non_transaction(rng)
            }
        }
        Shape::Rich => {
            if rng.chance(5, 8) {
                Content::Transaction(txn(rng, config))
            } else {
                non_transaction(rng)
            }
        }
    };
    let meta = match config.shape {
        Shape::Plain | Shape::Arithmetic => Vec::new(),
        Shape::Metadata => {
            let count = 2 + rng.below(5);
            meta(rng, count)
        }
        _ if rng.chance(1, 4) => meta(rng, 1),
        _ => Vec::new(),
    };
    Directive {
        date,
        content,
        meta,
    }
}

fn non_transaction(rng: &mut Rng) -> Content {
    let account = (*rng.pick(ACCOUNTS)).to_string();
    match rng.below(7) {
        0 => Content::Open {
            account,
            currencies: match rng.below(3) {
                0 => Vec::new(),
                1 => vec![(*rng.pick(CURRENCIES)).to_string()],
                _ => vec![
                    (*rng.pick(CURRENCIES)).to_string(),
                    (*rng.pick(CURRENCIES)).to_string(),
                ],
            },
            booking: rng
                .chance(1, 3)
                .then(|| (*rng.pick(BOOKINGS)).to_string()),
        },
        1 => Content::Close { account },
        2 => Content::Balance {
            account,
            amount: amount(rng, Shape::Realistic),
            tolerance: rng.chance(1, 3).then(|| {
                Expr::lit(Num::plain(Decimal::new(rng.between(1, 500), 2)))
            }),
        },
        3 => Content::Pad {
            account,
            source: "Equity:Opening-Balances".to_string(),
        },
        4 => Content::Price {
            currency: (*rng.pick(CURRENCIES)).to_string(),
            amount: amount(rng, Shape::Realistic),
        },
        5 => Content::Commodity {
            currency: (*rng.pick(CURRENCIES)).to_string(),
        },
        _ => Content::Event {
            name: "location".to_string(),
            value: (*rng.pick(&["Zürich", "São Paulo", "Lisbon"])).to_string(),
        },
    }
}

fn txn(rng: &mut Rng, config: Config) -> Txn {
    let rich = config.shape == Shape::Rich;
    let narration = match config.shape {
        Shape::Plain => Some("payment".to_string()),
        _ => Some((*rng.pick(NARRATIONS)).to_string()),
    };
    let payee = (rich
        || (config.shape == Shape::Realistic && rng.chance(1, 2)))
    .then(|| (*rng.pick(PAYEES)).to_string());

    let count = match config.shape {
        Shape::Plain | Shape::Metadata => 2,
        Shape::Rich => 2 + rng.below(3),
        _ => {
            if rng.chance(1, 6) {
                3
            } else {
                2
            }
        }
    };
    // The last posting carries no amount, the way a real ledger leans on the
    // implicit balancing posting.
    let postings = (0..count)
        .map(|i| posting(rng, config, i + 1 == count))
        .collect();

    Txn {
        flag: match config.shape {
            Shape::Plain => Flag::Char('*'),
            _ => match rng.below(4) {
                0 => Flag::Keyword,
                1 => Flag::Char('!'),
                _ => Flag::Char('*'),
            },
        },
        payee,
        narration,
        tags: if config.shape == Shape::Plain {
            Vec::new()
        } else if rich || rng.chance(1, 4) {
            let mut tags = vec![(*rng.pick(TAGS)).to_string()];
            if rich && rng.chance(1, 3) {
                let extra = (*rng.pick(TAGS)).to_string();
                if extra != tags[0] {
                    tags.push(extra);
                }
            }
            tags
        } else {
            Vec::new()
        },
        links: if rich && rng.chance(1, 2) {
            vec![(*rng.pick(LINKS)).to_string()]
        } else {
            Vec::new()
        },
        postings,
    }
}

fn posting(rng: &mut Rng, config: Config, last: bool) -> Posting {
    let rich = config.shape == Shape::Rich;
    let bare = last && config.shape == Shape::Realistic && rng.chance(1, 3);
    let amount = (!bare).then(|| amount(rng, config.shape));
    Posting {
        flag: (rich && rng.chance(1, 6)).then(|| *rng.pick(&['!', '*'])),
        account: (*rng.pick(ACCOUNTS)).to_string(),
        amount: amount.clone(),
        cost: (rich && amount.is_some() && rng.chance(1, 3)).then(|| Cost {
            total: rng.chance(1, 3),
            amount: rng.chance(3, 4).then(|| Amount {
                value: Expr::lit(Num::plain(Decimal::new(
                    rng.between(1, 90000),
                    2,
                ))),
                currency: (*rng.pick(CURRENCIES)).to_string(),
            }),
            date: rng.chance(1, 2).then(|| {
                let offset = rng.below(400);
                date(offset)
            }),
            date_first: false,
        }),
        price: (rich && amount.is_some() && rng.chance(1, 4)).then(|| {
            let value = Amount {
                value: Expr::lit(Num::plain(Decimal::new(
                    rng.between(1, 5000),
                    3,
                ))),
                currency: (*rng.pick(CURRENCIES)).to_string(),
            };
            if rng.chance(1, 2) {
                Price::Unit(value)
            } else {
                Price::Total(value)
            }
        }),
        meta: match config.shape {
            Shape::Metadata => {
                let count = 1 + rng.below(3);
                meta(rng, count)
            }
            Shape::Rich if rng.chance(1, 4) => meta(rng, 1),
            _ => Vec::new(),
        },
    }
}

fn meta(rng: &mut Rng, count: usize) -> Vec<(String, MetaValue)> {
    let mut used: Vec<String> = Vec::new();
    let mut out = Vec::new();
    for _ in 0..count {
        // Keys are unique: the parser keeps a map, so a repeat would silently
        // drop the earlier value and the model would be describing a file
        // whose meaning depends on ordering. Duplicates are pinned in the
        // corpus instead, where the intent is explicit.
        let key = (*rng.pick(META_KEYS)).to_string();
        if used.contains(&key) {
            continue;
        }
        used.push(key.clone());
        let value = match rng.below(3) {
            0 => MetaValue::Str(format!("note {}", rng.below(1000))),
            1 => MetaValue::Num(Expr::lit(Num::plain(Decimal::new(
                rng.between(-10000, 10000),
                2,
            )))),
            _ => MetaValue::Currency((*rng.pick(CURRENCIES)).to_string()),
        };
        out.push((key, value));
    }
    out
}

fn amount(rng: &mut Rng, shape: Shape) -> Amount {
    Amount {
        value: match shape {
            Shape::Arithmetic => expr(rng, 2),
            Shape::Rich if rng.chance(1, 4) => expr(rng, 1),
            _ => Expr::lit(number(rng)),
        },
        currency: (*rng.pick(CURRENCIES)).to_string(),
    }
}

fn number(rng: &mut Rng) -> Num {
    let value = Decimal::new(rng.between(-2_000_000, 2_000_000), 2);
    match rng.below(8) {
        0 => Num::grouped(value),
        1 if value >= Decimal::ZERO => Num::signed_plus(value),
        _ => Num::plain(value),
    }
}

/// An expression tree of at most `depth` nested levels.
fn expr(rng: &mut Rng, depth: usize) -> Expr {
    if depth == 0 {
        return Expr::lit(number(rng));
    }
    match rng.below(5) {
        0 => Expr::Group(Box::new(expr(rng, depth - 1))),
        1 => Expr::Signed(
            *rng.pick(&['-', '+']),
            Box::new(Expr::Group(Box::new(expr(rng, depth - 1)))),
        ),
        // A chain of one precedence class; the other class only appears
        // nested inside a group, which keeps the intended fold unambiguous.
        _ => {
            let ops: &[char] = if rng.chance(1, 2) {
                &['+', '-']
            } else {
                &['*', '/']
            };
            let terms = 1 + rng.below(3);
            let head = Box::new(Expr::lit(number(rng)));
            let tail = (0..terms)
                .map(|_| {
                    let op = *rng.pick(ops);
                    let rhs = if rng.chance(1, 3) {
                        Expr::Group(Box::new(expr(rng, depth - 1)))
                    } else {
                        Expr::lit(number(rng))
                    };
                    (op, rhs)
                })
                .collect();
            Expr::Chain { head, tail }
        }
    }
}

fn date(index: usize) -> Date {
    // Dates climb with the index so generated files look like real ones, and
    // stay inside the calendar so nothing depends on the parser's (absent)
    // calendar validation.
    Date::new(
        2020 + u16::try_from(index / 336 % 6).unwrap_or(0),
        u8::try_from(index / 28 % 12 + 1).unwrap_or(1),
        u8::try_from(index % 28 + 1).unwrap_or(1),
    )
}

/// Candidate ledgers that are strictly smaller than `ledger`.
///
/// Ordered cheapest-first: dropping half the file, then single entries, then
/// simplifying individual directives. Each candidate is still a valid ledger,
/// so the model keeps producing a matching dump.
#[must_use]
pub fn shrink(ledger: &Ledger) -> Vec<Ledger> {
    let mut out = Vec::new();
    let len = ledger.entries.len();

    if len > 1 {
        let half = len / 2;
        out.push(Ledger {
            entries: ledger.entries[..half].to_vec(),
        });
        out.push(Ledger {
            entries: ledger.entries[half..].to_vec(),
        });
    }
    for i in 0..len {
        let mut entries = ledger.entries.clone();
        entries.remove(i);
        out.push(Ledger { entries });
    }
    for i in 0..len {
        for simpler in simplify(&ledger.entries[i]) {
            let mut entries = ledger.entries.clone();
            entries[i] = simpler;
            out.push(Ledger { entries });
        }
    }
    out
}

fn simplify(entry: &Entry) -> Vec<Entry> {
    let Entry::Directive(directive) = entry else {
        return Vec::new();
    };
    let mut out = Vec::new();
    if !directive.meta.is_empty() {
        let mut smaller = directive.clone();
        smaller.meta.clear();
        out.push(Entry::Directive(smaller));
    }
    if let Content::Transaction(txn) = &directive.content {
        for i in 0..txn.postings.len() {
            let mut smaller = directive.clone();
            if let Content::Transaction(t) = &mut smaller.content {
                t.postings.remove(i);
            }
            out.push(Entry::Directive(smaller));
        }
        if !txn.tags.is_empty() || !txn.links.is_empty() {
            let mut smaller = directive.clone();
            if let Content::Transaction(t) = &mut smaller.content {
                t.tags.clear();
                t.links.clear();
            }
            out.push(Entry::Directive(smaller));
        }
        if txn.payee.is_some() {
            let mut smaller = directive.clone();
            if let Content::Transaction(t) = &mut smaller.content {
                t.payee = None;
            }
            out.push(Entry::Directive(smaller));
        }
        for i in 0..txn.postings.len() {
            let posting = &txn.postings[i];
            if posting.cost.is_none()
                && posting.price.is_none()
                && posting.meta.is_empty()
            {
                continue;
            }
            let mut smaller = directive.clone();
            if let Content::Transaction(t) = &mut smaller.content {
                t.postings[i].cost = None;
                t.postings[i].price = None;
                t.postings[i].meta.clear();
            }
            out.push(Entry::Directive(smaller));
        }
    }
    out
}

/// Repeatedly replace `ledger` with the first smaller candidate that still
/// satisfies `fails`, until nothing smaller does.
#[must_use]
pub fn minimize(ledger: Ledger, fails: impl Fn(&Ledger) -> bool) -> Ledger {
    let mut current = ledger;
    // A cap keeps a pathological shrink from hanging the test suite; in
    // practice this converges in a handful of rounds.
    for _ in 0..200 {
        let Some(smaller) = shrink(&current)
            .into_iter()
            .find(|candidate| fails(candidate))
        else {
            break;
        };
        current = smaller;
    }
    current
}
