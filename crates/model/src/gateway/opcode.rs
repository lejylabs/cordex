//! Gateway opcodes (topics/opcodes-and-status-codes.mdx, "Gateway Opcodes").

use crate::serde_util::int_enum;

int_enum! {
    /// Gateway opcode of a payload.
    pub enum OpCode: u8 {
        /// An event was dispatched (receive).
        Dispatch = 0,
        /// Keep the connection alive (send and receive).
        Heartbeat = 1,
        /// Start a new session (send).
        Identify = 2,
        /// Update the client's presence (send).
        PresenceUpdate = 3,
        /// Join, leave or move between voice channels (send; payload only, no voice).
        VoiceStateUpdate = 4,
        /// Resume a previous session (send).
        Resume = 6,
        /// Reconnect and resume immediately (receive).
        Reconnect = 7,
        /// Request guild members (send).
        RequestGuildMembers = 8,
        /// The session has been invalidated (receive).
        InvalidSession = 9,
        /// First payload after connecting, carries the heartbeat interval (receive).
        Hello = 10,
        /// Acknowledges a heartbeat (receive).
        HeartbeatAck = 11,
        /// Request soundboard sounds of guilds (send).
        RequestSoundboardSounds = 31,
        /// Request ephemeral channel data of a guild (send).
        RequestChannelInfo = 43,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_and_unknown() {
        assert_eq!(serde_json::from_str::<OpCode>("10").unwrap(), OpCode::Hello);
        assert_eq!(
            serde_json::to_string(&OpCode::RequestChannelInfo).unwrap(),
            "43"
        );
        assert_eq!(
            serde_json::from_str::<OpCode>("99").unwrap(),
            OpCode::Unknown(99)
        );
        assert_eq!(u8::from(OpCode::Unknown(99)), 99);
    }
}
