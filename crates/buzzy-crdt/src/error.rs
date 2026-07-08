use thiserror::Error;

/// Errors surfaced across the CRDT trait boundary. Kept engine-agnostic: concrete
/// engines map their internal errors into these variants so callers never see
/// Automerge/Loro/yrs types.
#[derive(Debug, Error)]
pub enum CrdtError {
    #[error("position {pos} out of bounds (document length {len})")]
    OutOfBounds { pos: usize, len: usize },

    #[error("failed to load document: {0}")]
    Load(String),

    #[error("failed to apply remote change: {0}")]
    ApplyRemote(String),

    #[error("engine error: {0}")]
    Engine(String),
}
