//! Error types for `cordex-voice`.

use cordex_model::id::Id;
use cordex_model::id::marker::GuildMarker;
use thiserror::Error;

/// Errors returned by [`crate::VoiceManager`] and [`crate::VoiceGateway`].
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum VoiceError {
    /// Sending an opcode 4 (`VOICE_STATE_UPDATE`) command to the gateway failed.
    #[error("gateway voice command failed: {source}")]
    Gateway {
        /// Underlying gateway error.
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },
    /// Timed out waiting for Discord to complete the voice handshake.
    #[error("timed out waiting for voice connection in guild {guild_id}")]
    Timeout {
        /// Guild whose voice connection timed out.
        guild_id: Id<GuildMarker>,
    },
    /// The bot was disconnected from voice while waiting for a connection.
    #[error("disconnected from voice in guild {guild_id}")]
    Disconnected {
        /// Guild where the disconnect occurred.
        guild_id: Id<GuildMarker>,
    },
    /// The voice manager's internal watch channel closed unexpectedly.
    #[error("voice manager closed")]
    Closed,
}

impl VoiceError {
    /// Wrap any gateway error into [`VoiceError::Gateway`].
    pub fn gateway(source: impl std::error::Error + Send + Sync + 'static) -> Self {
        Self::Gateway {
            source: Box::new(source),
        }
    }
}

#[cfg(feature = "gateway")]
impl From<cordex_gateway::GatewayError> for VoiceError {
    fn from(err: cordex_gateway::GatewayError) -> Self {
        Self::gateway(err)
    }
}
