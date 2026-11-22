//! Roles (topics/permissions.mdx, "Role Object").

use serde::{Deserialize, Serialize};

use crate::id::Id;
use crate::id::marker::{IntegrationMarker, RoleMarker, SkuMarker, UserMarker};
use crate::image_hash::ImageHash;
use crate::permissions::Permissions;
use crate::serde_util::{bitflags_serde_int, null_if_true, present};

/// A guild role.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Role {
    /// Role ID (the `@everyone` role has the guild's ID).
    pub id: Id<RoleMarker>,
    /// Role name.
    pub name: Box<str>,
    /// Deprecated single color; use [`Role::colors`].
    #[serde(default)]
    pub color: u32,
    /// Role colors (gradient with secondary/tertiary color).
    pub colors: Option<RoleColors>,
    /// Whether the role is shown separately in the member list.
    pub hoist: bool,
    /// Role icon hash.
    pub icon: Option<ImageHash>,
    /// Role unicode emoji.
    pub unicode_emoji: Option<Box<str>>,
    /// Position; equal positions are sorted by ID.
    pub position: i32,
    /// Permissions.
    pub permissions: Permissions,
    /// Whether the role is managed by an integration.
    pub managed: bool,
    /// Whether the role is mentionable.
    pub mentionable: bool,
    /// Tags.
    pub tags: Option<RoleTags>,
    /// Role flags.
    #[serde(default)]
    pub flags: RoleFlags,
}

impl Role {
    /// Mention string `<@&id>`.
    pub fn mention(&self) -> String {
        format!("<@&{}>", self.id)
    }

    /// Primary color: from `colors` if present, else the deprecated `color`.
    pub fn primary_color(&self) -> u32 {
        self.colors.as_ref().map_or(self.color, |c| c.primary_color)
    }
}

/// Role colors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct RoleColors {
    /// Primary color.
    pub primary_color: u32,
    /// Secondary color (makes the role a gradient).
    pub secondary_color: Option<u32>,
    /// Tertiary color (turns the gradient into a holographic style).
    pub tertiary_color: Option<u32>,
}

/// Role tags. Fields of type `null` in the docs are `true` when present.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[non_exhaustive]
pub struct RoleTags {
    /// Bot this role belongs to.
    pub bot_id: Option<Id<UserMarker>>,
    /// Integration this role belongs to.
    pub integration_id: Option<Id<IntegrationMarker>>,
    /// Whether this is the guild's Booster role.
    #[serde(
        default,
        deserialize_with = "present",
        serialize_with = "null_if_true",
        skip_serializing_if = "std::ops::Not::not"
    )]
    pub premium_subscriber: bool,
    /// Subscription SKU and listing of this role.
    pub subscription_listing_id: Option<Id<SkuMarker>>,
    /// Whether this role is available for purchase.
    #[serde(
        default,
        deserialize_with = "present",
        serialize_with = "null_if_true",
        skip_serializing_if = "std::ops::Not::not"
    )]
    pub available_for_purchase: bool,
    /// Whether this role is a linked role.
    #[serde(
        default,
        deserialize_with = "present",
        serialize_with = "null_if_true",
        skip_serializing_if = "std::ops::Not::not"
    )]
    pub guild_connections: bool,
}

bitflags::bitflags! {
    /// Role flags.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
    pub struct RoleFlags: u64 {
        /// Role can be selected in an onboarding prompt.
        const IN_PROMPT = 1 << 0;
    }
}

bitflags_serde_int!(RoleFlags, u64);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tags_with_null_fields() {
        let json = r#"{
          "id": "41771983423143936",
          "name": "WE DEM BOYZZ!!!!!!",
          "color": 3447003,
          "colors": {"primary_color": 3447003, "secondary_color": null, "tertiary_color": null},
          "hoist": true,
          "icon": "cf3ced8600b777c9486c6d8d84fb4327",
          "unicode_emoji": null,
          "position": 1,
          "permissions": "66321471",
          "managed": false,
          "mentionable": false,
          "flags": 0,
          "tags": {"premium_subscriber": null, "bot_id": "2"}
        }"#;
        let role: Role = serde_json::from_str(json).unwrap();
        assert_eq!(role.primary_color(), 3_447_003);
        assert!(role.permissions.contains(Permissions::ADMINISTRATOR));
        let tags = role.tags.clone().unwrap();
        assert!(tags.premium_subscriber);
        assert!(!tags.available_for_purchase);
        assert_eq!(tags.bot_id.unwrap().get(), 2);

        let out = serde_json::to_string(&role).unwrap();
        assert!(out.contains(r#""premium_subscriber":null"#));
        assert!(!out.contains("available_for_purchase"));
        assert_eq!(serde_json::from_str::<Role>(&out).unwrap(), role);
    }
}
