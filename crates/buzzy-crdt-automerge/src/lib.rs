//! # buzzy-crdt-automerge
//!
//! The Automerge-backed implementation of the `buzzy-crdt` traits.
//!
//! **Phase 0: placeholder.** The `automerge` dependency and the `CrdtEngine` /
//! `CrdtDocument` / `SyncSession` impls land in Phase 1, where `update_text`
//! delegates to Automerge's `Transaction::update_text`. Keeping this crate the
//! sole future home of `automerge` is what makes the dependency-boundary test
//! (`xtask/tests/architecture.rs`) meaningful: the daemon reaches a concrete
//! engine only through its single composition-root line.

/// Placeholder for the Automerge-backed engine. Implements `buzzy_crdt::CrdtEngine`
/// in Phase 1.
#[derive(Debug, Default)]
pub struct AutomergeEngine;

impl AutomergeEngine {
    pub fn new() -> Self {
        Self
    }
}
