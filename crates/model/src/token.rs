//! Bot token with redacted formatting (PLAN.md §14).

use std::fmt;
use std::sync::Arc;

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
}
