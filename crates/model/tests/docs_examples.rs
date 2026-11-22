//! Every model against the JSON examples of the official docs
//! (`tests/fixtures/*.json`, extracted from discord/discord-api-docs
//! commit 9b737ec). Each example must parse, and serialize → parse must
//! give the same value back.
//!
//! Fixes applied to the docs examples (docs bugs, not model bugs):
//! - `channel_voice.json`: trailing comma removed (invalid JSON in the docs).
//! - `guild_member.json`: the docs use `"user": {}`; the test fills in a user.

use drs_model::channel::{ChannelType, OverwriteType};
use drs_model::emoji::Emoji;
use drs_model::guild::{
    DefaultMessageNotificationLevel, GuildFeature, IncidentsData, MfaLevel, PremiumTier,
    VerificationLevel, WelcomeScreen,
};
use drs_model::sticker::{StickerFormatType, StickerType};
use drs_model::user::UserFlags;
use drs_model::{Channel, Guild, Member, Permissions, Role, Sticker, UnavailableGuild, User};
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
