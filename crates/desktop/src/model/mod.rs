//! What each page shows, read out of the core crate into plain Rust:
//! no gpui here, so every figure can be tested without a window.

pub mod account;
pub mod budget;
pub mod console;
pub mod investments;
pub mod liabilities;
pub mod overview;
pub mod reports;
pub mod search;

use bean_core::model::Day;
use rust_decimal::Decimal;
use serde_json::Value;

pub use bean_core::date::{add_days, add_months};

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
