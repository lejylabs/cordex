//! RAII typing indicator guard (`Client::start_typing`).

use std::time::Duration;

use cordex_model::id::Id;
use cordex_model::id::marker::ChannelMarker;

use crate::client::Client;

/// Interval between typing indicator refreshes (Discord shows typing for 10 s).
const TYPING_INTERVAL: Duration = Duration::from_secs(7);

/// Keeps Discord's "bot is typing…" indicator active in a channel until
/// dropped (or [`stop`](Self::stop) is called).
#[derive(Debug)]
pub struct Typing {
    handle: tokio::task::JoinHandle<()>,
}

impl Typing {
    pub(crate) fn start(client: Client, channel_id: Id<ChannelMarker>) -> Self {
        let handle = tokio::spawn(async move {
            loop {
                if let Err(error) = client.trigger_typing(channel_id).await {
                    tracing::debug!(%error, %channel_id, "typing indicator failed");
                    break;
                }
                tokio::time::sleep(TYPING_INTERVAL).await;
            }
        });
        Self { handle }
    }

    /// Stop sending typing indicators immediately.
    pub fn stop(self) {
        self.handle.abort();
    }
}

impl Drop for Typing {
    fn drop(&mut self) {
        self.handle.abort();
    }
}
