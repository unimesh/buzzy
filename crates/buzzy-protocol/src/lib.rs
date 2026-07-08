//! # buzzy-protocol
//!
//! Types shared across the two Buzzy protocols: the editor-facing JSON-RPC socket
//! ([`rpc`]) and the peer-facing QUIC wire format ([`wire`]). Engine-agnostic — no
//! CRDT-library types appear here.

pub mod ops;
pub mod rpc;
pub mod wire;

/// Socket JSON-RPC protocol version. Bump on any breaking change to the editor
/// surface so plugins can negotiate. Kept explicit from day one to keep the
/// backward-compatibility path (docs/mvp-lld.md) real rather than aspirational.
pub const PROTOCOL_VERSION: u32 = 1;
