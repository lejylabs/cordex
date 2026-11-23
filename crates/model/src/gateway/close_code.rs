//! Gateway close codes (topics/opcodes-and-status-codes.mdx, "Gateway Close
//! Event Codes") and what a shard does after each.

use crate::serde_util::int_enum;

int_enum! {
    /// WebSocket close code received from (or sent to) the gateway.
    pub enum CloseCode: u16 {
        /// Normal closure. Sent by us, it ends the session.
        Normal = 1000,
        /// Going away. Sent by us, it ends the session.
        GoingAway = 1001,
        /// Unknown error.
        UnknownError = 4000,
        /// An invalid opcode or payload was sent (our bug).
        UnknownOpcode = 4001,
        /// An invalid payload was sent (our bug).
        DecodeError = 4002,
        /// A payload was sent before identifying, or the session was invalidated.
        NotAuthenticated = 4003,
        /// The token is incorrect.
        AuthenticationFailed = 4004,
        /// More than one identify was sent (our bug).
        AlreadyAuthenticated = 4005,
        /// The sequence sent when resuming was invalid.
        InvalidSeq = 4007,
        /// Payloads were sent too quickly.
        RateLimited = 4008,
        /// The session timed out.
        SessionTimedOut = 4009,
        /// An invalid shard was sent when identifying.
        InvalidShard = 4010,
        /// The session would handle too many guilds; sharding is required.
        ShardingRequired = 4011,
        /// Invalid gateway API version.
        InvalidApiVersion = 4012,
        /// Invalid intents (wrong bitwise value).
        InvalidIntents = 4013,
        /// Disallowed intents (privileged intent not enabled or not approved).
        DisallowedIntents = 4014,
    }
}

/// What a shard does after a close.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CloseAction {
    /// Reconnect and resume the session.
    Resume,
    /// Reconnect with a new session (identify).
    Reidentify,
    /// Do not reconnect; the configuration or token must be fixed.
    Fatal,
}

impl CloseCode {
    /// Action for this code. Unknown codes and abnormal closes resume,
    /// because Discord marks every non-fatal code as reconnectable.
    pub const fn action(self) -> CloseAction {
        match self {
            Self::AuthenticationFailed
            | Self::InvalidShard
            | Self::ShardingRequired
            | Self::InvalidApiVersion
            | Self::InvalidIntents
            | Self::DisallowedIntents => CloseAction::Fatal,
            Self::Normal
            | Self::GoingAway
            | Self::NotAuthenticated
            | Self::InvalidSeq
            | Self::SessionTimedOut => CloseAction::Reidentify,
            Self::UnknownError
            | Self::UnknownOpcode
            | Self::DecodeError
            | Self::AlreadyAuthenticated
            | Self::RateLimited
            | Self::Unknown(_) => CloseAction::Resume,
        }
    }

    /// How to fix a fatal close, for error messages; `None` for codes the
    /// gateway handles on its own.
    pub const fn hint(self) -> Option<&'static str> {
        Some(match self {
            Self::AuthenticationFailed => {
                "the bot token is wrong or was reset; copy a new one from the developer portal (Bot > Reset Token)"
            }
            Self::DisallowedIntents => {
                "a privileged intent (MESSAGE_CONTENT, GUILD_MEMBERS or GUILD_PRESENCES) is not enabled; switch it on in the developer portal (Bot > Privileged Gateway Intents) or stop requesting it"
            }
            Self::InvalidIntents => {
                "the intents value is invalid; build it from the `Intents` flags"
            }
            Self::ShardingRequired => {
                "the bot is in too many guilds for one shard; use automatic sharding (the default) or more shards"
            }
            Self::InvalidShard => {
                "the shard id or shard count is invalid; check the shard settings"
            }
            Self::InvalidApiVersion => "the gateway API version is not supported; update cordex",
            _ => return None,
        })
    }

    /// Whether the docs mark this code as reconnectable.
    pub const fn can_reconnect(self) -> bool {
        !matches!(self.action(), CloseAction::Fatal)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_fatal_code_has_a_hint() {
        for code in [4004, 4010, 4011, 4012, 4013, 4014] {
            let code = CloseCode::from(code);
            assert!(!code.can_reconnect());
            assert!(code.hint().is_some(), "{code:?}");
        }
        assert!(CloseCode::from(4000).hint().is_none());
    }

    #[test]
    fn fatal_codes_match_the_docs() {
        // "Reconnect" column = false in the docs table.
        for code in [4004, 4010, 4011, 4012, 4013, 4014] {
            assert_eq!(CloseCode::from(code).action(), CloseAction::Fatal, "{code}");
        }
        for code in [4000, 4001, 4002, 4003, 4005, 4007, 4008, 4009] {
            assert!(CloseCode::from(code).can_reconnect(), "{code}");
        }
    }

    #[test]
    fn new_session_codes() {
        for code in [1000, 1001, 4003, 4007, 4009] {
            assert_eq!(
                CloseCode::from(code).action(),
                CloseAction::Reidentify,
                "{code}"
            );
        }
        assert_eq!(CloseCode::from(4999).action(), CloseAction::Resume);
        assert_eq!(CloseCode::from(1006), CloseCode::Unknown(1006));
    }
}
