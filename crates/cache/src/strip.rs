//! Clear the parts of an entry that the configuration switches off.

use cordex_model::channel::Channel;
use cordex_model::gateway::presence::PresenceUpdate;
use cordex_model::guild::{Guild, Member};
use cordex_model::message::Message;
use cordex_model::user::User;
use cordex_model::voice::VoiceState;

use crate::config::{CacheConfig, CacheParts};

pub(crate) fn user(u: &mut User, c: &CacheConfig) {
    if !c.has_part(CacheParts::USER_PROFILE) {
        u.avatar = None;
        u.banner = None;
        u.accent_color = None;
        u.avatar_decoration_data = None;
        u.collectibles = None;
        u.primary_guild = None;
    }
    // Never keep account-private fields the bot may receive about itself.
    u.email = None;
}

/// Members are stored without `user`; the user lives in the user store.
pub(crate) fn member(m: &mut Member, c: &CacheConfig) {
    m.user = None;
    if !c.has_part(CacheParts::MEMBER_ROLES) {
        m.roles = Vec::new();
    }
    if !c.has_part(CacheParts::MEMBER_PROFILE) {
        m.nick = None;
        m.avatar = None;
        m.banner = None;
        m.avatar_decoration_data = None;
        m.collectibles = None;
    }
}

/// Roles, emojis and stickers are own resources and not kept twice.
pub(crate) fn guild(g: &mut Guild, c: &CacheConfig) {
    g.roles = Vec::new();
    g.emojis = Vec::new();
    g.stickers = Vec::new();
    if !c.has_part(CacheParts::GUILD_FEATURES) {
        g.features = Vec::new();
    }
}

pub(crate) fn channel(ch: &mut Channel, c: &CacheConfig) {
    if !c.has_part(CacheParts::CHANNEL_OVERWRITES) {
        ch.permission_overwrites = Vec::new();
    }
    // Thread member of the bot and DM recipients are not cached data here.
    ch.member = None;
}

pub(crate) fn message(msg: &mut Message, c: &CacheConfig) {
    if !c.has_part(CacheParts::MESSAGE_CONTENT) {
        msg.content = Box::from("");
    }
    if !c.has_part(CacheParts::MESSAGE_EMBEDS) {
        msg.embeds = Vec::new();
    }
    if !c.has_part(CacheParts::MESSAGE_ATTACHMENTS) {
        msg.attachments = Vec::new();
    }
    if !c.has_part(CacheParts::MESSAGE_COMPONENTS) {
        msg.components = Vec::new();
    }
    // Nested copies would grow without bound; look them up by id instead.
    msg.referenced_message = None;
    msg.thread = None;
    user(&mut msg.author, c);
    if let Some(m) = msg.member.as_deref_mut() {
        member(m, c);
    }
}

pub(crate) fn voice_state(v: &mut VoiceState, c: &CacheConfig) {
    if c.has_part(CacheParts::VOICE_STATE_MEMBER) {
        if let Some(m) = v.member.as_deref_mut() {
            member(m, c);
        }
    } else {
        v.member = None;
    }
}

pub(crate) fn presence(p: &mut PresenceUpdate, c: &CacheConfig) {
    if !c.has_part(CacheParts::PRESENCE_ACTIVITIES) {
        p.activities = Vec::new();
    }
    // Presence users are partial (often only the id); the id is the key.
    p.user = None;
}
