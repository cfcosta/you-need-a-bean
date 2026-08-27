//! A tiny deterministic PRNG.
//!
//! Generated ledgers have to be identical on every machine and every run —
//! benchmark numbers are only comparable if the input is, and a failing
//! property test is only useful if its seed reproduces it. This is splitmix64,
//! which is short enough to audit and has no dependencies.

pub struct Rng(u64);

impl Rng {
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

    /// Uniform in `0..n`. Panics if `n` is zero.
    pub fn below(&mut self, n: usize) -> usize {
        assert!(n > 0, "below(0) has no answer");
        usize::try_from(self.next_u64() % n as u64).expect("fits")
    }

    /// Uniform in `lo..=hi`.
    pub fn between(&mut self, lo: i64, hi: i64) -> i64 {
        debug_assert!(lo <= hi);
        let span = (hi - lo + 1) as u64;
        lo + (self.next_u64() % span) as i64
    }

    pub fn pick<'a, T>(&mut self, xs: &'a [T]) -> &'a T {
        &xs[self.below(xs.len())]
    }

    /// True with probability `num / den`.
    pub fn chance(&mut self, num: u32, den: u32) -> bool {
        self.below(den as usize) < num as usize
    }
}
