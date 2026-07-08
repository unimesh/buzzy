//! # buzzy-net
//!
//! Peer networking: mDNS discovery, QUIC transport, and the sync-protocol driver.
//! Phase 2 fills this in. Phase 0 establishes the crate boundary — it depends on
//! `buzzy-crdt` (the [`SyncSession`](buzzy_crdt::SyncSession) trait) and
//! `buzzy-protocol` (the wire format), but never on a concrete CRDT engine. This
//! crate routes opaque `Vec<u8>` sync payloads; it does not interpret them.

/// Per-peer sync lifecycle (see docs/mvp-lld.md "Sync state machine").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncState {
    Disconnected,
    CatchUp,
    Live,
}
