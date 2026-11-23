//! Social SDK lobbies (resources/lobby.mdx).

use std::collections::HashMap;
use std::future::Future;

use cordex_model::id::Id;
use cordex_model::id::marker::{ChannelMarker, LobbyMarker, MessageMarker, UserMarker};
use cordex_model::lobby::{Lobby, LobbyInvite, LobbyMember, LobbyMemberFlags, LobbyMessage};
use cordex_model::message::MessageFlags;
use http::Method;
use serde::Serialize;

use crate::client::Client;
use crate::error::HttpError;
use crate::route::Route;

/// Max members per create / modify / bulk request.
pub const LOBBY_MEMBERS_PER_REQUEST: usize = 25;
/// Min idle timeout (seconds).
pub const LOBBY_IDLE_TIMEOUT_MIN: u32 = 5;
/// Max idle timeout (seconds, 7 days).
pub const LOBBY_IDLE_TIMEOUT_MAX: u32 = 604_800;
/// Max combined metadata string length.
pub const LOBBY_METADATA_MAX: usize = 1000;

json_request! {
    /// `POST /lobbies`.
    CreateLobby -> Lobby => send_json {
        /// Lobby metadata (max total length 1000).
        metadata: Option<HashMap<Box<str>, Box<str>>>,
        /// Up to 25 members.
        members: &[LobbyMemberWrite],
        /// Idle shutdown delay (5–604800).
        idle_timeout_seconds: u32,
    }
}

json_request! {
    /// `PUT /lobbies`.
    CreateOrJoinLobby -> Lobby => send_json {
        /// Secret identifying the lobby (max 250 characters).
        secret: &str,
        /// Idle shutdown delay (5–604800).
        idle_timeout_seconds: u32,
        /// Overwrites lobby metadata.
        lobby_metadata: Option<HashMap<Box<str>, Box<str>>>,
        /// Metadata for the calling member.
        member_metadata: Option<HashMap<Box<str>, Box<str>>>,
    }
}

json_request! {
    /// `PATCH /lobbies/{lobby_id}`.
    ModifyLobby -> Lobby => send_json {
        /// Overwrites lobby metadata.
        metadata: Option<HashMap<Box<str>, Box<str>>>,
        /// Replacement member list (up to 25).
        members: &[LobbyMemberWrite],
        /// Idle shutdown delay (5–604800).
        idle_timeout_seconds: u32,
    }
}

json_request! {
    /// `PUT /lobbies/{lobby_id}/members/{user_id}`.
    AddLobbyMember -> LobbyMember => send_json {
        /// Member metadata.
        metadata: Option<HashMap<Box<str>, Box<str>>>,
        /// Member flags.
        flags: LobbyMemberFlags,
        /// Extra display name (1–80); `None` clears it.
        additional_name: Option<&str>,
    }
}

json_request! {
    /// `PATCH /lobbies/{lobby_id}/channel-linking`.
    LinkLobbyChannel -> Lobby => send_json {
        /// Channel to link; omit to unlink.
        channel_id: Id<ChannelMarker>,
    }
}

json_request! {
    /// `POST /lobbies/{lobby_id}/messages`.
    SendLobbyMessage -> LobbyMessage => send_json {
        /// Non-empty content.
        content: &str,
        /// Dispatch-only metadata.
        metadata: Option<HashMap<Box<str>, Box<str>>>,
        /// Message flags (only Social SDK creatable flags).
        flags: MessageFlags,
    }
}

query_request! {
    /// `GET /lobbies/{lobby_id}/messages`.
    GetLobbyMessages -> Vec<LobbyMessage> {
        /// Page size (1–200, Discord default 50).
        limit: u8,
    }
}

/// Body for create / modify / bulk member writes.
#[derive(Debug, Clone, Serialize)]
pub struct LobbyMemberWrite {
    /// User to add or update.
    pub id: Id<UserMarker>,
    /// Member metadata.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<HashMap<Box<str>, Box<str>>>,
    /// Member flags.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flags: Option<LobbyMemberFlags>,
    /// Extra display name; `Some(None)` serializes as JSON null to clear.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub additional_name: Option<Option<Box<str>>>,
    /// When `true`, bulk update removes this user.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remove_member: Option<bool>,
}

impl Client {
    /// `POST /lobbies`.
    pub fn create_lobby(&self) -> CreateLobby {
        CreateLobby::new(self, Method::POST, Route::new("/lobbies"))
    }

    /// `PUT /lobbies`.
    pub fn create_or_join_lobby(&self) -> CreateOrJoinLobby {
        CreateOrJoinLobby::new(self, Method::PUT, Route::new("/lobbies"))
    }

    simple! {
        /// `GET /lobbies/{lobby_id}`.
        fn get_lobby(lobby_id: Id<LobbyMarker>) -> Lobby = get "/lobbies/{lobby_id}" => send_json
    }

    /// `PATCH /lobbies/{lobby_id}`.
    pub fn modify_lobby(&self, lobby_id: Id<LobbyMarker>) -> ModifyLobby {
        ModifyLobby::new(
            self,
            Method::PATCH,
            Route::raw("/lobbies/{lobby_id}", lobby_id),
        )
    }

    simple! {
        /// `DELETE /lobbies/{lobby_id}`.
        fn delete_lobby(lobby_id: Id<LobbyMarker>) -> () = delete "/lobbies/{lobby_id}" => send_empty
    }

    /// `PUT /lobbies/{lobby_id}/members/{user_id}`.
    pub fn add_lobby_member(
        &self,
        lobby_id: Id<LobbyMarker>,
        user_id: Id<UserMarker>,
    ) -> AddLobbyMember {
        AddLobbyMember::new(
            self,
            Method::PUT,
            Route::raw("/lobbies/{lobby_id}/members/{user_id}", (lobby_id, user_id)),
        )
    }

    /// `POST /lobbies/{lobby_id}/members/bulk`.
    pub fn bulk_update_lobby_members(
        &self,
        lobby_id: Id<LobbyMarker>,
        members: Vec<LobbyMemberWrite>,
    ) -> impl Future<Output = Result<Vec<LobbyMember>, HttpError>> + Send + 'static {
        let client = self.clone();
        async move {
            if members.is_empty() || members.len() > LOBBY_MEMBERS_PER_REQUEST {
                return Err(HttpError::Validation(
                    "bulk lobby member updates take 1-25 members".into(),
                ));
            }
            client
                .post(Route::raw("/lobbies/{lobby_id}/members/bulk", lobby_id))
                .json(&members)?
                .send_json()
                .await
        }
    }

    simple! {
        /// `DELETE /lobbies/{lobby_id}/members/{user_id}`.
        fn remove_lobby_member(lobby_id: Id<LobbyMarker>, user_id: Id<UserMarker>)
            -> () = delete "/lobbies/{lobby_id}/members/{user_id}" => send_empty
    }

    simple! {
        /// `DELETE /lobbies/{lobby_id}/members/@me`.
        fn leave_lobby(lobby_id: Id<LobbyMarker>)
            -> () = delete "/lobbies/{lobby_id}/members/@me" => send_empty
    }

    /// `PATCH /lobbies/{lobby_id}/channel-linking`.
    pub fn link_lobby_channel(&self, lobby_id: Id<LobbyMarker>) -> LinkLobbyChannel {
        LinkLobbyChannel::new(
            self,
            Method::PATCH,
            Route::raw("/lobbies/{lobby_id}/channel-linking", lobby_id),
        )
    }

    /// `POST /lobbies/{lobby_id}/messages`.
    pub fn send_lobby_message(&self, lobby_id: Id<LobbyMarker>) -> SendLobbyMessage {
        SendLobbyMessage::new(
            self,
            Method::POST,
            Route::raw("/lobbies/{lobby_id}/messages", lobby_id),
        )
    }

    /// `GET /lobbies/{lobby_id}/messages`.
    pub fn get_lobby_messages(&self, lobby_id: Id<LobbyMarker>) -> GetLobbyMessages {
        GetLobbyMessages::new(self, Route::raw("/lobbies/{lobby_id}/messages", lobby_id))
    }

    /// `PUT /lobbies/{lobby_id}/messages/{message_id}/moderation-metadata`.
    pub fn update_lobby_message_moderation_metadata(
        &self,
        lobby_id: Id<LobbyMarker>,
        message_id: Id<MessageMarker>,
        metadata: HashMap<Box<str>, Box<str>>,
    ) -> impl Future<Output = Result<(), HttpError>> + Send + 'static {
        let client = self.clone();
        async move {
            if metadata.len() > 5 {
                return Err(HttpError::Validation(
                    "lobby message moderation metadata allows at most 5 keys".into(),
                ));
            }
            client
                .put(Route::raw(
                    "/lobbies/{lobby_id}/messages/{message_id}/moderation-metadata",
                    (lobby_id, message_id),
                ))
                .json(&metadata)?
                .send_empty()
                .await
        }
    }

    simple! {
        /// `POST /lobbies/{lobby_id}/members/@me/invites`.
        fn create_lobby_channel_invite_for_self(lobby_id: Id<LobbyMarker>)
            -> LobbyInvite = post "/lobbies/{lobby_id}/members/@me/invites" => send_json
    }

    simple! {
        /// `POST /lobbies/{lobby_id}/members/{user_id}/invites`.
        fn create_lobby_channel_invite_for_user(
            lobby_id: Id<LobbyMarker>,
            user_id: Id<UserMarker>,
        ) -> LobbyInvite = post "/lobbies/{lobby_id}/members/{user_id}/invites" => send_json
    }
}
