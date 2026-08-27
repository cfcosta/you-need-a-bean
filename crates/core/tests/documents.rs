//! `document` directives: where the file is, and which transaction it
//! belongs to.

use std::path::{Path, PathBuf};

use bean_core::loader::load;
use bean_core::model::{Day, Ledger};

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/documents")
}

fn ledger() -> Ledger {
    Ledger::build(load(&fixtures().join("main.beancount")).unwrap())
}

/// Paths are compared against the fixture directory rather than spelled
/// out, so the assertion says what it means: this file, right here.
fn at(relative: &str) -> PathBuf {
    fixtures()
        .join(relative)
        .canonicalize()
        .unwrap_or_else(|_| {
            // A document naming a file nobody put there still has a path.
            fixtures().join(relative)
        })
}

fn day(y: u16, m: u8, d: u8) -> Day {
    (y, m, d)
}

fn paths(ledger: &Ledger, ids: &[usize]) -> Vec<PathBuf> {
    ids.iter()
        .map(|&i| ledger.document(i).unwrap().path.clone())
        .collect()
}

#[test]
fn resolves_each_path_against_the_file_that_declared_it() {
    // The coffee receipt is declared in `sub/`, written as `../receipts/…`,
    // and so lives beside the ledger root — not under `sub/`. Flattening the
    // directives loses the file each came from, so the loader is the last
    // place that can answer this.
    let ledger = ledger();
    let coffee = ledger.documents_on("Expenses:Coffee", day(2026, 1, 6));
    assert_eq!(paths(&ledger, coffee), vec![at("receipts/coffee.txt")]);

    let invoice = ledger.documents_on("Expenses:Software", day(2026, 1, 5));
    assert_eq!(
        paths(&ledger, invoice),
        vec![at("files/invoice.pdf"), at("files/gone.pdf")],
    );
    assert!(
        invoice
            .iter()
            .all(|&i| ledger.document(i).unwrap().path.is_absolute()),
        "a resolved path is absolute, whether or not the file is there",
    );
}

#[test]
fn keeps_the_account_and_date_the_directive_was_written_with() {
    let ledger = ledger();
    let [id] = ledger.documents_on("Liabilities:Card", day(2026, 1, 5)) else {
        panic!("one statement on the card");
    };
    let doc = ledger.document(*id).unwrap();
    assert_eq!(doc.account, "Liabilities:Card");
    assert_eq!(doc.date, day(2026, 1, 5));
    assert_eq!(doc.path.file_name().unwrap(), Path::new("statement.csv"));
}

#[test]
fn a_transaction_claims_the_documents_of_every_account_it_touches() {
    // The invoice hangs off the expense account and the statement off the
    // card, and one purchase produced both. Matching only the account being
    // inspected would show you half of your own paperwork.
    let ledger = ledger();
    let txn = ledger
        .txns
        .iter()
        .find(|t| t.payee.as_deref() == Some("Acme"))
        .expect("the subscription");

    assert_eq!(
        paths(&ledger, &ledger.documents_of(txn)),
        vec![
            at("files/invoice.pdf"),
            at("files/gone.pdf"),
            at("files/statement.csv"),
        ],
        "expense account first, in the order the postings were written",
    );
}

#[test]
fn a_document_without_a_transaction_that_day_belongs_to_nobody() {
    // 2026-01-09 has an invoice and no transaction. Nothing should reach
    // back and attach it to the nearest purchase.
    let ledger = ledger();
    assert_eq!(
        ledger
            .documents_on("Expenses:Software", day(2026, 1, 9))
            .len(),
        1,
    );
    for txn in &ledger.txns {
        let claimed = ledger.documents_of(txn);
        let claimed = paths(&ledger, &claimed);
        assert!(
            !claimed.iter().any(|p| p == &at("files/invoice.pdf"))
                || txn.date == day(2026, 1, 5),
            "{:?} claimed the 9th's invoice",
            txn.payee,
        );
    }
}

#[test]
fn a_transaction_with_no_paperwork_says_so() {
    let ledger = ledger();
    let coffee = ledger
        .txns
        .iter()
        .find(|t| t.payee.as_deref() == Some("Cafe"))
        .expect("the coffee");
    // Its own document is on the 6th, the same day, so it is claimed.
    assert_eq!(ledger.documents_of(coffee).len(), 1);

    assert!(
        ledger
            .documents_on("Assets:Bank", day(2026, 1, 6))
            .is_empty()
    );
}
