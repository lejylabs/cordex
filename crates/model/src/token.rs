//! Bot token with redacted formatting.

use std::fmt;
use std::sync::Arc;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// A bot token. `Debug` and `Display` never print the secret, so a token
/// cannot leak through logs, errors or panics.
///
/// Cheap to clone (shared). Use [`Token::expose`] only where the raw value
/// must be sent (Authorization header, Identify, Resume).
#[derive(Clone, PartialEq, Eq)]
pub struct Token(Arc<str>);

impl Token {
    /// Wraps a raw bot token (without the `Bot ` prefix). Surrounding
    /// whitespace is removed, a leading `Bot ` prefix is stripped.
    pub fn new(raw: &str) -> Self {
        let raw = raw.trim();
        let raw = raw.strip_prefix("Bot ").unwrap_or(raw);
        Self(Arc::from(raw))
    }

    /// The raw secret. Never log the result.
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for Token {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Token(***)")
    }
}

impl fmt::Display for Token {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("***")
    }
}

/// Continuation token for responding to an interaction (valid 15 minutes).
/// `Debug` and `Display` never print the secret.
#[derive(Clone, PartialEq, Eq)]
pub struct InteractionToken(Box<str>);

impl InteractionToken {
    /// Wraps a raw interaction token.
    pub fn new(raw: impl Into<Box<str>>) -> Self {
        Self(raw.into())
    }

    /// The raw secret. Never log the result.
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for InteractionToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("InteractionToken(***)")
    }
}

impl fmt::Display for InteractionToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("***")
    }
}

impl Serialize for InteractionToken {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for InteractionToken {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Box::<str>::deserialize(deserializer).map(Self)
    }
}

/// Incoming-webhook token. `Debug` and `Display` never print the secret.
#[derive(Clone, PartialEq, Eq)]
pub struct WebhookToken(Box<str>);

impl WebhookToken {
    /// Wraps a raw webhook token.
    pub fn new(raw: impl Into<Box<str>>) -> Self {
        Self(raw.into())
    }

    /// The raw secret. Never log the result.
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for WebhookToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("WebhookToken(***)")
    }
}

impl fmt::Display for WebhookToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("***")
    }
}

impl Serialize for WebhookToken {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for WebhookToken {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Box::<str>::deserialize(deserializer).map(Self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn never_prints_the_secret() {
        let token = Token::new("  Bot secret.value.123 \n");
        assert_eq!(token.expose(), "secret.value.123");
        assert_eq!(format!("{token:?}"), "Token(***)");
        assert_eq!(format!("{token}"), "***");
        assert!(!format!("{token:#?} {token}").contains("secret"));
    }

    #[test]
    fn interaction_token_never_prints_the_secret() {
        let token = InteractionToken::new("A_UNIQUE_TOKEN");
        assert_eq!(token.expose(), "A_UNIQUE_TOKEN");
        assert_eq!(format!("{token:?}"), "InteractionToken(***)");
        assert_eq!(format!("{token}"), "***");
        let json = serde_json::to_string(&token).unwrap();
        assert_eq!(json, r#""A_UNIQUE_TOKEN""#);
        let back: InteractionToken = serde_json::from_str(&json).unwrap();
        assert_eq!(back.expose(), "A_UNIQUE_TOKEN");
        assert!(!format!("{back:?}").contains("UNIQUE"));
    }

    #[test]
    fn webhook_token_never_prints_the_secret() {
        let token = WebhookToken::new("webhook-secret");
        assert_eq!(token.expose(), "webhook-secret");
        assert_eq!(format!("{token:?}"), "WebhookToken(***)");
        assert_eq!(format!("{token}"), "***");
        let json = serde_json::to_string(&token).unwrap();
        assert_eq!(json, r#""webhook-secret""#);
        let back: WebhookToken = serde_json::from_str(&json).unwrap();
        assert_eq!(back.expose(), "webhook-secret");
        assert!(!format!("{back:?}").contains("webhook-secret"));
    }
}
