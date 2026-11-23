//! CDN URL helpers (reference.mdx, "Image Formatting"). No network I/O.

use std::fmt;

use crate::id::Id;
use crate::id::marker::{
    ApplicationMarker, EmojiMarker, GuildMarker, RoleMarker, ScheduledEventMarker, StickerMarker,
    UserMarker,
};
use crate::image_hash::ImageHash;
use crate::sticker::StickerFormatType;

/// Discord CDN origin.
pub const CDN_BASE: &str = "https://cdn.discordapp.com";

/// Image format for a CDN URL.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ImageFormat {
    /// PNG.
    Png,
    /// JPEG.
    Jpeg,
    /// WebP.
    Webp,
    /// GIF (animated hashes default to this).
    Gif,
}

impl ImageFormat {
    /// File extension Discord expects.
    pub const fn ext(self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Jpeg => "jpg",
            Self::Webp => "webp",
            Self::Gif => "gif",
        }
    }

    /// GIF for animated hashes, otherwise PNG.
    pub fn for_hash(hash: &ImageHash) -> Self {
        if hash.is_animated() {
            Self::Gif
        } else {
            Self::Png
        }
    }
}

/// A CDN URL with optional size (`?size=` must be a power of two, 16–4096).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CdnUrl {
    path: String,
    size: Option<u16>,
}

impl CdnUrl {
    fn hashed(path: &str, hash: &ImageHash, format: ImageFormat) -> Self {
        Self {
            path: format!("{path}/{hash}.{}", format.ext()),
            size: None,
        }
    }

    fn static_path(path: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            size: None,
        }
    }

    /// Requested pixel size (16–4096, power of two). Invalid sizes are ignored.
    #[must_use]
    pub fn size(mut self, size: u16) -> Self {
        if (16..=4096).contains(&size) && size.is_power_of_two() {
            self.size = Some(size);
        }
        self
    }
}

impl fmt::Display for CdnUrl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(CDN_BASE)?;
        f.write_str(&self.path)?;
        if let Some(size) = self.size {
            write!(f, "?size={size}")?;
        }
        Ok(())
    }
}

/// User avatar, or the default avatar when `hash` is `None`.
pub fn user_avatar(
    user_id: Id<UserMarker>,
    hash: Option<&ImageHash>,
    discriminator: &str,
) -> CdnUrl {
    match hash {
        Some(hash) => CdnUrl::hashed(
            &format!("/avatars/{user_id}"),
            hash,
            ImageFormat::for_hash(hash),
        ),
        None => default_avatar(user_id, discriminator),
    }
}

/// Default avatar (`embed/avatars/{index}.png`).
///
/// Legacy tags use `discriminator % 5`; the new username system uses
/// `(user_id >> 22) % 6`.
pub fn default_avatar(user_id: Id<UserMarker>, discriminator: &str) -> CdnUrl {
    let index = if discriminator.is_empty() || discriminator == "0" {
        (user_id.get() >> 22) % 6
    } else {
        discriminator.parse::<u64>().unwrap_or(0) % 5
    };
    CdnUrl::static_path(format!("/embed/avatars/{index}.png"))
}

/// Member guild avatar.
pub fn member_avatar(
    guild_id: Id<GuildMarker>,
    user_id: Id<UserMarker>,
    hash: &ImageHash,
) -> CdnUrl {
    CdnUrl::hashed(
        &format!("/guilds/{guild_id}/users/{user_id}/avatars"),
        hash,
        ImageFormat::for_hash(hash),
    )
}

/// Guild icon.
pub fn guild_icon(guild_id: Id<GuildMarker>, hash: &ImageHash) -> CdnUrl {
    CdnUrl::hashed(
        &format!("/icons/{guild_id}"),
        hash,
        ImageFormat::for_hash(hash),
    )
}

/// Guild banner.
pub fn guild_banner(guild_id: Id<GuildMarker>, hash: &ImageHash) -> CdnUrl {
    CdnUrl::hashed(
        &format!("/banners/{guild_id}"),
        hash,
        ImageFormat::for_hash(hash),
    )
}

/// Guild invite splash.
pub fn guild_splash(guild_id: Id<GuildMarker>, hash: &ImageHash) -> CdnUrl {
    CdnUrl::hashed(
        &format!("/splashes/{guild_id}"),
        hash,
        ImageFormat::for_hash(hash),
    )
}

/// Guild discovery splash.
pub fn guild_discovery_splash(guild_id: Id<GuildMarker>, hash: &ImageHash) -> CdnUrl {
    CdnUrl::hashed(
        &format!("/discovery-splashes/{guild_id}"),
        hash,
        ImageFormat::for_hash(hash),
    )
}

/// Role icon.
pub fn role_icon(role_id: Id<RoleMarker>, hash: &ImageHash) -> CdnUrl {
    CdnUrl::hashed(&format!("/role-icons/{role_id}"), hash, ImageFormat::Png)
}

/// Custom emoji image (`gif` when animated).
pub fn emoji(emoji_id: Id<EmojiMarker>, animated: bool) -> CdnUrl {
    let ext = if animated { "gif" } else { "png" };
    CdnUrl::static_path(format!("/emojis/{emoji_id}.{ext}"))
}

/// Sticker image. Lottie stickers use `.json`.
pub fn sticker(sticker_id: Id<StickerMarker>, format: StickerFormatType) -> CdnUrl {
    let ext = match format {
        StickerFormatType::Gif => "gif",
        StickerFormatType::Lottie => "json",
        StickerFormatType::Png | StickerFormatType::Apng | StickerFormatType::Unknown(_) => "png",
    };
    CdnUrl::static_path(format!("/stickers/{sticker_id}.{ext}"))
}

/// Application icon.
pub fn application_icon(application_id: Id<ApplicationMarker>, hash: &ImageHash) -> CdnUrl {
    CdnUrl::hashed(
        &format!("/app-icons/{application_id}"),
        hash,
        ImageFormat::for_hash(hash),
    )
}

/// Scheduled event cover image.
pub fn scheduled_event_cover(event_id: Id<ScheduledEventMarker>, hash: &ImageHash) -> CdnUrl {
    CdnUrl::hashed(
        &format!("/guild-events/{event_id}"),
        hash,
        ImageFormat::for_hash(hash),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::id::marker::UserMarker;

    #[test]
    fn avatar_and_default() {
        let id = Id::<UserMarker>::new(80_351_110_224_678_912).unwrap();
        let hash = ImageHash::new("a_1269e74af4df7417b13759eae50c83dc");
        let url = user_avatar(id, Some(&hash), "0").size(128).to_string();
        assert_eq!(
            url,
            "https://cdn.discordapp.com/avatars/80351110224678912/a_1269e74af4df7417b13759eae50c83dc.gif?size=128"
        );
        let def = default_avatar(id, "0").to_string();
        let index = (id.get() >> 22) % 6;
        assert_eq!(
            def,
            format!("https://cdn.discordapp.com/embed/avatars/{index}.png")
        );
        let legacy = default_avatar(id, "1337").to_string();
        assert!(legacy.ends_with("/embed/avatars/2.png"));
        assert_eq!(CdnUrl::static_path("/x.png").size(15).size, None);
        assert_eq!(CdnUrl::static_path("/x.png").size(32).size, Some(32));
    }
}
