//! Attachments (resources/message.mdx, "Attachment Object").

use serde::{Deserialize, Serialize};

use crate::application::Application;
use crate::id::Id;
use crate::id::marker::AttachmentMarker;
use crate::serde_util::bitflags_serde_int;
use crate::timestamp::Timestamp;
use crate::user::User;

/// A file attached to a message.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Attachment {
    /// Attachment ID.
    pub id: Id<AttachmentMarker>,
    /// File name.
    pub filename: Box<str>,
    /// Title.
    pub title: Option<Box<str>>,
    /// Alt text (max. 1024 characters).
    pub description: Option<Box<str>>,
    /// Media type.
    pub content_type: Option<Box<str>>,
    /// Size in bytes.
    pub size: u64,
    /// Source URL.
    pub url: Box<str>,
    /// Proxied URL.
    #[serde(default)]
    pub proxy_url: Box<str>,
    /// Height (images and videos).
    pub height: Option<u32>,
    /// Width (images and videos).
    pub width: Option<u32>,
    /// Thumbhash placeholder.
    pub placeholder: Option<Box<str>>,
    /// Placeholder version.
    pub placeholder_version: Option<u8>,
    /// Whether the attachment is ephemeral (removed after a while).
    #[serde(default)]
    pub ephemeral: bool,
    /// Duration of an audio/video file in seconds (voice messages).
    pub duration_secs: Option<f64>,
    /// Base64 waveform (voice messages).
    pub waveform: Option<Box<str>>,
    /// Flags.
    #[serde(default)]
    pub flags: AttachmentFlags,
    /// Clips: users in the stream.
    #[serde(default)]
    pub clip_participants: Vec<User>,
    /// Clips: when the clip was created.
    pub clip_created_at: Option<Timestamp>,
    /// Clips: application in the stream.
    pub application: Option<Box<Application>>,
}

bitflags::bitflags! {
    /// Attachment flags.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
    pub struct AttachmentFlags: u64 {
        /// Clip from a stream.
        const IS_CLIP = 1 << 0;
        /// Thumbnail of a media channel thread.
        const IS_THUMBNAIL = 1 << 1;
        /// Edited with remix.
        const IS_REMIX = 1 << 2;
        /// Marked as spoiler.
        const IS_SPOILER = 1 << 3;
        /// Flagged as explicit.
        const CONTAINS_EXPLICIT_CONTENT = 1 << 4;
        /// Animated image.
        const IS_ANIMATED = 1 << 5;
        /// Flagged as gore.
        const CONTAINS_GORE_CONTENT = 1 << 6;
        /// Flagged as self-harm.
        const CONTAINS_SELF_HARM_CONTENT = 1 << 7;
    }
}

bitflags_serde_int!(AttachmentFlags, u64);
