//! Where a generated ledger's choices come from.
//!
//! [`crate::generate`] builds a ledger by asking a series of small questions —
//! how many postings, which currency, is there a cost — and every question
//! goes through [`Draw`]. Two things answer them:
//!
//! - [`TestCase`](hegel::TestCase), so property tests get hegeltest's shrinking
//!   and its on-disk database of past counterexamples. When a property fails,
//!   hegeltest replays the choice sequence with smaller answers until it has
//!   the simplest ledger that still fails.
//! - [`Seeded`], a four-line PRNG, so benchmarks measure the same bytes on
//!   every machine and every run. A shrinking generator cannot do that job:
//!   its answers depend on the framework's search state, not on a seed.
//!
//! One question-asking body, two answerers, so the ledgers a benchmark
//! measures and the ledgers a property explores are the same kind of thing.

/// A source of generation choices.
///
/// Only [`Draw::below`] and [`Draw::between`] are primitive; the rest are
/// written in terms of them, so both implementations agree by construction.
///
/// Every method is arranged so that *smaller answers mean simpler ledgers* —
/// [`Draw::chance`] is false at zero, [`Draw::pick`] takes the first element.
/// Shrinking works by making answers smaller, so this is what points it at
/// readable counterexamples rather than merely different ones.
pub trait Draw {
    /// Uniform in `0..n`. Panics if `n` is zero.
    fn below(&mut self, n: usize) -> usize;

    /// Uniform in `lo..=hi`.
    fn between(&mut self, lo: i64, hi: i64) -> i64;

    /// True with probability `num / den`, and false for the smallest draw.
    fn chance(&mut self, num: u32, den: u32) -> bool {
        self.below(den as usize) >= (den - num) as usize
    }

    /// One of `xs`, defaulting to the first. Panics if `xs` is empty.
    fn pick<'a, T>(&mut self, xs: &'a [T]) -> &'a T {
        &xs[self.below(xs.len())]
    }
}

/// splitmix64: short enough to audit, no dependencies, identical everywhere.
pub struct Seeded(u64);

impl Seeded {
    #[must_use]
    pub fn new(seed: u64) -> Self {
        Self(seed)
    }

    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
}

impl Draw for Seeded {
    fn below(&mut self, n: usize) -> usize {
        assert!(n > 0, "below(0) has no answer");
        usize::try_from(self.next_u64() % n as u64).expect("fits")
    }

    fn between(&mut self, lo: i64, hi: i64) -> i64 {
        debug_assert!(lo <= hi);
        let span = (hi - lo + 1) as u64;
        lo + (self.next_u64() % span) as i64
    }
}

/// Draws are `draw_silent` rather than `draw` on purpose: a ledger asks
/// thousands of these, and hegeltest's per-draw report would bury the one
/// thing worth reading. The property notes the rendered source instead, which
/// is the counterexample in the form you would paste into a file.
impl Draw for &hegel::TestCase {
    fn below(&mut self, n: usize) -> usize {
        assert!(n > 0, "below(0) has no answer");
        self.draw_silent(
            hegel::generators::integers::<usize>()
                .min_value(0)
                .max_value(n - 1),
        )
    }

    fn between(&mut self, lo: i64, hi: i64) -> i64 {
        debug_assert!(lo <= hi);
        self.draw_silent(
            hegel::generators::integers::<i64>()
                .min_value(lo)
                .max_value(hi),
        )
    }
}
