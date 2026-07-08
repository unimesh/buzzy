//! Length-prefixed binary framing for peer-to-peer QUIC messages.
//!
//! Frame layout (see docs/mvp-lld.md "Sync Protocol"):
//! ```text
//! bytes 0-3: payload length (u32 LE, excludes this 8-byte header)
//! bytes 4-7: message type   (u32 LE)
//! bytes 8-N: payload (message-type-specific)
//! ```

/// Fixed frame header size: 4-byte length + 4-byte type.
pub const HEADER_LEN: usize = 8;

/// Peer message types. Discriminants are the on-wire type IDs and must stay stable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum MessageType {
    Hello = 0x01,
    SyncRequest = 0x02,
    SyncResponse = 0x03,
    Change = 0x04,
    Ack = 0x05,
    Presence = 0x06,
    ShareOffer = 0x07,
    ShareAccept = 0x08,
    Goodbye = 0x09,
}

impl MessageType {
    /// Decode a wire type ID, or `None` if unknown.
    pub fn from_u32(v: u32) -> Option<Self> {
        Some(match v {
            0x01 => Self::Hello,
            0x02 => Self::SyncRequest,
            0x03 => Self::SyncResponse,
            0x04 => Self::Change,
            0x05 => Self::Ack,
            0x06 => Self::Presence,
            0x07 => Self::ShareOffer,
            0x08 => Self::ShareAccept,
            0x09 => Self::Goodbye,
            _ => return None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn message_type_ids_round_trip() {
        for ty in [
            MessageType::Hello,
            MessageType::Change,
            MessageType::Goodbye,
        ] {
            assert_eq!(MessageType::from_u32(ty as u32), Some(ty));
        }
        assert_eq!(MessageType::from_u32(0xFF), None);
    }
}
