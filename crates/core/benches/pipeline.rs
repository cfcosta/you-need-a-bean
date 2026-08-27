//! What loading a ledger actually costs, end to end.
//!
//! `crates/conformance/benches/parse.rs` measures the parser on a string in
//! memory. This measures the thing the CLI runs: read a file, follow its
//! includes, parse every one, then build the model on top. A parser rewrite
//! should be judged on both — a 2× faster parser is worth little if parsing
//! is a third of the wall clock.
//!
//! Fixtures are written to a temp directory from a fixed seed, so the numbers
//! are comparable run to run.

use std::hint::black_box;
use std::path::{Path, PathBuf};

use bean_core::loader::{self, LoadedLedger};
use bean_core::model::Ledger;
use criterion::{
    BenchmarkId, Criterion, Throughput, criterion_group, criterion_main,
};

/// A deterministic ledger of `directives` transactions, written to disk.
///
/// Deliberately not the conformance generator: `bean_core` must not depend on
/// the conformance crate, and what this needs is bulk, not coverage.
fn write_flat(dir: &Path, directives: usize) -> PathBuf {
    let accounts = [
        "Expenses:Groceries",
        "Expenses:Rent",
        "Expenses:Transport:Fuel",
        "Income:Salary",
        "Assets:Bank:Checking",
        "Assets:Cash",
    ];
    let mut text = String::from(
        "option \"title\" \"benchmark\"\noption \"operating_currency\" \"USD\"\n\n",
    );
    for account in accounts {
        text.push_str(&format!("2019-01-01 open {account} USD\n"));
    }
    for i in 0..directives {
        let year = 2020 + i / 4032;
        let month = i / 336 % 12 + 1;
        let day = i % 28 + 1;
        let amount = (i % 9973) as f64 / 100.0;
        text.push_str(&format!(
            "{year:04}-{month:02}-{day:02} * \"Payee {i}\" \"narration {i}\"\n  \
             {}  {amount:.2} USD\n  {}\n",
            accounts[i % 4],
            accounts[4 + i % 2],
        ));
    }
    let path = dir.join("root.beancount");
    std::fs::write(&path, text).expect("write the benchmark ledger");
    path
}

/// The same content split across `parts` files behind `include` directives,
/// which is how a real ledger is organised.
fn write_split(dir: &Path, directives: usize, parts: usize) -> PathBuf {
    let months = dir.join("months");
    std::fs::create_dir_all(&months).expect("create the months directory");
    let per_part = directives.div_ceil(parts);
    let mut root = String::from("option \"title\" \"benchmark\"\n");
    root.push_str("2019-01-01 open Assets:Cash USD\n");
    root.push_str("2019-01-01 open Expenses:Groceries USD\n");
    for part in 0..parts {
        let mut text = String::new();
        for i in 0..per_part {
            let n = part * per_part + i;
            text.push_str(&format!(
                "2026-01-{:02} * \"narration {n}\"\n  \
                 Expenses:Groceries  {}.00 USD\n  Assets:Cash\n",
                n % 28 + 1,
                n % 500,
            ));
        }
        std::fs::write(months.join(format!("{part:04}.beancount")), text)
            .expect("write a month file");
        root.push_str(&format!("include \"months/{part:04}.beancount\"\n"));
    }
    let path = dir.join("split.beancount");
    std::fs::write(&path, root).expect("write the split root");
    path
}

/// A scratch directory that cleans itself up. `std::env::temp_dir` plus the
/// process id is enough; pulling in a tempdir crate for a benchmark is not
/// worth the dependency.
struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        let path = std::env::temp_dir()
            .join(format!("bean-pipeline-bench-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).expect("create the scratch directory");
        Self(path)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn bytes(path: &Path) -> u64 {
    std::fs::metadata(path).map(|m| m.len()).unwrap_or(0)
}

/// Loading one file: read, parse, collect.
fn load(c: &mut Criterion) {
    let scratch = Scratch::new();
    let mut group = c.benchmark_group("load");
    for directives in [1_000usize, 10_000, 50_000] {
        let dir = scratch.0.join(format!("flat-{directives}"));
        std::fs::create_dir_all(&dir).expect("create the fixture directory");
        let path = write_flat(&dir, directives);
        group.throughput(Throughput::Bytes(bytes(&path)));
        group.bench_with_input(
            BenchmarkId::from_parameter(directives),
            &path,
            |b, path| {
                b.iter(|| {
                    loader::load(black_box(path))
                        .expect("the fixture loads")
                        .directives
                        .len()
                })
            },
        );
    }
    group.finish();
}

/// The same content behind `include` directives, which adds path resolution,
/// deduplication and one `read_to_string` per file.
fn includes(c: &mut Criterion) {
    let scratch = Scratch::new();
    let mut group = c.benchmark_group("includes");
    for parts in [1usize, 12, 120] {
        let dir = scratch.0.join(format!("split-{parts}"));
        std::fs::create_dir_all(&dir).expect("create the fixture directory");
        let path = write_split(&dir, 12_000, parts);
        group.bench_with_input(
            BenchmarkId::from_parameter(parts),
            &path,
            |b, path| {
                b.iter(|| {
                    loader::load(black_box(path))
                        .expect("the fixture loads")
                        .directives
                        .len()
                })
            },
        );
    }
    group.finish();
}

/// Where the time goes once the bytes are read: parsing, or building the
/// model on top of the parse? A parser rewrite only moves the first number.
fn stages(c: &mut Criterion) {
    let scratch = Scratch::new();
    let dir = scratch.0.join("stages");
    std::fs::create_dir_all(&dir).expect("create the fixture directory");
    let path = write_flat(&dir, 20_000);

    let mut group = c.benchmark_group("stages");
    group.throughput(Throughput::Bytes(bytes(&path)));
    group.bench_function("load", |b| {
        b.iter(|| {
            loader::load(black_box(&path))
                .expect("loads")
                .directives
                .len()
        })
    });
    group.bench_function("load+build", |b| {
        b.iter(|| {
            let loaded = loader::load(black_box(&path)).expect("loads");
            Ledger::build(loaded).txns.len()
        })
    });
    // `build` alone, with the load hoisted out of the timed section.
    group.bench_function("build", |b| {
        b.iter_batched(
            || loader::load(&path).expect("loads"),
            |loaded: LoadedLedger| Ledger::build(loaded).txns.len(),
            criterion::BatchSize::LargeInput,
        )
    });
    group.finish();
}

criterion_group!(benches, load, includes, stages);
criterion_main!(benches);
