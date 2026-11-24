//! Abstraction over the gateway command sender used to dispatch opcode 4
//! (`VOICE_STATE_UPDATE`).

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use cordex_model::gateway::command::UpdateVoiceState;

use crate::error::VoiceError;

/// Boxed future returned by [`VoiceGateway::send_voice_state`].
pub type BoxVoiceFuture<'a> = Pin<Box<dyn Future<Output = Result<(), VoiceError>> + Send + 'a>>;

/// Minimal gateway interface required by [`crate::VoiceManager`].
///
/// Decouples voice signaling from any specific shard or cluster implementation.
/// When the `gateway` feature is enabled (default), this trait is implemented
/// for [`cordex_gateway::Shard`] and [`cordex_gateway::ShardManager`].
pub trait VoiceGateway: Send + Sync {
    /// Send an opcode 4 (`VOICE_STATE_UPDATE`) payload for `update.guild_id`.
    fn send_voice_state(&self, update: UpdateVoiceState) -> BoxVoiceFuture<'_>;
}

impl<T: VoiceGateway + ?Sized> VoiceGateway for Arc<T> {
    fn send_voice_state(&self, update: UpdateVoiceState) -> BoxVoiceFuture<'_> {
        (**self).send_voice_state(update)
    }
}

#[cfg(feature = "gateway")]
impl<C: cordex_gateway::Connector + 'static> VoiceGateway for cordex_gateway::Shard<C> {
    fn send_voice_state(&self, update: UpdateVoiceState) -> BoxVoiceFuture<'_> {
        Box::pin(async move {
            self.update_voice_state(&update)
                .await
                .map_err(VoiceError::from)
        })
    }
}

#[cfg(feature = "gateway")]
impl<C: cordex_gateway::Connector + 'static> VoiceGateway for cordex_gateway::ShardManager<C> {
    fn send_voice_state(&self, update: UpdateVoiceState) -> BoxVoiceFuture<'_> {
        Box::pin(async move {
            self.update_voice_state(&update)
                .await
                .map_err(VoiceError::from)
        })
    }
}
