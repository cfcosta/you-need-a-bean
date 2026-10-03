use bean_core::date::{
    Day, MonthKey, add_days, add_months, add_months_clamped, days_between,
    epoch_days, from_epoch_days,
};
use hegel::{TestCase, generators as gs};
use jiff::{ToSpan, civil::Date};

#[hegel::composite]
fn valid_day(tc: &TestCase) -> Day {
    // Leave room on both sides for the generated offsets below.
    let year: u16 = tc.draw(gs::integers().min_value(101).max_value(9_899));
    let month: u8 = tc.draw(gs::integers().min_value(1).max_value(12));
    let month_start = Date::new(year as i16, month as i8, 1).unwrap();
    let day: u8 = tc.draw(
        gs::integers()
            .min_value(1)
            .max_value(month_start.days_in_month() as u8),
    );
    (year, month, day)
}

fn reference_date((year, month, day): Day) -> Date {
    Date::new(year as i16, month as i8, day as i8).unwrap()
}

fn day(date: Date) -> Day {
    (date.year() as u16, date.month() as u8, date.day() as u8)
}

#[hegel::test]
fn day_offsets_match_jiff(tc: TestCase) {
    let start: Day = tc.draw(valid_day());
    let offset: i64 =
        tc.draw(gs::integers().min_value(-36_500_i64).max_value(36_500_i64));
    let expected = reference_date(start).checked_add(offset.days()).unwrap();

    assert_eq!(add_days(start, offset), day(expected));
}

#[hegel::test]
fn month_offsets_match_jiff(tc: TestCase) {
    let start: Day = tc.draw(valid_day());
    let months: i64 =
        tc.draw(gs::integers().min_value(0_i64).max_value(1_200_i64));
    let expected = reference_date(start).checked_add(months.months()).unwrap();

    assert_eq!(add_months_clamped(start, months as u32), day(expected));
    assert_eq!(
        add_months(MonthKey::new(start.0, start.1), months as u32),
        MonthKey::new(expected.year() as u16, expected.month() as u8),
    );
}

#[hegel::test]
fn day_distances_match_jiff(tc: TestCase) {
    let from: Day = tc.draw(valid_day());
    let to: Day = tc.draw(valid_day());
    let expected = (reference_date(to) - reference_date(from)).get_days();

    assert_eq!(days_between(from, to), i64::from(expected));
}

#[hegel::test]
fn epoch_days_round_trip(tc: TestCase) {
    let date: Day = tc.draw(valid_day());

    assert_eq!(from_epoch_days(epoch_days(date)), date);
}

#[test]
fn unix_epoch_is_zero() {
    assert_eq!(epoch_days((1970, 1, 1)), 0);
    assert_eq!(from_epoch_days(0), (1970, 1, 1));
}
