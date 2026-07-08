use serde::{Deserialize, Serialize};

/// A position-based edit as it appears on the editor socket. Serializes to the
/// tagged form the plugins send, e.g. `{"type":"insert","pos":45,"text":"hi"}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Op {
    Insert { pos: usize, text: String },
    Delete { pos: usize, len: usize },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn op_round_trips_through_json() {
        let op = Op::Insert {
            pos: 45,
            text: "hello world".into(),
        };
        let json = serde_json::to_string(&op).unwrap_or_default();
        assert_eq!(json, r#"{"type":"insert","pos":45,"text":"hello world"}"#);
        let back: Op = serde_json::from_str(&json).unwrap_or(Op::Delete { pos: 0, len: 0 });
        assert_eq!(op, back);
    }
}
