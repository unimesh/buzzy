//! The three traits that define the CRDT engine boundary. Transcribed from
//! docs/mvp-lld.md; the concrete implementation is selected at binary composition
//! time in `buzzy-daemon/src/main.rs` and nowhere else.

use crate::error::CrdtError;

/// The core abstraction. Any CRDT library implements this to plug into Buzzy.
pub trait CrdtEngine: Send + Sync + 'static {
    type Document: CrdtDocument;
    type SyncState: SyncSession;

    /// Create a fresh document seeded with `initial_text`.
    fn create_document(&self, initial_text: &str) -> Result<Self::Document, CrdtError>;

    /// Reconstruct a document from its persisted byte form (`.buzzy/state/<uuid>.bin`).
    fn load_document(&self, bytes: &[u8]) -> Result<Self::Document, CrdtError>;

    /// Begin a sync session with a newly connected peer.
    fn create_sync_session(&self) -> Self::SyncState;
}

/// Operations on a single collaborative document.
pub trait CrdtDocument: Send + Sync {
    /// Current text content.
    fn text(&self) -> String;

    /// Apply a position-based insert.
    fn insert(&mut self, pos: usize, text: &str) -> Result<(), CrdtError>;

    /// Apply a position-based delete.
    fn delete(&mut self, pos: usize, len: usize) -> Result<(), CrdtError>;

    /// Reconcile the document with externally modified text. The engine diffs its
    /// current state against `new_content` and applies the resulting ops (for
    /// Automerge, this is `Transaction::update_text`).
    fn update_text(&mut self, new_content: &str) -> Result<(), CrdtError>;

    /// Serialize the whole document for persistence.
    fn save(&self) -> Vec<u8>;

    /// Serialize only the changes since the last save (for incremental sync).
    fn save_incremental(&mut self) -> Vec<u8>;

    /// Apply a remote change received from a peer.
    fn apply_remote_change(&mut self, bytes: &[u8]) -> Result<(), CrdtError>;

    /// The latest local change as bytes, for broadcasting to peers.
    fn latest_change(&self) -> Option<Vec<u8>>;
}

/// Per-peer sync state. Encapsulates whichever sync algorithm the engine uses.
pub trait SyncSession: Send + Sync {
    /// Produce the next sync message to send, or `None` once converged.
    fn generate_message(&mut self, doc: &dyn CrdtDocument) -> Option<Vec<u8>>;

    /// Process a sync message from the peer, applying any changes to `doc`.
    fn receive_message(&mut self, doc: &mut dyn CrdtDocument, msg: &[u8]) -> Result<(), CrdtError>;

    /// Whether both sides have converged.
    fn is_synced(&self) -> bool;
}
