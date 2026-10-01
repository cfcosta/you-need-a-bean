//! Page 4: what the portfolio is worth, what it cost, how it moved.

use bean_core::{
    model::{Day, Ledger, MonthKey},
    reports::{AssetClass, InvestmentPerformance, Position},
};
use rust_decimal::Decimal;

/// How far back the performance chart reaches.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Range {
    Months(u32),
    Ytd,
    All,
}

impl Range {
    pub const CHOICES: [(Range, &'static str); 6] = [
        (Range::Months(1), "1M"),
        (Range::Months(3), "3M"),
        (Range::Months(6), "6M"),
        (Range::Ytd, "YTD"),
        (Range::Months(12), "1Y"),
        (Range::All, "ALL"),
    ];

    /// The day the range opens on: performance is measured from that
    /// day's closing valuation.
    pub fn start(self, today: Day, first: Option<MonthKey>) -> Day {
        match self {
            Range::Ytd => (today.0 - 1, 12, 31),
            Range::All => first.map_or(today, |m| m.prev().end_of_month()),
            Range::Months(n) => {
                let total =
                    i32::from(today.0) * 12 + i32::from(today.1) - 1 - n as i32;
                let (y, m) = ((total / 12) as u16, (total % 12 + 1) as u8);
                let last = MonthKey::new(y, m).days_in_month();
                (y, m, today.2.min(last))
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sort {
    Value,
    Gain,
    Name,
}

#[derive(Clone, Debug)]
pub struct Investments {
    pub value: Decimal,
    pub basis: Decimal,
    pub gain: Decimal,
    pub ret: Option<Decimal>,
    /// Share of the value whose cost the ledger knows.
    pub coverage: Option<Decimal>,
    pub positions: Vec<Position>,
    pub classes: Vec<AssetClass>,
    pub range: Range,
    pub performance: InvestmentPerformance,
}

impl Investments {
    pub fn build(ledger: &Ledger, today: Day, cur: &str, range: Range) -> Self {
        let v = ledger.reports_view(today, 6, cur).investments;
        let start = range.start(today, ledger.first_txn_month).min(today);
        Self {
            value: v.total,
            basis: v.basis,
            gain: v.gain,
            ret: v.ret,
            coverage: v.coverage,
            positions: v.items,
            classes: v.classes,
            range,
            performance: ledger.investment_performance(start, today, cur),
        }
    }

    /// The positions matching `filter` (currency, label or account,
    /// case-insensitive), in the chosen order.
    pub fn holdings(&self, filter: &str, sort: Sort) -> Vec<&Position> {
        let needle = filter.trim().to_lowercase();
        let mut out: Vec<&Position> = self
            .positions
            .iter()
            .filter(|p| {
                needle.is_empty()
                    || p.currency.to_lowercase().contains(&needle)
                    || p.label.to_lowercase().contains(&needle)
                    || p.locations.iter().any(|l| {
                        l.account.to_lowercase().contains(&needle)
                            || l.label.to_lowercase().contains(&needle)
                    })
            })
            .collect();
        match sort {
            Sort::Value => out.sort_by_key(|p| std::cmp::Reverse(p.value)),
            Sort::Gain => out.sort_by(|a, b| {
                b.gain.unwrap_or_default().cmp(&a.gain.unwrap_or_default())
            }),
            Sort::Name => out.sort_by(|a, b| a.currency.cmp(&b.currency)),
        }
        out
    }
}

/// Names each month once, at its first point; the last point carries
/// its day when it is not a month end (`10-01`).
pub fn month_labels(days: &[Day]) -> Vec<String> {
    const MONTHS: [&str; 12] = [
        "jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct",
        "nov", "dec",
    ];
    let last = days.len().saturating_sub(1);
    let mut seen = None;
    days.iter()
        .enumerate()
        .map(|(k, d)| {
            let month = (d.0, d.1);
            if k == last
                && k > 0
                && d.2 != MonthKey::new(d.0, d.1).days_in_month()
            {
                return format!("{:02}-{:02}", d.1, d.2);
            }
            if seen == Some(month) {
                return String::new();
            }
            seen = Some(month);
            MONTHS[usize::from(d.1) - 1].to_owned()
        })
        .collect()
}
