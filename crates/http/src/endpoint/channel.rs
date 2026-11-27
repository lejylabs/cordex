//! Channel endpoints.

use std::future::{Future, IntoFuture};
use std::pin::Pin;

use cordex_model::channel::{
    AutoArchiveDuration, Channel, ChannelFlags, ChannelType, DefaultReaction, ForumLayoutType,
    ForumTag, OverwriteType, PermissionOverwrite, SortOrderType, VideoQualityMode,
};
use cordex_model::id::Id;
use cordex_model::id::marker::{
    ChannelMarker, EmojiMarker, GenericMarker, GuildMarker, TagMarker,
};
use cordex_model::permissions::Permissions;
use serde::Serialize;

use crate::client::Client;
use crate::error::HttpError;
use crate::route::Route;

impl Client {
    /// `GET /channels/{channel_id}`.
    pub fn get_channel(
        &self,
        channel_id: Id<ChannelMarker>,
    ) -> impl Future<Output = Result<Channel, HttpError>> + Send + 'static {
        let client = self.clone();
        async move {
            client
                .get(Route::raw("/channels/{channel_id}", channel_id))
                .send_json()
                .await
        }
    }

    /// `PATCH /channels/{channel_id}`.
    pub fn update_channel(&self, channel_id: Id<ChannelMarker>) -> UpdateChannel {
        UpdateChannel {
            client: self.clone(),
            channel_id,
            body: UpdateChannelBody::default(),
            reason: None,
        }
    }

    /// `DELETE /channels/{channel_id}` (or close a DM).
    pub fn delete_channel(&self, channel_id: Id<ChannelMarker>) -> DeleteChannel {
        DeleteChannel {
            client: self.clone(),
            channel_id,
            reason: None,
        }
    }

    /// `PUT /channels/{channel_id}/permissions/{overwrite_id}`.
    pub fn edit_channel_permission<T>(
        &self,
        channel_id: Id<ChannelMarker>,
        overwrite_id: Id<T>,
    ) -> EditChannelPermission {
        EditChannelPermission {
            client: self.clone(),
            channel_id,
            overwrite_id: overwrite_id.cast(),
            kind: OverwriteType::Role,
            allow: Permissions::empty(),
            deny: Permissions::empty(),
            reason: None,
        }
    }

    /// `DELETE /channels/{channel_id}/permissions/{overwrite_id}`.
    pub fn delete_channel_permission<T>(
        &self,
        channel_id: Id<ChannelMarker>,
        overwrite_id: Id<T>,
    ) -> DeleteChannelPermission {
        DeleteChannelPermission {
            client: self.clone(),
            channel_id,
            overwrite_id: overwrite_id.cast(),
            reason: None,
        }
    }

    /// `POST /guilds/{guild_id}/channels`.
    pub fn create_guild_channel(
        &self,
        guild_id: Id<GuildMarker>,
        name: impl Into<String>,
    ) -> CreateGuildChannel {
        CreateGuildChannel {
            client: self.clone(),
            guild_id,
            body: CreateChannelBody {
                name: name.into(),
                ..CreateChannelBody::default()
            },
            reason: None,
        }
    }
}

/// Delete or close a channel.
pub struct DeleteChannel {
    client: Client,
    channel_id: Id<ChannelMarker>,
    reason: Option<String>,
}

impl DeleteChannel {
    /// Audit-log reason.
    pub fn reason(mut self, reason: impl Into<String>) -> Result<Self, HttpError> {
        super::set_reason(&mut self.reason, reason)?;
        Ok(self)
    }

    /// Send the request.
    pub async fn send(self) -> Result<Channel, HttpError> {
        self.client
            .delete(Route::raw("/channels/{channel_id}", self.channel_id))
            .maybe_reason(self.reason.as_deref())?
            .send_json()
            .await
    }
}

impl IntoFuture for DeleteChannel {
    type Output = Result<Channel, HttpError>;
    type IntoFuture = Pin<Box<dyn Future<Output = Self::Output> + Send>>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(self.send())
    }
}

/// Forum or media channel tag payload for creating or updating `available_tags`.
///
/// Omit `id` to create a new tag; set `id` (or convert from an existing
/// [`ForumTag`]) to preserve or update an existing tag.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ForumTagInput {
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<Id<TagMarker>>,
    name: String,
    moderated: bool,
    emoji_id: Option<Id<EmojiMarker>>,
    emoji_name: Option<String>,
}

impl ForumTagInput {
    /// Create a new tag with the given name (0–20 characters).
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            id: None,
            name: name.into(),
            moderated: false,
            emoji_id: None,
            emoji_name: None,
        }
    }

    /// Existing tag ID (set when updating an existing tag).
    #[must_use]
    pub fn id(mut self, id: Id<TagMarker>) -> Self {
        self.id = Some(id);
        self
    }

    /// Whether only moderators (`MANAGE_THREADS`) can apply or remove this tag.
    #[must_use]
    pub fn moderated(mut self, moderated: bool) -> Self {
        self.moderated = moderated;
        self
    }

    /// Custom guild emoji ID for this tag (clears any Unicode emoji name).
    #[must_use]
    pub fn emoji_id(mut self, id: Id<EmojiMarker>) -> Self {
        self.emoji_id = Some(id);
        self.emoji_name = None;
        self
    }

    /// Unicode emoji for this tag (clears any custom emoji ID).
    #[must_use]
    pub fn emoji_name(mut self, name: impl Into<String>) -> Self {
        self.emoji_id = None;
        self.emoji_name = Some(name.into());
        self
    }
}

impl From<&ForumTag> for ForumTagInput {
    fn from(tag: &ForumTag) -> Self {
        Self {
            id: Some(tag.id),
            name: tag.name.to_string(),
            moderated: tag.moderated,
            emoji_id: tag.emoji_id,
            emoji_name: tag.emoji_name.as_deref().map(str::to_owned),
        }
    }
}

impl From<ForumTag> for ForumTagInput {
    fn from(tag: ForumTag) -> Self {
        Self::from(&tag)
    }
}

#[derive(Serialize, Default)]
#[allow(clippy::option_option)] // Discord: omit vs JSON null (detach parent / clear defaults).
struct UpdateChannelBody {
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    kind: Option<ChannelType>,
    #[serde(skip_serializing_if = "Option::is_none")]
    position: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    topic: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    nsfw: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    rate_limit_per_user: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    bitrate: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    user_limit: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    parent_id: Option<Option<Id<ChannelMarker>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    archived: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    auto_archive_duration: Option<AutoArchiveDuration>,
    #[serde(skip_serializing_if = "Option::is_none")]
    locked: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    invitable: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    permission_overwrites: Option<Vec<PermissionOverwrite>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    flags: Option<ChannelFlags>,
    #[serde(skip_serializing_if = "Option::is_none")]
    applied_tags: Option<Vec<Id<TagMarker>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    available_tags: Option<Vec<ForumTagInput>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    default_reaction_emoji: Option<Option<DefaultReaction>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    default_thread_rate_limit_per_user: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    default_sort_order: Option<Option<SortOrderType>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    default_forum_layout: Option<ForumLayoutType>,
    #[serde(skip_serializing_if = "Option::is_none")]
    default_auto_archive_duration: Option<AutoArchiveDuration>,
    #[serde(skip_serializing_if = "Option::is_none")]
    rtc_region: Option<Option<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    video_quality_mode: Option<VideoQualityMode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    icon: Option<String>,
}

/// `PATCH /channels/{channel_id}`.
pub struct UpdateChannel {
    client: Client,
    channel_id: Id<ChannelMarker>,
    body: UpdateChannelBody,
    reason: Option<String>,
}

impl UpdateChannel {
    /// Channel or thread name (1–100 characters).
    pub fn name(mut self, name: impl Into<String>) -> Result<Self, HttpError> {
        let name = name.into();
        channel_name_len(&name)?;
        self.body.name = Some(name);
        Ok(self)
    }

    /// Channel type conversion (text ↔ announcement only).
    #[must_use]
    pub fn kind(mut self, kind: ChannelType) -> Self {
        self.body.kind = Some(kind);
        self
    }

    /// Replace permission overwrites (an empty list removes all overwrites).
    #[must_use]
    pub fn permission_overwrites(mut self, overwrites: Vec<PermissionOverwrite>) -> Self {
        self.body.permission_overwrites = Some(overwrites);
        self
    }

    /// Sorting position.
    #[must_use]
    pub fn position(mut self, position: i32) -> Self {
        self.body.position = Some(position);
        self
    }

    /// Topic / guidelines.
    #[must_use]
    pub fn topic(mut self, topic: impl Into<String>) -> Self {
        self.body.topic = Some(topic.into());
        self
    }

    /// Age-restricted channel.
    #[must_use]
    pub fn nsfw(mut self, nsfw: bool) -> Self {
        self.body.nsfw = Some(nsfw);
        self
    }

    /// Slowmode in seconds.
    #[must_use]
    pub fn rate_limit_per_user(mut self, seconds: u32) -> Self {
        self.body.rate_limit_per_user = Some(seconds);
        self
    }

    /// Voice bitrate.
    #[must_use]
    pub fn bitrate(mut self, bitrate: u32) -> Self {
        self.body.bitrate = Some(bitrate);
        self
    }

    /// Voice user limit.
    #[must_use]
    pub fn user_limit(mut self, limit: u32) -> Self {
        self.body.user_limit = Some(limit);
        self
    }

    /// Parent category (or `None` to detach).
    #[must_use]
    pub fn parent_id(mut self, parent_id: Option<Id<ChannelMarker>>) -> Self {
        self.body.parent_id = Some(parent_id);
        self
    }

    /// Archive or unarchive a thread.
    #[must_use]
    pub fn archived(mut self, archived: bool) -> Self {
        self.body.archived = Some(archived);
        self
    }

    /// Thread auto-archive duration.
    #[must_use]
    pub fn auto_archive_duration(mut self, duration: AutoArchiveDuration) -> Self {
        self.body.auto_archive_duration = Some(duration);
        self
    }

    /// Lock a thread.
    #[must_use]
    pub fn locked(mut self, locked: bool) -> Self {
        self.body.locked = Some(locked);
        self
    }

    /// Whether non-moderators can add members (private threads).
    #[must_use]
    pub fn invitable(mut self, invitable: bool) -> Self {
        self.body.invitable = Some(invitable);
        self
    }

    /// Channel flags (e.g. `PINNED` on a forum/media thread, `REQUIRE_TAG` on a
    /// forum channel, `HIDE_MEDIA_DOWNLOAD_OPTIONS` on a media channel).
    #[must_use]
    pub fn flags(mut self, flags: ChannelFlags) -> Self {
        self.body.flags = Some(flags);
        self
    }

    /// Sets the IDs of tags applied to a forum/media thread (max. 5 tags;
    /// an empty list clears all tags).
    #[must_use]
    pub fn applied_tags(mut self, tags: impl Into<Vec<Id<TagMarker>>>) -> Self {
        self.body.applied_tags = Some(tags.into());
        self
    }

    /// Sets the available tags on a forum/media channel (max. 20 tags).
    #[must_use]
    pub fn available_tags(mut self, tags: impl Into<Vec<ForumTagInput>>) -> Self {
        self.body.available_tags = Some(tags.into());
        self
    }

    /// Default reaction emoji for a forum/media channel (or `None` to clear).
    #[must_use]
    pub fn default_reaction_emoji(mut self, emoji: Option<DefaultReaction>) -> Self {
        self.body.default_reaction_emoji = Some(emoji);
        self
    }

    /// Initial slowmode in seconds for new threads in a text/forum/media channel (0–21600).
    #[must_use]
    pub fn default_thread_rate_limit_per_user(mut self, seconds: u32) -> Self {
        self.body.default_thread_rate_limit_per_user = Some(seconds);
        self
    }

    /// Default post sort order for a forum/media channel (or `None` to clear).
    #[must_use]
    pub fn default_sort_order(mut self, order: Option<SortOrderType>) -> Self {
        self.body.default_sort_order = Some(order);
        self
    }

    /// Default layout view for a forum channel.
    #[must_use]
    pub fn default_forum_layout(mut self, layout: ForumLayoutType) -> Self {
        self.body.default_forum_layout = Some(layout);
        self
    }

    /// Default auto-archive duration for new threads in this channel.
    #[must_use]
    pub fn default_auto_archive_duration(mut self, duration: AutoArchiveDuration) -> Self {
        self.body.default_auto_archive_duration = Some(duration);
        self
    }

    /// Voice/stage region ID (or `None` for automatic).
    #[must_use]
    pub fn rtc_region(mut self, region: Option<impl Into<String>>) -> Self {
        self.body.rtc_region = Some(region.map(Into::into));
        self
    }

    /// Camera video quality mode for a voice/stage channel.
    #[must_use]
    pub fn video_quality_mode(mut self, mode: VideoQualityMode) -> Self {
        self.body.video_quality_mode = Some(mode);
        self
    }

    /// Base64-encoded image data URI for a group DM icon.
    #[must_use]
    pub fn icon(mut self, icon_data_uri: impl Into<String>) -> Self {
        self.body.icon = Some(icon_data_uri.into());
        self
    }

    /// Audit-log reason.
    pub fn reason(mut self, reason: impl Into<String>) -> Result<Self, HttpError> {
        super::set_reason(&mut self.reason, reason)?;
        Ok(self)
    }

    /// Send the request.
    pub async fn send(self) -> Result<Channel, HttpError> {
        self.client
            .patch(Route::raw("/channels/{channel_id}", self.channel_id))
            .maybe_reason(self.reason.as_deref())?
            .json(&self.body)?
            .send_json()
            .await
    }
}

impl IntoFuture for UpdateChannel {
    type Output = Result<Channel, HttpError>;
    type IntoFuture = Pin<Box<dyn Future<Output = Self::Output> + Send>>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(self.send())
    }
}

#[derive(Serialize)]
struct OverwriteBody {
    #[serde(rename = "type")]
    kind: OverwriteType,
    allow: Permissions,
    deny: Permissions,
}

/// `PUT /channels/{channel_id}/permissions/{overwrite_id}`.
pub struct EditChannelPermission {
    client: Client,
    channel_id: Id<ChannelMarker>,
    overwrite_id: Id<GenericMarker>,
    kind: OverwriteType,
    allow: Permissions,
    deny: Permissions,
    reason: Option<String>,
}

impl EditChannelPermission {
    /// Overwrite targets a member instead of a role.
    #[must_use]
    pub fn member(mut self) -> Self {
        self.kind = OverwriteType::Member;
        self
    }

    /// Overwrite targets a role (default).
    #[must_use]
    pub fn role(mut self) -> Self {
        self.kind = OverwriteType::Role;
        self
    }

    /// Allowed permissions.
    #[must_use]
    pub fn allow(mut self, allow: Permissions) -> Self {
        self.allow = allow;
        self
    }

    /// Denied permissions.
    #[must_use]
    pub fn deny(mut self, deny: Permissions) -> Self {
        self.deny = deny;
        self
    }

    /// Audit-log reason.
    pub fn reason(mut self, reason: impl Into<String>) -> Result<Self, HttpError> {
        super::set_reason(&mut self.reason, reason)?;
        Ok(self)
    }

    /// Send the request.
    pub async fn send(self) -> Result<(), HttpError> {
        self.client
            .put(Route::raw(
                "/channels/{channel_id}/permissions/{overwrite_id}",
                (self.channel_id, self.overwrite_id),
            ))
            .maybe_reason(self.reason.as_deref())?
            .json(&OverwriteBody {
                kind: self.kind,
                allow: self.allow,
                deny: self.deny,
            })?
            .send_empty()
            .await
    }
}

impl IntoFuture for EditChannelPermission {
    type Output = Result<(), HttpError>;
    type IntoFuture = Pin<Box<dyn Future<Output = Self::Output> + Send>>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(self.send())
    }
}

/// `DELETE /channels/{channel_id}/permissions/{overwrite_id}`.
pub struct DeleteChannelPermission {
    client: Client,
    channel_id: Id<ChannelMarker>,
    overwrite_id: Id<GenericMarker>,
    reason: Option<String>,
}

impl DeleteChannelPermission {
    /// Audit-log reason.
    pub fn reason(mut self, reason: impl Into<String>) -> Result<Self, HttpError> {
        super::set_reason(&mut self.reason, reason)?;
        Ok(self)
    }

    /// Send the request.
    pub async fn send(self) -> Result<(), HttpError> {
        self.client
            .delete(Route::raw(
                "/channels/{channel_id}/permissions/{overwrite_id}",
                (self.channel_id, self.overwrite_id),
            ))
            .maybe_reason(self.reason.as_deref())?
            .send_empty()
            .await
    }
}

impl IntoFuture for DeleteChannelPermission {
    type Output = Result<(), HttpError>;
    type IntoFuture = Pin<Box<dyn Future<Output = Self::Output> + Send>>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(self.send())
    }
}

#[derive(Serialize, Default)]
struct CreateChannelBody {
    name: String,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    kind: Option<ChannelType>,
    #[serde(skip_serializing_if = "Option::is_none")]
    topic: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    bitrate: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    user_limit: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    rate_limit_per_user: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    position: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    parent_id: Option<Id<ChannelMarker>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    nsfw: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    permission_overwrites: Option<Vec<PermissionOverwrite>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    default_auto_archive_duration: Option<AutoArchiveDuration>,
    #[serde(skip_serializing_if = "Option::is_none")]
    default_reaction_emoji: Option<DefaultReaction>,
    #[serde(skip_serializing_if = "Option::is_none")]
    available_tags: Option<Vec<ForumTagInput>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    default_sort_order: Option<SortOrderType>,
    #[serde(skip_serializing_if = "Option::is_none")]
    default_forum_layout: Option<ForumLayoutType>,
    #[serde(skip_serializing_if = "Option::is_none")]
    default_thread_rate_limit_per_user: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    flags: Option<ChannelFlags>,
    #[serde(skip_serializing_if = "Option::is_none")]
    rtc_region: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    video_quality_mode: Option<VideoQualityMode>,
}

/// `POST /guilds/{guild_id}/channels`.
pub struct CreateGuildChannel {
    client: Client,
    guild_id: Id<GuildMarker>,
    body: CreateChannelBody,
    reason: Option<String>,
}

impl CreateGuildChannel {
    /// Channel type (default guild text).
    #[must_use]
    pub fn kind(mut self, kind: ChannelType) -> Self {
        self.body.kind = Some(kind);
        self
    }

    /// Initial permission overwrites for the new channel.
    #[must_use]
    pub fn permission_overwrites(mut self, overwrites: Vec<PermissionOverwrite>) -> Self {
        self.body.permission_overwrites = Some(overwrites);
        self
    }

    /// Topic.
    #[must_use]
    pub fn topic(mut self, topic: impl Into<String>) -> Self {
        self.body.topic = Some(topic.into());
        self
    }

    /// Voice bitrate.
    #[must_use]
    pub fn bitrate(mut self, bitrate: u32) -> Self {
        self.body.bitrate = Some(bitrate);
        self
    }

    /// Voice user limit.
    #[must_use]
    pub fn user_limit(mut self, limit: u32) -> Self {
        self.body.user_limit = Some(limit);
        self
    }

    /// Slowmode in seconds.
    #[must_use]
    pub fn rate_limit_per_user(mut self, seconds: u32) -> Self {
        self.body.rate_limit_per_user = Some(seconds);
        self
    }

    /// Sorting position.
    #[must_use]
    pub fn position(mut self, position: i32) -> Self {
        self.body.position = Some(position);
        self
    }

    /// Parent category.
    #[must_use]
    pub fn parent_id(mut self, parent_id: Id<ChannelMarker>) -> Self {
        self.body.parent_id = Some(parent_id);
        self
    }

    /// Age-restricted channel.
    #[must_use]
    pub fn nsfw(mut self, nsfw: bool) -> Self {
        self.body.nsfw = Some(nsfw);
        self
    }

    /// Default auto-archive duration for new threads in this channel.
    #[must_use]
    pub fn default_auto_archive_duration(mut self, duration: AutoArchiveDuration) -> Self {
        self.body.default_auto_archive_duration = Some(duration);
        self
    }

    /// Default reaction emoji for a forum/media channel.
    #[must_use]
    pub fn default_reaction_emoji(mut self, emoji: DefaultReaction) -> Self {
        self.body.default_reaction_emoji = Some(emoji);
        self
    }

    /// Initial available tags for a forum/media channel (max. 20 tags).
    #[must_use]
    pub fn available_tags(mut self, tags: impl Into<Vec<ForumTagInput>>) -> Self {
        self.body.available_tags = Some(tags.into());
        self
    }

    /// Default post sort order for a forum/media channel.
    #[must_use]
    pub fn default_sort_order(mut self, order: SortOrderType) -> Self {
        self.body.default_sort_order = Some(order);
        self
    }

    /// Default layout view for a forum channel.
    #[must_use]
    pub fn default_forum_layout(mut self, layout: ForumLayoutType) -> Self {
        self.body.default_forum_layout = Some(layout);
        self
    }

    /// Initial slowmode in seconds for new threads in this channel (0–21600).
    #[must_use]
    pub fn default_thread_rate_limit_per_user(mut self, seconds: u32) -> Self {
        self.body.default_thread_rate_limit_per_user = Some(seconds);
        self
    }

    /// Channel flags (e.g. `REQUIRE_TAG`, `HIDE_MEDIA_DOWNLOAD_OPTIONS`).
    #[must_use]
    pub fn flags(mut self, flags: ChannelFlags) -> Self {
        self.body.flags = Some(flags);
        self
    }

    /// Voice/stage region ID (omit for automatic).
    #[must_use]
    pub fn rtc_region(mut self, region: impl Into<String>) -> Self {
        self.body.rtc_region = Some(region.into());
        self
    }

    /// Camera video quality mode for a voice/stage channel.
    #[must_use]
    pub fn video_quality_mode(mut self, mode: VideoQualityMode) -> Self {
        self.body.video_quality_mode = Some(mode);
        self
    }

    /// Audit-log reason.
    pub fn reason(mut self, reason: impl Into<String>) -> Result<Self, HttpError> {
        super::set_reason(&mut self.reason, reason)?;
        Ok(self)
    }

    /// Send the request.
    pub async fn send(self) -> Result<Channel, HttpError> {
        channel_name_len(&self.body.name)?;
        self.client
            .post(Route::raw("/guilds/{guild_id}/channels", self.guild_id))
            .maybe_reason(self.reason.as_deref())?
            .json(&self.body)?
            .send_json()
            .await
    }
}

impl IntoFuture for CreateGuildChannel {
    type Output = Result<Channel, HttpError>;
    type IntoFuture = Pin<Box<dyn Future<Output = Self::Output> + Send>>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(self.send())
    }
}

fn channel_name_len(name: &str) -> Result<(), HttpError> {
    let n = name.chars().count();
    if (1..=100).contains(&n) {
        Ok(())
    } else {
        Err(HttpError::validation(
            format!("channel name is {n} characters; must be 1–100").into_boxed_str(),
        ))
    }
}

impl crate::client::Client {
    simple! {
        /// `GET /channels/{channel_id}/invites`. Needs `MANAGE_CHANNELS`.
        fn get_channel_invites(channel_id: cordex_model::id::Id<cordex_model::id::marker::ChannelMarker>)
            -> Vec<cordex_model::invite::Invite> = get "/channels/{channel_id}/invites" => send_json
    }

    simple! {
        /// `POST /channels/{channel_id}/typing`: show "is typing" for about 10 s.
        fn trigger_typing(channel_id: cordex_model::id::Id<cordex_model::id::marker::ChannelMarker>)
            -> () = post "/channels/{channel_id}/typing" => send_empty
    }

    simple! {
        /// `DELETE /channels/{channel_id}/recipients/{user_id}`: remove a user
        /// from a group DM.
        fn remove_group_dm_recipient(
            channel_id: cordex_model::id::Id<cordex_model::id::marker::ChannelMarker>,
            user_id: cordex_model::id::Id<cordex_model::id::marker::UserMarker>,
        ) -> () = delete "/channels/{channel_id}/recipients/{user_id}" => send_empty
    }

    /// `PUT /channels/{channel_id}/recipients/{user_id}`: add a user to a
    /// group DM with their `OAuth2` access token (`gdm.join` scope).
    pub fn add_group_dm_recipient(
        &self,
        channel_id: cordex_model::id::Id<cordex_model::id::marker::ChannelMarker>,
        user_id: cordex_model::id::Id<cordex_model::id::marker::UserMarker>,
        access_token: impl Into<String>,
        nick: Option<&str>,
    ) -> impl std::future::Future<Output = Result<(), crate::error::HttpError>> + Send + 'static
    {
        let client = self.clone();
        let body = serde_json::json!({ "access_token": access_token.into(), "nick": nick });
        async move {
            client
                .put(crate::route::Route::raw(
                    "/channels/{channel_id}/recipients/{user_id}",
                    (channel_id, user_id),
                ))
                .json(&body)?
                .send_empty()
                .await
        }
    }

    /// `PUT /channels/{channel_id}/voice-status`: set (or with `None`
    /// clear) a voice channel's status, max. 500 characters.
    pub fn set_voice_channel_status(
        &self,
        channel_id: cordex_model::id::Id<cordex_model::id::marker::ChannelMarker>,
        status: Option<String>,
    ) -> impl std::future::Future<Output = Result<(), crate::error::HttpError>> + Send + 'static
    {
        let client = self.clone();
        async move {
            if status.as_ref().is_some_and(|s| s.chars().count() > 500) {
                return Err(crate::error::HttpError::Validation(
                    "voice channel status is longer than 500 characters".into(),
                ));
            }
            client
                .put(crate::route::Route::raw(
                    "/channels/{channel_id}/voice-status",
                    channel_id,
                ))
                .json(&serde_json::json!({ "status": status }))?
                .send_empty()
                .await
        }
    }

    /// `POST /channels/{channel_id}/followers`: follow an announcement
    /// channel into `target` (needs `MANAGE_WEBHOOKS` there).
    pub fn follow_announcement_channel(
        &self,
        channel_id: cordex_model::id::Id<cordex_model::id::marker::ChannelMarker>,
        target: cordex_model::id::Id<cordex_model::id::marker::ChannelMarker>,
    ) -> impl std::future::Future<
        Output = Result<cordex_model::channel::FollowedChannel, crate::error::HttpError>,
    > + Send
    + 'static {
        let client = self.clone();
        async move {
            client
                .post(crate::route::Route::raw(
                    "/channels/{channel_id}/followers",
                    channel_id,
                ))
                .json(&serde_json::json!({ "webhook_channel_id": target }))?
                .send_json()
                .await
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn channel_bodies_serialize_permission_overwrites_and_empty_clear() {
        let ov = PermissionOverwrite::role(
            Id::new(1).unwrap(),
            Permissions::VIEW_CHANNEL,
            Permissions::empty(),
        );
        let create = CreateChannelBody {
            name: "private".into(),
            permission_overwrites: Some(vec![ov]),
            ..CreateChannelBody::default()
        };
        let create_json = serde_json::to_value(&create).unwrap();
        assert_eq!(
            create_json["permission_overwrites"],
            serde_json::json!([{"id": "1", "type": 0, "allow": "1024", "deny": "0"}])
        );

        let update = UpdateChannelBody {
            permission_overwrites: Some(Vec::new()),
            ..UpdateChannelBody::default()
        };
        let update_json = serde_json::to_value(&update).unwrap();
        assert_eq!(update_json["permission_overwrites"], serde_json::json!([]));
    }

    #[test]
    fn forum_channel_and_thread_fields_serialize() {
        let tag_id: Id<TagMarker> = Id::new(42).unwrap();
        let update = UpdateChannelBody {
            applied_tags: Some(vec![tag_id]),
            flags: Some(ChannelFlags::PINNED),
            default_reaction_emoji: Some(None),
            default_sort_order: Some(Some(SortOrderType::CreationDate)),
            default_forum_layout: Some(ForumLayoutType::GalleryView),
            default_thread_rate_limit_per_user: Some(15),
            default_auto_archive_duration: Some(AutoArchiveDuration::ThreeDays),
            available_tags: Some(vec![
                ForumTagInput::new("bug").moderated(true).emoji_name("🐛"),
                ForumTagInput::new("help").id(tag_id),
            ]),
            rtc_region: Some(None),
            video_quality_mode: Some(VideoQualityMode::Full),
            icon: Some("data:image/png;base64,AAAA".into()),
            ..UpdateChannelBody::default()
        };
        let json = serde_json::to_value(&update).unwrap();
        assert_eq!(json["applied_tags"], serde_json::json!(["42"]));
        assert_eq!(json["flags"], serde_json::json!(2));
        assert_eq!(json["default_reaction_emoji"], serde_json::Value::Null);
        assert_eq!(json["default_sort_order"], serde_json::json!(1));
        assert_eq!(json["default_forum_layout"], serde_json::json!(2));
        assert_eq!(json["default_thread_rate_limit_per_user"], serde_json::json!(15));
        assert_eq!(json["default_auto_archive_duration"], serde_json::json!(4320));
        assert_eq!(json["available_tags"][0]["name"], "bug");
        assert!(json["available_tags"][0].get("id").is_none());
        assert_eq!(json["available_tags"][1]["id"], "42");
        assert_eq!(json["rtc_region"], serde_json::Value::Null);
        assert_eq!(json["video_quality_mode"], serde_json::json!(2));
        assert_eq!(json["icon"], "data:image/png;base64,AAAA");

        let create = CreateChannelBody {
            name: "support".into(),
            kind: Some(ChannelType::GuildForum),
            default_reaction_emoji: Some(DefaultReaction::unicode("👍")),
            default_sort_order: Some(SortOrderType::LatestActivity),
            default_forum_layout: Some(ForumLayoutType::ListView),
            default_thread_rate_limit_per_user: Some(5),
            default_auto_archive_duration: Some(AutoArchiveDuration::Day),
            flags: Some(ChannelFlags::REQUIRE_TAG),
            available_tags: Some(vec![ForumTagInput::new("question")]),
            ..CreateChannelBody::default()
        };
        let create_json = serde_json::to_value(&create).unwrap();
        assert_eq!(create_json["type"], serde_json::json!(15));
        assert_eq!(create_json["flags"], serde_json::json!(16));
        assert_eq!(create_json["default_reaction_emoji"]["emoji_name"], "👍");
        assert_eq!(create_json["available_tags"][0]["name"], "question");
    }
}
