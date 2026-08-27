//! What "the beancount parser" is required to do, written down so it can be
//! replaced.
//!
//! The app talks to a parser through a narrow surface: hand it the text of a
//! beancount file, get back a stream of entries. Everything downstream —
//! [`bean_core::loader`], the model, the queries — is a pure function of that
//! stream. So the whole contract can be pinned by fixing one thing: the
//! *canonical dump*, a deterministic, line-oriented rendering of everything a
//! parse observes ([`dump`]).
//!
//! Three layers use it:
//!
//! 1. **Corpus goldens** ([`corpus`]) — `corpus/*.beancount` next to
//!    `corpus/*.expected`. Every accepted construct, every rejected one, and
//!    every known quirk has a case. A replacement parser is correct when the
//!    goldens still match byte for byte.
//! 2. **A reference model** ([`ast`]) — an independent beancount AST that can
//!    render *both* source text and the dump that parsing it must produce. The
//!    parser is the thing in the middle; if it drops or mangles anything, the
//!    two sides disagree.
//! 3. **Randomized differential testing** ([`generate`]) — generation over that
//!    AST, so the model is exercised well past what hand-written cases reach.
//!    The choices come from [`draw`], which hegeltest answers when a property
//!    is searching for a counterexample and a seeded PRNG answers when a
//!    benchmark needs the same bytes twice.
//!
//! `benches/parse.rs` benchmarks the same corpus, so a rewrite can be measured
//! as well as verified.

pub mod ast;
pub mod corpus;
pub mod draw;
pub mod dump;
pub mod generate;

pub use crate::dump::{dump, without_line_numbers};
