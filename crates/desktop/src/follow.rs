//! Following the ledger's files: reread once an edit has settled.

use bean_core::watch::Fingerprint;

/// Decides, tick by tick, when a change to the files is worth a reread.
/// An editor writes a file in pieces, so a change has to look the same on
/// two ticks in a row before it counts.
pub struct Settle {
    settled: Fingerprint,
    pending: Option<Fingerprint>,
}

impl Settle {
    pub fn new(settled: Fingerprint) -> Self {
        Self {
            settled,
            pending: None,
        }
    }

    /// Feed what the files look like now; `true` means reread them.
    pub fn tick(&mut self, seen: Fingerprint) -> bool {
        if seen == self.settled {
            self.pending = None;
            return false;
        }
        if self.pending.as_ref() != Some(&seen) {
            self.pending = Some(seen);
            return false;
        }
        self.pending = None;
        true
    }

    /// After a reread, what the files look like from now on.
    pub fn settle(&mut self, now: Fingerprint) {
        self.settled = now;
        self.pending = None;
    }
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf};

    use bean_core::watch::fingerprint;

    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir()
            .join(format!("bean-desktop-{name}-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        dir.join("main.beancount")
    }

    #[test]
    fn a_change_counts_once_it_holds_for_two_ticks() {
        let path = scratch("settle");
        fs::write(&path, "a").unwrap();
        let mut s = Settle::new(fingerprint(&[&path]));
        assert!(!s.tick(fingerprint(&[&path])));
        fs::write(&path, "ab").unwrap();
        assert!(
            !s.tick(fingerprint(&[&path])),
            "first sight of a change waits"
        );
        assert!(
            s.tick(fingerprint(&[&path])),
            "the same change twice rereads"
        );
        s.settle(fingerprint(&[&path]));
        assert!(!s.tick(fingerprint(&[&path])));
    }

    #[test]
    fn a_file_still_being_written_does_not_count() {
        let path = scratch("moving");
        fs::write(&path, "a").unwrap();
        let mut s = Settle::new(fingerprint(&[&path]));
        fs::write(&path, "ab").unwrap();
        assert!(!s.tick(fingerprint(&[&path])));
        fs::write(&path, "abc").unwrap();
        assert!(!s.tick(fingerprint(&[&path])), "it moved again: wait");
        assert!(s.tick(fingerprint(&[&path])));
    }
}
