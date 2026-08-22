//! A fingerprint has to notice every way a ledger's files can change on
//! disk, and stay quiet about everything else.

use std::fs;
use std::path::{Path, PathBuf};

use bean_core::watch::fingerprint;

/// A directory of this test's own, emptied first so a previous run cannot
/// leak into this one.
fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir()
        .join(format!("ynab-watch-{}", std::process::id()))
        .join(name);
    fs::remove_dir_all(&dir).ok();
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn write(path: &Path, text: &str) {
    fs::write(path, text).unwrap();
}

#[test]
fn untouched_files_look_the_same() {
    let dir = scratch("untouched");
    let main = dir.join("main.beancount");
    write(&main, "option \"title\" \"One\"\n");
    let files = vec![main];

    assert_eq!(fingerprint(&files), fingerprint(&files));
}

#[test]
fn notices_a_file_being_rewritten() {
    let dir = scratch("rewritten");
    let main = dir.join("main.beancount");
    write(&main, "option \"title\" \"One\"\n");
    let files = vec![main.clone()];
    let before = fingerprint(&files);

    write(&main, "option \"title\" \"Another title entirely\"\n");
    assert_ne!(fingerprint(&files), before);
}

#[test]
fn notices_a_file_going_missing_and_coming_back() {
    let dir = scratch("missing");
    let main = dir.join("main.beancount");
    write(&main, "option \"title\" \"One\"\n");
    let files = vec![main.clone()];
    let before = fingerprint(&files);

    fs::remove_file(&main).unwrap();
    let gone = fingerprint(&files);
    assert_ne!(gone, before);

    // A file that is not there yet is a stamp of its own, not an error, so
    // the watcher keeps going and sees it return.
    write(&main, "option \"title\" \"One\"\n");
    assert_ne!(fingerprint(&files), gone);
}

#[test]
fn watches_only_the_files_it_was_given() {
    let dir = scratch("unwatched");
    let main = dir.join("main.beancount");
    write(&main, "option \"title\" \"One\"\n");
    let files = vec![main];
    let before = fingerprint(&files);

    write(&dir.join("scratch.txt"), "not part of the ledger\n");
    assert_eq!(fingerprint(&files), before);
}

#[test]
fn each_file_is_stamped_separately() {
    let dir = scratch("separately");
    let main = dir.join("main.beancount");
    let side = dir.join("2026-08.beancount");
    write(&main, "include \"2026-08.beancount\"\n");
    write(&side, "; nothing yet\n");
    let files = vec![main, side.clone()];
    let before = fingerprint(&files);

    // The root did not move, but an included file did.
    write(&side, "; a transaction landed here\n");
    assert_ne!(fingerprint(&files), before);
}
