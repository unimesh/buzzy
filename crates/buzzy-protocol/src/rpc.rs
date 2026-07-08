//! JSON-RPC 2.0 message shapes for the editor <-> daemon Unix socket. Messages are
//! newline-delimited on the wire (see docs/mvp-lld.md "Socket Protocol").

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The `"jsonrpc"` version string every message carries.
pub const JSONRPC_VERSION: &str = "2.0";

/// A request or notification from an editor. A `None` `id` marks a notification
/// (fire-and-forget, no response expected).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Request {
    pub jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<u64>,
    pub method: String,
    #[serde(default)]
    pub params: Value,
}

/// A response from the daemon. Exactly one of `result` / `error` is set.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Response {
    pub jsonrpc: String,
    pub id: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<RpcError>,
}

/// JSON-RPC error object.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcError {
    pub code: i64,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}
