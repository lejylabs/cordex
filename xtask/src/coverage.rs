//! `cargo xtask coverage`: compare the library with the official API docs.
//!
//! The docs come from `discord/discord-api-docs` at the commit pinned in
//! `docs/discord-api-docs.rev`, downloaded as a tarball by hash into
//! `target/discord-api-docs/<sha>` (needs `curl` and `tar`). Measured today:
//!
//! - REST endpoints (`<Route method="…">` in the docs, the
//!   `` `METHOD /path` `` doc comments in `cordex-http`),
//! - gateway receive events, gateway opcodes, gateway close codes,
//! - component types and the value tables of the model enums in
//!   [`ENUMS`],
//! - object fields ([`FIELDS`]) and bitflags ([`FLAGS`]); docs tables that
//!   are not mapped yet are listed in the report.
//!
//! Flags: `--check` fails if `docs/coverage.md` is out of date,
//! `--rev <sha|latest>` measures another docs commit and writes the report
//! to `target/coverage-<rev>.md` instead (used by the docs watch workflow),
//! `--pin` moves the pin to the measured commit.

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::{read, workspace_root};

const DOCS_REPO: &str = "https://github.com/discord/discord-api-docs";
const REV_FILE: &str = "docs/discord-api-docs.rev";
const REPORT: &str = "docs/coverage.md";

/// Docs folders that describe the bot API (changes there go into the
/// issue checklist).
const BOT_PAGES: &[&str] = &[
    "developers/resources/",
    "developers/events/",
    "developers/interactions/",
    "developers/components/",
    "developers/topics/",
    "developers/change-log.mdx",
];

/// Docs field tables compared with model structs: (file in
/// `crates/model/src`, struct, docs page, heading). An empty struct means
/// cordex has no type for it yet (every field counts as missing).
const FIELDS: &[(&str, &str, &str, &str)] = &[
    (
        "component.rs",
        "RadioGroup",
        "components/reference.mdx",
        "Radio Group Structure",
    ),
    (
        "component.rs",
        "ChoiceOption",
        "components/reference.mdx",
        "Radio Group Option Structure",
    ),
    (
        "interaction.rs",
        "SubmittedComponent",
        "components/reference.mdx",
        "Radio Group Interaction Response Structure",
    ),
    (
        "component.rs",
        "CheckboxGroup",
        "components/reference.mdx",
        "Checkbox Group Structure",
    ),
    (
        "component.rs",
        "ChoiceOption",
        "components/reference.mdx",
        "Checkbox Group Option Structure",
    ),
    (
        "interaction.rs",
        "SubmittedComponent",
        "components/reference.mdx",
        "Checkbox Group Interaction Response Structure",
    ),
    (
        "component.rs",
        "Checkbox",
        "components/reference.mdx",
        "Checkbox Structure",
    ),
    (
        "interaction.rs",
        "SubmittedComponent",
        "components/reference.mdx",
        "Checkbox Interaction Response Structure",
    ),
    (
        "gateway/payload.rs",
        "Envelope",
        "events/gateway-events.mdx",
        "Payload Structure",
    ),
    (
        "message/mod.rs",
        "MessageReference",
        "resources/message.mdx",
        "Message Reference Structure",
    ),
    (
        "message/mod.rs",
        "MessageSnapshot",
        "resources/message.mdx",
        "Message Snapshot Structure",
    ),
    (
        "webhook_event.rs",
        "WebhookEventPayload",
        "events/webhook-events.mdx",
        "Payload Structure",
    ),
    (
        "webhook_event.rs",
        "WebhookEventBody",
        "events/webhook-events.mdx",
        "Event Body Object",
    ),
    (
        "component.rs",
        "ActionRow",
        "components/reference.mdx",
        "Action Row Structure",
    ),
    (
        "component.rs",
        "Button",
        "components/reference.mdx",
        "Button Structure",
    ),
    (
        "component.rs",
        "StringSelect",
        "components/reference.mdx",
        "String Select Structure",
    ),
    (
        "component.rs",
        "SelectOption",
        "components/reference.mdx",
        "Select Option Structure",
    ),
    (
        "interaction.rs",
        "MessageComponentData+SubmittedComponent",
        "components/reference.mdx",
        "String Select Interaction Response Structure",
    ),
    (
        "component.rs",
        "TextInput",
        "components/reference.mdx",
        "Text Input Structure",
    ),
    (
        "interaction.rs",
        "SubmittedComponent",
        "components/reference.mdx",
        "Text Input Interaction Response Structure",
    ),
    (
        "component.rs",
        "SelectMenu",
        "components/reference.mdx",
        "User Select Structure",
    ),
    (
        "component.rs",
        "SelectDefaultValue",
        "components/reference.mdx",
        "Select Default Value Structure",
    ),
    (
        "interaction.rs",
        "MessageComponentData+SubmittedComponent",
        "components/reference.mdx",
        "User Select Interaction Response Structure",
    ),
    (
        "component.rs",
        "SelectMenu",
        "components/reference.mdx",
        "Role Select Structure",
    ),
    (
        "interaction.rs",
        "MessageComponentData+SubmittedComponent",
        "components/reference.mdx",
        "Role Select Interaction Response Structure",
    ),
    (
        "component.rs",
        "SelectMenu",
        "components/reference.mdx",
        "Mentionable Select Structure",
    ),
    (
        "interaction.rs",
        "MessageComponentData+SubmittedComponent",
        "components/reference.mdx",
        "Mentionable Select Interaction Response Structure",
    ),
    (
        "component.rs",
        "SelectMenu",
        "components/reference.mdx",
        "Channel Select Structure",
    ),
    (
        "interaction.rs",
        "MessageComponentData+SubmittedComponent",
        "components/reference.mdx",
        "Channel Select Interaction Response Structure",
    ),
    (
        "component.rs",
        "Section",
        "components/reference.mdx",
        "Section Structure",
    ),
    (
        "component.rs",
        "TextDisplay",
        "components/reference.mdx",
        "Text Display Structure",
    ),
    (
        "interaction.rs",
        "SubmittedComponent",
        "components/reference.mdx",
        "Text Display Interaction Response Structure",
    ),
    (
        "component.rs",
        "Thumbnail",
        "components/reference.mdx",
        "Thumbnail Structure",
    ),
    (
        "component.rs",
        "MediaGallery",
        "components/reference.mdx",
        "Media Gallery Structure",
    ),
    (
        "component.rs",
        "MediaGalleryItem",
        "components/reference.mdx",
        "Media Gallery Item Structure",
    ),
    (
        "component.rs",
        "FileComponent",
        "components/reference.mdx",
        "File Structure",
    ),
    (
        "component.rs",
        "Separator",
        "components/reference.mdx",
        "Separator Structure",
    ),
    (
        "component.rs",
        "Container",
        "components/reference.mdx",
        "Container Structure",
    ),
    (
        "component.rs",
        "Label",
        "components/reference.mdx",
        "Label Structure",
    ),
    (
        "interaction.rs",
        "SubmittedComponent",
        "components/reference.mdx",
        "Label Interaction Response Structure",
    ),
    (
        "component.rs",
        "FileUpload",
        "components/reference.mdx",
        "File Upload Structure",
    ),
    (
        "interaction.rs",
        "SubmittedComponent",
        "components/reference.mdx",
        "File Upload Interaction Response Structure",
    ),
    (
        "component.rs",
        "UnfurledMediaItem",
        "components/reference.mdx",
        "Unfurled Media Item Structure",
    ),
    (
        "component.rs",
        "ContentScanMetadata",
        "components/reference.mdx",
        "Content Scan Metadata Structure",
    ),
    (
        "gateway/event.rs",
        "Ready",
        "events/gateway-events.mdx",
        "Ready Event Fields",
    ),
    (
        "auto_moderation.rs",
        "AutoModerationActionExecution",
        "events/gateway-events.mdx",
        "Auto Moderation Action Execution Event Fields",
    ),
    (
        "gateway/event.rs",
        "ThreadListSync",
        "events/gateway-events.mdx",
        "Thread List Sync Event Fields",
    ),
    (
        "channel.rs",
        "ThreadMember",
        "events/gateway-events.mdx",
        "Thread Member Update Event Extra Fields",
    ),
    (
        "gateway/event.rs",
        "ThreadMembersUpdate",
        "events/gateway-events.mdx",
        "Thread Members Update Event Fields",
    ),
    (
        "gateway/event.rs",
        "ChannelPinsUpdate",
        "events/gateway-events.mdx",
        "Channel Pins Update Event Fields",
    ),
    (
        "gateway/event.rs",
        "AvailableGuild",
        "events/gateway-events.mdx",
        "Guild Create Extra Fields",
    ),
    (
        "audit_log.rs",
        "AuditLogEntry",
        "events/gateway-events.mdx",
        "Guild Audit Log Entry Create Event Extra Fields",
    ),
    (
        "gateway/event.rs",
        "GuildBanAdd",
        "events/gateway-events.mdx",
        "Guild Ban Add Event Fields",
    ),
    (
        "gateway/event.rs",
        "GuildBanAdd",
        "events/gateway-events.mdx",
        "Guild Ban Remove Event Fields",
    ),
    (
        "gateway/event.rs",
        "GuildEmojisUpdate",
        "events/gateway-events.mdx",
        "Guild Emojis Update Event Fields",
    ),
    (
        "gateway/event.rs",
        "GuildStickersUpdate",
        "events/gateway-events.mdx",
        "Guild Stickers Update Event Fields",
    ),
    (
        "gateway/event.rs",
        "GuildIntegrationsUpdate",
        "events/gateway-events.mdx",
        "Guild Integrations Update Event Fields",
    ),
    (
        "guild/member.rs",
        "Member",
        "events/gateway-events.mdx",
        "Guild Member Add Extra Fields",
    ),
    (
        "gateway/event.rs",
        "GuildMemberRemove",
        "events/gateway-events.mdx",
        "Guild Member Remove Event Fields",
    ),
    (
        "gateway/event.rs",
        "GuildMemberUpdate",
        "events/gateway-events.mdx",
        "Guild Member Update Event Fields",
    ),
    (
        "gateway/event.rs",
        "GuildMembersChunk",
        "events/gateway-events.mdx",
        "Guild Members Chunk Event Fields",
    ),
    (
        "gateway/event.rs",
        "GuildRoleCreate",
        "events/gateway-events.mdx",
        "Guild Role Create Event Fields",
    ),
    (
        "gateway/event.rs",
        "GuildRoleCreate",
        "events/gateway-events.mdx",
        "Guild Role Update Event Fields",
    ),
    (
        "gateway/event.rs",
        "GuildRoleDelete",
        "events/gateway-events.mdx",
        "Guild Role Delete Event Fields",
    ),
    (
        "scheduled_event.rs",
        "GuildScheduledEventUserAdd",
        "events/gateway-events.mdx",
        "Guild Scheduled Event User Add Event Fields",
    ),
    (
        "scheduled_event.rs",
        "GuildScheduledEventUserAdd",
        "events/gateway-events.mdx",
        "Guild Scheduled Event User Remove Event Fields",
    ),
    (
        "soundboard.rs",
        "GuildSoundboardSoundDelete",
        "events/gateway-events.mdx",
        "Guild Soundboard Sound Delete Event Fields",
    ),
    (
        "soundboard.rs",
        "SoundboardSounds",
        "events/gateway-events.mdx",
        "Guild Soundboard Sounds Update Event Fields",
    ),
    (
        "soundboard.rs",
        "SoundboardSounds",
        "events/gateway-events.mdx",
        "Soundboard Sounds Event Fields",
    ),
    (
        "gateway/event.rs",
        "IntegrationEvent",
        "events/gateway-events.mdx",
        "Integration Create Event Additional Fields",
    ),
    (
        "gateway/event.rs",
        "IntegrationEvent",
        "events/gateway-events.mdx",
        "Integration Update Event Additional Fields",
    ),
    (
        "gateway/event.rs",
        "IntegrationDelete",
        "events/gateway-events.mdx",
        "Integration Delete Event Fields",
    ),
    (
        "invite.rs",
        "InviteCreate",
        "events/gateway-events.mdx",
        "Invite Create Event Fields",
    ),
    (
        "invite.rs",
        "InviteDelete",
        "events/gateway-events.mdx",
        "Invite Delete Event Fields",
    ),
    (
        "message/mod.rs",
        "Message",
        "events/gateway-events.mdx",
        "Message Create Extra Fields",
    ),
    (
        "gateway/event.rs",
        "MessageDelete",
        "events/gateway-events.mdx",
        "Message Delete Event Fields",
    ),
    (
        "gateway/event.rs",
        "MessageDeleteBulk",
        "events/gateway-events.mdx",
        "Message Delete Bulk Event Fields",
    ),
    (
        "gateway/event.rs",
        "MessageReactionAdd",
        "events/gateway-events.mdx",
        "Message Reaction Add Event Fields",
    ),
    (
        "gateway/event.rs",
        "MessageReactionRemove",
        "events/gateway-events.mdx",
        "Message Reaction Remove Event Fields",
    ),
    (
        "gateway/event.rs",
        "MessageReactionRemoveAll",
        "events/gateway-events.mdx",
        "Message Reaction Remove All Event Fields",
    ),
    (
        "gateway/event.rs",
        "MessageReactionRemoveEmoji",
        "events/gateway-events.mdx",
        "Message Reaction Remove Emoji Event Fields",
    ),
    (
        "gateway/presence.rs",
        "PresenceUpdate",
        "events/gateway-events.mdx",
        "Presence Update Event Fields",
    ),
    (
        "gateway/event.rs",
        "TypingStart",
        "events/gateway-events.mdx",
        "Typing Start Event Fields",
    ),
    (
        "soundboard.rs",
        "VoiceChannelEffectSend",
        "events/gateway-events.mdx",
        "Voice Channel Effect Send Event Fields",
    ),
    (
        "gateway/event.rs",
        "VoiceServerUpdate",
        "events/gateway-events.mdx",
        "Voice Server Update Event Fields",
    ),
    (
        "gateway/event.rs",
        "WebhooksUpdate",
        "events/gateway-events.mdx",
        "Webhooks Update Event Fields",
    ),
    (
        "poll.rs",
        "MessagePollVote",
        "events/gateway-events.mdx",
        "Message Poll Vote Add Fields",
    ),
    (
        "poll.rs",
        "MessagePollVote",
        "events/gateway-events.mdx",
        "Message Poll Vote Remove Fields",
    ),
    (
        "gateway/event.rs",
        "RateLimited",
        "events/gateway-events.mdx",
        "Rate Limited Fields",
    ),
    (
        "webhook_event.rs",
        "ApplicationAuthorized",
        "events/webhook-events.mdx",
        "Application Authorized Structure",
    ),
    (
        "webhook_event.rs",
        "ApplicationDeauthorized",
        "events/webhook-events.mdx",
        "Application Deauthorized Structure",
    ),
    (
        "lobby.rs",
        "LobbyMessageDelete",
        "events/webhook-events.mdx",
        "Lobby Message Delete Structure",
    ),
    (
        "lobby.rs",
        "LobbyMessage",
        "events/webhook-events.mdx",
        "Lobby Message Structure",
    ),
    (
        "lobby.rs",
        "SdkDmMessage",
        "events/webhook-events.mdx",
        "SDK DM Message Structure",
    ),
    (
        "message/mod.rs",
        "Message",
        "events/webhook-events.mdx",
        "Additional Fields",
    ),
    (
        "lobby.rs",
        "Lobby",
        "resources/lobby.mdx",
        "Lobby Structure",
    ),
    (
        "lobby.rs",
        "LobbyMember",
        "resources/lobby.mdx",
        "Lobby Member Structure",
    ),
    (
        "identity_profile.rs",
        "ApplicationIdentity",
        "resources/application-identity-profile.mdx",
        "Application Identity Structure",
    ),
    (
        "identity_profile.rs",
        "ApplicationIdentityProfile",
        "resources/application-identity-profile.mdx",
        "Application Identity Profile Structure",
    ),
    (
        "identity_profile.rs",
        "ProfileData",
        "resources/application-identity-profile.mdx",
        "Profile Data Structure",
    ),
    (
        "identity_profile.rs",
        "PrimaryProfileData",
        "resources/application-identity-profile.mdx",
        "Primary Profile Data Structure",
    ),
    (
        "identity_profile.rs",
        "DynamicStringField",
        "resources/application-identity-profile.mdx",
        "Dynamic String Field Structure",
    ),
    (
        "identity_profile.rs",
        "DynamicNumberField",
        "resources/application-identity-profile.mdx",
        "Dynamic Number Field Structure",
    ),
    (
        "identity_profile.rs",
        "DynamicMediaField",
        "resources/application-identity-profile.mdx",
        "Dynamic Media Field Structure",
    ),
    (
        "identity_profile.rs",
        "Media",
        "resources/application-identity-profile.mdx",
        "Media Structure",
    ),
    (
        "gateway/bot.rs",
        "SessionStartLimit",
        "events/gateway.mdx",
        "Session Start Limit Structure",
    ),
    (
        "application.rs",
        "RoleConnectionMetadata",
        "resources/application-role-connection-metadata.mdx",
        "Application Role Connection Metadata Structure",
    ),
    ("user.rs", "User", "resources/user.mdx", "User Structure"),
    (
        "user.rs",
        "AvatarDecorationData",
        "resources/user.mdx",
        "Avatar Decoration Data Structure",
    ),
    (
        "user.rs",
        "Collectibles",
        "resources/user.mdx",
        "Collectible Structure",
    ),
    (
        "user.rs",
        "Nameplate",
        "resources/user.mdx",
        "Nameplate Structure",
    ),
    (
        "user.rs",
        "Connection",
        "resources/user.mdx",
        "Connection Structure",
    ),
    (
        "user.rs",
        "ApplicationRoleConnection",
        "resources/user.mdx",
        "Application Role Connection Structure",
    ),
    (
        "guild/mod.rs",
        "Guild",
        "resources/guild.mdx",
        "Guild Structure",
    ),
    (
        "guild/extra.rs",
        "GuildPreview",
        "resources/guild.mdx",
        "Guild Preview Structure",
    ),
    (
        "guild/extra.rs",
        "GuildWidgetSettings",
        "resources/guild.mdx",
        "Guild Widget Settings Structure",
    ),
    (
        "guild/extra.rs",
        "GuildWidget",
        "resources/guild.mdx",
        "Guild Widget Structure",
    ),
    (
        "guild/member.rs",
        "Member",
        "resources/guild.mdx",
        "Guild Member Structure",
    ),
    (
        "guild/integration.rs",
        "Integration",
        "resources/guild.mdx",
        "Integration Structure",
    ),
    (
        "guild/integration.rs",
        "IntegrationAccount",
        "resources/guild.mdx",
        "Integration Account Structure",
    ),
    (
        "application.rs",
        "Application",
        "resources/guild.mdx",
        "Integration Application Structure",
    ),
    (
        "guild/member.rs",
        "Ban",
        "resources/guild.mdx",
        "Ban Structure",
    ),
    (
        "guild/mod.rs",
        "WelcomeScreen",
        "resources/guild.mdx",
        "Welcome Screen Structure",
    ),
    (
        "guild/mod.rs",
        "WelcomeScreenChannel",
        "resources/guild.mdx",
        "Welcome Screen Channel Structure",
    ),
    (
        "guild/extra.rs",
        "Onboarding",
        "resources/guild.mdx",
        "Guild Onboarding Structure",
    ),
    (
        "guild/extra.rs",
        "OnboardingPrompt",
        "resources/guild.mdx",
        "Onboarding Prompt Structure",
    ),
    (
        "guild/extra.rs",
        "OnboardingPromptOption",
        "resources/guild.mdx",
        "Prompt Option Structure",
    ),
    (
        "guild/mod.rs",
        "IncidentsData",
        "resources/guild.mdx",
        "Incidents Data Structure",
    ),
    (
        "channel.rs",
        "Channel",
        "resources/channel.mdx",
        "Channel Structure",
    ),
    (
        "channel.rs",
        "FollowedChannel",
        "resources/channel.mdx",
        "Followed Channel Structure",
    ),
    (
        "channel.rs",
        "PermissionOverwrite",
        "resources/channel.mdx",
        "Overwrite Structure",
    ),
    (
        "channel.rs",
        "ThreadMetadata",
        "resources/channel.mdx",
        "Thread Metadata Structure",
    ),
    (
        "channel.rs",
        "ThreadMember",
        "resources/channel.mdx",
        "Thread Member Structure",
    ),
    (
        "channel.rs",
        "DefaultReaction",
        "resources/channel.mdx",
        "Default Reaction Structure",
    ),
    (
        "channel.rs",
        "ForumTag",
        "resources/channel.mdx",
        "Forum Tag Structure",
    ),
    (
        "message/mod.rs",
        "Message",
        "resources/message.mdx",
        "Message Structure",
    ),
    (
        "message/mod.rs",
        "MessageActivity",
        "resources/message.mdx",
        "Message Activity Structure",
    ),
    (
        "interaction.rs",
        "InteractionMetadata",
        "resources/message.mdx",
        "Application Command Interaction Metadata Structure",
    ),
    (
        "interaction.rs",
        "InteractionMetadata",
        "resources/message.mdx",
        "Message Component Interaction Metadata Structure",
    ),
    (
        "interaction.rs",
        "InteractionMetadata",
        "resources/message.mdx",
        "Modal Submit Interaction Metadata Structure",
    ),
    (
        "message/mod.rs",
        "MessageCall",
        "resources/message.mdx",
        "Message Call Object Structure",
    ),
    (
        "message/mod.rs",
        "Reaction",
        "resources/message.mdx",
        "Reaction Structure",
    ),
    (
        "message/mod.rs",
        "ReactionCountDetails",
        "resources/message.mdx",
        "Reaction Count Details Structure",
    ),
    (
        "message/embed.rs",
        "Embed",
        "resources/message.mdx",
        "Embed Structure",
    ),
    (
        "message/embed.rs",
        "EmbedVideo",
        "resources/message.mdx",
        "Embed Video Structure",
    ),
    (
        "message/embed.rs",
        "EmbedMedia",
        "resources/message.mdx",
        "Embed Image Structure",
    ),
    (
        "message/embed.rs",
        "EmbedProvider",
        "resources/message.mdx",
        "Embed Provider Structure",
    ),
    (
        "message/embed.rs",
        "EmbedAuthor",
        "resources/message.mdx",
        "Embed Author Structure",
    ),
    (
        "message/embed.rs",
        "EmbedFooter",
        "resources/message.mdx",
        "Embed Footer Structure",
    ),
    (
        "message/embed.rs",
        "EmbedField",
        "resources/message.mdx",
        "Embed Field Structure",
    ),
    (
        "message/attachment.rs",
        "Attachment",
        "resources/message.mdx",
        "Attachment Structure",
    ),
    (
        "message/mod.rs",
        "ChannelMention",
        "resources/message.mdx",
        "Channel Mention Structure",
    ),
    (
        "message/mod.rs",
        "RoleSubscriptionData",
        "resources/message.mdx",
        "Role Subscription Data Object Structure",
    ),
    (
        "message/mod.rs",
        "MessagePin",
        "resources/message.mdx",
        "Message Pin Object Structure",
    ),
    (
        "message/mod.rs",
        "SharedClientTheme",
        "resources/message.mdx",
        "Shared Client Theme Object Structure",
    ),
    (
        "emoji.rs",
        "Emoji",
        "resources/emoji.mdx",
        "Emoji Structure",
    ),
    (
        "sticker.rs",
        "Sticker",
        "resources/sticker.mdx",
        "Sticker Structure",
    ),
    (
        "sticker.rs",
        "StickerItem",
        "resources/sticker.mdx",
        "Sticker Item Structure",
    ),
    (
        "sticker.rs",
        "StickerPack",
        "resources/sticker.mdx",
        "Sticker Pack Structure",
    ),
    (
        "invite.rs",
        "Invite",
        "resources/invite.mdx",
        "Invite Structure",
    ),
    (
        "invite.rs",
        "Invite",
        "resources/invite.mdx",
        "Invite Metadata Structure",
    ),
    (
        "webhook.rs",
        "Webhook",
        "resources/webhook.mdx",
        "Webhook Structure",
    ),
    (
        "voice.rs",
        "VoiceState",
        "resources/voice.mdx",
        "Voice State Structure",
    ),
    (
        "voice.rs",
        "VoiceRegion",
        "resources/voice.mdx",
        "Voice Region Structure",
    ),
    (
        "application.rs",
        "Application",
        "resources/application.mdx",
        "Application Structure",
    ),
    (
        "application.rs",
        "InstallParams",
        "resources/application.mdx",
        "Install Params Structure",
    ),
    (
        "auto_moderation.rs",
        "AutoModerationRule",
        "resources/auto-moderation.mdx",
        "Auto Moderation Rule Structure",
    ),
    (
        "auto_moderation.rs",
        "AutoModerationAction",
        "resources/auto-moderation.mdx",
        "Auto Moderation Action Structure",
    ),
    (
        "audit_log.rs",
        "AuditLog",
        "resources/audit-log.mdx",
        "Audit Log Structure",
    ),
    (
        "audit_log.rs",
        "AuditLogEntry",
        "resources/audit-log.mdx",
        "Audit Log Entry Structure",
    ),
    (
        "audit_log.rs",
        "AuditLogChange",
        "resources/audit-log.mdx",
        "Audit Log Change Structure",
    ),
    (
        "scheduled_event.rs",
        "ScheduledEvent",
        "resources/guild-scheduled-event.mdx",
        "Guild Scheduled Event Structure",
    ),
    (
        "scheduled_event.rs",
        "ScheduledEventUser",
        "resources/guild-scheduled-event.mdx",
        "Guild Scheduled Event User Structure",
    ),
    (
        "scheduled_event.rs",
        "RecurrenceRule",
        "resources/guild-scheduled-event.mdx",
        "Guild Scheduled Event Recurrence Rule Structure",
    ),
    (
        "scheduled_event.rs",
        "RecurrenceNWeekday",
        "resources/guild-scheduled-event.mdx",
        "Guild Scheduled Event Recurrence Rule - N_Weekday Structure",
    ),
    (
        "stage.rs",
        "StageInstance",
        "resources/stage-instance.mdx",
        "Stage Instance Structure",
    ),
    (
        "entitlement.rs",
        "Entitlement",
        "resources/entitlement.mdx",
        "Entitlement Structure",
    ),
    ("sku.rs", "Sku", "resources/sku.mdx", "SKU Structure"),
    (
        "soundboard.rs",
        "SoundboardSound",
        "resources/soundboard.mdx",
        "Soundboard Sound Structure",
    ),
    (
        "poll.rs",
        "Poll",
        "resources/poll.mdx",
        "Poll Object Structure",
    ),
    (
        "poll.rs",
        "PollCreate",
        "resources/poll.mdx",
        "Poll Create Request Object Structure",
    ),
    (
        "poll.rs",
        "PollMedia",
        "resources/poll.mdx",
        "Poll Media Object Structure",
    ),
    (
        "poll.rs",
        "PollAnswer",
        "resources/poll.mdx",
        "Poll Answer Object Structure",
    ),
    (
        "poll.rs",
        "PollResults",
        "resources/poll.mdx",
        "Poll Results Object Structure",
    ),
    (
        "poll.rs",
        "PollAnswerCount",
        "resources/poll.mdx",
        "Poll Answer Count Object Structure",
    ),
    (
        "guild/template.rs",
        "GuildTemplate",
        "resources/guild-template.mdx",
        "Guild Template Structure",
    ),
    (
        "command.rs",
        "ApplicationCommand",
        "interactions/application-commands.mdx",
        "Application Command Structure",
    ),
    (
        "command.rs",
        "CommandOption",
        "interactions/application-commands.mdx",
        "Application Command Option Structure",
    ),
    (
        "command.rs",
        "CommandOptionChoice",
        "interactions/application-commands.mdx",
        "Application Command Option Choice Structure",
    ),
    (
        "command.rs",
        "GuildCommandPermissions",
        "interactions/application-commands.mdx",
        "Guild Application Command Permissions Structure",
    ),
    (
        "command.rs",
        "CommandPermission",
        "interactions/application-commands.mdx",
        "Application Command Permissions Structure",
    ),
    (
        "interaction.rs",
        "Interaction",
        "interactions/receiving-and-responding.mdx",
        "Interaction Structure",
    ),
    (
        "interaction.rs",
        "CommandData",
        "interactions/receiving-and-responding.mdx",
        "Application Command Data Structure",
    ),
    (
        "interaction.rs",
        "MessageComponentData+SubmittedComponent",
        "interactions/receiving-and-responding.mdx",
        "Message Component Data Structure",
    ),
    (
        "interaction.rs",
        "ModalSubmitData",
        "interactions/receiving-and-responding.mdx",
        "Modal Submit Data Structure",
    ),
    (
        "interaction.rs",
        "Resolved",
        "interactions/receiving-and-responding.mdx",
        "Resolved Data Structure",
    ),
    (
        "interaction.rs",
        "CommandDataOption",
        "interactions/receiving-and-responding.mdx",
        "Application Command Interaction Data Option Structure",
    ),
    (
        "interaction.rs",
        "MessageInteraction",
        "interactions/receiving-and-responding.mdx",
        "Message Interaction Structure",
    ),
    (
        "interaction.rs",
        "InteractionResponse",
        "interactions/receiving-and-responding.mdx",
        "Interaction Response Structure",
    ),
    (
        "gateway/command.rs",
        "Identify",
        "events/gateway-events.mdx",
        "Identify Structure",
    ),
    (
        "gateway/command.rs",
        "Resume",
        "events/gateway-events.mdx",
        "Resume Structure",
    ),
    (
        "gateway/command.rs",
        "RequestGuildMembers",
        "events/gateway-events.mdx",
        "Request Guild Members Structure",
    ),
    (
        "gateway/command.rs",
        "RequestSoundboardSounds",
        "events/gateway-events.mdx",
        "Request Soundboard Sounds Structure",
    ),
    (
        "gateway/command.rs",
        "RequestChannelInfo",
        "events/gateway-events.mdx",
        "Request Channel Info Structure",
    ),
    (
        "gateway/command.rs",
        "UpdateVoiceState",
        "events/gateway-events.mdx",
        "Gateway Voice State Update Structure",
    ),
    (
        "gateway/presence.rs",
        "UpdatePresence",
        "events/gateway-events.mdx",
        "Gateway Presence Update Structure",
    ),
    (
        "gateway/payload.rs",
        "Hello",
        "events/gateway-events.mdx",
        "Hello Structure",
    ),
    (
        "gateway/event.rs",
        "ChannelInfo",
        "events/gateway-events.mdx",
        "Channel Info Structure",
    ),
    (
        "gateway/event.rs",
        "ChannelInfoChannel",
        "events/gateway-events.mdx",
        "Channel Info Channel Structure",
    ),
    (
        "gateway/presence.rs",
        "PresenceActivity",
        "events/gateway-events.mdx",
        "Activity Structure",
    ),
    (
        "gateway/event.rs",
        "RateLimitedMeta",
        "events/gateway-events.mdx",
        "Request Guild Member Rate Limit Metadata Structure",
    ),
    (
        "guild/role.rs",
        "Role",
        "topics/permissions.mdx",
        "Role Structure",
    ),
    (
        "guild/role.rs",
        "RoleTags",
        "topics/permissions.mdx",
        "Role Tags Structure",
    ),
];

/// Docs flag tables compared with model bitflags: (file in
/// `crates/model/src`, bitflags type, docs page, heading). An empty type
/// means cordex has no bitflags type for it yet.
const FLAGS: &[(&str, &str, &str, &str)] = &[
    (
        "application.rs",
        "ApplicationFlags",
        "resources/application.mdx",
        "Application Flags",
    ),
    (
        "channel.rs",
        "ChannelFlags",
        "resources/channel.mdx",
        "Channel Flags",
    ),
    (
        "guild/mod.rs",
        "SystemChannelFlags",
        "resources/guild.mdx",
        "System Channel Flags",
    ),
    (
        "guild/member.rs",
        "MemberFlags",
        "resources/guild.mdx",
        "Guild Member Flags",
    ),
    (
        "invite.rs",
        "GuildInviteFlags",
        "resources/invite.mdx",
        "Guild Invite Flags",
    ),
    (
        "message/mod.rs",
        "MessageFlags",
        "resources/message.mdx",
        "Message Flags",
    ),
    (
        "message/embed.rs",
        "EmbedFlags",
        "resources/message.mdx",
        "Embed Flags",
    ),
    (
        "message/embed.rs",
        "EmbedMediaFlags",
        "resources/message.mdx",
        "Embed Media Flags",
    ),
    (
        "message/attachment.rs",
        "AttachmentFlags",
        "resources/message.mdx",
        "Attachment Flags",
    ),
    ("sku.rs", "SkuFlags", "resources/sku.mdx", "SKU Flags"),
    ("user.rs", "UserFlags", "resources/user.mdx", "User Flags"),
    (
        "permissions.rs",
        "Permissions",
        "topics/permissions.mdx",
        "Bitwise Permission Flags",
    ),
    (
        "guild/role.rs",
        "RoleFlags",
        "topics/permissions.mdx",
        "Role Flags",
    ),
    (
        "gateway/presence.rs",
        "ActivityFlags",
        "events/gateway-events.mdx",
        "Activity Flags",
    ),
    (
        "component.rs",
        "UnfurledMediaItemFlags",
        "components/reference.mdx",
        "Unfurled Media Item Flags",
    ),
    (
        "component.rs",
        "ContentScanFlags",
        "components/reference.mdx",
        "Content Scan Metadata Flags",
    ),
    (
        "lobby.rs",
        "LobbyMemberFlags",
        "resources/lobby.mdx",
        "Lobby Member Flags",
    ),
];

/// Model enums whose values are compared with a docs table:
/// (file in `crates/model/src`, enum, docs page in `developers/`, heading).
const ENUMS: &[(&str, &str, &str, &str)] = &[
    (
        "identity_profile.rs",
        "DynamicFieldType",
        "resources/application-identity-profile.mdx",
        "Dynamic Field Types",
    ),
    (
        "webhook_event.rs",
        "WebhookEventType",
        "events/webhook-events.mdx",
        "Webhook Types",
    ),
    ("sku.rs", "SkuType", "resources/sku.mdx", "SKU Types"),
    (
        "sku.rs",
        "SubscriptionStatus",
        "resources/subscription.mdx",
        "Subscription Statuses",
    ),
    (
        "application.rs",
        "RoleConnectionMetadataType",
        "resources/application-role-connection-metadata.mdx",
        "Application Role Connection Metadata Type",
    ),
    (
        "guild/extra.rs",
        "OnboardingMode",
        "resources/guild.mdx",
        "Onboarding Mode",
    ),
    (
        "guild/extra.rs",
        "OnboardingPromptType",
        "resources/guild.mdx",
        "Prompt Types",
    ),
    (
        "interaction.rs",
        "InteractionType",
        "interactions/receiving-and-responding.mdx",
        "Interaction Type",
    ),
    (
        "interaction.rs",
        "InteractionCallbackType",
        "interactions/receiving-and-responding.mdx",
        "Interaction Callback Type",
    ),
    (
        "command.rs",
        "CommandType",
        "interactions/application-commands.mdx",
        "Application Command Types",
    ),
    (
        "command.rs",
        "CommandOptionType",
        "interactions/application-commands.mdx",
        "Application Command Option Type",
    ),
    (
        "command.rs",
        "CommandPermissionType",
        "interactions/application-commands.mdx",
        "Application Command Permission Type",
    ),
    (
        "command.rs",
        "InteractionContextType",
        "interactions/receiving-and-responding.mdx",
        "Interaction Context Types",
    ),
    (
        "command.rs",
        "EntryPointHandlerType",
        "interactions/application-commands.mdx",
        "Entry Point Command Handler Types",
    ),
    (
        "component.rs",
        "ButtonStyle",
        "components/reference.mdx",
        "Button Styles",
    ),
    (
        "component.rs",
        "TextInputStyle",
        "components/reference.mdx",
        "Text Input Styles",
    ),
    (
        "channel.rs",
        "ChannelType",
        "resources/channel.mdx",
        "Channel Types",
    ),
    (
        "channel.rs",
        "VideoQualityMode",
        "resources/channel.mdx",
        "Video Quality Modes",
    ),
    (
        "channel.rs",
        "SortOrderType",
        "resources/channel.mdx",
        "Sort Order Types",
    ),
    (
        "channel.rs",
        "ForumLayoutType",
        "resources/channel.mdx",
        "Forum Layout Types",
    ),
    (
        "message/mod.rs",
        "MessageType",
        "resources/message.mdx",
        "Message Types",
    ),
    (
        "message/mod.rs",
        "MessageActivityType",
        "resources/message.mdx",
        "Message Activity Types",
    ),
    (
        "message/mod.rs",
        "MessageReferenceType",
        "resources/message.mdx",
        "Message Reference Types",
    ),
    (
        "message/mod.rs",
        "ReactionType",
        "resources/message.mdx",
        "Reaction Types",
    ),
    (
        "audit_log.rs",
        "AuditLogEvent",
        "resources/audit-log.mdx",
        "Audit Log Events",
    ),
    (
        "auto_moderation.rs",
        "AutoModerationTriggerType",
        "resources/auto-moderation.mdx",
        "Trigger Types",
    ),
    (
        "auto_moderation.rs",
        "KeywordPresetType",
        "resources/auto-moderation.mdx",
        "Keyword Preset Types",
    ),
    (
        "auto_moderation.rs",
        "AutoModerationEventType",
        "resources/auto-moderation.mdx",
        "Event Types",
    ),
    (
        "auto_moderation.rs",
        "AutoModerationActionType",
        "resources/auto-moderation.mdx",
        "Action Types",
    ),
    (
        "guild/mod.rs",
        "DefaultMessageNotificationLevel",
        "resources/guild.mdx",
        "Default Message Notification Level",
    ),
    (
        "guild/mod.rs",
        "ExplicitContentFilterLevel",
        "resources/guild.mdx",
        "Explicit Content Filter Level",
    ),
    (
        "guild/mod.rs",
        "MfaLevel",
        "resources/guild.mdx",
        "MFA Level",
    ),
    (
        "guild/mod.rs",
        "VerificationLevel",
        "resources/guild.mdx",
        "Verification Level",
    ),
    (
        "guild/mod.rs",
        "NsfwLevel",
        "resources/guild.mdx",
        "Guild Age-Restriction Level",
    ),
    (
        "guild/mod.rs",
        "PremiumTier",
        "resources/guild.mdx",
        "Premium Tier",
    ),
    (
        "guild/integration.rs",
        "IntegrationExpireBehavior",
        "resources/guild.mdx",
        "Integration Expire Behaviors",
    ),
    (
        "invite.rs",
        "InviteType",
        "resources/invite.mdx",
        "Invite Types",
    ),
    (
        "invite.rs",
        "InviteTargetType",
        "resources/invite.mdx",
        "Invite Target Types",
    ),
    (
        "scheduled_event.rs",
        "ScheduledEventPrivacyLevel",
        "resources/guild-scheduled-event.mdx",
        "Guild Scheduled Event Privacy Level",
    ),
    (
        "scheduled_event.rs",
        "ScheduledEventEntityType",
        "resources/guild-scheduled-event.mdx",
        "Guild Scheduled Event Entity Types",
    ),
    (
        "scheduled_event.rs",
        "ScheduledEventStatus",
        "resources/guild-scheduled-event.mdx",
        "Guild Scheduled Event Status",
    ),
    (
        "scheduled_event.rs",
        "RecurrenceFrequency",
        "resources/guild-scheduled-event.mdx",
        "Guild Scheduled Event Recurrence Rule - Frequency",
    ),
    (
        "scheduled_event.rs",
        "RecurrenceWeekday",
        "resources/guild-scheduled-event.mdx",
        "Guild Scheduled Event Recurrence Rule - Weekday",
    ),
    (
        "scheduled_event.rs",
        "RecurrenceMonth",
        "resources/guild-scheduled-event.mdx",
        "Guild Scheduled Event Recurrence Rule - Month",
    ),
    (
        "sticker.rs",
        "StickerType",
        "resources/sticker.mdx",
        "Sticker Types",
    ),
    (
        "sticker.rs",
        "StickerFormatType",
        "resources/sticker.mdx",
        "Sticker Format Types",
    ),
    (
        "stage.rs",
        "StagePrivacyLevel",
        "resources/stage-instance.mdx",
        "Privacy Level",
    ),
    (
        "user.rs",
        "PremiumType",
        "resources/user.mdx",
        "Premium Types",
    ),
    (
        "webhook.rs",
        "WebhookType",
        "resources/webhook.mdx",
        "Webhook Types",
    ),
    (
        "entitlement.rs",
        "EntitlementType",
        "resources/entitlement.mdx",
        "Entitlement Types",
    ),
    (
        "application.rs",
        "ApplicationIntegrationType",
        "resources/application.mdx",
        "Application Integration Types",
    ),
    (
        "application.rs",
        "EventWebhookStatus",
        "resources/application.mdx",
        "Application Event Webhook Status",
    ),
    (
        "application.rs",
        "MembershipState",
        "topics/teams.mdx",
        "Membership State Enum",
    ),
    (
        "gateway/presence.rs",
        "ActivityType",
        "events/gateway-events.mdx",
        "Activity Types",
    ),
    (
        "gateway/presence.rs",
        "StatusDisplayType",
        "events/gateway-events.mdx",
        "Status Display Types",
    ),
    (
        "poll.rs",
        "PollLayoutType",
        "resources/poll.mdx",
        "Layout Type",
    ),
];

struct Item {
    /// Stable identity across docs versions (route, event name, value).
    key: String,
    covered: bool,
    name: String,
    detail: String,
}

struct Area {
    title: &'static str,
    items: Vec<Item>,
}

pub(crate) fn coverage(args: &[String]) -> Result<(), String> {
    let root = workspace_root();
    let check = args.iter().any(|a| a == "--check");
    let pin = args.iter().any(|a| a == "--pin");
    let rev_arg = args
        .iter()
        .position(|a| a == "--rev")
        .and_then(|i| args.get(i + 1))
        .cloned();
    let pinned = read(&root.join(REV_FILE))?.trim().to_owned();
    let wanted = rev_arg.clone().unwrap_or_else(|| pinned.clone());

    let cache = root.join("target").join("discord-api-docs");
    let rev = if wanted == "latest" {
        latest_rev()?
    } else {
        wanted
    };
    let docs = fetch_docs(&cache, &rev)?;
    let areas = measure(&root, &docs.join("developers"))?;
    let unmapped = unmapped_tables(&docs)?;
    let report = render(&rev, &areas, &unmapped);
    print_summary(&rev, &areas);
    println!("  docs tables not compared yet: {}", unmapped.len());

    if check {
        let current = read(&root.join(REPORT)).unwrap_or_default();
        if current.replace("\r\n", "\n") != report {
            return Err(format!(
                "{REPORT} is out of date; run `cargo xtask coverage` and commit it"
            ));
        }
        return Ok(());
    }
    if rev_arg.is_some() && !pin {
        let out = root
            .join("target")
            .join(format!("coverage-{}.md", short(&rev)));
        write(&out, &report)?;
        println!("report: {}", out.display());
        if rev != pinned {
            println!("docs changed: {DOCS_REPO}/compare/{pinned}...{rev}");
            // Measure the pinned docs too and describe the difference.
            let old_docs = fetch_docs(&cache, &pinned)?;
            let old = measure(&root, &old_docs.join("developers"))?;
            let pages = changed_pages(&old_docs, &docs)?;
            let changes = render_changes(&pinned, &rev, &old, &areas, &pages);
            let out = root.join("target").join("docs-changes.md");
            write(&out, &changes)?;
            println!("changes: {}", out.display());
        }
        return Ok(());
    }
    if pin {
        write(&root.join(REV_FILE), &format!("{rev}\n"))?;
    }
    write(&root.join(REPORT), &report)?;
    println!("wrote {REPORT}");
    Ok(())
}

fn write(path: &Path, text: &str) -> Result<(), String> {
    fs::write(path, text).map_err(|e| format!("{}: {e}", path.display()))
}

fn short(rev: &str) -> &str {
    rev.get(..12).unwrap_or(rev)
}

fn run_tool(dir: &Path, program: &str, args: &[&str]) -> Result<String, String> {
    let out = Command::new(program)
        .args(args)
        .current_dir(dir)
        .output()
        .map_err(|e| format!("{program} {}: {e}", args.join(" ")))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).trim().to_owned())
    } else {
        Err(format!(
            "{program} {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        ))
    }
}

/// Hash of the newest docs commit (GitHub API, no git objects needed).
fn latest_rev() -> Result<String, String> {
    let sha = run_tool(
        Path::new("."),
        "curl",
        &[
            "-sSfL",
            "-H",
            "Accept: application/vnd.github.sha",
            "https://api.github.com/repos/discord/discord-api-docs/commits/HEAD",
        ],
    )?;
    if sha.len() == 40 && sha.chars().all(|c| c.is_ascii_hexdigit()) {
        Ok(sha)
    } else {
        Err(format!(
            "unexpected answer for the latest docs commit: {sha}"
        ))
    }
}

/// Download and unpack one docs commit into `cache/<sha>` (once) and
/// return that folder. A tarball by hash avoids git negotiation, which
/// some CI runners route through stale caches.
fn fetch_docs(cache: &Path, rev: &str) -> Result<PathBuf, String> {
    if rev.len() != 40 || !rev.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(format!(
            "docs revision must be a full commit hash, got {rev:?}"
        ));
    }
    let dir = cache.join(rev);
    if dir.join("developers").is_dir() {
        return Ok(dir);
    }
    let tmp = cache.join(format!("{rev}.partial"));
    let _ = fs::remove_dir_all(&tmp);
    fs::create_dir_all(&tmp).map_err(|e| format!("{}: {e}", tmp.display()))?;
    let url = format!("https://codeload.github.com/discord/discord-api-docs/tar.gz/{rev}");
    // Relative paths only: GNU tar reads `C:` in a path as a remote host.
    run_tool(&tmp, "curl", &["-sSfL", "-o", "docs.tar.gz", &url])?;
    run_tool(
        &tmp,
        "tar",
        &["-xzf", "docs.tar.gz", "--strip-components=1"],
    )?;
    let _ = fs::remove_file(tmp.join("docs.tar.gz"));
    if !tmp.join("developers").is_dir() {
        return Err(format!(
            "{url} has no developers/ folder (docs restructured?)"
        ));
    }
    let _ = fs::remove_dir_all(&dir);
    fs::rename(&tmp, &dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    Ok(dir)
}

/// `developers/…` pages that differ between two unpacked docs commits.
fn changed_pages(old: &Path, new: &Path) -> Result<String, String> {
    let mut old_files = Vec::new();
    let mut new_files = Vec::new();
    mdx_files(&old.join("developers"), &mut old_files)?;
    mdx_files(&new.join("developers"), &mut new_files)?;
    let rel = |root: &Path, file: &Path| {
        file.strip_prefix(root)
            .map_or_else(|_| file.display().to_string(), |p| p.display().to_string())
            .replace('\\', "/")
    };
    let mut pages = BTreeSet::new();
    for file in &new_files {
        let name = rel(new, file);
        if fs::read(file).ok() != fs::read(old.join(&name)).ok() {
            pages.insert(name);
        }
    }
    for file in &old_files {
        let name = rel(old, file);
        if !new.join(&name).exists() {
            pages.insert(name);
        }
    }
    Ok(pages.into_iter().collect::<Vec<_>>().join("\n"))
}

fn measure(root: &Path, docs: &Path) -> Result<Vec<Area>, String> {
    let model = root.join("crates/model/src");
    Ok(vec![
        endpoints(root, docs)?,
        events(&model, docs)?,
        number_table(
            "Gateway opcodes",
            docs,
            "topics/opcodes-and-status-codes.mdx",
            "Gateway Opcodes",
            &enum_values(&read(&model.join("gateway/opcode.rs"))?, "OpCode"),
        )?,
        number_table(
            "Gateway close codes",
            docs,
            "topics/opcodes-and-status-codes.mdx",
            "Gateway Close Event Codes",
            &enum_values(&read(&model.join("gateway/close_code.rs"))?, "CloseCode"),
        )?,
        number_table(
            "Component types",
            docs,
            "components/reference.mdx",
            "Component Types",
            &match_arm_numbers(&read(&model.join("component.rs"))?),
        )?,
        enums(&model, docs)?,
        fields(&model, docs)?,
        flags(&model, docs)?,
    ])
}

// ---- docs parsing ---------------------------------------------------------

fn mdx_files(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
    let entries = fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            mdx_files(&path, out)?;
        } else if path.extension().is_some_and(|e| e == "mdx") {
            out.push(path);
        }
    }
    Ok(())
}

/// `[\{channel.id\}](/link)` → `{}`, `[text](/link)` → `text`, no query.
fn normalize_doc_path(raw: &str) -> String {
    let mut text = String::new();
    let mut rest = raw;
    while let Some(start) = rest.find('[') {
        text.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        let link = after.find("](").and_then(|mid| {
            after[mid + 2..]
                .find(')')
                .map(|end| (mid, mid + 2 + end + 1))
        });
        if let Some((mid, next)) = link {
            text.push_str(&after[..mid]);
            rest = &after[next..];
        } else {
            text.push('[');
            rest = after;
        }
    }
    text.push_str(rest);
    normalize_template(&text.replace('\\', ""))
}

/// `{anything}` → `{}`, strip a query string and trailing spaces.
fn normalize_template(path: &str) -> String {
    let path = path.split('?').next().unwrap_or(path).trim();
    let mut out = String::with_capacity(path.len());
    let mut depth = 0;
    for c in path.chars() {
        match c {
            '{' => {
                if depth == 0 {
                    out.push_str("{}");
                }
                depth += 1;
            }
            '}' => depth -= 1,
            _ if depth == 0 => out.push(c),
            _ => {}
        }
    }
    out
}

fn heading_text(line: &str) -> Option<&str> {
    line.starts_with('#')
        .then(|| line.trim_start_matches('#').trim())
}

/// Rows of the first table after `heading`, as trimmed cells.
fn table_after<'a>(text: &'a str, heading: &str) -> Option<Vec<Vec<&'a str>>> {
    let mut lines = text.lines();
    lines.find(|l| heading_text(l) == Some(heading))?;
    let mut rows = Vec::new();
    let mut in_table = false;
    for line in lines {
        let line = line.trim();
        if line.starts_with('|') {
            in_table = true;
            let cells: Vec<&str> = line.trim_matches('|').split('|').map(str::trim).collect();
            if !cells
                .iter()
                .all(|c| c.chars().all(|ch| matches!(ch, '-' | ':' | ' ')))
            {
                rows.push(cells);
            }
        } else if in_table || heading_text(line).is_some() {
            break;
        }
    }
    // First row is the header.
    (!rows.is_empty()).then(|| rows.split_off(1))
}

fn strip_cell(cell: &str) -> String {
    normalize_doc_path(cell)
        .replace(['`', '*'], "")
        .trim()
        .to_owned()
}

fn read_doc(docs: &Path, page: &str) -> Result<String, String> {
    read(&docs.join(page)).map_err(|e| format!("docs page missing (docs restructured?): {e}"))
}

// ---- areas ----------------------------------------------------------------

fn endpoints(root: &Path, docs: &Path) -> Result<Area, String> {
    let ours = http_endpoints(root)?;
    let mut files = Vec::new();
    mdx_files(docs, &mut files)?;
    files.sort();
    let mut seen = BTreeSet::new();
    let mut items = Vec::new();
    for file in files {
        // The change log quotes old and removed routes.
        if file.file_name().is_some_and(|n| n == "change-log.mdx") {
            continue;
        }
        let text = read(&file)?;
        let page = file
            .strip_prefix(docs)
            .map_or_else(|_| file.display().to_string(), |p| p.display().to_string())
            .replace('\\', "/");
        let mut heading = String::new();
        for line in text.lines() {
            if let Some(h) = heading_text(line) {
                h.clone_into(&mut heading);
            }
            let Some(start) = line.find("<Route method=\"") else {
                continue;
            };
            let rest = &line[start + 15..];
            let Some((method, rest)) = rest.split_once("\">") else {
                continue;
            };
            let Some((path, _)) = rest.split_once("</Route>") else {
                continue;
            };
            // Routes the docs mark as deprecated are not counted.
            if heading.to_ascii_lowercase().contains("(deprecated)") {
                continue;
            }
            let key = format!("{method} {}", normalize_doc_path(path));
            if seen.insert(key.clone()) {
                items.push(Item {
                    covered: ours.contains(&key),
                    name: heading.clone(),
                    detail: format!("`{key}` ({page})"),
                    key,
                });
            }
        }
    }
    Ok(Area {
        title: "REST endpoints",
        items,
    })
}

/// `` `METHOD /path` `` from the doc comments in `cordex-http`. Every
/// `Route::raw` template in `endpoint/` must have such a comment, so the
/// count cannot drift silently.
fn http_endpoints(root: &Path) -> Result<BTreeSet<String>, String> {
    let dir = root.join("crates/http/src/endpoint");
    let mut documented = BTreeSet::new();
    let mut templates = BTreeSet::new();
    let entries = fs::read_dir(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    for entry in entries.flatten() {
        let text = read(&entry.path())?;
        for line in text.lines() {
            let line = line.trim();
            if !line.starts_with("///") {
                continue;
            }
            let mut rest = line;
            while let Some(start) = rest.find('`') {
                let after = &rest[start + 1..];
                let Some(end) = after.find('`') else { break };
                let code = &after[..end];
                if let Some((method, path)) = code.split_once(' ')
                    && matches!(method, "GET" | "POST" | "PUT" | "PATCH" | "DELETE")
                    && path.starts_with('/')
                {
                    documented.insert(format!("{method} {}", normalize_template(path)));
                }
                rest = &after[end + 1..];
            }
        }
        let mut rest = text.as_str();
        while let Some(start) = rest.find("Route::raw(") {
            rest = &rest[start + 11..];
            let trimmed = rest.trim_start();
            if let Some(body) = trimmed.strip_prefix('"')
                && let Some(end) = body.find('"')
            {
                templates.insert(normalize_template(&body[..end]));
            }
        }
    }
    let paths: BTreeSet<&str> = documented
        .iter()
        .filter_map(|k| k.split_once(' ').map(|(_, p)| p))
        .collect();
    let missing: Vec<&String> = templates
        .iter()
        .filter(|t| !paths.contains(t.as_str()))
        .collect();
    if !missing.is_empty() {
        return Err(format!(
            "routes without a `METHOD /path` doc comment in cordex-http: {missing:?}"
        ));
    }
    Ok(documented)
}

fn events(model: &Path, docs: &Path) -> Result<Area, String> {
    let event_rs = read(&model.join("gateway/event.rs"))?;
    let ours: BTreeSet<String> = event_rs
        .lines()
        .filter_map(|l| {
            // `Self::X(_) => "NAME",` or a lone `"NAME"` line when rustfmt
            // wrapped the arm.
            let rhs = l
                .split_once("=> \"")
                .map(|(_, rhs)| rhs)
                .or_else(|| l.trim().strip_prefix('"'))?;
            let name = rhs.split('"').next()?;
            name.chars()
                .all(|c| c.is_ascii_uppercase() || c == '_')
                .then(|| name.to_owned())
        })
        .collect();
    let opcodes = enum_variants(&read(&model.join("gateway/opcode.rs"))?, "OpCode");
    let page = "events/gateway-events.mdx";
    let text = read_doc(docs, page)?;
    let rows = table_after(&text, "Receive Events")
        .ok_or("heading 'Receive Events' not found in gateway-events.mdx")?;
    let items = rows
        .iter()
        .filter_map(|row| row.first())
        .map(|cell| {
            let name = strip_cell(cell);
            let dispatch = name.to_uppercase().replace(' ', "_");
            let camel = name.replace(' ', "");
            let covered = ours.contains(&dispatch) || opcodes.contains(&camel);
            Item {
                covered,
                detail: format!("`{dispatch}`"),
                key: dispatch,
                name,
            }
        })
        .collect();
    Ok(Area {
        title: "Gateway receive events",
        items,
    })
}

fn number_table(
    title: &'static str,
    docs: &Path,
    page: &str,
    heading: &str,
    ours: &BTreeSet<i64>,
) -> Result<Area, String> {
    Ok(Area {
        title,
        items: table_numbers(docs, page, heading)?
            .into_iter()
            .map(|(value, name)| Item {
                key: value.to_string(),
                covered: ours.contains(&value),
                name,
                detail: format!("`{value}`"),
            })
            .collect(),
    })
}

/// (value, name) per row: the first numeric cell and the first other cell.
fn table_numbers(docs: &Path, page: &str, heading: &str) -> Result<Vec<(i64, String)>, String> {
    let text = read_doc(docs, page)?;
    let rows = table_after(&text, heading)
        .ok_or_else(|| format!("heading '{heading}' not found in {page} (docs restructured?)"))?;
    Ok(rows
        .iter()
        .filter_map(|row| {
            let cells: Vec<String> = row.iter().map(|c| strip_cell(c)).collect();
            let value = cells.iter().find_map(|c| c.parse::<i64>().ok())?;
            let name = cells
                .iter()
                .find(|c| c.parse::<i64>().is_err())
                .cloned()
                .unwrap_or_default();
            Some((value, name))
        })
        .collect())
}

fn enums(model: &Path, docs: &Path) -> Result<Area, String> {
    let mut items = Vec::new();
    for (file, name, page, heading) in ENUMS {
        let ours = enum_values(&read(&model.join(file))?, name);
        if ours.is_empty() {
            return Err(format!("enum {name} not found in {file}"));
        }
        for (value, label) in table_numbers(docs, page, heading)? {
            items.push(Item {
                key: format!("{name}={value}"),
                covered: ours.contains(&value),
                name: format!("{name}: {label}"),
                detail: format!("`{value}`"),
            });
        }
    }
    Ok(Area {
        title: "Enum values",
        items,
    })
}

fn fields(model: &Path, docs: &Path) -> Result<Area, String> {
    let mut items = Vec::new();
    for (file, name, page, heading) in FIELDS {
        let ours = if name.is_empty() {
            BTreeSet::new()
        } else {
            // `A+B`: the table describes several cordex types together.
            let mut found = BTreeSet::new();
            for part in name.split('+') {
                let fields = struct_fields(model, &read(&model.join(file))?, part, 0)?
                    .ok_or_else(|| format!("struct {part} not found in {file}"))?;
                found.extend(fields);
            }
            if *file == "component.rs" {
                // `component_struct!` adds `id`; `type` comes from the
                // `Component` enum's serializer.
                found.extend(["id".to_owned(), "type".to_owned()]);
            }
            found
        };
        let text = read_doc(docs, page)?;
        let rows = table_after(&text, heading).ok_or_else(|| {
            format!("heading '{heading}' not found in {page} (docs restructured?)")
        })?;
        for row in rows {
            let Some(field) = row.first().map(|c| doc_field_name(c)) else {
                continue;
            };
            let deprecated = row
                .iter()
                .skip(1)
                .any(|c| c.to_ascii_lowercase().contains("deprecated"));
            if field.is_empty() || deprecated {
                continue;
            }
            let label = if name.is_empty() {
                heading.trim_end_matches(" Structure")
            } else {
                name
            };
            items.push(Item {
                key: format!("{heading}.{field}"),
                covered: ours.contains(&field),
                name: format!("{label}.{field}"),
                detail: format!("({page}, {heading})"),
            });
        }
    }
    Ok(Area {
        title: "Object fields",
        items,
    })
}

/// `` `nick?` `` / `id \[1\]` / `**field**` → `nick` / `id` / `field`.
fn doc_field_name(cell: &str) -> String {
    let text = strip_cell(cell);
    let first = text.split_whitespace().next().unwrap_or("");
    let first = first.split('[').next().unwrap_or("");
    first
        .trim_end_matches('?')
        .trim_matches(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .to_owned()
}

fn flags(model: &Path, docs: &Path) -> Result<Area, String> {
    let mut items = Vec::new();
    for (file, name, page, heading) in FLAGS {
        let ours = if name.is_empty() {
            BTreeSet::new()
        } else {
            let bits = flag_bits(&read(&model.join(file))?, name);
            if bits.is_empty() {
                return Err(format!("bitflags {name} not found in {file}"));
            }
            bits
        };
        let text = read_doc(docs, page)?;
        let rows = table_after(&text, heading).ok_or_else(|| {
            format!("heading '{heading}' not found in {page} (docs restructured?)")
        })?;
        for row in rows {
            let Some(bit) = row.iter().find_map(|c| shift_value(c)) else {
                continue;
            };
            let flag = row.first().map(|c| doc_field_name(c)).unwrap_or_default();
            let label = if name.is_empty() { heading } else { name };
            items.push(Item {
                key: format!("{heading}={bit}"),
                covered: ours.contains(&bit),
                name: format!("{label}: {flag}"),
                detail: format!("`1 << {bit}`"),
            });
        }
    }
    Ok(Area {
        title: "Bitflags",
        items,
    })
}

/// `1 << 12` inside a cell (also `` `(1 << 12)` `` and compact `` `1<<0` ``).
fn shift_value(cell: &str) -> Option<u32> {
    let compact = cell.replace(['`', ' '], "");
    let (_, rest) = compact.split_once("1<<")?;
    let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
    digits.parse().ok()
}

// ---- source parsing -------------------------------------------------------

/// Serialized field names of `pub struct NAME` (renames applied, flattened
/// structs expanded). `None` when the struct is not in `text`.
fn struct_fields(
    model: &Path,
    text: &str,
    name: &str,
    depth: u8,
) -> Result<Option<BTreeSet<String>>, String> {
    let plain = format!("pub struct {name} {{");
    let generic = format!("pub struct {name}<");
    let mut lines = text.lines();
    if !lines.any(|l| {
        let l = l.trim_start();
        l.starts_with(&plain) || (l.starts_with(&generic) && l.ends_with('{'))
    }) {
        return Ok(None);
    }
    let mut out = BTreeSet::new();
    let mut attrs = String::new();
    for line in lines {
        let line = line.trim();
        if line == "}" {
            break;
        }
        if line.starts_with("#[serde(") {
            attrs.push_str(line);
            continue;
        }
        let Some(rest) = line.strip_prefix("pub ") else {
            continue;
        };
        let Some((field, ty)) = rest.split_once(':') else {
            continue;
        };
        let field = field.trim().trim_start_matches("r#");
        if attrs.contains("flatten") && depth < 3 {
            let inner = ty
                .trim()
                .trim_end_matches(',')
                .trim_start_matches("Option<")
                .trim_start_matches("Box<")
                .trim_end_matches('>');
            let inner = inner.rsplit("::").next().unwrap_or(inner);
            if let Some(found) = find_struct(model, inner, depth + 1)? {
                out.extend(found);
            }
        } else if let Some(renamed) = attrs
            .split("rename = \"")
            .nth(1)
            .and_then(|r| r.split('"').next())
        {
            out.insert(renamed.to_owned());
        } else if field == "kind" {
            // Rust has no `type` field; manual Deserialize impls map it.
            out.insert("type".to_owned());
        } else {
            out.insert(field.to_owned());
        }
        attrs.clear();
    }
    Ok(Some(out))
}

/// Fields of a struct anywhere in `crates/model/src`.
fn find_struct(model: &Path, name: &str, depth: u8) -> Result<Option<BTreeSet<String>>, String> {
    let mut files = Vec::new();
    rs_files(model, &mut files)?;
    for file in files {
        if let Some(found) = struct_fields(model, &read(&file)?, name, depth)? {
            return Ok(Some(found));
        }
    }
    Ok(None)
}

fn rs_files(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
    let entries = fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            rs_files(&path, out)?;
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
    Ok(())
}

/// Bit numbers of `const X = 1 << N;` in `pub struct NAME: u64 {`.
fn flag_bits(text: &str, name: &str) -> BTreeSet<u32> {
    let header = format!("pub struct {name}:");
    text.lines()
        .skip_while(|l| !l.trim_start().starts_with(&header))
        .skip(1)
        .take_while(|l| l.trim() != "}")
        .filter_map(shift_value)
        .collect()
}

/// Lines of `enum NAME … {` up to the closing `}`.
fn enum_block<'a>(text: &'a str, name: &str) -> impl Iterator<Item = &'a str> {
    let needle = format!("enum {name}");
    text.lines()
        .skip_while(move |l| {
            !l.contains(&needle)
                || l.split(&needle)
                    .nth(1)
                    .and_then(|r| r.chars().next())
                    .is_some_and(char::is_alphanumeric)
        })
        .skip(1)
        .take_while(|l| l.trim() != "}")
}

/// `Variant = 12,` values of an `int_enum!` / C-like enum.
fn enum_values(text: &str, name: &str) -> BTreeSet<i64> {
    enum_block(text, name)
        .filter_map(|l| {
            let (_, value) = l.split_once(" = ")?;
            value.trim().trim_end_matches(',').parse().ok()
        })
        .collect()
}

fn enum_variants(text: &str, name: &str) -> BTreeSet<String> {
    enum_block(text, name)
        .filter_map(|l| {
            let (variant, _) = l.trim().split_once(" = ")?;
            Some(variant.to_owned())
        })
        .collect()
}

/// `12 => …` match arms (the component type switch).
fn match_arm_numbers(text: &str) -> BTreeSet<i64> {
    text.lines()
        .filter_map(|l| l.trim().split_once(" => ")?.0.parse().ok())
        .collect()
}

// ---- output ---------------------------------------------------------------

fn counts(items: &[Item]) -> (usize, usize) {
    (items.iter().filter(|i| i.covered).count(), items.len())
}

fn percent(covered: usize, total: usize) -> String {
    if total == 0 {
        return "–".to_owned();
    }
    let tenths = covered * 1000 / total;
    format!("{}.{} %", tenths / 10, tenths % 10)
}

fn print_summary(rev: &str, areas: &[Area]) {
    println!("discord-api-docs {}", short(rev));
    let (mut c, mut t) = (0, 0);
    for area in areas {
        let (covered, total) = counts(&area.items);
        c += covered;
        t += total;
        println!(
            "  {:<24} {covered:>4}/{total:<4} {}",
            area.title,
            percent(covered, total)
        );
    }
    println!(
        "  {:<24} {c:>4}/{t:<4} {}",
        "total (measured)",
        percent(c, t)
    );
}

/// Docs tables deliberately not compared, with the reason.
const SKIPPED_TABLES: &[(&str, &str)] = &[
    (
        "Thread Fields",
        "prose list of channel fields (channel object is compared)",
    ),
    (
        "Forum and Media Channel Fields",
        "prose list of channel fields (channel object is compared)",
    ),
    (
        "Forum and Media Channel Thread Fields",
        "prose list of channel fields (channel object is compared)",
    ),
    (
        "Entitlement Create Structure",
        "no own table: the entitlement object",
    ),
    (
        "Entitlement Update Structure",
        "no own table: the entitlement object",
    ),
    (
        "Entitlement Delete Structure",
        "no own table: the entitlement object",
    ),
    (
        "Lobby Message Create Structure",
        "no own table: the lobby message object",
    ),
    (
        "Lobby Message Update Structure",
        "no own table: the lobby message object",
    ),
    (
        "Game Direct Message Create Structure",
        "no own table: the SDK DM message object",
    ),
    (
        "Game Direct Message Update Structure",
        "no own table: the SDK DM message object",
    ),
    (
        "Game Direct Message Delete Structure",
        "no own table: the SDK DM message object",
    ),
    (
        "Response Structure",
        "list wrappers, unwrapped by the REST methods",
    ),
    (
        "Attachment Request Structure",
        "request side, built by the upload builders",
    ),
    ("Invite Stage Instance Structure", "deprecated"),
    (
        "Rate Limit Metadata for Opcode Structure",
        "index of the per-opcode tables",
    ),
    (
        "Rate Limit Response Structure",
        "REST error body, handled by the client",
    ),
    (
        "Additional Fields",
        "generic heading of the Social SDK message events",
    ),
    (
        "Poll Result Embed Fields",
        "embed field names, not an object",
    ),
    (
        "Interaction Callback Data Structure",
        "split into sub-tables per callback type",
    ),
];

/// Pages outside the bot API scope (non-goals): RPC and voice connections.
const OUT_OF_SCOPE_PAGES: &[&str] = &[
    "developers/topics/rpc.mdx",
    "developers/topics/voice-connections.mdx",
];

/// `Structure` / `Fields` tables on bot API pages that are neither mapped in
/// [`FIELDS`] nor in [`SKIPPED_TABLES`]: work still to do for the field
/// coverage.
fn unmapped_tables(root: &Path) -> Result<Vec<String>, String> {
    let mut files = Vec::new();
    mdx_files(&root.join("developers"), &mut files)?;
    files.sort();
    let mut out = Vec::new();
    for file in files {
        let page = file
            .strip_prefix(root)
            .map_or_else(|_| file.display().to_string(), |p| p.display().to_string())
            .replace('\\', "/");
        if !BOT_PAGES.iter().any(|p| page.starts_with(p))
            || OUT_OF_SCOPE_PAGES.contains(&page.as_str())
        {
            continue;
        }
        let short_page = page.trim_start_matches("developers/");
        for line in read(&file)?.lines() {
            let Some(heading) = heading_text(line) else {
                continue;
            };
            if !(heading.ends_with(" Structure") || heading.ends_with(" Fields")) {
                continue;
            }
            let mapped = FIELDS
                .iter()
                .any(|(_, _, p, h)| *p == short_page && *h == heading);
            let skipped = SKIPPED_TABLES.iter().any(|(h, _)| *h == heading);
            if !mapped && !skipped {
                out.push(format!("{heading} ({short_page})"));
            }
        }
    }
    Ok(out)
}

fn render(rev: &str, areas: &[Area], unmapped: &[String]) -> String {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "# API coverage\n\n\
         Generated by `cargo xtask coverage` against \
         [`discord-api-docs@{short}`]({DOCS_REPO}/tree/{rev}). Do not edit by hand.\n\n\
         Measured: REST endpoints, gateway receive events, opcodes and close codes, \
         component types, the value tables of the model enums, object fields and bitflags \
         (for the mapped docs tables).\n\n\
         | Area | Covered | Total | % |\n|---|---:|---:|---:|",
        short = short(rev)
    );
    let (mut c, mut t) = (0, 0);
    for area in areas {
        let (covered, total) = counts(&area.items);
        c += covered;
        t += total;
        let _ = writeln!(
            out,
            "| {} | {covered} | {total} | {} |",
            area.title,
            percent(covered, total)
        );
    }
    let _ = writeln!(
        out,
        "| **Total (measured)** | **{c}** | **{t}** | **{}** |",
        percent(c, t)
    );
    for area in areas {
        let (covered, total) = counts(&area.items);
        let _ = writeln!(out, "\n## {} ({covered}/{total})\n", area.title);
        let missing: Vec<&Item> = area.items.iter().filter(|i| !i.covered).collect();
        if missing.is_empty() {
            let _ = writeln!(out, "Everything covered.");
            continue;
        }
        let _ = writeln!(out, "Missing:\n");
        for item in missing {
            let _ = writeln!(out, "- {} {}", item.name, item.detail);
        }
    }
    let _ = writeln!(
        out,
        "\n## Docs tables not compared yet ({})\n",
        unmapped.len()
    );
    if unmapped.is_empty() {
        let _ = writeln!(
            out,
            "Every object and field table of the bot API pages is compared."
        );
    } else {
        let _ = writeln!(
            out,
            "These field tables are not mapped to a cordex type yet, so their fields are not \
             in the numbers above:\n"
        );
        for table in unmapped {
            let _ = writeln!(out, "- {table}");
        }
    }
    out
}

/// Markdown issue body: what the new docs add or remove compared to the
/// pinned ones, as a checklist.
fn render_changes(old_rev: &str, new_rev: &str, old: &[Area], new: &[Area], pages: &str) -> String {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "The official API docs changed: [`{}...{}`]({DOCS_REPO}/compare/{old_rev}...{new_rev}).\n",
        short(old_rev),
        short(new_rev)
    );
    let mut any = false;
    for (old_area, new_area) in old.iter().zip(new) {
        let before: BTreeSet<&str> = old_area.items.iter().map(|i| i.key.as_str()).collect();
        let after: BTreeSet<&str> = new_area.items.iter().map(|i| i.key.as_str()).collect();
        let added: Vec<&Item> = new_area
            .items
            .iter()
            .filter(|i| !before.contains(i.key.as_str()))
            .collect();
        let removed: Vec<&Item> = old_area
            .items
            .iter()
            .filter(|i| !after.contains(i.key.as_str()))
            .collect();
        if added.is_empty() && removed.is_empty() {
            continue;
        }
        any = true;
        let _ = writeln!(out, "### {}\n", new_area.title);
        for item in added {
            let mark = if item.covered { "x" } else { " " };
            let _ = writeln!(out, "- [{mark}] **new:** {} {}", item.name, item.detail);
        }
        for item in removed {
            let _ = writeln!(
                out,
                "- [ ] **removed:** {} {} (deprecate or drop it in cordex)",
                item.name, item.detail
            );
        }
        out.push('\n');
    }
    if !any {
        let _ = writeln!(
            out,
            "No endpoint, event, opcode, close code, component type or enum value was added or \
             removed. Check the changed pages for new fields, flags or behaviour.\n"
        );
    }
    let (bot, other): (Vec<&str>, Vec<&str>) = pages
        .lines()
        .filter(|l| {
            Path::new(l)
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("mdx"))
        })
        // Pages that describe the bot API; the rest (Social SDK,
        // activities, guides) is listed folded.
        .partition(|p| BOT_PAGES.iter().any(|prefix| p.starts_with(prefix)));
    if !bot.is_empty() {
        let _ = writeln!(out, "### Changed bot API pages\n");
        for page in bot {
            let _ = writeln!(out, "- [ ] [`{page}`]({DOCS_REPO}/blob/{new_rev}/{page})");
        }
        out.push('\n');
    }
    if !other.is_empty() {
        let _ = writeln!(
            out,
            "<details><summary>{} other changed pages (Social SDK, activities, guides)</summary>\n",
            other.len()
        );
        for page in other {
            let _ = writeln!(out, "- [`{page}`]({DOCS_REPO}/blob/{new_rev}/{page})");
        }
        let _ = writeln!(out, "\n</details>\n");
    }
    let total = |areas: &[Area]| {
        areas.iter().fold((0, 0), |(c, t), a| {
            let (ac, at) = counts(&a.items);
            (c + ac, t + at)
        })
    };
    let (oc, ot) = total(old);
    let (nc, nt) = total(new);
    let _ = writeln!(
        out,
        "Coverage (measured): {oc}/{ot} ({}) before, {nc}/{nt} ({}) with the new docs.\n\n\
         When done: `cargo xtask coverage --rev latest --pin`, commit `docs/coverage.md` and \
         `docs/discord-api-docs.rev`, close this issue.",
        percent(oc, ot),
        percent(nc, nt)
    );
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn doc_paths_normalize_like_templates() {
        let doc = r"/channels/[\{channel.id\}](/developers/resources/channel#channel-object)/messages/[\{message.id\}](/x)";
        assert_eq!(normalize_doc_path(doc), "/channels/{}/messages/{}");
        assert_eq!(
            normalize_template("/channels/{channel_id}/messages/{message_id}"),
            "/channels/{}/messages/{}"
        );
        assert_eq!(
            normalize_template("/guilds/{guild_id}/members/search?query"),
            "/guilds/{}/members/search"
        );
    }

    #[test]
    fn tables_and_enums_parse() {
        let doc = "###### Button Styles\n\n| Name | Value | Note |\n|---|---|---|\n| Primary | `1` | x |\n| [Link](/a) | 5 | y |\n\n## Next\n";
        let rows = table_after(doc, "Button Styles").unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(strip_cell(rows[1][0]), "Link");

        let src = "int_enum! {\n    pub enum ButtonStyle: u8 {\n        /// x\n        Primary = 1,\n        Link = 5,\n    }\n}\npub enum ButtonStyleX: u8 {\n    Other = 9,\n}\n";
        assert_eq!(enum_values(src, "ButtonStyle"), BTreeSet::from([1, 5]));
        assert_eq!(
            match_arm_numbers("  1 => a,\n  17 => b,\n x => c"),
            BTreeSet::from([1, 17])
        );
    }

    #[test]
    fn changes_list_new_and_removed_items() {
        let item = |key: &str, covered| Item {
            key: key.to_owned(),
            covered,
            name: key.to_owned(),
            detail: String::new(),
        };
        let old = [Area {
            title: "REST endpoints",
            items: vec![item("GET /a", true), item("GET /gone", true)],
        }];
        let new = [Area {
            title: "REST endpoints",
            items: vec![item("GET /a", true), item("POST /b", false)],
        }];
        let pages = "developers/resources/x.mdx\nimages/y.png\n";
        let text = render_changes("aaaa", "bbbb", &old, &new, pages);
        assert!(text.contains("- [ ] **new:** POST /b"), "{text}");
        assert!(text.contains("**removed:** GET /gone"), "{text}");
        assert!(text.contains("developers/resources/x.mdx"), "{text}");
        assert!(!text.contains("y.png"), "{text}");
        assert!(!text.contains("GET /a "), "{text}");
    }

    #[test]
    fn doc_cells_parse() {
        assert_eq!(doc_field_name("nick?"), "nick");
        assert_eq!(doc_field_name("`guild_id`"), "guild_id");
        assert_eq!(doc_field_name(r"type\* \[1\]"), "type");
        assert_eq!(doc_field_name("**id**"), "id");
        assert_eq!(shift_value("`0x0000000000000002` `(1 << 1)`"), Some(1));
        assert_eq!(shift_value("`1 << 18`"), Some(18));
        assert_eq!(shift_value("`1<<0`"), Some(0));
        assert_eq!(shift_value("none"), None);
    }

    #[test]
    fn struct_fields_apply_serde_attributes() {
        let src = "pub struct Inner {\n    pub extra: u8,\n}\n\
                   pub struct Outer<'a> {\n    /// doc\n    pub id: u64,\n    \
                   #[serde(rename = \"type\")]\n    pub kind: u8,\n    \
                   #[serde(default, skip_serializing_if = \"Option::is_none\")]\n    \
                   pub nick: Option<&'a str>,\n    pub kind2: u8,\n}\n\
                   pub struct Other {\n    pub kind: u8,\n}\n";
        let dir = std::env::temp_dir().join(format!("cordex-xtask-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("a.rs"), src).unwrap();
        let fields = struct_fields(&dir, src, "Outer", 0).unwrap().unwrap();
        let expected: BTreeSet<String> = ["id", "type", "nick", "kind2"]
            .into_iter()
            .map(String::from)
            .collect();
        assert_eq!(fields, expected);
        // A field called `kind` without rename counts as `type`.
        let other = struct_fields(&dir, src, "Other", 0).unwrap().unwrap();
        assert!(other.contains("type"));
        assert!(struct_fields(&dir, src, "Missing", 0).unwrap().is_none());
        let _ = fs::remove_dir_all(&dir);

        let flags = "bitflags::bitflags! {\n    pub struct F: u64 {\n        const A = 1 << 0;\n        \
                     const B = 1 << 7;\n    }\n}\n";
        assert_eq!(flag_bits(flags, "F"), BTreeSet::from([0, 7]));
    }
}
