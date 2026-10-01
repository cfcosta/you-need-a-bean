//! The example ledger the design canvas was drawn from, and the day it
//! was drawn on.

#![allow(dead_code)]

use std::path::PathBuf;

use bean_core::{
    loader::load,
    model::{Day, Ledger},
};
use rust_decimal::Decimal;

pub const TODAY: Day = (2026, 10, 1);

pub fn overview_ledger() -> Ledger {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/overview.beancount");
    Ledger::build(load(&path).expect("the example ledger loads"))
}

pub fn d(s: &str) -> Decimal {
    s.parse().unwrap()
}

/// The made-up ledger with a real one's rough edges; see
/// `tests/fixtures/build-rough.py`.
pub fn rough_ledger() -> Ledger {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/rough.beancount");
    Ledger::build(load(&path).expect("the rough ledger loads"))
}
