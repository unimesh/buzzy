//! # buzzy-crdt
//!
//! The abstraction boundary at the heart of Buzzy. Any CRDT library (Automerge,
//! Loro, yrs, or a custom engine) plugs in by implementing these traits. The
//! daemon, networking, and CLI depend only on this crate — never on a concrete
//! engine. The dependency-boundary test in `xtask/tests/architecture.rs` enforces
//! that `automerge` is only ever reachable from `buzzy-crdt-automerge`.

pub mod error;
pub mod traits;
pub mod types;

pub use error::CrdtError;
pub use traits::{CrdtDocument, CrdtEngine, SyncSession};
pub use types::{DocId, Operation, PeerId, SyncMessage};
