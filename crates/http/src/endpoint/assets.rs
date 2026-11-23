//! Emojis (guild and application), stickers and soundboard sounds.

use std::future::Future;

use cordex_model::emoji::Emoji;
use cordex_model::id::Id;
use cordex_model::id::marker::{
    ApplicationMarker, ChannelMarker, EmojiMarker, GuildMarker, RoleMarker, SoundMarker,
    StickerMarker, StickerPackMarker,
};
use cordex_model::soundboard::SoundboardSound;
use cordex_model::sticker::{Sticker, StickerPack};
use http::Method;
use serde::Deserialize;

use crate::client::Client;
use crate::error::HttpError;
use crate::multipart::{File, encode_form};
use crate::route::Route;

/// Max. size of an emoji image (256 KiB).
pub const EMOJI_MAX_BYTES: usize = 256 * 1024;
/// Max. size of a sticker file (512 KiB).
pub const STICKER_MAX_BYTES: usize = 512 * 1024;
/// Max. size of a soundboard sound (512 KiB).
pub const SOUND_MAX_BYTES: usize = 512 * 1024;

#[derive(Deserialize)]
struct Items<T> {
    items: Vec<T>,
}

#[derive(Deserialize)]
struct StickerPacks {
    sticker_packs: Vec<StickerPack>,
}

/// Rough size check of a base64 data URI before upload.
fn check_data_uri(data_uri: &str, max: usize, what: &str) -> Result<(), HttpError> {
    if !data_uri.starts_with("data:") {
        return Err(HttpError::Validation(
            format!("{what} must be a data URI (data:<mime>;base64,...)").into(),
        ));
    }
    let payload = data_uri.split_once(',').map_or("", |(_, b64)| b64);
    if payload.len() / 4 * 3 > max {
        return Err(HttpError::Validation(
            format!("{what} is larger than {} KiB", max / 1024).into(),
        ));
    }
    Ok(())
}

json_request! {
    /// `PATCH` of a guild emoji.
    UpdateGuildEmoji -> Emoji => send_json {
        /// New name.
        name: &str,
        /// Roles allowed to use it (`None` = everyone).
        roles: Option<&[Id<RoleMarker>]>,
    }
}

json_request! {
    /// `PATCH` of a guild sticker.
    UpdateGuildSticker -> Sticker => send_json {
        /// Name (2-30 characters).
        name: &str,
        /// Description (empty or 2-100 characters).
        description: Option<&str>,
        /// Autocomplete tags (max. 200 characters).
        tags: &str,
    }
}

json_request! {
    /// `POST` / `PATCH` of a guild soundboard sound.
    GuildSoundRequest -> SoundboardSound => send_json {
        /// Name (2-32 characters).
        name: &str,
        /// Volume (0.0-1.0).
        volume: Option<f64>,
        /// Custom emoji ID.
        emoji_id: Option<Id<EmojiMarker>>,
        /// Unicode emoji.
        emoji_name: Option<&str>,
    }
}

impl Client {
    // ---- guild emojis ------------------------------------------------------

    simple! {
        /// `GET /guilds/{guild_id}/emojis`.
        fn get_guild_emojis(guild_id: Id<GuildMarker>)
            -> Vec<Emoji> = get "/guilds/{guild_id}/emojis" => send_json
    }

    simple! {
        /// `GET /guilds/{guild_id}/emojis/{emoji_id}`.
        fn get_guild_emoji(guild_id: Id<GuildMarker>, emoji_id: Id<EmojiMarker>)
            -> Emoji = get "/guilds/{guild_id}/emojis/{emoji_id}" => send_json
    }

    /// `POST /guilds/{guild_id}/emojis`: `image` is a data URI (max. 256
    /// KiB). Needs `CREATE_GUILD_EXPRESSIONS`.
    pub fn create_guild_emoji(
        &self,
        guild_id: Id<GuildMarker>,
        name: &str,
        image: &str,
        roles: &[Id<RoleMarker>],
    ) -> impl Future<Output = Result<Emoji, HttpError>> + Send + 'static {
        let client = self.clone();
        let check = check_data_uri(image, EMOJI_MAX_BYTES, "emoji image");
        let body = serde_json::json!({ "name": name, "image": image, "roles": roles });
        async move {
            check?;
            client
                .post(Route::raw("/guilds/{guild_id}/emojis", guild_id))
                .json(&body)?
                .send_json()
                .await
        }
    }

    /// `PATCH /guilds/{guild_id}/emojis/{emoji_id}`.
    pub fn update_guild_emoji(
        &self,
        guild_id: Id<GuildMarker>,
        emoji_id: Id<EmojiMarker>,
    ) -> UpdateGuildEmoji {
        UpdateGuildEmoji::new(
            self,
            Method::PATCH,
            Route::raw("/guilds/{guild_id}/emojis/{emoji_id}", (guild_id, emoji_id)),
        )
    }

    simple! {
        /// `DELETE /guilds/{guild_id}/emojis/{emoji_id}`.
        fn delete_guild_emoji(guild_id: Id<GuildMarker>, emoji_id: Id<EmojiMarker>)
            -> () = delete "/guilds/{guild_id}/emojis/{emoji_id}" => send_empty
    }

    // ---- application emojis ------------------------------------------------

    /// `GET /applications/{application_id}/emojis`: emojis owned by the
    /// app, usable everywhere.
    pub fn get_application_emojis(
        &self,
        application_id: Id<ApplicationMarker>,
    ) -> impl Future<Output = Result<Vec<Emoji>, HttpError>> + Send + 'static {
        let client = self.clone();
        async move {
            let list: Items<Emoji> = client
                .get(Route::raw(
                    "/applications/{application_id}/emojis",
                    application_id,
                ))
                .send_json()
                .await?;
            Ok(list.items)
        }
    }

    simple! {
        /// `GET /applications/{application_id}/emojis/{emoji_id}`.
        fn get_application_emoji(application_id: Id<ApplicationMarker>, emoji_id: Id<EmojiMarker>)
            -> Emoji = get "/applications/{application_id}/emojis/{emoji_id}" => send_json
    }

    /// `POST /applications/{application_id}/emojis`: `image` is a data URI
    /// (max. 256 KiB).
    pub fn create_application_emoji(
        &self,
        application_id: Id<ApplicationMarker>,
        name: &str,
        image: &str,
    ) -> impl Future<Output = Result<Emoji, HttpError>> + Send + 'static {
        let client = self.clone();
        let check = check_data_uri(image, EMOJI_MAX_BYTES, "emoji image");
        let body = serde_json::json!({ "name": name, "image": image });
        async move {
            check?;
            client
                .post(Route::raw(
                    "/applications/{application_id}/emojis",
                    application_id,
                ))
                .json(&body)?
                .send_json()
                .await
        }
    }

    /// `PATCH /applications/{application_id}/emojis/{emoji_id}`: rename.
    pub fn update_application_emoji(
        &self,
        application_id: Id<ApplicationMarker>,
        emoji_id: Id<EmojiMarker>,
        name: &str,
    ) -> impl Future<Output = Result<Emoji, HttpError>> + Send + 'static {
        let client = self.clone();
        let body = serde_json::json!({ "name": name });
        async move {
            client
                .patch(Route::raw(
                    "/applications/{application_id}/emojis/{emoji_id}",
                    (application_id, emoji_id),
                ))
                .json(&body)?
                .send_json()
                .await
        }
    }

    simple! {
        /// `DELETE /applications/{application_id}/emojis/{emoji_id}`.
        fn delete_application_emoji(application_id: Id<ApplicationMarker>, emoji_id: Id<EmojiMarker>)
            -> () = delete "/applications/{application_id}/emojis/{emoji_id}" => send_empty
    }

    // ---- stickers ----------------------------------------------------------

    simple! {
        /// `GET /stickers/{sticker_id}`.
        fn get_sticker(sticker_id: Id<StickerMarker>)
            -> Sticker = get "/stickers/{sticker_id}" => send_json
    }

    /// `GET /sticker-packs`: Discord's standard sticker packs.
    pub fn get_sticker_packs(
        &self,
    ) -> impl Future<Output = Result<Vec<StickerPack>, HttpError>> + Send + 'static {
        let client = self.clone();
        async move {
            let list: StickerPacks = client
                .get(Route::raw("/sticker-packs", ()))
                .send_json()
                .await?;
            Ok(list.sticker_packs)
        }
    }

    simple! {
        /// `GET /sticker-packs/{pack_id}`.
        fn get_sticker_pack(pack_id: Id<StickerPackMarker>)
            -> StickerPack = get "/sticker-packs/{pack_id}" => send_json
    }

    simple! {
        /// `GET /guilds/{guild_id}/stickers`.
        fn get_guild_stickers(guild_id: Id<GuildMarker>)
            -> Vec<Sticker> = get "/guilds/{guild_id}/stickers" => send_json
    }

    simple! {
        /// `GET /guilds/{guild_id}/stickers/{sticker_id}`.
        fn get_guild_sticker(guild_id: Id<GuildMarker>, sticker_id: Id<StickerMarker>)
            -> Sticker = get "/guilds/{guild_id}/stickers/{sticker_id}" => send_json
    }

    /// `POST /guilds/{guild_id}/stickers`: upload a PNG, APNG, GIF or
    /// Lottie JSON file (max. 512 KiB). `tags` are autocomplete keywords.
    pub fn create_guild_sticker(
        &self,
        guild_id: Id<GuildMarker>,
        name: &str,
        description: &str,
        tags: &str,
        file: &File,
    ) -> impl Future<Output = Result<Sticker, HttpError>> + Send + 'static {
        let client = self.clone();
        let body = if file.data.len() > STICKER_MAX_BYTES {
            Err(HttpError::Validation(
                "sticker file is larger than 512 KiB".into(),
            ))
        } else {
            encode_form(
                &[("name", name), ("description", description), ("tags", tags)],
                "file",
                file,
            )
        };
        async move {
            client
                .post(Route::raw("/guilds/{guild_id}/stickers", guild_id))
                .encoded(body?)
                .send_json()
                .await
        }
    }

    /// `PATCH /guilds/{guild_id}/stickers/{sticker_id}`.
    pub fn update_guild_sticker(
        &self,
        guild_id: Id<GuildMarker>,
        sticker_id: Id<StickerMarker>,
    ) -> UpdateGuildSticker {
        UpdateGuildSticker::new(
            self,
            Method::PATCH,
            Route::raw(
                "/guilds/{guild_id}/stickers/{sticker_id}",
                (guild_id, sticker_id),
            ),
        )
    }

    simple! {
        /// `DELETE /guilds/{guild_id}/stickers/{sticker_id}`.
        fn delete_guild_sticker(guild_id: Id<GuildMarker>, sticker_id: Id<StickerMarker>)
            -> () = delete "/guilds/{guild_id}/stickers/{sticker_id}" => send_empty
    }

    // ---- soundboard --------------------------------------------------------

    /// `POST /channels/{channel_id}/send-soundboard-sound`: play a sound in
    /// the voice channel the bot is connected to. `source_guild_id` is
    /// needed for sounds of another guild.
    pub fn send_soundboard_sound(
        &self,
        channel_id: Id<ChannelMarker>,
        sound_id: Id<SoundMarker>,
        source_guild_id: Option<Id<GuildMarker>>,
    ) -> impl Future<Output = Result<(), HttpError>> + Send + 'static {
        let client = self.clone();
        let body = serde_json::json!({ "sound_id": sound_id, "source_guild_id": source_guild_id });
        async move {
            client
                .post(Route::raw(
                    "/channels/{channel_id}/send-soundboard-sound",
                    channel_id,
                ))
                .json(&body)?
                .send_empty()
                .await
        }
    }

    simple! {
        /// `GET /soundboard-default-sounds`.
        fn get_default_soundboard_sounds()
            -> Vec<SoundboardSound> = get "/soundboard-default-sounds" => send_json
    }

    /// `GET /guilds/{guild_id}/soundboard-sounds`.
    pub fn get_guild_soundboard_sounds(
        &self,
        guild_id: Id<GuildMarker>,
    ) -> impl Future<Output = Result<Vec<SoundboardSound>, HttpError>> + Send + 'static {
        let client = self.clone();
        async move {
            let list: Items<SoundboardSound> = client
                .get(Route::raw("/guilds/{guild_id}/soundboard-sounds", guild_id))
                .send_json()
                .await?;
            Ok(list.items)
        }
    }

    simple! {
        /// `GET /guilds/{guild_id}/soundboard-sounds/{sound_id}`.
        fn get_guild_soundboard_sound(guild_id: Id<GuildMarker>, sound_id: Id<SoundMarker>)
            -> SoundboardSound = get "/guilds/{guild_id}/soundboard-sounds/{sound_id}" => send_json
    }

    /// `POST /guilds/{guild_id}/soundboard-sounds`: `sound` is an MP3 or OGG
    /// data URI (max. 512 KiB, max. 5.2 s).
    pub fn create_guild_soundboard_sound(
        &self,
        guild_id: Id<GuildMarker>,
        name: &str,
        sound: &str,
    ) -> GuildSoundRequest {
        let req = GuildSoundRequest::new(
            self,
            Method::POST,
            Route::raw("/guilds/{guild_id}/soundboard-sounds", guild_id),
        );
        match check_data_uri(sound, SOUND_MAX_BYTES, "sound") {
            Ok(()) => req.name(name).set("sound", sound),
            Err(e) => {
                let mut req = req;
                req.error = Some(e.to_string());
                req
            }
        }
    }

    /// `PATCH /guilds/{guild_id}/soundboard-sounds/{sound_id}`.
    pub fn update_guild_soundboard_sound(
        &self,
        guild_id: Id<GuildMarker>,
        sound_id: Id<SoundMarker>,
    ) -> GuildSoundRequest {
        GuildSoundRequest::new(
            self,
            Method::PATCH,
            Route::raw(
                "/guilds/{guild_id}/soundboard-sounds/{sound_id}",
                (guild_id, sound_id),
            ),
        )
    }

    simple! {
        /// `DELETE /guilds/{guild_id}/soundboard-sounds/{sound_id}`.
        fn delete_guild_soundboard_sound(guild_id: Id<GuildMarker>, sound_id: Id<SoundMarker>)
            -> () = delete "/guilds/{guild_id}/soundboard-sounds/{sound_id}" => send_empty
    }
}
