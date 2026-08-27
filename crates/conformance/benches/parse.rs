//! Benchmarks for the beancount parser.
//!
//! Five groups, each answering a different question:
//!
//! - `throughput` — how many bytes per second, at three file sizes. This is
//!   the number to quote and the one to watch for regressions.
//! - `shape` — which *kind* of content is slow. A regression that only shows
//!   up here is a regression in one rule, not in the whole parser.
//! - `api` — what the two entry points cost relative to each other, and what
//!   the conformance dump adds on top.
//! - `real` — the ledgers in `examples/`, so the numbers stay tied to files a
//!   person actually wrote.
//! - `pathological` — inputs designed to be awkward. These exist to catch
//!   accidental quadratic behaviour, not to be fast.
//!
//! Inputs are generated from fixed seeds, so the bytes measured today are the
//! bytes measured next month. To compare a rewrite against the current
//! parser:
//!
//! ```sh
//! cargo bench -p you-need-a-bean-conformance -- --save-baseline before
//! # ... swap the parser ...
//! cargo bench -p you-need-a-bean-conformance -- --baseline before
//! ```

use std::{fmt::Write as _, hint::black_box};

use bean_conformance::generate::{self, Config, Shape};
use beancount_parser::{BeancountFile, parse, parse_iter};
use criterion::{
    BenchmarkId, Criterion, Throughput, criterion_group, criterion_main,
};
use rust_decimal::Decimal;

/// Fixed so a given benchmark always measures the same bytes.
const SEED: u64 = 0x5EED_B3A4;

/// Parse into `Decimal`, which is what the application uses. `f64` would be
/// faster and would measure a parser nobody runs.
fn parse_all(input: &str) -> usize {
    let file: BeancountFile<Decimal> =
        parse(input).expect("benchmark input parses");
    file.directives.len()
}

fn generated(shape: Shape, directives: usize) -> String {
    generate::seeded(SEED, Config::new(shape, directives))
        .render()
        .text
}

/// Bytes per second at three sizes: one month, one year, one lifetime.
fn throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("throughput");
    for directives in [1_000usize, 10_000, 100_000] {
        let input = generated(Shape::Realistic, directives);
        group.throughput(Throughput::Bytes(input.len() as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(directives),
            &input,
            |b, input| b.iter(|| parse_all(black_box(input))),
        );
    }
    group.finish();
}

/// The same byte budget spent on different content, so a regression can be
/// attributed to a rule rather than to "the parser".
fn shape(c: &mut Criterion) {
    let mut group = c.benchmark_group("shape");
    for shape in Shape::ALL {
        let input = generated(shape, 5_000);
        group.throughput(Throughput::Bytes(input.len() as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(shape.name()),
            &input,
            |b, input| b.iter(|| parse_all(black_box(input))),
        );
    }
    group.finish();
}

/// What each entry point costs. `parse_iter` is what `bean_core::loader`
/// uses; `dump` is the conformance harness itself, measured so a slow suite
/// can be told apart from a slow parser.
fn api(c: &mut Criterion) {
    let input = generated(Shape::Realistic, 10_000);
    let mut group = c.benchmark_group("api");
    group.throughput(Throughput::Bytes(input.len() as u64));

    group.bench_function("parse", |b| b.iter(|| parse_all(black_box(&input))));
    group.bench_function("parse_iter/collect", |b| {
        b.iter(|| {
            parse_iter::<Decimal>(black_box(&input))
                .collect::<Result<Vec<_>, _>>()
                .expect("benchmark input parses")
                .len()
        })
    });
    // Streaming without collecting: the cost of the grammar with the
    // allocation of the result vector taken out.
    group.bench_function("parse_iter/count", |b| {
        b.iter(|| parse_iter::<Decimal>(black_box(&input)).count())
    });
    group.bench_function("dump", |b| {
        b.iter(|| bean_conformance::dump(black_box(&input)).len())
    });
    group.finish();
}

/// Real ledgers, so the numbers stay honest about real files.
fn real(c: &mut Criterion) {
    let files: [(&str, &str); 4] = [
        ("basic", include_str!("../corpus/real/basic.beancount")),
        ("starter", include_str!("../corpus/real/starter.beancount")),
        ("vesting", include_str!("../corpus/real/vesting.beancount")),
        (
            "example-head",
            include_str!("../corpus/real/example-head.beancount"),
        ),
    ];
    let mut group = c.benchmark_group("real");
    for (name, input) in files {
        group.throughput(Throughput::Bytes(input.len() as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(name),
            input,
            |b, input| b.iter(|| parse_all(black_box(input))),
        );
    }
    group.finish();
}

/// Awkward on purpose. Each of these has a plausible way to be accidentally
/// quadratic; if one of them stops scaling linearly, that is the bug.
fn pathological(c: &mut Criterion) {
    let one_huge_transaction = {
        let mut s =
            String::from("2026-01-01 * \"one transaction, many legs\"\n");
        for i in 0..20_000 {
            s.push_str("  Assets:Cash  ");
            s.push_str(&i.to_string());
            s.push_str(".00 USD\n");
        }
        s
    };
    let mostly_comments = {
        let mut s = String::new();
        for i in 0..20_000 {
            s.push_str("; a comment line that carries no information at all\n");
            if i % 10 == 0 {
                s.push_str("2026-01-01 open Assets:Cash\n");
            }
        }
        s
    };
    let long_strings = {
        let mut s = String::new();
        let narration = "x".repeat(2_000);
        for _ in 0..500 {
            s.push_str("2026-01-01 * \"");
            s.push_str(&narration);
            s.push_str("\"\n  Assets:Cash 1 USD\n");
        }
        s
    };
    let deep_expressions = {
        // Well under the recursion limit documented in tests/grammar.rs.
        let expr = format!("{}1 + 1{}", "(".repeat(16), ")".repeat(16));
        let mut s = String::new();
        for _ in 0..5_000 {
            s.push_str("2026-01-01 price AAA ");
            s.push_str(&expr);
            s.push_str(" USD\n");
        }
        s
    };
    let deep_meta_stack = {
        // Every pushed key is copied onto every directive that follows, so a
        // deep stack multiplies the work per directive. This is the shape that
        // would expose that as quadratic.
        let mut s = String::new();
        for i in 0..200 {
            let _ = writeln!(s, "pushmeta k{i}: \"v\"");
        }
        for _ in 0..10_000 {
            s.push_str("2026-01-01 open Assets:Cash\n");
        }
        s
    };
    let error_at_the_end = {
        // The parse must not be quadratic in how far it got before failing.
        let mut s = generated(Shape::Realistic, 20_000);
        s.push_str("2026-13-01 open Assets:Broken\n");
        s
    };

    let cases: [(&str, &str); 6] = [
        ("one-huge-transaction", &one_huge_transaction),
        ("mostly-comments", &mostly_comments),
        ("long-strings", &long_strings),
        ("deep-expressions", &deep_expressions),
        ("deep-meta-stack", &deep_meta_stack),
        ("error-at-the-end", &error_at_the_end),
    ];

    let mut group = c.benchmark_group("pathological");
    for (name, input) in cases {
        group.throughput(Throughput::Bytes(input.len() as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(name),
            input,
            |b, input| {
                // These can fail to parse; counting entries keeps the work the
                // same either way.
                b.iter(|| parse_iter::<Decimal>(black_box(input)).count())
            },
        );
    }
    group.finish();
}

criterion_group!(benches, throughput, shape, api, real, pathological);
criterion_main!(benches);
