//! What each page shows, read out of the core crate into plain Rust:
//! no gpui here, so every figure can be tested without a window.

pub mod account;
pub mod budget;
pub mod overview;
pub mod reports;

use bean_core::model::{Day, MonthKey};
use rust_decimal::Decimal;
use serde_json::Value;

/// A JSON number as a Decimal, through its shortest decimal spelling so
/// `89054.58` stays `89054.58`.
pub(crate) fn dec(v: &Value) -> Decimal {
    match v {
        Value::Number(n) => n.to_string().parse().unwrap_or_default(),
        _ => Decimal::ZERO,
    }
}

pub(crate) fn text(v: &Value) -> String {
    v.as_str().unwrap_or_default().to_owned()
}

pub(crate) fn day(v: &Value) -> Option<Day> {
    v.as_str().and_then(bean_core::home::parse_date)
}

/// Days since 1970-01-01 (Howard Hinnant's `days_from_civil`).
pub fn ordinal(d: Day) -> i64 {
    let (y, m, dd) = (i64::from(d.0), i64::from(d.1), i64::from(d.2));
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + dd - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// The day `days` after `d`.
pub fn add_days(d: Day, days: i64) -> Day {
    let z = ordinal(d) + days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let dd = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    (y as u16, m as u8, dd as u8)
}

/// The month `months` after `m`.
pub fn add_months(m: MonthKey, months: u32) -> MonthKey {
    (0..months).fold(m, |m, _| m.next())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn days_round_trip_across_months_and_years() {
        assert_eq!(add_days((2026, 10, 1), 30), (2026, 10, 31));
        assert_eq!(add_days((2026, 10, 1), 31), (2026, 11, 1));
        assert_eq!(add_days((2026, 12, 31), 1), (2027, 1, 1));
        assert_eq!(add_days((2028, 2, 28), 1), (2028, 2, 29));
        assert_eq!(ordinal((1970, 1, 1)), 0);
    }

    #[test]
    fn months_add_across_years() {
        assert_eq!(
            add_months(MonthKey::new(2026, 10), 36),
            MonthKey::new(2029, 10)
        );
    }
}
