//! Notices when the files a ledger was read from change on disk.
//!
//! Only those files are watched, which is all beancount's `include` needs:
//! a file joins the ledger by being named from a file that is already in
//! it, so the edit that adds it is itself a change. The gap is a glob
//! include that starts matching a file nobody named — that one waits for
//! the next reload.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// What a set of files looked like at one moment. Two fingerprints over
/// the same paths compare equal for as long as none of them is written
/// to, removed or replaced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fingerprint(Vec<(PathBuf, Option<Stamp>)>);

/// A file's length and modification time — enough to spot an edit without
/// reading the file back on every tick.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Stamp {
    len: u64,
    modified: Option<SystemTime>,
}

/// Stat every path. One that cannot be read stamps as nothing at all, so
/// a file going missing, or arriving, is a change like any other.
pub fn fingerprint<P: AsRef<Path>>(files: &[P]) -> Fingerprint {
    Fingerprint(
        files
            .iter()
            .map(|path| {
                let stamp = std::fs::metadata(path).ok().map(|meta| Stamp {
                    len: meta.len(),
                    modified: meta.modified().ok(),
                });
                (path.as_ref().to_path_buf(), stamp)
            })
            .collect(),
    )
}
