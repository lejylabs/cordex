//! Allowed mentions (resources/message.mdx, "Allowed Mentions Object").

use serde::{Deserialize, Serialize};

use crate::id::Id;
use crate::id::marker::{RoleMarker, UserMarker};
use crate::serde_util::str_enum;

str_enum! {
    /// Kind of mention that may be parsed from content.
    pub enum AllowedMentionType {
        /// Role mentions.
        Roles = "roles",
        /// User mentions.
        Users = "users",
        /// `@everyone` and `@here`.
        Everyone = "everyone",
    }
}

/// Which mentions in message content actually ping.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[non_exhaustive]
pub struct AllowedMentions {
    /// Mention types to parse from the content.
    ///
    /// Mutually exclusive with the matching `roles`/`users` allow-lists:
    /// if `parse` contains `"users"`, do not also send `users`. An empty
    /// list suppresses all parsed mentions (unlike omitting the field).
    #[serde(default)]
    pub parse: Vec<AllowedMentionType>,
    /// Role IDs to mention (max. 100).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub roles: Vec<Id<RoleMarker>>,
    /// User IDs to mention (max. 100).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub users: Vec<Id<UserMarker>>,
    /// Whether a reply mentions the author of the referenced message.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub replied_user: bool,
}

impl AllowedMentions {
    /// Parse nothing (no pings).
    pub fn none() -> Self {
        Self::default()
    }

    /// Default for regular messages: parse users, roles and everyone.
    pub fn all() -> Self {
        Self {
            parse: vec![
                AllowedMentionType::Users,
                AllowedMentionType::Roles,
                AllowedMentionType::Everyone,
            ],
            roles: Vec::new(),
            users: Vec::new(),
            replied_user: false,
        }
    }

    /// Default for interactions and webhooks: parse user mentions only.
    pub fn users_only() -> Self {
        Self {
            parse: vec![AllowedMentionType::Users],
            roles: Vec::new(),
            users: Vec::new(),
            replied_user: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suppress_all_serializes_empty_parse() {
        let json = serde_json::to_string(&AllowedMentions::none()).unwrap();
        assert_eq!(json, r#"{"parse":[]}"#);
    }
}
