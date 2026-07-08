//! # buzzy-crdt-mock
//!
//! A plain text buffer with last-writer-wins semantics, implementing the
//! `buzzy-crdt` traits. It is **not** a real CRDT and does not converge under
//! concurrent edits. It exists for two reasons:
//!
//! 1. **Validation** — a second implementation of the trait boundary is the only
//!    real proof the abstraction isn't accidentally Automerge-shaped.
//! 2. **Test fixture** — deterministic and dependency-free, so daemon and network
//!    tests can run without pulling in a CRDT engine.

use buzzy_crdt::{CrdtDocument, CrdtEngine, CrdtError, SyncSession};

/// Engine handle. Stateless.
#[derive(Debug, Default)]
pub struct MockEngine;

impl MockEngine {
    pub fn new() -> Self {
        Self
    }
}

/// A document backed by a `String`.
#[derive(Debug, Clone)]
pub struct MockDoc {
    text: String,
}

/// Trivial sync session: one exchange transfers the full text, then converged.
#[derive(Debug, Default)]
pub struct MockSync {
    synced: bool,
}

impl CrdtEngine for MockEngine {
    type Document = MockDoc;
    type SyncState = MockSync;

    fn create_document(&self, initial_text: &str) -> Result<MockDoc, CrdtError> {
        Ok(MockDoc {
            text: initial_text.to_owned(),
        })
    }

    fn load_document(&self, bytes: &[u8]) -> Result<MockDoc, CrdtError> {
        let text = String::from_utf8(bytes.to_vec()).map_err(|e| CrdtError::Load(e.to_string()))?;
        Ok(MockDoc { text })
    }

    fn create_sync_session(&self) -> MockSync {
        MockSync::default()
    }
}

impl CrdtDocument for MockDoc {
    fn text(&self) -> String {
        self.text.clone()
    }

    fn insert(&mut self, pos: usize, text: &str) -> Result<(), CrdtError> {
        if pos > self.text.len() || !self.text.is_char_boundary(pos) {
            return Err(CrdtError::OutOfBounds {
                pos,
                len: self.text.len(),
            });
        }
        self.text.insert_str(pos, text);
        Ok(())
    }

    fn delete(&mut self, pos: usize, len: usize) -> Result<(), CrdtError> {
        let end = pos.checked_add(len).ok_or(CrdtError::OutOfBounds {
            pos,
            len: self.text.len(),
        })?;
        if end > self.text.len()
            || !self.text.is_char_boundary(pos)
            || !self.text.is_char_boundary(end)
        {
            return Err(CrdtError::OutOfBounds {
                pos,
                len: self.text.len(),
            });
        }
        self.text.replace_range(pos..end, "");
        Ok(())
    }

    fn update_text(&mut self, new_content: &str) -> Result<(), CrdtError> {
        self.text = new_content.to_owned();
        Ok(())
    }

    fn save(&self) -> Vec<u8> {
        self.text.clone().into_bytes()
    }

    fn save_incremental(&mut self) -> Vec<u8> {
        self.text.clone().into_bytes()
    }

    fn apply_remote_change(&mut self, bytes: &[u8]) -> Result<(), CrdtError> {
        self.text =
            String::from_utf8(bytes.to_vec()).map_err(|e| CrdtError::ApplyRemote(e.to_string()))?;
        Ok(())
    }

    fn latest_change(&self) -> Option<Vec<u8>> {
        Some(self.text.clone().into_bytes())
    }
}

impl SyncSession for MockSync {
    fn generate_message(&mut self, doc: &dyn CrdtDocument) -> Option<Vec<u8>> {
        if self.synced {
            None
        } else {
            self.synced = true;
            Some(doc.text().into_bytes())
        }
    }

    fn receive_message(&mut self, doc: &mut dyn CrdtDocument, msg: &[u8]) -> Result<(), CrdtError> {
        doc.apply_remote_change(msg)?;
        self.synced = true;
        Ok(())
    }

    fn is_synced(&self) -> bool {
        self.synced
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc(text: &str) -> MockDoc {
        MockEngine::new().create_document(text).unwrap_or(MockDoc {
            text: String::new(),
        })
    }

    #[test]
    fn insert_and_delete() {
        let mut d = doc("hello");
        d.insert(5, " world").unwrap_or_default();
        assert_eq!(d.text(), "hello world");
        d.delete(0, 6).unwrap_or_default();
        assert_eq!(d.text(), "world");
    }

    #[test]
    fn out_of_bounds_is_rejected() {
        let mut d = doc("hi");
        assert!(d.insert(99, "x").is_err());
        assert!(d.delete(1, 99).is_err());
    }

    #[test]
    fn save_load_round_trip() {
        let d = doc("persist me");
        let bytes = d.save();
        let loaded = MockEngine::new().load_document(&bytes).unwrap_or(MockDoc {
            text: String::new(),
        });
        assert_eq!(loaded.text(), "persist me");
    }
}
