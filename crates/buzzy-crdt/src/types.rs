//! Engine-agnostic identifiers and operations. These deliberately avoid any
//! CRDT-library types so they can cross the trait boundary and the wire.

/// Stable per-document identifier. Backed by a string in this skeleton; becomes a
/// UUID newtype once persistence lands (see `.buzzy/index.json` in docs/mvp-lld.md).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DocId(pub String);

/// A peer's identity: the hex-encoded Ed25519 public key.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PeerId(pub String);

/// A position-based edit, expressed in absolute character offsets. Editors send
/// these; the daemon applies them via [`crate::CrdtDocument`]; the engine maps
/// them to its internal representation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Operation {
    Insert { pos: usize, text: String },
    Delete { pos: usize, len: usize },
}

/// Opaque sync payload exchanged between peers. Contents are engine-specific
/// (Automerge's Bloom-filter protocol, yrs's state vectors, …); `buzzy-net`
/// routes these bytes without interpreting them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncMessage(pub Vec<u8>);
