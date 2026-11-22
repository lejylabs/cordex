//! Polls (resources/poll.mdx).

use serde::{Deserialize, Serialize};

use crate::emoji::Emoji;
use crate::id::Id;
use crate::id::marker::{ChannelMarker, GuildMarker, MessageMarker, UserMarker};
use crate::serde_util::int_enum;
use crate::timestamp::Timestamp;

/// A poll in a message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Poll {
    /// Question (only `text`).
    pub question: PollMedia,
    /// Answers (max. 10).
    pub answers: Vec<PollAnswer>,
    /// When the poll ends; `None` for polls that never expire.
    pub expiry: Option<Timestamp>,
    /// Whether multiple answers can be selected.
    #[serde(default)]
    pub allow_multiselect: bool,
    /// Layout.
    #[serde(default = "PollLayoutType::default_value")]
    pub layout_type: PollLayoutType,
    /// Results; may be missing while counts are not available.
    pub results: Option<PollResults>,
}

/// Text and/or emoji of a question or answer.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[non_exhaustive]
pub struct PollMedia {
    /// Text (question max. 300, answer max. 55 characters).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<Box<str>>,
    /// Emoji (answers only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub emoji: Option<Emoji>,
}

impl PollMedia {
    /// Text only.
    pub fn text(text: impl Into<Box<str>>) -> Self {
        Self {
            text: Some(text.into()),
            emoji: None,
        }
    }
}

/// An answer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct PollAnswer {
    /// Answer ID (set by Discord, starts at 1).
    pub answer_id: u32,
    /// Answer content.
    pub poll_media: PollMedia,
}

int_enum! {
    /// Poll layout.
    pub enum PollLayoutType: u8 {
        /// The default layout.
        Default = 1,
    }
}

impl PollLayoutType {
    const fn default_value() -> Self {
        Self::Default
    }
}

/// Poll results.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct PollResults {
    /// Whether the counts are final.
    pub is_finalized: bool,
    /// Counts per answer (answers without votes may be missing).
    #[serde(default)]
    pub answer_counts: Vec<PollAnswerCount>,
}

/// Votes for one answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct PollAnswerCount {
    /// Answer ID.
    pub id: u32,
    /// Votes.
    pub count: u32,
    /// Whether the current user voted for it.
    pub me_voted: bool,
}

/// Poll to send with a message (resources/poll.mdx, "Poll Create Request").
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct PollCreate {
    /// Question.
    pub question: PollMedia,
    /// Answers (only `poll_media`).
    pub answers: Vec<PollCreateAnswer>,
    /// Duration in hours (max. 768, default 24).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration: Option<u16>,
    /// Allow multiple answers (default false).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub allow_multiselect: bool,
    /// Layout (default `Default`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layout_type: Option<PollLayoutType>,
}

/// Answer of a poll to send.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct PollCreateAnswer {
    /// Answer content.
    pub poll_media: PollMedia,
}

/// `MESSAGE_POLL_VOTE_ADD` / `MESSAGE_POLL_VOTE_REMOVE` payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct MessagePollVote {
    /// Voter.
    pub user_id: Id<UserMarker>,
    /// Channel.
    pub channel_id: Id<ChannelMarker>,
    /// Message with the poll.
    pub message_id: Id<MessageMarker>,
    /// Guild.
    pub guild_id: Option<Id<GuildMarker>>,
    /// Answer (starts at 1).
    pub answer_id: u32,
}

impl PollCreate {
    /// Poll with a question and text answers.
    pub fn new<I, S>(question: impl Into<Box<str>>, answers: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<Box<str>>,
    {
        Self {
            question: PollMedia::text(question),
            answers: answers
                .into_iter()
                .map(|a| PollCreateAnswer {
                    poll_media: PollMedia::text(a),
                })
                .collect(),
            duration: None,
            allow_multiselect: false,
            layout_type: None,
        }
    }
}
