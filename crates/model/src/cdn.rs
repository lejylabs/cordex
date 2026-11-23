//! CDN URL helpers (reference.mdx, "Image Formatting"). No network I/O.

use std::fmt;

use crate::id::Id;
use crate::id::marker::{
    ApplicationMarker, EmojiMarker, GuildMarker, RoleMarker, ScheduledEventMarker, StickerMarker,
    UserMarker,
};
use crate::image_hash::ImageHash;
use crate::sticker::StickerFormatType;
use crate::user::Discriminator;

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

/// A CDN URL; set a size with [`CdnUrl::size`] and a format with
/// [`CdnUrl::format`], then format it with `to_string()`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CdnUrl {
    base: &'static str,
    /// Path without the extension.
    path: String,
    ext: &'static str,
    size: Option<u16>,
}

/// Base URL of GIF stickers (they are not on the CDN host).
pub const MEDIA_BASE: &str = "https://media.discordapp.net";

impl CdnUrl {
    fn hashed(path: &str, hash: &ImageHash, format: ImageFormat) -> Self {
        Self {
            base: CDN_BASE,
            path: format!("{path}/{hash}"),
            ext: format.ext(),
            size: None,
        }
    }

    fn fixed(base: &'static str, path: impl Into<String>, ext: &'static str) -> Self {
        Self {
            base,
            path: path.into(),
            ext,
            size: None,
        }
    }

    /// Image size: a power of two from 16 to 4096 (other values are
    /// ignored).
    #[must_use]
    pub fn size(mut self, size: u16) -> Self {
        if (16..=4096).contains(&size) && size.is_power_of_two() {
            self.size = Some(size);
        }
        self
    }

    /// Another image format (e.g. `Webp` for smaller files). Ignored for
    /// default avatars, Lottie and GIF stickers, which exist in one format
    /// only.
    #[must_use]
    pub fn format(mut self, format: ImageFormat) -> Self {
        if self.base == CDN_BASE && !self.path.starts_with("/embed/") && self.ext != "json" {
            self.ext = format.ext();
        }
        self
    }
}

impl fmt::Display for CdnUrl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.base)?;
        f.write_str(&self.path)?;
        f.write_str(".")?;
        f.write_str(self.ext)?;
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
    discriminator: Discriminator,
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
pub fn default_avatar(user_id: Id<UserMarker>, discriminator: Discriminator) -> CdnUrl {
    let index = if discriminator.is_none() {
        (user_id.get() >> 22) % 6
    } else {
        u64::from(discriminator.get() % 5)
    };
    CdnUrl::fixed(CDN_BASE, format!("/embed/avatars/{index}"), "png")
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
    CdnUrl::fixed(CDN_BASE, format!("/emojis/{emoji_id}"), ext)
}

/// Sticker image. Lottie stickers use `.json`.
pub fn sticker(sticker_id: Id<StickerMarker>, format: StickerFormatType) -> CdnUrl {
    let path = format!("/stickers/{sticker_id}");
    match format {
        // GIF stickers are served from the media host, not the CDN.
        StickerFormatType::Gif => CdnUrl::fixed(MEDIA_BASE, path, "gif"),
        StickerFormatType::Lottie => CdnUrl::fixed(CDN_BASE, path, "json"),
        StickerFormatType::Png | StickerFormatType::Apng | StickerFormatType::Unknown(_) => {
            CdnUrl::fixed(CDN_BASE, path, "png")
        }
    }
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
        let url = user_avatar(id, Some(&hash), Discriminator::default())
            .size(128)
            .to_string();
        assert_eq!(
            url,
            "https://cdn.discordapp.com/avatars/80351110224678912/a_1269e74af4df7417b13759eae50c83dc.gif?size=128"
        );
        let def = default_avatar(id, Discriminator::default()).to_string();
        let index = (id.get() >> 22) % 6;
        assert_eq!(
            def,
            format!("https://cdn.discordapp.com/embed/avatars/{index}.png")
        );
        let legacy = default_avatar(id, Discriminator::new(1337)).to_string();
        assert!(legacy.ends_with("/embed/avatars/2.png"));
        assert_eq!(CdnUrl::fixed(CDN_BASE, "/x", "png").size(15).size, None);
        assert_eq!(CdnUrl::fixed(CDN_BASE, "/x", "png").size(32).size, Some(32));
    }

    #[test]
    fn formats_and_sticker_hosts() {
        let guild = Id::<GuildMarker>::new(1).unwrap();
        let hash = ImageHash::new("1269e74af4df7417b13759eae50c83dc");
        assert_eq!(
            guild_icon(guild, &hash)
                .format(ImageFormat::Webp)
                .size(64)
                .to_string(),
            "https://cdn.discordapp.com/icons/1/1269e74af4df7417b13759eae50c83dc.webp?size=64"
        );
        let sticker_id = Id::<StickerMarker>::new(5).unwrap();
        assert_eq!(
            sticker(sticker_id, StickerFormatType::Gif).to_string(),
            "https://media.discordapp.net/stickers/5.gif"
        );
        assert_eq!(
            sticker(sticker_id, StickerFormatType::Lottie)
                .format(ImageFormat::Png)
                .to_string(),
            "https://cdn.discordapp.com/stickers/5.json"
        );
        let user = Id::<UserMarker>::new(80_351_110_224_678_912).unwrap();
        let def = default_avatar(user, Discriminator::default()).format(ImageFormat::Webp);
        assert_eq!(def.ext, "png");
    }
}
