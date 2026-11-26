#![doc = include_str!("../README.md")]

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
pub mod format;
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
pub use format::{
    escape_markdown, escape_mentions, split_message, split_message_2000, split_message_4096,
};
pub use guild::{Ban, Guild, Member, Role, UnavailableGuild};
pub use id::{
    ApplicationId, AttachmentId, ChannelId, CommandId, EmojiId, GuildId, Id, InteractionId,
    MessageId, RoleId, UserId, WebhookId,
};
pub use image_hash::ImageHash;
pub use invite::{Invite, parse_invite_url};
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

#[cfg(test)]
mod size_of_regressions {
    use std::mem::size_of;

    use crate::channel::Channel;
    use crate::gateway::Event;
    use crate::guild::{Guild, Member};
    use crate::interaction::Interaction;
    use crate::message::Message;
    use crate::user::User;

    /// Upper bounds for core model types (64-bit) so accidental struct bloat
    /// or unboxed enum variants are caught immediately in CI.
    #[test]
    fn core_model_sizes_stay_bounded() {
        assert!(size_of::<Event>() <= 48, "Event: {}", size_of::<Event>());
        assert!(size_of::<User>() <= 256, "User: {}", size_of::<User>());
        assert!(
            size_of::<Member>() <= 384,
            "Member: {}",
            size_of::<Member>()
        );
        assert!(
            size_of::<Channel>() <= 512,
            "Channel: {}",
            size_of::<Channel>()
        );
        assert!(size_of::<Guild>() <= 768, "Guild: {}", size_of::<Guild>());
        assert!(
            size_of::<Message>() <= 640,
            "Message: {}",
            size_of::<Message>()
        );
        assert!(
            size_of::<Interaction>() <= 768,
            "Interaction: {}",
            size_of::<Interaction>()
        );
    }
}
