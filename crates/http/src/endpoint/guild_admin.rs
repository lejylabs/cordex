//! Guild settings and sub-resources: modify guild, channel positions,
//! member list/search, prune, widget, welcome screen, onboarding,
//! integrations, voice regions, vanity URL, incident actions.

use std::future::Future;

use bytes::Bytes;
use cordex_model::guild::{
    BulkBan, Guild, GuildFeature, GuildPreview, GuildWidget, GuildWidgetSettings, Integration,
    Member, Onboarding, OnboardingMode, OnboardingPrompt, PruneCount, Role, RoleMemberCounts,
    VanityUrl, WelcomeScreen, WelcomeScreenChannel,
};
use cordex_model::guild::{
    DefaultMessageNotificationLevel, ExplicitContentFilterLevel, IncidentsData, SystemChannelFlags,
    VerificationLevel,
};
use cordex_model::id::Id;
use cordex_model::id::marker::{
    ChannelMarker, GuildMarker, IntegrationMarker, RoleMarker, UserMarker,
};
use cordex_model::timestamp::Timestamp;
use cordex_model::voice::VoiceRegion;
use http::Method;
use serde::Serialize;

use crate::client::Client;
use crate::error::HttpError;
use crate::route::Route;

json_request! {
    /// `PATCH /guilds/{guild_id}`: change guild settings. Images are data
    /// URIs; `None` removes them.
    UpdateGuild -> Guild => send_json {
        /// Name (2-100 characters).
        name: &str,
        /// Verification level.
        verification_level: Option<VerificationLevel>,
        /// Default notification level.
        default_message_notifications: Option<DefaultMessageNotificationLevel>,
        /// Explicit content filter.
        explicit_content_filter: Option<ExplicitContentFilterLevel>,
        /// AFK channel.
        afk_channel_id: Option<Id<ChannelMarker>>,
        /// AFK timeout in seconds (60, 300, 900, 1800, 3600).
        afk_timeout: u32,
        /// Icon (data URI).
        icon: Option<&str>,
        /// Splash (data URI, needs `INVITE_SPLASH`).
        splash: Option<&str>,
        /// Discovery splash (data URI, needs `DISCOVERABLE`).
        discovery_splash: Option<&str>,
        /// Banner (data URI, needs `BANNER`).
        banner: Option<&str>,
        /// System message channel.
        system_channel_id: Option<Id<ChannelMarker>>,
        /// System channel flags.
        system_channel_flags: SystemChannelFlags,
        /// Rules channel (community guilds).
        rules_channel_id: Option<Id<ChannelMarker>>,
        /// Moderator updates channel (community guilds).
        public_updates_channel_id: Option<Id<ChannelMarker>>,
        /// Preferred locale (community guilds), e.g. `"en-US"`.
        preferred_locale: Option<&str>,
        /// Enabled features (only the mutable ones change).
        features: &[GuildFeature],
        /// Description (community guilds).
        description: Option<&str>,
        /// Boost progress bar.
        premium_progress_bar_enabled: bool,
        /// Safety alerts channel (community guilds).
        safety_alerts_channel_id: Option<Id<ChannelMarker>>,
    }
}

json_request! {
    /// `PATCH /guilds/{guild_id}/members/@me`: the bot's own nickname and
    /// guild profile.
    UpdateCurrentMember -> Member => send_json {
        /// Nickname (`None` removes it).
        nick: Option<&str>,
        /// Guild banner (data URI).
        banner: Option<&str>,
        /// Guild avatar (data URI).
        avatar: Option<&str>,
        /// Guild bio.
        bio: Option<&str>,
    }
}

json_request! {
    /// `PUT /guilds/{guild_id}/members/{user_id}`: add a user with their
    /// `OAuth2` access token (`guilds.join` scope). Discord answers with
    /// the member, or with an empty body if they were already a member.
    AddGuildMember -> crate::Response => send {
        /// Nickname (needs `MANAGE_NICKNAMES`).
        nick: &str,
        /// Roles (needs `MANAGE_ROLES`).
        roles: &[Id<RoleMarker>],
        /// Server mute (needs `MUTE_MEMBERS`).
        mute: bool,
        /// Server deafen (needs `DEAFEN_MEMBERS`).
        deaf: bool,
    }
}

json_request! {
    /// `POST /guilds/{guild_id}/prune`: kick inactive members.
    BeginGuildPrune -> PruneCount => send_json {
        /// Days of inactivity (1-30, default 7).
        days: u8,
        /// Return the number of pruned members (keep `false` on large
        /// guilds).
        compute_prune_count: bool,
        /// Also prune members with these roles.
        include_roles: &[Id<RoleMarker>],
    }
}

json_request! {
    /// `PATCH /guilds/{guild_id}/widget`.
    UpdateGuildWidget -> GuildWidgetSettings => send_json {
        /// Widget on or off.
        enabled: bool,
        /// Channel the widget invite points to.
        channel_id: Option<Id<ChannelMarker>>,
    }
}

json_request! {
    /// `PATCH /guilds/{guild_id}/welcome-screen`.
    UpdateWelcomeScreen -> WelcomeScreen => send_json {
        /// Welcome screen on or off.
        enabled: bool,
        /// Shown channels (max. 5).
        welcome_channels: &[WelcomeScreenChannel],
        /// Description.
        description: Option<&str>,
    }
}

json_request! {
    /// `PUT /guilds/{guild_id}/onboarding`: replace the onboarding flow.
    UpdateOnboarding -> Onboarding => send_json {
        /// Prompts.
        prompts: &[OnboardingPrompt],
        /// Channels members get automatically.
        default_channel_ids: &[Id<ChannelMarker>],
        /// Onboarding on or off.
        enabled: bool,
        /// Mode.
        mode: OnboardingMode,
    }
}

json_request! {
    /// `PUT /guilds/{guild_id}/incident-actions`: pause invites or DMs
    /// (max. 24 h ahead; `None` lifts the pause).
    UpdateIncidentActions -> IncidentsData => send_json {
        /// Invites disabled until.
        invites_disabled_until: Option<Timestamp>,
        /// DMs between members disabled until.
        dms_disabled_until: Option<Timestamp>,
    }
}

query_request! {
    /// `GET /guilds/{guild_id}/members` (needs the `GUILD_MEMBERS` intent).
    GetGuildMembers -> Vec<Member> {
        /// Max. members (1-1000, default 1).
        limit: u16,
        /// Members after this user ID.
        after: Id<UserMarker>,
    }
}

query_request! {
    /// `GET /guilds/{guild_id}/members/search`.
    SearchGuildMembers -> Vec<Member> {
        /// Max. members (1-1000, default 1).
        limit: u16,
    }
}

query_request! {
    /// `GET /guilds/{guild_id}/prune`.
    GetGuildPruneCount -> PruneCount {
        /// Days of inactivity (1-30, default 7).
        days: u8,
    }
}

impl GetGuildPruneCount {
    /// Also count members with these roles.
    pub fn include_roles(mut self, roles: &[Id<RoleMarker>]) -> Self {
        let list: Vec<String> = roles.iter().map(ToString::to_string).collect();
        self.query.push(("include_roles", list.join(",")));
        self
    }
}

#[derive(Serialize)]
struct ChannelPosition {
    id: Id<ChannelMarker>,
    position: i32,
}

impl Client {
    simple! {
        /// `GET /guilds/{guild_id}/preview`.
        fn get_guild_preview(guild_id: Id<GuildMarker>)
            -> GuildPreview = get "/guilds/{guild_id}/preview" => send_json
    }

    /// `PATCH /guilds/{guild_id}`. Needs `MANAGE_GUILD`.
    pub fn update_guild(&self, guild_id: Id<GuildMarker>) -> UpdateGuild {
        UpdateGuild::new(
            self,
            Method::PATCH,
            Route::raw("/guilds/{guild_id}", guild_id),
        )
    }

    /// `PATCH /guilds/{guild_id}/channels`: move channels (`(id, position)`
    /// pairs).
    pub fn update_channel_positions(
        &self,
        guild_id: Id<GuildMarker>,
        positions: impl IntoIterator<Item = (Id<ChannelMarker>, i32)>,
    ) -> impl Future<Output = Result<(), HttpError>> + Send + 'static {
        let client = self.clone();
        let positions: Vec<ChannelPosition> = positions
            .into_iter()
            .map(|(id, position)| ChannelPosition { id, position })
            .collect();
        async move {
            client
                .patch(Route::raw("/guilds/{guild_id}/channels", guild_id))
                .json(&positions)?
                .send_empty()
                .await
        }
    }

    /// `GET /guilds/{guild_id}/members`.
    pub fn get_guild_members(&self, guild_id: Id<GuildMarker>) -> GetGuildMembers {
        GetGuildMembers::new(self, Route::raw("/guilds/{guild_id}/members", guild_id))
    }

    /// `GET /guilds/{guild_id}/members/search`: members whose username or
    /// nickname starts with `query`.
    pub fn search_guild_members(
        &self,
        guild_id: Id<GuildMarker>,
        query: &str,
    ) -> SearchGuildMembers {
        let mut req = SearchGuildMembers::new(
            self,
            Route::raw("/guilds/{guild_id}/members/search", guild_id),
        );
        req.query.push(("query", query.to_owned()));
        req
    }

    /// `PUT /guilds/{guild_id}/members/{user_id}`.
    pub fn add_guild_member(
        &self,
        guild_id: Id<GuildMarker>,
        user_id: Id<UserMarker>,
        access_token: &str,
    ) -> AddGuildMember {
        AddGuildMember::new(
            self,
            Method::PUT,
            Route::raw("/guilds/{guild_id}/members/{user_id}", (guild_id, user_id)),
        )
        .set("access_token", access_token)
    }

    /// `PATCH /guilds/{guild_id}/members/@me`.
    pub fn update_current_member(&self, guild_id: Id<GuildMarker>) -> UpdateCurrentMember {
        UpdateCurrentMember::new(
            self,
            Method::PATCH,
            Route::raw("/guilds/{guild_id}/members/@me", guild_id),
        )
    }

    /// `PATCH /guilds/{guild_id}/members/@me/nick` (deprecated by Discord in
    /// favour of [`Self::update_current_member`], still documented).
    pub fn update_current_user_nick(
        &self,
        guild_id: Id<GuildMarker>,
        nick: Option<&str>,
    ) -> impl Future<Output = Result<(), HttpError>> + Send + 'static {
        let client = self.clone();
        let body = serde_json::json!({ "nick": nick });
        async move {
            client
                .patch(Route::raw("/guilds/{guild_id}/members/@me/nick", guild_id))
                .json(&body)?
                .send_empty()
                .await
        }
    }

    /// `POST /guilds/{guild_id}/bulk-ban`: ban up to 200 users and delete
    /// their messages of the last `delete_message_seconds` (max. 7 days).
    pub fn bulk_ban(
        &self,
        guild_id: Id<GuildMarker>,
        user_ids: Vec<Id<UserMarker>>,
        delete_message_seconds: u32,
    ) -> impl Future<Output = Result<BulkBan, HttpError>> + Send + 'static {
        let client = self.clone();
        async move {
            if user_ids.is_empty() || user_ids.len() > 200 {
                return Err(HttpError::Validation("bulk ban takes 1-200 users".into()));
            }
            if delete_message_seconds > 604_800 {
                return Err(HttpError::Validation(
                    "delete_message_seconds is at most 604800 (7 days)".into(),
                ));
            }
            client
                .post(Route::raw("/guilds/{guild_id}/bulk-ban", guild_id))
                .json(&serde_json::json!({
                    "user_ids": user_ids,
                    "delete_message_seconds": delete_message_seconds,
                }))?
                .send_json()
                .await
        }
    }

    simple! {
        /// `GET /guilds/{guild_id}/roles/{role_id}`.
        fn get_guild_role(guild_id: Id<GuildMarker>, role_id: Id<RoleMarker>)
            -> Role = get "/guilds/{guild_id}/roles/{role_id}" => send_json
    }

    simple! {
        /// `GET /guilds/{guild_id}/roles/member-counts`: members per role
        /// (without `@everyone`).
        fn get_role_member_counts(guild_id: Id<GuildMarker>)
            -> RoleMemberCounts = get "/guilds/{guild_id}/roles/member-counts" => send_json
    }

    /// `GET /guilds/{guild_id}/prune`: how many members a prune would kick.
    pub fn get_guild_prune_count(&self, guild_id: Id<GuildMarker>) -> GetGuildPruneCount {
        GetGuildPruneCount::new(self, Route::raw("/guilds/{guild_id}/prune", guild_id))
    }

    /// `POST /guilds/{guild_id}/prune`. Needs `MANAGE_GUILD` and
    /// `KICK_MEMBERS`.
    pub fn begin_guild_prune(&self, guild_id: Id<GuildMarker>) -> BeginGuildPrune {
        BeginGuildPrune::new(
            self,
            Method::POST,
            Route::raw("/guilds/{guild_id}/prune", guild_id),
        )
    }

    simple! {
        /// `GET /guilds/{guild_id}/regions`.
        fn get_guild_voice_regions(guild_id: Id<GuildMarker>)
            -> Vec<VoiceRegion> = get "/guilds/{guild_id}/regions" => send_json
    }

    simple! {
        /// `GET /guilds/{guild_id}/integrations`.
        fn get_guild_integrations(guild_id: Id<GuildMarker>)
            -> Vec<Integration> = get "/guilds/{guild_id}/integrations" => send_json
    }

    simple! {
        /// `DELETE /guilds/{guild_id}/integrations/{integration_id}`.
        fn delete_guild_integration(guild_id: Id<GuildMarker>, integration_id: Id<IntegrationMarker>)
            -> () = delete "/guilds/{guild_id}/integrations/{integration_id}" => send_empty
    }

    simple! {
        /// `GET /guilds/{guild_id}/widget`: widget settings.
        fn get_guild_widget_settings(guild_id: Id<GuildMarker>)
            -> GuildWidgetSettings = get "/guilds/{guild_id}/widget" => send_json
    }

    /// `PATCH /guilds/{guild_id}/widget`.
    pub fn update_guild_widget(&self, guild_id: Id<GuildMarker>) -> UpdateGuildWidget {
        UpdateGuildWidget::new(
            self,
            Method::PATCH,
            Route::raw("/guilds/{guild_id}/widget", guild_id),
        )
    }

    simple! {
        /// `GET /guilds/{guild_id}/widget.json`: public widget data.
        fn get_guild_widget(guild_id: Id<GuildMarker>)
            -> GuildWidget = get "/guilds/{guild_id}/widget.json" => send_json
    }

    /// `GET /guilds/{guild_id}/widget.png`: the widget image as PNG bytes.
    pub fn get_guild_widget_image(
        &self,
        guild_id: Id<GuildMarker>,
        style: Option<&'static str>,
    ) -> impl Future<Output = Result<Bytes, HttpError>> + Send + 'static {
        let client = self.clone();
        async move {
            let mut req = client.get(Route::raw("/guilds/{guild_id}/widget.png", guild_id));
            if let Some(style) = style {
                req = req.query("style", style);
            }
            Ok(req.send().await?.body().clone())
        }
    }

    simple! {
        /// `GET /guilds/{guild_id}/vanity-url`.
        fn get_guild_vanity_url(guild_id: Id<GuildMarker>)
            -> VanityUrl = get "/guilds/{guild_id}/vanity-url" => send_json
    }

    simple! {
        /// `GET /guilds/{guild_id}/welcome-screen`.
        fn get_welcome_screen(guild_id: Id<GuildMarker>)
            -> WelcomeScreen = get "/guilds/{guild_id}/welcome-screen" => send_json
    }

    /// `PATCH /guilds/{guild_id}/welcome-screen`.
    pub fn update_welcome_screen(&self, guild_id: Id<GuildMarker>) -> UpdateWelcomeScreen {
        UpdateWelcomeScreen::new(
            self,
            Method::PATCH,
            Route::raw("/guilds/{guild_id}/welcome-screen", guild_id),
        )
    }

    simple! {
        /// `GET /guilds/{guild_id}/onboarding`.
        fn get_onboarding(guild_id: Id<GuildMarker>)
            -> Onboarding = get "/guilds/{guild_id}/onboarding" => send_json
    }

    /// `PUT /guilds/{guild_id}/onboarding`.
    pub fn update_onboarding(&self, guild_id: Id<GuildMarker>) -> UpdateOnboarding {
        UpdateOnboarding::new(
            self,
            Method::PUT,
            Route::raw("/guilds/{guild_id}/onboarding", guild_id),
        )
    }

    /// `PUT /guilds/{guild_id}/incident-actions`.
    pub fn update_incident_actions(&self, guild_id: Id<GuildMarker>) -> UpdateIncidentActions {
        UpdateIncidentActions::new(
            self,
            Method::PUT,
            Route::raw("/guilds/{guild_id}/incident-actions", guild_id),
        )
    }
}
