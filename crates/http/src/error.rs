//! REST client errors.

use std::fmt;
use std::time::Duration;

use bytes::Bytes;
use http::StatusCode;
use serde_json::value::RawValue;

use crate::limiter::RateLimitScope;

/// An error from the REST client.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum HttpError {
    /// Discord returned a JSON error body (4xx/5xx other than a handled 429).
    #[error("Discord API error {status}: {code} {message}")]
    Discord {
        /// HTTP status.
        status: StatusCode,
        /// Discord JSON error code (`0` if the body was not JSON).
        code: u32,
        /// Discord error message.
        message: Box<str>,
        /// Nested `errors` object, if present.
        errors: Option<Box<RawValue>>,
    },
    /// A 429 whose wait exceeded the retry-after cap, or retries were exhausted.
    #[error("rate limited for {retry_after:?} (global: {global})")]
    RateLimited {
        /// How long Discord asked us to wait.
        retry_after: Duration,
        /// `true` if this 429 applies to every request.
        global: bool,
        /// `X-RateLimit-Scope`, when Discord sent it.
        scope: Option<RateLimitScope>,
    },
    /// The token was rejected (HTTP 401); the client will not send further requests.
    #[error("unauthorized: token rejected")]
    Unauthorized,
    /// The whole-request timeout elapsed.
    #[error("request timed out")]
    Timeout,
    /// TLS, DNS, TCP or HTTP transport failure.
    #[error("transport: {0}")]
    Transport(Box<str>),
    /// The response body was not the expected JSON.
    #[error("failed to deserialize response")]
    Deserialize {
        /// Raw body that failed to parse.
        body: Bytes,
        /// Serde error.
        #[source]
        source: serde_json::Error,
    },
    /// A builder or local check failed before the request was sent.
    #[error("validation: {0}")]
    Validation(Box<str>),
    /// Too many 401/403/429 responses in the sliding window.
    #[error("invalid request limit reached ({count}/{threshold})")]
    InvalidRequestLimit {
        /// Counted invalid responses in the current window.
        count: u32,
        /// Configured reject threshold.
        threshold: u32,
    },
    /// The response body exceeded the configured size cap.
    #[error("response body exceeded {limit} bytes")]
    ResponseTooLarge {
        /// Configured cap in bytes.
        limit: usize,
    },
}

impl HttpError {
    /// Discord JSON error code, when this is a [`HttpError::Discord`].
    pub fn discord_code(&self) -> Option<DiscordErrorCode> {
        match self {
            Self::Discord { code, .. } => Some(DiscordErrorCode::from(*code)),
            _ => None,
        }
    }

    /// Nested Discord field errors flattened to `field.path` → message pairs.
    pub fn flatten_errors(&self) -> Vec<(String, String)> {
        match self {
            Self::Discord {
                errors: Some(raw), ..
            } => flatten_discord_errors(raw),
            _ => Vec::new(),
        }
    }

    pub(crate) fn transport(err: impl fmt::Display) -> Self {
        Self::Transport(err.to_string().into_boxed_str())
    }

    pub(crate) fn validation(msg: impl Into<Box<str>>) -> Self {
        Self::Validation(msg.into())
    }
}

/// Documented Discord JSON error codes, plus [`DiscordErrorCode::Unknown`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum DiscordErrorCode {
    /// `0` general error (no further details, or a non-JSON body).
    General,
    /// `10003` unknown channel.
    UnknownChannel,
    /// `10004` unknown guild.
    UnknownGuild,
    /// `10007` unknown member.
    UnknownMember,
    /// `10008` unknown message.
    UnknownMessage,
    /// `10013` unknown user.
    UnknownUser,
    /// `10014` unknown emoji.
    UnknownEmoji,
    /// `10015` unknown webhook.
    UnknownWebhook,
    /// `10062` unknown interaction.
    UnknownInteraction,
    /// `10063` unknown application command.
    UnknownApplicationCommand,
    /// `20012` not authorized for this application.
    NotAuthorizedForApplication,
    /// `30001` maximum number of guilds reached.
    MaximumGuilds,
    /// `40001` unauthorized.
    Unauthorized,
    /// `40004` sending messages has been temporarily disabled.
    SendMessagesDisabled,
    /// `40060` interaction has already been acknowledged.
    InteractionAlreadyAcknowledged,
    /// `50001` missing access.
    MissingAccess,
    /// `50006` cannot send an empty message.
    EmptyMessage,
    /// `50013` missing permissions.
    MissingPermissions,
    /// `50014` invalid authentication token.
    InvalidToken,
    /// `50035` invalid form body.
    InvalidFormBody,
    /// `50055` invalid guild.
    InvalidGuild,
    /// `200000` message blocked by Auto Moderation.
    AutoModBlockedMessage,
    /// A code this version does not name yet.
    Unknown(u32),
}

impl From<u32> for DiscordErrorCode {
    fn from(value: u32) -> Self {
        match value {
            0 => Self::General,
            10003 => Self::UnknownChannel,
            10004 => Self::UnknownGuild,
            10007 => Self::UnknownMember,
            10008 => Self::UnknownMessage,
            10013 => Self::UnknownUser,
            10014 => Self::UnknownEmoji,
            10015 => Self::UnknownWebhook,
            10062 => Self::UnknownInteraction,
            10063 => Self::UnknownApplicationCommand,
            20012 => Self::NotAuthorizedForApplication,
            30001 => Self::MaximumGuilds,
            40001 => Self::Unauthorized,
            40004 => Self::SendMessagesDisabled,
            40060 => Self::InteractionAlreadyAcknowledged,
            50001 => Self::MissingAccess,
            50006 => Self::EmptyMessage,
            50013 => Self::MissingPermissions,
            50014 => Self::InvalidToken,
            50035 => Self::InvalidFormBody,
            50055 => Self::InvalidGuild,
            200_000 => Self::AutoModBlockedMessage,
            other => Self::Unknown(other),
        }
    }
}

impl From<DiscordErrorCode> for u32 {
    fn from(value: DiscordErrorCode) -> Self {
        match value {
            DiscordErrorCode::General => 0,
            DiscordErrorCode::UnknownChannel => 10003,
            DiscordErrorCode::UnknownGuild => 10004,
            DiscordErrorCode::UnknownMember => 10007,
            DiscordErrorCode::UnknownMessage => 10008,
            DiscordErrorCode::UnknownUser => 10013,
            DiscordErrorCode::UnknownEmoji => 10014,
            DiscordErrorCode::UnknownWebhook => 10015,
            DiscordErrorCode::UnknownInteraction => 10062,
            DiscordErrorCode::UnknownApplicationCommand => 10063,
            DiscordErrorCode::NotAuthorizedForApplication => 20012,
            DiscordErrorCode::MaximumGuilds => 30001,
            DiscordErrorCode::Unauthorized => 40001,
            DiscordErrorCode::SendMessagesDisabled => 40004,
            DiscordErrorCode::InteractionAlreadyAcknowledged => 40060,
            DiscordErrorCode::MissingAccess => 50001,
            DiscordErrorCode::EmptyMessage => 50006,
            DiscordErrorCode::MissingPermissions => 50013,
            DiscordErrorCode::InvalidToken => 50014,
            DiscordErrorCode::InvalidFormBody => 50035,
            DiscordErrorCode::InvalidGuild => 50055,
            DiscordErrorCode::AutoModBlockedMessage => 200_000,
            DiscordErrorCode::Unknown(other) => other,
        }
    }
}

/// Walk a Discord nested `errors` object into `field.path` → message pairs.
pub fn flatten_discord_errors(raw: &RawValue) -> Vec<(String, String)> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(raw.get()) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    walk_errors("", &value, &mut out);
    out
}

fn walk_errors(prefix: &str, value: &serde_json::Value, out: &mut Vec<(String, String)>) {
    let Some(obj) = value.as_object() else {
        if let Some(arr) = value.as_array() {
            for (i, item) in arr.iter().enumerate() {
                let next = if prefix.is_empty() {
                    format!("[{i}]")
                } else {
                    format!("{prefix}[{i}]")
                };
                walk_errors(&next, item, out);
            }
        }
        return;
    };

    if let Some(errors) = obj.get("_errors").and_then(serde_json::Value::as_array) {
        for err in errors {
            if let Some(msg) = err.get("message").and_then(serde_json::Value::as_str) {
                out.push((prefix.to_owned(), msg.to_owned()));
            }
        }
    }

    for (key, child) in obj {
        if key == "_errors" {
            continue;
        }
        let next = if prefix.is_empty() {
            key.clone()
        } else {
            format!("{prefix}.{key}")
        };
        walk_errors(&next, child, out);
    }
}

pub(crate) fn discord_from_body(status: StatusCode, body: &Bytes) -> HttpError {
    #[derive(serde::Deserialize)]
    struct Body {
        message: Option<Box<str>>,
        code: Option<u32>,
        errors: Option<Box<RawValue>>,
    }

    if body.is_empty() {
        return HttpError::Discord {
            status,
            code: 0,
            message: status
                .canonical_reason()
                .unwrap_or("error")
                .to_owned()
                .into_boxed_str(),
            errors: None,
        };
    }

    match std::str::from_utf8(body)
        .ok()
        .and_then(|s| serde_json::from_str::<Body>(s).ok())
    {
        Some(parsed) => HttpError::Discord {
            status,
            code: parsed.code.unwrap_or(0),
            message: parsed.message.unwrap_or_default(),
            errors: parsed.errors,
        },
        None => HttpError::Discord {
            status,
            code: 0,
            message: String::from_utf8_lossy(body).into_owned().into_boxed_str(),
            errors: None,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flatten_nested_form_errors() {
        let raw = RawValue::from_string(
            r#"{
                "content": { "_errors": [{ "code": "BASE_TYPE_REQUIRED", "message": "This field is required" }] },
                "embeds": { "0": { "title": { "_errors": [{ "code": "BASE", "message": "Must be 256 or fewer" }] } } }
            }"#
            .to_owned(),
        )
        .unwrap();
        let flat = flatten_discord_errors(&raw);
        assert!(
            flat.iter()
                .any(|(p, m)| p == "content" && m.contains("required"))
        );
        assert!(
            flat.iter()
                .any(|(p, m)| p == "embeds.0.title" && m.contains("256"))
        );
    }

    #[test]
    fn unknown_error_code_survives() {
        let code = DiscordErrorCode::from(99_999);
        assert_eq!(code, DiscordErrorCode::Unknown(99_999));
        assert_eq!(u32::from(code), 99_999);
        assert_eq!(u32::from(DiscordErrorCode::UnknownMessage), 10008);
    }
}
