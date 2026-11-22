//! Every model against the JSON examples of the official docs
//! (`tests/fixtures/*.json`, extracted from discord/discord-api-docs
//! commit 9b737ec, plus filled `application_id` on the slash interaction
//! example). Each example must parse, and serialize → parse must
//! give the same value back.
//!
//! Fixes applied to the docs examples (docs bugs, not model bugs):
//! - `channel_voice.json`: trailing comma removed (invalid JSON in the docs).
//! - `guild_member.json`: the docs use `"user": {}`; the test fills in a user.

use cordex_model::channel::{ChannelType, OverwriteType};
use cordex_model::emoji::Emoji;
use cordex_model::guild::{
    DefaultMessageNotificationLevel, GuildFeature, IncidentsData, MfaLevel, PremiumTier,
    VerificationLevel, WelcomeScreen,
};
use cordex_model::sticker::{StickerFormatType, StickerType};
use cordex_model::user::UserFlags;
use cordex_model::{Channel, Guild, Member, Permissions, Role, Sticker, UnavailableGuild, User};
use serde::Serialize;
use serde::de::DeserializeOwned;

/// Parses, re-serializes and parses again; both values must be equal.
/// (A helper, not a `#[test]`, so it panics with messages instead of `unwrap`.)
#[track_caller]
fn round_trip<T: DeserializeOwned + Serialize + PartialEq + std::fmt::Debug>(json: &str) -> T {
    let value: T = serde_json::from_str(json).unwrap_or_else(|e| panic!("parse: {e}"));
    let out = serde_json::to_string(&value).unwrap_or_else(|e| panic!("serialize: {e}"));
    let again: T = serde_json::from_str(&out).unwrap_or_else(|e| panic!("re-parse: {e}\n{out}"));
    assert_eq!(value, again);
    value
}

#[test]
fn user() {
    let user: User = round_trip(include_str!("fixtures/user.json"));
    assert_eq!(user.username.as_ref(), "Nelly");
    assert_eq!(user.public_flags, Some(UserFlags::HYPESQUAD_ONLINE_HOUSE_1));
    assert_eq!(user.accent_color, Some(16_711_680));
}

#[test]
fn role() {
    let role: Role = round_trip(include_str!("fixtures/role.json"));
    assert_eq!(role.name.as_ref(), "WE DEM BOYZZ!!!!!!");
    assert!(role.hoist);
    assert_eq!(role.position, 1);
    assert_eq!(role.permissions, Permissions::from_bits_retain(66_321_471));
    assert_eq!(role.mention(), "<@&41771983423143936>");
}

#[test]
fn guild() {
    let guild: Guild = round_trip(include_str!("fixtures/guild.json"));
    assert_eq!(guild.name.as_ref(), "Discord Testers");
    assert_eq!(guild.verification_level, VerificationLevel::High);
    assert_eq!(
        guild.default_message_notifications,
        DefaultMessageNotificationLevel::OnlyMentions
    );
    assert_eq!(guild.mfa_level, MfaLevel::Elevated);
    assert_eq!(guild.premium_tier, PremiumTier::Tier3);
    assert!(guild.has_feature(&GuildFeature::Community));
    // "MORE_EMOJI" is in the docs example but not in the feature list.
    assert!(guild.has_feature(&GuildFeature::Unknown("MORE_EMOJI".into())));
    assert_eq!(guild.max_members, Some(250_000));
}

#[test]
fn unavailable_guild() {
    let guild: UnavailableGuild = round_trip(include_str!("fixtures/unavailable_guild.json"));
    assert!(guild.unavailable);
}

#[test]
fn guild_member() {
    let json = include_str!("fixtures/guild_member.json").replace(
        r#""user": {}"#,
        r#""user": {"id": "1", "username": "x", "discriminator": "0", "global_name": null, "avatar": null}"#,
    );
    let member: Member = round_trip(&json);
    assert_eq!(member.display_name(), Some("NOT API SUPPORT"));
    assert!(member.joined_at.is_some());
    assert!(!member.deaf && !member.mute);
}

#[test]
fn welcome_screen_and_incidents() {
    let screen: WelcomeScreen = round_trip(include_str!("fixtures/welcome_screen.json"));
    assert_eq!(screen.welcome_channels.len(), 5);
    assert_eq!(screen.welcome_channels[0].emoji_name.as_deref(), Some("📡"));

    let incidents: IncidentsData = round_trip(include_str!("fixtures/incidents_data.json"));
    assert!(incidents.invites_disabled_until.is_some());
    assert!(incidents.dms_disabled_until.is_none());
}

#[test]
fn channels() {
    let cases: [(&str, ChannelType); 7] = [
        (
            include_str!("fixtures/channel_text.json"),
            ChannelType::GuildText,
        ),
        (
            include_str!("fixtures/channel_announcement.json"),
            ChannelType::GuildAnnouncement,
        ),
        (
            include_str!("fixtures/channel_voice.json"),
            ChannelType::GuildVoice,
        ),
        (include_str!("fixtures/channel_dm.json"), ChannelType::Dm),
        (
            include_str!("fixtures/channel_group_dm.json"),
            ChannelType::GroupDm,
        ),
        (
            include_str!("fixtures/channel_category.json"),
            ChannelType::GuildCategory,
        ),
        (
            include_str!("fixtures/channel_thread.json"),
            ChannelType::PublicThread,
        ),
    ];
    for (json, kind) in cases {
        let channel: Channel = round_trip(json);
        assert_eq!(channel.kind, kind);
    }
}

#[test]
fn channel_details() {
    let group: Channel = round_trip(include_str!("fixtures/channel_group_dm.json"));
    assert_eq!(group.recipients.len(), 2);

    let thread: Channel = round_trip(include_str!("fixtures/channel_thread.json"));
    assert!(thread.is_thread());
    let meta = thread.thread_metadata.unwrap();
    assert!(!meta.archived && !meta.locked);

    let text: Channel = round_trip(include_str!("fixtures/channel_text.json"));
    for overwrite in &text.permission_overwrites {
        assert!(matches!(
            overwrite.kind,
            OverwriteType::Role | OverwriteType::Member
        ));
    }
}

#[test]
fn emojis() {
    let custom: Emoji = round_trip(include_str!("fixtures/emoji.json"));
    assert_eq!(custom.roles.len(), 2);
    assert_eq!(custom.mention(), "<:LUL:41771983429993937>");

    let unicode: Emoji = round_trip(include_str!("fixtures/emoji_standard.json"));
    assert!(unicode.is_unicode());
    assert_eq!(unicode.mention(), "🔥");

    // Reaction event forms from the docs ("Custom Emoji Examples").
    let animated: Emoji =
        round_trip(r#"{"id": "41771983429993937", "name": "LUL", "animated": true}"#);
    assert_eq!(animated.mention(), "<a:LUL:41771983429993937>");
    let deleted: Emoji = round_trip(r#"{"id": "41771983429993937", "name": null}"#);
    assert!(deleted.name.is_none());
}

#[test]
fn sticker() {
    let sticker: Sticker = round_trip(include_str!("fixtures/sticker.json"));
    assert_eq!(sticker.kind, StickerType::Standard);
    assert_eq!(sticker.format_type, StickerFormatType::Lottie);
    assert_eq!(sticker.sort_value, Some(12));
    // The docs example has escaped surrogate pairs in `tags`.
    assert!(sticker.tags.contains('👋'));
}

#[test]
fn message() {
    use cordex_model::message::MessageType;
    let msg: cordex_model::Message = round_trip(include_str!("fixtures/message.json"));
    assert_eq!(msg.kind, MessageType::Default);
    assert_eq!(msg.content.as_ref(), "Supa Hot");
    assert_eq!(msg.reactions[0].count_details.normal, 1);
    assert!(msg.reactions[0].emoji.is_unicode());
    assert!(
        msg.author
            .avatar
            .as_ref()
            .is_some_and(cordex_model::ImageHash::is_animated)
    );
    assert_eq!(
        msg.link(),
        "https://discord.com/channels/@me/290926798999357250/334385199974967042"
    );
}

#[test]
fn crossposted_message() {
    use cordex_model::message::MessageFlags;
    let msg: cordex_model::Message = round_trip(include_str!("fixtures/message_crosspost.json"));
    assert!(msg.flags.contains(MessageFlags::IS_CROSSPOST));
    let reference = msg.message_reference.unwrap();
    assert!(reference.message_id.is_some() && reference.channel_id.is_some());
}

#[test]
fn application() {
    use cordex_model::application::Application;
    let app: Application = round_trip(include_str!("fixtures/application.json"));
    assert!(!app.name.is_empty());
    assert!(!app.verify_key.is_empty());
}

#[test]
fn application_command() {
    use cordex_model::command::{ApplicationCommand, CommandOptionType, CommandType};

    let cmd: ApplicationCommand = round_trip(include_str!("fixtures/application_command.json"));
    assert_eq!(cmd.kind, CommandType::ChatInput);
    assert_eq!(cmd.name.as_ref(), "blep");
    assert_eq!(cmd.options.len(), 2);
    assert_eq!(cmd.options[0].kind, CommandOptionType::String);
    assert_eq!(cmd.options[0].choices.len(), 3);
    assert!(cmd.options[0].required);
    assert_eq!(cmd.options[1].kind, CommandOptionType::Boolean);
}

#[test]
fn interactions() {
    use cordex_model::command::CommandType;
    use cordex_model::interaction::{Interaction, InteractionData, InteractionType};

    // Docs slash example omits `application_id`; filled like guild_member's `"user": {}`.
    let slash_json = include_str!("fixtures/interaction_slash.json").replace(
        r#""id": "786008729715212338""#,
        r#""application_id": "1", "id": "786008729715212338""#,
    );
    let slash: Interaction = round_trip(&slash_json);
    assert_eq!(slash.kind, InteractionType::ApplicationCommand);
    let InteractionData::Command(data) = slash.data.as_ref().unwrap() else {
        panic!("expected command data");
    };
    assert_eq!(&*data.name, "cardsearch");
    assert_eq!(
        data.option("cardname").unwrap().value.as_ref().unwrap(),
        &cordex_model::interaction::CommandOptionValue::String("The Gitrog Monster".into())
    );
    assert_eq!(slash.author().unwrap().username.as_ref(), "Mason");
    assert!(!format!("{slash:?}").contains("A_UNIQUE_TOKEN"));

    let user: Interaction = round_trip(include_str!("fixtures/interaction_user.json"));
    let InteractionData::Command(data) = user.data.as_ref().unwrap() else {
        panic!("expected command data");
    };
    assert_eq!(data.kind, CommandType::User);
    assert!(data.target_id.is_some());
    assert_eq!(data.resolved.as_ref().unwrap().users.len(), 1);

    let message: Interaction = round_trip(include_str!("fixtures/interaction_message.json"));
    let InteractionData::Command(data) = message.data.as_ref().unwrap() else {
        panic!("expected command data");
    };
    assert_eq!(data.kind, CommandType::Message);
    assert_eq!(data.resolved.as_ref().unwrap().messages.len(), 1);
}

#[test]
fn entitlement() {
    use cordex_model::entitlement::{Entitlement, EntitlementType};

    let ent: Entitlement = round_trip(include_str!("fixtures/entitlement.json"));
    assert_eq!(ent.kind, EntitlementType::ApplicationSubscription);
    assert!(!ent.deleted);
    assert_eq!(ent.consumed, Some(false));
}

#[test]
fn voice_state() {
    use cordex_model::voice::VoiceState;

    let state: VoiceState = round_trip(include_str!("fixtures/voice_state.json"));
    assert!(state.is_connected());
    assert!(state.self_mute);
    assert!(!state.deaf);
}

#[test]
fn invite() {
    use cordex_model::invite::{Invite, InviteTargetType, InviteType};

    let invite: Invite = round_trip(include_str!("fixtures/invite.json"));
    assert_eq!(invite.kind, InviteType::Guild);
    assert_eq!(&*invite.code, "0vCdhLbwjZZTWZLD");
    assert_eq!(invite.target_type, Some(InviteTargetType::Stream));
    assert_eq!(
        invite.guild.as_ref().unwrap().name.as_ref(),
        "CS:GO Fraggers Only"
    );
    assert!(invite.expires_at.is_none());
}

#[test]
fn auto_moderation_rule() {
    use cordex_model::auto_moderation::{
        AutoModerationActionType, AutoModerationEventType, AutoModerationRule,
        AutoModerationTriggerType,
    };

    let rule: AutoModerationRule = round_trip(include_str!("fixtures/auto_moderation_rule.json"));
    assert_eq!(rule.trigger_type, AutoModerationTriggerType::Keyword);
    assert_eq!(rule.event_type, AutoModerationEventType::MessageSend);
    assert_eq!(rule.actions.len(), 3);
    assert_eq!(rule.actions[0].kind, AutoModerationActionType::BlockMessage);
    assert_eq!(rule.trigger_metadata.keyword_filter.len(), 4);
    assert!(rule.enabled);
}

#[test]
fn webhooks() {
    use cordex_model::webhook::{Webhook, WebhookType};

    let incoming: Webhook = round_trip(include_str!("fixtures/webhook_incoming.json"));
    assert_eq!(incoming.kind, WebhookType::Incoming);
    assert!(incoming.token.is_some());
    assert!(!format!("{incoming:?}").contains("3d89bb7572e0fb30"));

    let follower: Webhook = round_trip(include_str!("fixtures/webhook_follower.json"));
    assert_eq!(follower.kind, WebhookType::ChannelFollower);
    assert_eq!(
        follower.source_channel.as_ref().unwrap().name.as_deref(),
        Some("announcements")
    );

    let app: Webhook = round_trip(include_str!("fixtures/webhook_application.json"));
    assert_eq!(app.kind, WebhookType::Application);
    assert!(app.channel_id.is_none() && app.guild_id.is_none());
}

#[test]
fn shared_client_theme() {
    use cordex_model::message::SharedClientTheme;

    // The docs example wraps the object in its message field name.
    #[derive(serde::Deserialize, serde::Serialize, PartialEq, Debug)]
    struct Wrapper {
        shared_client_theme: SharedClientTheme,
    }
    let theme = round_trip::<Wrapper>(include_str!("fixtures/shared_client_theme.json"))
        .shared_client_theme;
    assert_eq!(theme.colors.len(), 5);
    assert_eq!(
        theme.base_theme,
        Some(cordex_model::message::BaseTheme::Dark)
    );
}

#[test]
fn stage_instance() {
    use cordex_model::stage::{StageInstance, StagePrivacyLevel};

    let stage: StageInstance = round_trip(include_str!("fixtures/stage_instance.json"));
    assert_eq!(stage.privacy_level, StagePrivacyLevel::Public);
    assert_eq!(&*stage.topic, "Testing Testing, 123");
    assert!(stage.guild_scheduled_event_id.is_some());
}

#[test]
fn soundboard() {
    use cordex_model::soundboard::SoundboardSound;

    let default: SoundboardSound = round_trip(include_str!("fixtures/soundboard_default.json"));
    assert_eq!(&*default.name, "quack");
    assert_eq!(default.emoji_name.as_deref(), Some("🦆"));
    assert!(default.guild_id.is_none());

    let guild: SoundboardSound = round_trip(include_str!("fixtures/soundboard_guild.json"));
    assert_eq!(&*guild.name, "Yay");
    assert!(guild.guild_id.is_some());
}

#[test]
fn presence_activity() {
    use cordex_model::gateway::presence::{ActivityType, PresenceActivity};

    let simple: PresenceActivity = round_trip(include_str!("fixtures/activity.json"));
    assert_eq!(simple.kind, ActivityType::Streaming);
    assert_eq!(&*simple.name, "Twitch");

    let rich: PresenceActivity = round_trip(include_str!("fixtures/activity_rich.json"));
    assert_eq!(rich.kind, ActivityType::Playing);
    assert_eq!(rich.party.as_ref().unwrap().size, Some([2, 2]));
    assert!(!format!("{rich:?}").contains("025ed05c71f639de"));
}

#[test]
fn integration_partial() {
    use cordex_model::guild::{Integration, IntegrationType};

    let integration: Integration = round_trip(include_str!("fixtures/integration_partial.json"));
    assert_eq!(integration.kind, IntegrationType::Twitch);
    assert_eq!(
        integration.account.as_ref().unwrap().name.as_ref(),
        "twitchusername"
    );
}
