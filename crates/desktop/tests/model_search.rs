//! Searching the ledger the way the web app does: every term must
//! appear somewhere in the transaction, newest first.

mod common;

use bean_desktop::model::search::search;
use common::{TODAY, d, overview_ledger};

#[test]
fn every_term_must_match_newest_first() {
    let ledger = overview_ledger();
    let hits = search(&ledger, TODAY, "coffee");
    assert_eq!(hits.len(), 8);
    assert_eq!(hits[0].date, (2026, 8, 22));
    assert_eq!(hits[7].date, (2026, 1, 22));
    assert!(search(&ledger, TODAY, "coffee 2026-08").len() == 1);
    assert!(search(&ledger, TODAY, "coffee nothing-like-this").is_empty());
}

#[test]
fn a_hit_shows_the_category_and_what_left_the_bank() {
    let ledger = overview_ledger();
    let hit = &search(&ledger, TODAY, "sunday coffee")[0];
    assert_eq!(hit.payee, "Sunday Coffee");
    assert_eq!(hit.category, "Expenses:Food:Coffee");
    assert_eq!(hit.bank, "Assets:Bank:Everyday");
    assert_eq!(hit.amount, d("-62"));
    assert_eq!(hit.postings.len(), 2);
    assert_eq!(hit.line, Some(373));
    assert_eq!(hit.file.as_deref(), Some("overview.beancount"));
}

#[test]
fn tags_links_and_metadata_are_searchable_without_their_sigils() {
    let ledger = overview_ledger();
    assert_eq!(
        search(&ledger, TODAY, "#coffee").len(),
        search(&ledger, TODAY, "coffee").len()
    );
    assert!(
        search(&ledger, TODAY, "").is_empty(),
        "an empty prompt lists nothing"
    );
}
