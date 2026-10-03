//! Civil-date arithmetic shared by the core, SQL, and desktop crates.
//!
//! A [`Day`] is a valid proleptic Gregorian calendar date. Conversions use
//! Howard Hinnant's civil-date algorithms and count days from the Unix epoch.

use std::fmt;

/// A concrete calendar date as `(year, month, day)`.
pub type Day = (u16, u8, u8);

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
        let (year, month) = s.split_once('-')?;
        if year.len() != 4 || month.len() != 2 {
            return None;
        }
        let year: u16 = year.parse().ok()?;
        let month: u8 = month.parse().ok()?;
        (1..=12).contains(&month).then(|| Self::new(year, month))
    }

    pub fn next(self) -> Self {
        add_months(self, 1)
    }

    pub fn days_in_month(self) -> u8 {
        match self.month {
            1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
            4 | 6 | 9 | 11 => 30,
            _ => {
                let year = self.year;
                let leap = year.is_multiple_of(4)
                    && (!year.is_multiple_of(100) || year.is_multiple_of(400));
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
        let year = u16::try_from(ordinal / 12)
            .expect("month offset falls outside MonthKey's supported range");
        Self::new(year, (ordinal % 12 + 1) as u8)
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

const UNIX_EPOCH_OFFSET: i64 = 719_468;

/// The number of whole days from 1970-01-01 to `day`.
pub fn epoch_days((year, month, day): Day) -> i64 {
    let (year, month, day) =
        (i64::from(year), i64::from(month), i64::from(day));
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let year_of_era = year - era * 400;
    let month_from_march = (month + 9) % 12;
    let day_of_year = (153 * month_from_march + 2) / 5 + day - 1;
    let day_of_era =
        year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - UNIX_EPOCH_OFFSET
}

/// Converts a count of whole days from 1970-01-01 into a calendar date.
///
/// # Panics
///
/// Panics when the resulting year cannot be represented by [`Day`].
pub fn from_epoch_days(days: i64) -> Day {
    let zero_based_march_day = days
        .checked_add(UNIX_EPOCH_OFFSET)
        .expect("epoch day is outside the supported range");
    let era = zero_based_march_day.div_euclid(146_097);
    let day_of_era = zero_based_march_day - era * 146_097;
    let year_of_era = (day_of_era - day_of_era / 1460 + day_of_era / 36_524
        - day_of_era / 146_096)
        / 365;
    let day_of_year =
        day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_from_march = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_from_march + 2) / 5 + 1;
    let month = if month_from_march < 10 {
        month_from_march + 3
    } else {
        month_from_march - 9
    };
    let year = year_of_era + era * 400 + i64::from(month <= 2);

    (
        u16::try_from(year)
            .expect("epoch day falls outside Day's supported year range"),
        month as u8,
        day as u8,
    )
}

/// The calendar day `offset` days after `day`.
///
/// # Panics
///
/// Panics when the result cannot be represented by [`Day`].
pub fn add_days(day: Day, offset: i64) -> Day {
    let days = epoch_days(day)
        .checked_add(offset)
        .expect("day offset is outside the supported range");
    from_epoch_days(days)
}

/// The calendar month `offset` months after `month`.
///
/// # Panics
///
/// Panics when the result cannot be represented by [`MonthKey`].
pub fn add_months(month: MonthKey, offset: u32) -> MonthKey {
    let ordinal = month
        .ordinal()
        .checked_add(offset)
        .expect("month offset is outside the supported range");
    MonthKey::from_ordinal(ordinal)
}

/// The calendar date `offset` months after `day`.
///
/// If the destination month is shorter, the result is clamped to its last
/// day. For example, one month after January 31 is February 28 or 29.
///
/// # Panics
///
/// Panics when the result cannot be represented by [`Day`].
pub fn add_months_clamped(day: Day, offset: u32) -> Day {
    let month = add_months(MonthKey::new(day.0, day.1), offset);
    (month.year, month.month, day.2.min(month.days_in_month()))
}

/// Whole days from `from` to `to`, negative when `to` came first.
pub fn days_between(from: Day, to: Day) -> i64 {
    epoch_days(to) - epoch_days(from)
}
