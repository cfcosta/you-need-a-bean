//! Golden tests: a directory of `.beancount` files, each paired with the
//! `.expected` dump parsing it must produce.
//!
//! The pairs are the human-readable half of the contract. A rewrite is
//! finished when every case here reproduces its `.expected` byte for byte.
//! When a change to the parser is *meant* to change behaviour, re-bless with
//! `BLESS=1 cargo test -p you-need-a-bean-conformance` and review the diff:
//! the diff is the behaviour change, stated in full.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use crate::dump;

/// One `.beancount` file and the golden next to it.
#[derive(Debug)]
pub struct Case {
    pub name: String,
    pub input: PathBuf,
    pub expected: PathBuf,
}

impl Case {
    /// The source text.
    ///
    /// # Panics
    /// If the input file cannot be read.
    #[must_use]
    pub fn source(&self) -> String {
        std::fs::read_to_string(&self.input).unwrap_or_else(|err| {
            panic!("read {}: {err}", self.input.display())
        })
    }
}

/// The corpus directory shipped with this crate.
#[must_use]
pub fn dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("corpus")
}

/// Every case in `dir`, sorted by name so failures report in a stable order.
///
/// # Panics
/// If the directory cannot be read.
#[must_use]
pub fn cases(dir: &Path) -> Vec<Case> {
    let mut cases: Vec<Case> = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(current) = stack.pop() {
        let entries = std::fs::read_dir(&current)
            .unwrap_or_else(|err| panic!("read {}: {err}", current.display()));
        for entry in entries {
            let path = entry.expect("readable directory entry").path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if path.extension().is_some_and(|ext| ext == "beancount") {
                let name = path
                    .strip_prefix(dir)
                    .unwrap_or(&path)
                    .with_extension("")
                    .to_string_lossy()
                    .replace('\\', "/");
                cases.push(Case {
                    name,
                    expected: path.with_extension("expected"),
                    input: path,
                });
            }
        }
    }
    cases.sort_by(|a, b| a.name.cmp(&b.name));
    cases
}

/// Whether the run should rewrite goldens instead of asserting on them.
///
/// `BLESS` is the name used here; `UPDATE_EXPECT` is honoured too because
/// several Rust tools use it and muscle memory is worth accommodating.
#[must_use]
pub fn blessing() -> bool {
    ["BLESS", "UPDATE_EXPECT"].iter().any(|key| {
        std::env::var_os(key).is_some_and(|v| v != "0" && !v.is_empty())
    })
}

/// Check every case, returning a report of the mismatches.
///
/// In blessing mode the goldens are rewritten and the report is empty.
///
/// # Panics
/// If a golden cannot be read or written.
#[must_use]
pub fn check(cases: &[Case]) -> Report {
    let bless = blessing();
    let mut report = Report::default();
    for case in cases {
        let actual = dump(&case.source());
        if bless {
            let previous = std::fs::read_to_string(&case.expected).ok();
            if previous.as_deref() != Some(actual.as_str()) {
                std::fs::write(&case.expected, &actual).unwrap_or_else(|err| {
                    panic!("write {}: {err}", case.expected.display())
                });
                report.blessed.push(case.name.clone());
            }
            continue;
        }
        let Ok(expected) = std::fs::read_to_string(&case.expected) else {
            report.missing.push(case.name.clone());
            continue;
        };
        if expected != actual {
            report.mismatched.push(Mismatch {
                name: case.name.clone(),
                expected,
                actual,
            });
        }
    }
    report
}

#[derive(Debug, Default)]
pub struct Report {
    pub mismatched: Vec<Mismatch>,
    /// Cases with no golden at all — a new case that was never blessed.
    pub missing: Vec<String>,
    /// Cases whose golden was rewritten in blessing mode.
    pub blessed: Vec<String>,
}

impl Report {
    #[must_use]
    pub fn is_clean(&self) -> bool {
        self.mismatched.is_empty() && self.missing.is_empty()
    }

    /// A failure message with a line-level diff per mismatch.
    #[must_use]
    pub fn describe(&self) -> String {
        let mut out = String::new();
        for name in &self.missing {
            let _ = writeln!(
                out,
                "{name}: no .expected golden; run with BLESS=1 to create it"
            );
        }
        for mismatch in &self.mismatched {
            let _ = writeln!(out, "{}", mismatch.describe());
        }
        if !out.is_empty() {
            out.push_str(
                "\nrun `BLESS=1 cargo test -p you-need-a-bean-conformance` \
                 to accept these, and review the diff\n",
            );
        }
        out
    }
}

#[derive(Debug)]
pub struct Mismatch {
    pub name: String,
    pub expected: String,
    pub actual: String,
}

impl Mismatch {
    #[must_use]
    pub fn describe(&self) -> String {
        let mut out = format!("{}: dump differs from golden\n", self.name);
        out.push_str(&diff(&self.expected, &self.actual));
        out
    }
}

/// A minimal line diff: enough to point at the first divergence without
/// pulling in a diffing dependency.
#[must_use]
pub fn diff(expected: &str, actual: &str) -> String {
    let expected: Vec<&str> = expected.lines().collect();
    let actual: Vec<&str> = actual.lines().collect();
    let first = (0..expected.len().max(actual.len()))
        .find(|&i| expected.get(i) != actual.get(i))
        .unwrap_or(0);
    let from = first.saturating_sub(3);
    let to = (first + 4).min(expected.len().max(actual.len()));

    let mut out = String::new();
    let _ = writeln!(out, "  first difference at line {}", first + 1);
    for i in from..to {
        match (expected.get(i), actual.get(i)) {
            (Some(e), Some(a)) if e == a => {
                let _ = writeln!(out, "   {e}");
            }
            (e, a) => {
                if let Some(e) = e {
                    let _ = writeln!(out, "  -{e}");
                }
                if let Some(a) = a {
                    let _ = writeln!(out, "  +{a}");
                }
            }
        }
    }
    out
}
