//! Pure baseline / dirty / conflict model for an open editor document.
//!
//! The view layer holds the live buffer in a signal; this type tracks the
//! on-disk baseline (text + hash + mtime) used to derive dirtiness, drive the
//! save-time conflict check, and reset after a successful write or revert.

/// The last-known on-disk state of an open document.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Baseline {
    /// Text as last read from / written to disk. Reverting restores this.
    pub disk_text: String,
    /// Content hash matching `disk_text` — sent as the conflict expectation.
    pub hash: String,
    /// Modification timestamp (Unix ms) when known.
    pub modified_ms: Option<i64>,
}

impl Baseline {
    #[must_use]
    pub fn new(disk_text: String, hash: String, modified_ms: Option<i64>) -> Self {
        Self {
            disk_text,
            hash,
            modified_ms,
        }
    }

    /// `true` when the live buffer differs from the on-disk baseline.
    #[must_use]
    pub fn is_dirty(&self, buffer: &str) -> bool {
        self.disk_text != buffer
    }

    /// Update the baseline after a successful save so the document is clean
    /// again without re-reading from disk.
    pub fn after_save(&mut self, saved_text: String, hash: String, modified_ms: Option<i64>) {
        self.disk_text = saved_text;
        self.hash = hash;
        self.modified_ms = modified_ms;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base() -> Baseline {
        Baseline::new("hello".into(), "abc".into(), Some(10))
    }

    #[test]
    fn dirty_tracks_buffer_vs_disk() {
        let b = base();
        assert!(!b.is_dirty("hello"));
        assert!(b.is_dirty("hello world"));
    }

    #[test]
    fn after_save_clears_dirty_and_updates_baseline() {
        let mut b = base();
        b.after_save("hello world".into(), "def".into(), Some(20));
        assert!(!b.is_dirty("hello world"));
        assert!(b.is_dirty("hello"));
        assert_eq!(b.hash, "def");
        assert_eq!(b.modified_ms, Some(20));
    }

    #[test]
    fn revert_restores_baseline_text() {
        let b = base();
        // Reverting means setting the buffer back to disk_text; then clean.
        let reverted = b.disk_text.clone();
        assert!(!b.is_dirty(&reverted));
    }
}
