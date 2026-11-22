//! Message reaction endpoints.

use std::future::{Future, IntoFuture};
use std::pin::Pin;

use drs_model::id::Id;
use drs_model::id::marker::{ChannelMarker, MessageMarker, UserMarker};

use crate::client::Client;
use crate::error::HttpError;
use crate::route::Route;

impl Client {
    /// `PUT /channels/{channel_id}/messages/{message_id}/reactions/{emoji}/@me`.
    ///
    /// `emoji` is a Unicode emoji or `name:id` for a custom emoji.
    pub fn create_reaction(
        &self,
        channel_id: Id<ChannelMarker>,
        message_id: Id<MessageMarker>,
        emoji: impl Into<String>,
    ) -> impl Future<Output = Result<(), HttpError>> + Send + 'static {
        let client = self.clone();
        let emoji = emoji.into();
        async move {
            client
                .put(Route::raw(
                    "/channels/{channel_id}/messages/{message_id}/reactions/{emoji}/@me",
                    (channel_id, message_id, emoji),
                ))
                .send_empty()
                .await
        }
    }

    /// `DELETE /channels/{channel_id}/messages/{message_id}/reactions/{emoji}/@me`.
    pub fn delete_own_reaction(
        &self,
        channel_id: Id<ChannelMarker>,
        message_id: Id<MessageMarker>,
        emoji: impl Into<String>,
    ) -> impl Future<Output = Result<(), HttpError>> + Send + 'static {
        let client = self.clone();
        let emoji = emoji.into();
        async move {
            client
                .delete(Route::raw(
                    "/channels/{channel_id}/messages/{message_id}/reactions/{emoji}/@me",
                    (channel_id, message_id, emoji),
                ))
                .send_empty()
                .await
        }
    }

    /// `DELETE /channels/{channel_id}/messages/{message_id}/reactions/{emoji}/{user_id}`.
    pub fn delete_user_reaction(
        &self,
        channel_id: Id<ChannelMarker>,
        message_id: Id<MessageMarker>,
        emoji: impl Into<String>,
        user_id: Id<UserMarker>,
    ) -> DeleteUserReaction {
        DeleteUserReaction {
            client: self.clone(),
            channel_id,
            message_id,
            emoji: emoji.into(),
            user_id,
        }
    }

    /// `DELETE /channels/{channel_id}/messages/{message_id}/reactions`.
    pub fn delete_all_reactions(
        &self,
        channel_id: Id<ChannelMarker>,
        message_id: Id<MessageMarker>,
    ) -> impl Future<Output = Result<(), HttpError>> + Send + 'static {
        let client = self.clone();
        async move {
            client
                .delete(Route::raw(
                    "/channels/{channel_id}/messages/{message_id}/reactions",
                    (channel_id, message_id),
                ))
                .send_empty()
                .await
        }
    }

    /// `DELETE /channels/{channel_id}/messages/{message_id}/reactions/{emoji}`.
    pub fn delete_all_reactions_for_emoji(
        &self,
        channel_id: Id<ChannelMarker>,
        message_id: Id<MessageMarker>,
        emoji: impl Into<String>,
    ) -> impl Future<Output = Result<(), HttpError>> + Send + 'static {
        let client = self.clone();
        let emoji = emoji.into();
        async move {
            client
                .delete(Route::raw(
                    "/channels/{channel_id}/messages/{message_id}/reactions/{emoji}",
                    (channel_id, message_id, emoji),
                ))
                .send_empty()
                .await
        }
    }
}

/// Delete another user's reaction.
pub struct DeleteUserReaction {
    client: Client,
    channel_id: Id<ChannelMarker>,
    message_id: Id<MessageMarker>,
    emoji: String,
    user_id: Id<UserMarker>,
}

impl DeleteUserReaction {
    /// Send the request.
    pub async fn send(self) -> Result<(), HttpError> {
        self.client
            .delete(Route::raw(
                "/channels/{channel_id}/messages/{message_id}/reactions/{emoji}/{user_id}",
                (self.channel_id, self.message_id, self.emoji, self.user_id),
            ))
            .send_empty()
            .await
    }
}

impl IntoFuture for DeleteUserReaction {
    type Output = Result<(), HttpError>;
    type IntoFuture = Pin<Box<dyn Future<Output = Self::Output> + Send>>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(self.send())
    }
}
