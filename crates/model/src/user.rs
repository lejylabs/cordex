//! Users (resources/user.mdx, "User Object").

use serde::{Deserialize, Serialize};

use crate::id::Id;
use crate::id::marker::{GuildMarker, SkuMarker, UserMarker};
use crate::image_hash::ImageHash;
use crate::serde_util::{bitflags_serde_int, int_enum};

/// Legacy four-digit user tag (`name#1337`), stored as a number instead of
/// a string. `0` means the new username system without a tag, which every
/// user except bots has today.
///
/// ```
/// use cordex_model::user::Discriminator;
///
/// let tag: Discriminator = serde_json::from_str(r#""0042""#).unwrap();
/// assert_eq!(tag.get(), 42);
/// assert_eq!(tag.to_string(), "0042");
/// assert!(Discriminator::default().is_none());
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Discriminator(u16);

impl Discriminator {
    /// Tag from a number (`0` = no tag).
    pub const fn new(value: u16) -> Self {
        Self(value)
    }

    /// The tag as a number.
    pub const fn get(self) -> u16 {
        self.0
    }

    /// Whether the user has no legacy tag (new username system).
    pub const fn is_none(self) -> bool {
        self.0 == 0
    }
}

impl std::fmt::Display for Discriminator {
    /// `0` without a tag, otherwise four digits (`0042`), as Discord sends it.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.0 == 0 {
            f.write_str("0")
        } else {
            write!(f, "{:04}", self.0)
        }
    }
}

impl Serialize for Discriminator {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for Discriminator {
    /// Accepts the documented string and a plain number; anything that is
    /// not a tag reads as `0` instead of failing the whole user.
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Visitor;

        impl serde::de::Visitor<'_> for Visitor {
            type Value = Discriminator;

            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("a discriminator string")
            }

            fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<Self::Value, E> {
                Ok(Discriminator(v.parse().unwrap_or(0)))
            }

            fn visit_u64<E: serde::de::Error>(self, v: u64) -> Result<Self::Value, E> {
                Ok(Discriminator(u16::try_from(v).unwrap_or(0)))
            }

            fn visit_unit<E: serde::de::Error>(self) -> Result<Self::Value, E> {
                Ok(Discriminator(0))
            }
        }

        deserializer.deserialize_any(Visitor)
    }
}

/// A Discord user.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct User {
    /// User ID.
    pub id: Id<UserMarker>,
    /// Username, not unique across the platform. Empty on some partial
    /// users (presence updates send only `id`).
    #[serde(default)]
    pub username: Box<str>,
    /// Legacy Discord tag; `0` for users on the new username system (and
    /// when a partial user omits it). Only bots still have one.
    #[serde(default)]
    pub discriminator: Discriminator,
    /// Display name, if set.
    pub global_name: Option<Box<str>>,
    /// Avatar hash.
    pub avatar: Option<ImageHash>,
    /// Whether the user belongs to an `OAuth2` application.
    #[serde(default)]
    pub bot: bool,
    /// Whether the user is an official Discord system user.
    #[serde(default)]
    pub system: bool,
    /// Whether two-factor authentication is enabled (`OAuth2` `identify`).
    pub mfa_enabled: Option<bool>,
    /// Banner hash.
    pub banner: Option<ImageHash>,
    /// Banner color as an RGB integer.
    pub accent_color: Option<u32>,
    /// Chosen language (`OAuth2` `identify`).
    pub locale: Option<Box<str>>,
    /// Whether the email is verified (`OAuth2` `email`).
    pub verified: Option<bool>,
    /// Email (`OAuth2` `email`).
    pub email: Option<Box<str>>,
    /// Account flags.
    pub flags: Option<UserFlags>,
    /// Nitro subscription type.
    pub premium_type: Option<PremiumType>,
    /// Public account flags.
    pub public_flags: Option<UserFlags>,
    /// Avatar decoration.
    pub avatar_decoration_data: Option<AvatarDecorationData>,
    /// Collectibles (nameplate).
    pub collectibles: Option<Collectibles>,
    /// Primary guild (server tag).
    pub primary_guild: Option<PrimaryGuild>,
}

impl User {
    /// Name to show: display name if set, else username.
    pub fn display_name(&self) -> &str {
        self.global_name.as_deref().unwrap_or(&self.username)
    }

    /// Mention string `<@id>`.
    pub fn mention(&self) -> String {
        format!("<@{}>", self.id)
    }

    /// CDN URL of the avatar, or the default avatar when none is set.
    pub fn avatar_url(&self) -> crate::cdn::CdnUrl {
        crate::cdn::user_avatar(self.id, self.avatar.as_ref(), self.discriminator)
    }
}

bitflags::bitflags! {
    /// User account flags.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
    pub struct UserFlags: u64 {
        /// Discord employee.
        const STAFF = 1 << 0;
        /// Partnered server owner.
        const PARTNER = 1 << 1;
        /// HypeSquad Events member.
        const HYPESQUAD = 1 << 2;
        /// Bug Hunter level 1.
        const BUG_HUNTER_LEVEL_1 = 1 << 3;
        /// House Bravery member.
        const HYPESQUAD_ONLINE_HOUSE_1 = 1 << 6;
        /// House Brilliance member.
        const HYPESQUAD_ONLINE_HOUSE_2 = 1 << 7;
        /// House Balance member.
        const HYPESQUAD_ONLINE_HOUSE_3 = 1 << 8;
        /// Early Nitro supporter.
        const PREMIUM_EARLY_SUPPORTER = 1 << 9;
        /// User is a team.
        const TEAM_PSEUDO_USER = 1 << 10;
        /// Bug Hunter level 2.
        const BUG_HUNTER_LEVEL_2 = 1 << 14;
        /// Verified bot.
        const VERIFIED_BOT = 1 << 16;
        /// Early verified bot developer.
        const VERIFIED_DEVELOPER = 1 << 17;
        /// Moderator Programs alumni.
        const CERTIFIED_MODERATOR = 1 << 18;
        /// Bot uses only HTTP interactions and is shown in the online member list.
        const BOT_HTTP_INTERACTIONS = 1 << 19;
    }
}

bitflags_serde_int!(UserFlags, u64);

int_enum! {
    /// Nitro subscription type.
    pub enum PremiumType: u8 {
        /// No Nitro.
        None = 0,
        /// Nitro Classic.
        NitroClassic = 1,
        /// Nitro.
        Nitro = 2,
        /// Nitro Basic.
        NitroBasic = 3,
    }
}

/// Avatar decoration of a user or member.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct AvatarDecorationData {
    /// Decoration asset hash.
    pub asset: ImageHash,
    /// SKU of the decoration.
    pub sku_id: Id<SkuMarker>,
}

/// Collectibles of a user or member (excluding avatar decorations).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Collectibles {
    /// Nameplate.
    pub nameplate: Option<Nameplate>,
}

/// Nameplate collectible.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Nameplate {
    /// SKU of the nameplate.
    pub sku_id: Id<SkuMarker>,
    /// Path to the nameplate asset.
    pub asset: Box<str>,
    /// Label (currently unused).
    pub label: Box<str>,
    /// Background color name (`crimson`, `berry`, `sky`, …).
    pub palette: Box<str>,
}

/// The user's primary guild (server tag).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct PrimaryGuild {
    /// Primary guild ID.
    pub identity_guild_id: Option<Id<GuildMarker>>,
    /// Whether the server tag is displayed; `None` if the system cleared it.
    pub identity_enabled: Option<bool>,
    /// Server tag text (max. 4 characters).
    pub tag: Option<Box<str>>,
    /// Server tag badge hash.
    pub badge: Option<ImageHash>,
}

/// A connected account (`GET /users/@me/connections`, needs the
/// `connections` `OAuth2` scope).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
#[allow(
    clippy::struct_excessive_bools,
    reason = "mirrors the flags of Discord's connection object"
)]
pub struct Connection {
    /// Account ID on the service.
    pub id: Box<str>,
    /// Account name.
    pub name: Box<str>,
    /// Service (`"github"`, `"twitch"`, …).
    #[serde(rename = "type")]
    pub kind: Box<str>,
    /// Whether the connection is revoked.
    #[serde(default)]
    pub revoked: bool,
    /// Whether the connection is verified.
    #[serde(default)]
    pub verified: bool,
    /// Whether friend sync is on.
    #[serde(default)]
    pub friend_sync: bool,
    /// Whether activities of this connection show in presence updates.
    #[serde(default)]
    pub show_activity: bool,
    /// Whether the connection has a Discord-linked `OAuth2` token.
    #[serde(default)]
    pub two_way_link: bool,
    /// Visibility (0 = only you, 1 = everyone).
    #[serde(default)]
    pub visibility: u8,
    /// Server integrations of this connection (partial integrations, kept
    /// as JSON).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub integrations: Vec<serde_json::Value>,
}

/// The current user's role connection for one application.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ApplicationRoleConnection {
    /// Vanity name of the platform (max. 50 characters).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub platform_name: Option<Box<str>>,
    /// Username on the platform (max. 100 characters).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub platform_username: Option<Box<str>>,
    /// Metadata values (keys from the application's role connection
    /// metadata, values as strings).
    #[serde(default)]
    pub metadata: std::collections::BTreeMap<Box<str>, Box<str>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    const DOC_EXAMPLE: &str = r#"{
      "id": "80351110224678912",
      "username": "Nelly",
      "global_name": null,
      "discriminator": "1337",
      "avatar": "8342729096ea3675442027381ff50dfe",
      "verified": true,
      "email": "nelly@discord.com",
      "flags": 64,
      "banner": "06c16474723fe537c283b8efa61a30c8",
      "accent_color": 16711680,
      "premium_type": 0,
      "public_flags": 64,
      "avatar_decoration_data": {
        "sku_id": "1144058844004233369",
        "asset": "a_fed43ab12698df65902ba06727e20c0e"
      },
      "collectibles": {
        "nameplate": {
          "sku_id": "2247558840304243311",
          "asset": "nameplates/nameplates/twilight/",
          "label": "",
          "palette": "cobalt"
        }
      },
      "primary_guild": {
        "identity_guild_id": "1234647491267808778",
        "identity_enabled": true,
        "tag": "DISC",
        "badge": "7d1734ae5a615e82bc7a4033b98fade8"
      }
    }"#;

    #[test]
    fn docs_example() {
        let user: User = serde_json::from_str(DOC_EXAMPLE).unwrap();
        assert_eq!(user.id.get(), 80_351_110_224_678_912);
        assert_eq!(user.display_name(), "Nelly");
        assert_eq!(user.mention(), "<@80351110224678912>");
        assert!(!user.bot);
        assert_eq!(user.flags, Some(UserFlags::HYPESQUAD_ONLINE_HOUSE_1));
        assert_eq!(user.premium_type, Some(PremiumType::None));
        assert!(user.avatar_decoration_data.unwrap().asset.is_animated());
        assert_eq!(&*user.primary_guild.unwrap().tag.unwrap(), "DISC");
        assert_eq!(user.discriminator, Discriminator::new(1337));

        // Round trip keeps every value.
        let again: User = serde_json::from_str(
            &serde_json::to_string(&serde_json::from_str::<User>(DOC_EXAMPLE).unwrap()).unwrap(),
        )
        .unwrap();
        assert_eq!(again, serde_json::from_str::<User>(DOC_EXAMPLE).unwrap());
    }

    #[test]
    fn minimal_bot_user_and_unknown_fields() {
        let json = r#"{"id":"1","username":"zeny","discriminator":"0","global_name":"Zeny",
                       "avatar":null,"bot":true,"some_future_field":{"x":1}}"#;
        let user: User = serde_json::from_str(json).unwrap();
        assert!(user.bot);
        assert_eq!(user.display_name(), "Zeny");
        assert!(user.flags.is_none());
        assert!(user.discriminator.is_none());
    }

    #[test]
    fn discriminator_forms() {
        let read = |json: &str| serde_json::from_str::<Discriminator>(json).unwrap();
        assert_eq!(read(r#""0""#), Discriminator::default());
        assert_eq!(read(r#""0001""#).get(), 1);
        assert_eq!(read("7").get(), 7);
        assert_eq!(read("null").get(), 0);
        assert_eq!(read(r#""x""#).get(), 0);
        assert_eq!(read("70000").get(), 0);
        assert!(serde_json::from_str::<Discriminator>("[]").is_err());
        assert_eq!(
            serde_json::to_string(&Discriminator::new(1)).unwrap(),
            r#""0001""#
        );
        assert_eq!(
            serde_json::to_string(&Discriminator::new(0)).unwrap(),
            r#""0""#
        );
        let missing: User = serde_json::from_str(r#"{"id":"1"}"#).unwrap();
        assert!(missing.discriminator.is_none());
    }
}
