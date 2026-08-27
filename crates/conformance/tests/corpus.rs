//! The golden corpus: every `.beancount` file under `corpus/` must dump to
//! the `.expected` file beside it.
//!
//! These are the cases a person chose, and each one is there because getting
//! it wrong would be a real bug. `corpus/quirks/` is the interesting part:
//! behaviour that is surprising but load-bearing, documented in the source
//! file itself.

use bean_conformance::corpus;

#[test]
fn every_case_matches_its_golden() {
    let dir = corpus::dir();
    let cases = corpus::cases(&dir);
    assert!(
        cases.len() > 40,
        "the corpus lost cases: found only {} under {}",
        cases.len(),
        dir.display(),
    );

    let report = corpus::check(&cases);
    if !report.blessed.is_empty() {
        eprintln!("blessed {} golden(s):", report.blessed.len());
        for name in &report.blessed {
            eprintln!("  {name}");
        }
    }
    assert!(report.is_clean(), "\n{}", report.describe());
}

/// Every case must have a golden and every golden a case, or a rename leaves
/// an orphan nobody notices.
#[test]
fn no_orphaned_goldens() {
    let dir = corpus::dir();
    let cases = corpus::cases(&dir);
    let mut orphans: Vec<String> = Vec::new();
    let mut stack = vec![dir.clone()];
    while let Some(current) = stack.pop() {
        for entry in std::fs::read_dir(&current).expect("readable corpus") {
            let path = entry.expect("readable entry").path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|e| e == "expected")
                && !path.with_extension("beancount").exists()
            {
                orphans.push(path.display().to_string());
            }
        }
    }
    assert!(
        orphans.is_empty(),
        "goldens with no input file (delete them): {orphans:#?}",
    );
    assert!(!cases.is_empty(), "the corpus is empty");
}
