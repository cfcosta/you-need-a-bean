//! How figures are written: grouped thousands, a real minus sign (U+2212),
//! and a plus only where the design asks for one.

use rust_decimal::{Decimal, RoundingStrategy, prelude::ToPrimitive};

const MINUS: char = '\u{2212}';

fn group(digits: &str) -> String {
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, ch) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(ch);
    }
    out
}

/// `value` rounded to `places`, grouped, with a true minus sign.
pub fn fixed(value: Decimal, places: u32) -> String {
    let rounded = value
        .round_dp_with_strategy(places, RoundingStrategy::MidpointAwayFromZero);
    let negative = rounded.is_sign_negative() && !rounded.is_zero();
    let text = format!("{:.*}", places as usize, rounded.abs());
    let (int, frac) = match text.split_once('.') {
        Some((i, f)) => (i, Some(f)),
        None => (text.as_str(), None),
    };
    let mut out = String::new();
    if negative {
        out.push(MINUS);
    }
    out.push_str(&group(int));
    if let Some(frac) = frac {
        out.push('.');
        out.push_str(frac);
    }
    out
}

/// Cents, grouped: `89,054.58`, `−9,679.69`.
pub fn money(value: Decimal) -> String {
    fixed(value, 2)
}

/// Cents with a plus on gains: `+6,200.00`.
pub fn signed(value: Decimal) -> String {
    let text = money(value);
    if value.round_dp(2) > Decimal::ZERO {
        format!("+{text}")
    } else {
        text
    }
}

/// Whole units, grouped: `76,255`.
pub fn whole(value: Decimal) -> String {
    fixed(value, 0)
}

/// A ratio as a percentage: `0.1933` → `19.3%`.
pub fn percent(ratio: Decimal, places: u32) -> String {
    format!("{}%", fixed(ratio * Decimal::ONE_HUNDRED, places))
}

/// A ratio as a plain float for drawing, clamped to 0..=`max`.
pub fn share(ratio: Decimal, max: f32) -> f32 {
    ratio.to_f32().unwrap_or(0.0).clamp(0.0, max)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(s: &str) -> Decimal {
        s.parse().unwrap()
    }

    #[test]
    fn money_groups_thousands_and_keeps_cents() {
        assert_eq!(money(d("89054.58")), "89,054.58");
        assert_eq!(money(d("0")), "0.00");
        assert_eq!(money(d("1234567.5")), "1,234,567.50");
    }

    #[test]
    fn money_uses_a_true_minus_sign() {
        assert_eq!(money(d("-9679.69")), "−9,679.69");
    }

    #[test]
    fn negative_zero_is_never_written() {
        assert_eq!(money(d("-0.001")), "0.00");
    }

    #[test]
    fn signed_money_adds_a_plus_to_gains_only() {
        assert_eq!(signed(d("6200")), "+6,200.00");
        assert_eq!(signed(d("-62")), "−62.00");
        assert_eq!(signed(d("0")), "0.00");
    }

    #[test]
    fn whole_rounds_half_away_from_zero() {
        assert_eq!(whole(d("76254.58")), "76,255");
        assert_eq!(whole(d("2074.5")), "2,075");
        assert_eq!(whole(d("-9679.69")), "−9,680");
    }

    #[test]
    fn percent_rounds_to_the_given_places() {
        assert_eq!(percent(d("0.1933"), 1), "19.3%");
        assert_eq!(percent(d("1.0404"), 0), "104%");
        assert_eq!(percent(d("0.66"), 0), "66%");
    }
}
