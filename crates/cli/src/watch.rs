//! Keeps the served ledger in step with the files it was read from.
//!
//! The ledger is parsed once and answered from memory, so nothing would
//! notice an edit on its own. This stats the loaded files on a timer and
//! reloads when they move. A reload that fails leaves the last good
//! ledger in place — you are usually mid-edit when that happens, and half
//! a ledger is worse than a slightly old one.

use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use bean_core::loader::load;
use bean_core::model::Ledger;
use bean_core::watch::fingerprint;

use crate::api::AppState;

/// How often the ledger's files are stat-ed. Two ticks have to agree
/// before anything is parsed, so it doubles as the debounce.
pub const POLL: Duration = Duration::from_millis(500);

/// Follow `root`'s ledger for as long as the server runs.
///
/// The files are stamped when this is *called*, not when the returned
/// future is first polled, so an edit landing between the load and the
/// first tick still counts as a change.
pub fn follow(
    root: PathBuf,
    state: Arc<AppState>,
    poll: Duration,
) -> impl Future<Output = ()> {
    let mut settled = fingerprint(&state.snapshot().ledger.files);

    async move {
        let mut pending = None;

        loop {
            tokio::time::sleep(poll).await;
            // Read the file list back each time: a reload can add or drop
            // includes, and those are what we watch from then on.
            let seen = fingerprint(&state.snapshot().ledger.files);
            if seen == settled {
                pending = None;
                continue;
            }
            // An editor writes a file in pieces. Wait for two ticks that
            // agree, so half a file is not parsed as a broken ledger.
            if pending.as_ref() != Some(&seen) {
                pending = Some(seen);
                continue;
            }
            pending = None;
            reload(&root, &state).await;
            settled = fingerprint(&state.snapshot().ledger.files);
        }
    }
}

async fn reload(root: &Path, state: &AppState) {
    let path = root.to_path_buf();
    let start = Instant::now();
    // Parsing a real ledger takes long enough to stall the runtime.
    let built =
        tokio::task::spawn_blocking(move || load(&path).map(Ledger::build))
            .await;
    let parse_ms = start.elapsed().as_millis() as u64;

    match built {
        Ok(Ok(ledger)) => {
            for warning in &ledger.warnings {
                eprintln!("warning: {warning}");
            }
            let snapshot = state.install(ledger, parse_ms);
            println!(
                "reloaded #{}: {} directives across {} files in {parse_ms}ms",
                snapshot.revision,
                snapshot.ledger.directives,
                snapshot.ledger.files.len(),
            );
        }
        Ok(Err(error)) => {
            eprintln!(
                "error: reload failed, still serving the last good ledger"
            );
            // The terminal gets the diagnostic, with the offending line drawn
            // under it. The browser has no way to draw one, so it gets the
            // same error as a line of text with the location still in it.
            let summary = error.summary();
            eprintln!("{:?}", miette::Report::new(error));
            state.reload_failed(summary);
        }
        Err(error) => {
            eprintln!("error: reload did not finish: {error}");
            state.reload_failed(format!("reload did not finish: {error}"));
        }
    }
}
