//! Discord API v10 models: snowflakes, flags, REST resources and gateway
//! payloads.
//!
//! No async or network dependencies; checked by `cargo xtask layout-check`.
//!
//! Reference: <https://github.com/discord/discord-api-docs> (the official
//! developer documentation wins over every other source).

pub mod application;
pub mod audit_log;
pub mod auto_moderation;
pub mod cdn;
pub mod channel;
pub mod command;
pub mod component;
pub mod emoji;
pub mod entitlement;
pub mod error;
pub mod gateway;
pub mod guild;
pub mod id;
pub mod identity_profile;
pub mod image_hash;
pub mod interaction;
pub mod invite;
pub mod lobby;
pub mod mention;
pub mod message;
pub mod permission_calc;
pub mod permissions;
pub mod poll;
pub mod scheduled_event;
pub mod sku;
pub mod soundboard;
pub mod stage;
pub mod sticker;
pub mod timestamp;
pub mod token;
pub mod user;
pub mod voice;
pub mod webhook;
pub mod webhook_event;

mod serde_util;

pub use channel::{Channel, ChannelType};
pub use emoji::Emoji;
pub use error::ParseError;
pub use guild::{Ban, Guild, Member, Role, UnavailableGuild};
pub use id::Id;
pub use image_hash::ImageHash;
pub use invite::Invite;
pub use mention::{Mentionable, ParsedMention, parse_mentions};
pub use message::Message;
pub use message::embed::EmbedError;
pub use permissions::Permissions;
pub use scheduled_event::ScheduledEvent;
pub use soundboard::SoundboardSound;
pub use stage::StageInstance;
pub use sticker::Sticker;
pub use timestamp::{Timestamp, TimestampStyle};
pub use token::{InteractionToken, Token, WebhookToken};
pub use user::User;
pub use webhook::{Webhook, parse_webhook_url};

/// Discord API version this crate models.
pub const API_VERSION: u8 = 10;
