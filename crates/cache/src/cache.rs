//! The in-memory cache.

use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::hash::Hash;
use std::sync::Arc;

use cordex_model::channel::Channel;
use cordex_model::emoji::Emoji;
use cordex_model::gateway::Event;
use cordex_model::gateway::event::{GuildCreate, GuildMemberUpdate};
use cordex_model::gateway::presence::PresenceUpdate;
use cordex_model::guild::{Guild, Member, Role};
use cordex_model::id::Id;
use cordex_model::id::marker::{
    ChannelMarker, EmojiMarker, GuildMarker, MessageMarker, RoleMarker, ScheduledEventMarker,
    SoundMarker, StageMarker, StickerMarker, UserMarker,
};
use cordex_model::message::Message;
use cordex_model::scheduled_event::ScheduledEvent;
use cordex_model::soundboard::SoundboardSound;
use cordex_model::stage::StageInstance;
use cordex_model::sticker::Sticker;
use cordex_model::user::User;
use cordex_model::voice::VoiceState;
use parking_lot::RwLock;

use crate::config::{CacheConfig, CacheParts, ResourceType};
use crate::strip;

type GuildId = Id<GuildMarker>;
type ChannelId = Id<ChannelMarker>;
type UserId = Id<UserMarker>;

/// Flat map of shared entries.
type Flat<K, V> = RwLock<HashMap<K, Arc<V>>>;
/// Entries grouped by guild, so a guild delete drops them in one step.
type ByGuild<K, V> = RwLock<HashMap<GuildId, HashMap<K, Arc<V>>>>;

/// Entry counts per resource (metrics).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct CacheStats {
    /// Available guilds.
    pub guilds: usize,
    /// Guilds marked unavailable.
    pub unavailable_guilds: usize,
    /// Channels.
    pub channels: usize,
    /// Threads.
    pub threads: usize,
    /// Users.
    pub users: usize,
    /// Members over all guilds.
    pub members: usize,
    /// Roles.
    pub roles: usize,
    /// Emojis.
    pub emojis: usize,
    /// Stickers.
    pub stickers: usize,
    /// Soundboard sounds.
    pub soundboard_sounds: usize,
    /// Voice states.
    pub voice_states: usize,
    /// Presences.
    pub presences: usize,
    /// Stage instances.
    pub stage_instances: usize,
    /// Scheduled events.
    pub scheduled_events: usize,
    /// Messages over all channels.
    pub messages: usize,
}

/// A [`InMemoryCache::permissions`] lookup is missing a resource.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum PermissionCacheError {
    /// Guild is not cached.
    Guild,
    /// Member is not cached.
    Member,
    /// Channel (or the parent channel of a thread) is not cached.
    Channel,
    /// `@everyone` or one of the member's roles is not cached.
    Roles,
    /// [`CacheParts::MEMBER_ROLES`](crate::CacheParts::MEMBER_ROLES) is
    /// off, so the member's roles are unknown.
    MemberRolesOff,
    /// [`CacheParts::CHANNEL_OVERWRITES`](crate::CacheParts::CHANNEL_OVERWRITES)
    /// is off, so the channel overwrites are unknown.
    OverwritesOff,
}

impl std::fmt::Display for PermissionCacheError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Guild => "guild is not cached",
            Self::Member => "member is not cached",
            Self::Channel => "channel is not cached",
            Self::Roles => "roles are not cached",
            Self::MemberRolesOff => "member roles are not cached (enable CacheParts::MEMBER_ROLES)",
            Self::OverwritesOff => {
                "channel overwrites are not cached (enable CacheParts::CHANNEL_OVERWRITES)"
            }
        })
    }
}

impl std::error::Error for PermissionCacheError {}

/// In-memory cache fed with [`InMemoryCache::update`]. Thread-safe; share it
/// with `Arc`. Reads return `Arc` clones and never hold a lock across an
/// `.await`.
///
/// ```
/// use cordex_cache::{CacheConfig, InMemoryCache, ResourceType};
///
/// let cache = InMemoryCache::new(CacheConfig::new().resources(ResourceType::GUILDS));
/// // In the event loop: `cache.update(&event);`
/// assert_eq!(cache.stats().guilds, 0);
/// ```
#[derive(Debug, Default)]
pub struct InMemoryCache {
    config: RwLock<CacheConfig>,
    current_user: RwLock<Option<Arc<User>>>,
    guilds: Flat<GuildId, Guild>,
    unavailable: RwLock<HashSet<GuildId>>,
    channels: Flat<ChannelId, Channel>,
    threads: Flat<ChannelId, Channel>,
    users: Flat<UserId, User>,
    members: ByGuild<UserId, Member>,
    roles: ByGuild<Id<RoleMarker>, Role>,
    emojis: ByGuild<Id<EmojiMarker>, Emoji>,
    stickers: ByGuild<Id<StickerMarker>, Sticker>,
    sounds: ByGuild<Id<SoundMarker>, SoundboardSound>,
    voice_states: ByGuild<UserId, VoiceState>,
    presences: ByGuild<UserId, PresenceUpdate>,
    stage_instances: ByGuild<Id<StageMarker>, StageInstance>,
    scheduled_events: ByGuild<Id<ScheduledEventMarker>, ScheduledEvent>,
    messages: RwLock<HashMap<ChannelId, VecDeque<Arc<Message>>>>,
}

impl InMemoryCache {
    /// Cache with `config`.
    pub fn new(config: CacheConfig) -> Self {
        Self {
            config: RwLock::new(config),
            ..Self::default()
        }
    }

    /// Current configuration.
    pub fn config(&self) -> CacheConfig {
        self.config.read().clone()
    }

    /// Change the configuration while running. Data of resources and parts
    /// that are switched off is dropped (or cleared) at once; newly enabled
    /// ones fill from the next events. Lower limits trim existing data.
    pub fn set_config(&self, config: CacheConfig) {
        // Holding the write lock waits for running `update` calls (they hold
        // the read lock), so no stale data slips in after the cleanup.
        let mut guard = self.config.write();
        self.apply_config(&config);
        *guard = config;
    }

    fn apply_config(&self, c: &CacheConfig) {
        use ResourceType as R;
        if c.has(R::GUILDS) {
            restrip_flat(&self.guilds, |g| strip::guild(g, c));
        } else {
            self.guilds.write().clear();
            self.unavailable.write().clear();
        }
        if c.has(R::CHANNELS) {
            restrip_flat(&self.channels, |ch| strip::channel(ch, c));
        } else {
            self.channels.write().clear();
        }
        if c.has(R::THREADS) {
            restrip_flat(&self.threads, |ch| strip::channel(ch, c));
        } else {
            self.threads.write().clear();
        }
        if c.has(R::USERS) {
            restrip_flat(&self.users, |u| strip::user(u, c));
            if let Some(u) = self.current_user.write().as_mut() {
                strip::user(Arc::make_mut(u), c);
            }
        } else {
            self.users.write().clear();
            *self.current_user.write() = None;
        }
        if c.has(R::MEMBERS) {
            restrip_by_guild(&self.members, |m| strip::member(m, c));
            if let Some(cap) = c.member_cap() {
                trim_by_guild(&self.members, cap);
            }
        } else {
            self.members.write().clear();
        }
        clear_unless(c.has(R::ROLES), &self.roles);
        clear_unless(c.has(R::EMOJIS), &self.emojis);
        clear_unless(c.has(R::STICKERS), &self.stickers);
        clear_unless(c.has(R::SOUNDBOARD_SOUNDS), &self.sounds);
        clear_unless(c.has(R::STAGE_INSTANCES), &self.stage_instances);
        clear_unless(c.has(R::SCHEDULED_EVENTS), &self.scheduled_events);
        if c.has(R::VOICE_STATES) {
            restrip_by_guild(&self.voice_states, |v| strip::voice_state(v, c));
        } else {
            self.voice_states.write().clear();
        }
        if c.has(R::PRESENCES) {
            restrip_by_guild(&self.presences, |p| strip::presence(p, c));
            if let Some(cap) = c.presence_cap() {
                trim_by_guild(&self.presences, cap);
            }
        } else {
            self.presences.write().clear();
        }
        let size = c.messages_per_channel();
        if c.has(R::MESSAGES) && size > 0 {
            let mut messages = self.messages.write();
            for ring in messages.values_mut() {
                while ring.len() > size {
                    ring.pop_front();
                }
                for m in ring.iter_mut() {
                    strip::message(Arc::make_mut(m), c);
                }
            }
        } else {
            self.messages.write().clear();
        }
    }

    /// Apply one gateway event. Events for switched-off resources are
    /// ignored before any work is done.
    #[allow(
        clippy::too_many_lines,
        reason = "one flat match over every cached event reads better than many tiny functions"
    )]
    pub fn update(&self, event: &Event) {
        use ResourceType as R;
        let guard = self.config.read();
        let c: &CacheConfig = &guard;
        if c.enabled_resources().is_empty() {
            return;
        }
        match event {
            Event::Ready(ready) => {
                if c.has(R::USERS) {
                    let mut user = ready.user.clone();
                    strip::user(&mut user, c);
                    *self.current_user.write() = Some(Arc::new(user));
                }
                if c.has(R::GUILDS) {
                    let mut unavailable = self.unavailable.write();
                    for g in &ready.guilds {
                        unavailable.insert(g.id);
                    }
                }
            }
            Event::UserUpdate(user) if c.has(R::USERS) => {
                let mut user = (**user).clone();
                strip::user(&mut user, c);
                *self.current_user.write() = Some(Arc::new(user));
            }
            Event::GuildCreate(GuildCreate::Available(g)) => self.guild_create(g, c),
            Event::GuildCreate(GuildCreate::Unavailable(g)) => {
                if c.has(R::GUILDS) {
                    self.unavailable.write().insert(g.id);
                }
            }
            Event::GuildUpdate(guild) if c.has(R::GUILDS) => {
                let mut guild = (**guild).clone();
                strip::guild(&mut guild, c);
                self.guilds.write().insert(guild.id, Arc::new(guild));
            }
            Event::GuildDelete(g) => {
                if g.unavailable {
                    if c.has(R::GUILDS) {
                        self.unavailable.write().insert(g.id);
                    }
                } else {
                    self.remove_guild(g.id);
                }
            }
            Event::ChannelCreate(ch) | Event::ChannelUpdate(ch) if c.has(R::CHANNELS) => {
                let mut ch = (**ch).clone();
                strip::channel(&mut ch, c);
                self.channels.write().insert(ch.id, Arc::new(ch));
            }
            Event::ChannelDelete(ch) => {
                self.channels.write().remove(&ch.id);
                self.messages.write().remove(&ch.id);
            }
            Event::ThreadCreate(ch) | Event::ThreadUpdate(ch) if c.has(R::THREADS) => {
                let mut ch = (**ch).clone();
                strip::channel(&mut ch, c);
                self.threads.write().insert(ch.id, Arc::new(ch));
            }
            Event::ThreadDelete(ch) => {
                self.threads.write().remove(&ch.id);
                self.messages.write().remove(&ch.id);
            }
            Event::ThreadListSync(sync) if c.has(R::THREADS) => {
                let mut threads = self.threads.write();
                threads.retain(|_, t| {
                    t.guild_id != Some(sync.guild_id)
                        || !(sync.channel_ids.is_empty()
                            || t.parent_id.is_some_and(|p| sync.channel_ids.contains(&p)))
                });
                for t in &sync.threads {
                    let mut t = t.clone();
                    t.guild_id.get_or_insert(sync.guild_id);
                    strip::channel(&mut t, c);
                    threads.insert(t.id, Arc::new(t));
                }
            }
            Event::GuildMemberAdd(m) => {
                if let Some(gid) = m.guild_id {
                    self.insert_member(gid, m, c);
                }
            }
            Event::GuildMemberUpdate(u) => self.member_update(u, c),
            Event::GuildMemberRemove(r) => {
                if c.has(R::MEMBERS) {
                    if let Some(g) = self.members.write().get_mut(&r.guild_id) {
                        g.remove(&r.user.id);
                    }
                    self.release_user(r.user.id);
                }
            }
            Event::GuildMembersChunk(chunk) => {
                for m in &chunk.members {
                    self.insert_member(chunk.guild_id, m, c);
                }
            }
            Event::GuildRoleCreate(r) | Event::GuildRoleUpdate(r) if c.has(R::ROLES) => {
                insert_by_guild(&self.roles, r.guild_id, r.role.id, r.role.clone());
            }
            Event::GuildRoleDelete(r) if c.has(R::ROLES) => {
                if let Some(g) = self.roles.write().get_mut(&r.guild_id) {
                    g.remove(&r.role_id);
                }
            }
            Event::GuildEmojisUpdate(e) if c.has(R::EMOJIS) => {
                replace_guild(
                    &self.emojis,
                    e.guild_id,
                    e.emojis
                        .iter()
                        .filter_map(|em| em.id.map(|id| (id, em.clone()))),
                );
            }
            Event::GuildStickersUpdate(s) if c.has(R::STICKERS) => {
                replace_guild(
                    &self.stickers,
                    s.guild_id,
                    s.stickers.iter().map(|st| (st.id, st.clone())),
                );
            }
            Event::GuildSoundboardSoundCreate(s) | Event::GuildSoundboardSoundUpdate(s)
                if c.has(R::SOUNDBOARD_SOUNDS) =>
            {
                if let Some(gid) = s.guild_id {
                    insert_by_guild(&self.sounds, gid, s.sound_id, (**s).clone());
                }
            }
            Event::GuildSoundboardSoundDelete(d) if c.has(R::SOUNDBOARD_SOUNDS) => {
                if let Some(g) = self.sounds.write().get_mut(&d.guild_id) {
                    g.remove(&d.sound_id);
                }
            }
            Event::GuildSoundboardSoundsUpdate(s) | Event::SoundboardSounds(s)
                if c.has(R::SOUNDBOARD_SOUNDS) =>
            {
                replace_guild(
                    &self.sounds,
                    s.guild_id,
                    s.soundboard_sounds.iter().map(|x| (x.sound_id, x.clone())),
                );
            }
            Event::VoiceStateUpdate(v) if c.has(R::VOICE_STATES) => {
                if let Some(gid) = v.guild_id {
                    self.insert_voice_state(gid, v, c);
                }
            }
            Event::PresenceUpdate(p) if c.has(R::PRESENCES) => {
                if let Some(gid) = p.guild_id {
                    self.insert_presence(gid, p, c);
                }
            }
            Event::StageInstanceCreate(s) | Event::StageInstanceUpdate(s)
                if c.has(R::STAGE_INSTANCES) =>
            {
                insert_by_guild(&self.stage_instances, s.guild_id, s.id, (**s).clone());
            }
            Event::StageInstanceDelete(s) if c.has(R::STAGE_INSTANCES) => {
                if let Some(g) = self.stage_instances.write().get_mut(&s.guild_id) {
                    g.remove(&s.id);
                }
            }
            Event::GuildScheduledEventCreate(e) | Event::GuildScheduledEventUpdate(e)
                if c.has(R::SCHEDULED_EVENTS) =>
            {
                insert_by_guild(&self.scheduled_events, e.guild_id, e.id, (**e).clone());
            }
            Event::GuildScheduledEventDelete(e) if c.has(R::SCHEDULED_EVENTS) => {
                if let Some(g) = self.scheduled_events.write().get_mut(&e.guild_id) {
                    g.remove(&e.id);
                }
            }
            // Without the presences intent `GUILD_CREATE` carries almost no
            // members; the invoking member of an interaction is complete.
            Event::InteractionCreate(i) => {
                if let (Some(gid), Some(member)) = (i.guild_id, i.member.as_deref()) {
                    self.insert_member(gid, member, c);
                }
            }
            Event::MessageCreate(m) => self.message_create(m, c),
            Event::MessageUpdate(m) => self.message_update(m, c),
            Event::MessageDelete(d) => {
                if let Some(ring) = self.messages.write().get_mut(&d.channel_id) {
                    ring.retain(|m| m.id != d.id);
                }
            }
            Event::MessageDeleteBulk(d) => {
                if let Some(ring) = self.messages.write().get_mut(&d.channel_id) {
                    let ids: HashSet<_> = d.ids.iter().collect();
                    ring.retain(|m| !ids.contains(&m.id));
                }
            }
            _ => {}
        }
    }

    fn guild_create(&self, g: &cordex_model::gateway::event::AvailableGuild, c: &CacheConfig) {
        use ResourceType as R;
        let gid = g.guild.id;
        if c.has(R::GUILDS) {
            self.unavailable.write().remove(&gid);
            let mut guild = g.guild.clone();
            strip::guild(&mut guild, c);
            self.guilds.write().insert(gid, Arc::new(guild));
        }
        if c.has(R::CHANNELS) {
            let mut channels = self.channels.write();
            channels.retain(|_, ch| ch.guild_id != Some(gid));
            for ch in &g.channels {
                let mut ch = ch.clone();
                ch.guild_id = Some(gid);
                strip::channel(&mut ch, c);
                channels.insert(ch.id, Arc::new(ch));
            }
        }
        if c.has(R::THREADS) {
            let mut threads = self.threads.write();
            threads.retain(|_, t| t.guild_id != Some(gid));
            for t in &g.threads {
                let mut t = t.clone();
                t.guild_id = Some(gid);
                strip::channel(&mut t, c);
                threads.insert(t.id, Arc::new(t));
            }
        }
        if c.has(R::MEMBERS) {
            self.members.write().remove(&gid);
            for m in &g.members {
                self.insert_member(gid, m, c);
            }
        }
        if c.has(R::ROLES) {
            replace_guild(
                &self.roles,
                gid,
                g.guild.roles.iter().map(|r| (r.id, r.clone())),
            );
        }
        if c.has(R::EMOJIS) {
            replace_guild(
                &self.emojis,
                gid,
                g.guild
                    .emojis
                    .iter()
                    .filter_map(|e| e.id.map(|id| (id, e.clone()))),
            );
        }
        if c.has(R::STICKERS) {
            replace_guild(
                &self.stickers,
                gid,
                g.guild.stickers.iter().map(|s| (s.id, s.clone())),
            );
        }
        if c.has(R::SOUNDBOARD_SOUNDS) {
            replace_guild(
                &self.sounds,
                gid,
                g.soundboard_sounds.iter().map(|s| {
                    let mut s = s.clone();
                    s.guild_id = Some(gid);
                    (s.sound_id, s)
                }),
            );
        }
        if c.has(R::VOICE_STATES) {
            self.voice_states.write().remove(&gid);
            for v in &g.voice_states {
                self.insert_voice_state(gid, v, c);
            }
        }
        if c.has(R::PRESENCES) {
            self.presences.write().remove(&gid);
            for p in &g.presences {
                self.insert_presence(gid, p, c);
            }
        }
        if c.has(R::STAGE_INSTANCES) {
            replace_guild(
                &self.stage_instances,
                gid,
                g.stage_instances.iter().map(|s| (s.id, s.clone())),
            );
        }
        if c.has(R::SCHEDULED_EVENTS) {
            replace_guild(
                &self.scheduled_events,
                gid,
                g.guild_scheduled_events.iter().map(|e| (e.id, e.clone())),
            );
        }
    }

    /// Guild left or deleted: drop it and everything that belongs to it.
    fn remove_guild(&self, gid: GuildId) {
        self.guilds.write().remove(&gid);
        self.unavailable.write().remove(&gid);
        let mut gone_channels: Vec<ChannelId> = Vec::new();
        self.channels.write().retain(|id, ch| {
            let keep = ch.guild_id != Some(gid);
            if !keep {
                gone_channels.push(*id);
            }
            keep
        });
        self.threads.write().retain(|id, t| {
            let keep = t.guild_id != Some(gid);
            if !keep {
                gone_channels.push(*id);
            }
            keep
        });
        {
            let mut messages = self.messages.write();
            messages.retain(|cid, ring| {
                !gone_channels.contains(cid) && ring.front().is_none_or(|m| m.guild_id != Some(gid))
            });
        }
        let users: Vec<UserId> = self
            .members
            .write()
            .remove(&gid)
            .map(|m| m.into_keys().collect())
            .unwrap_or_default();
        for u in users {
            self.release_user(u);
        }
        self.roles.write().remove(&gid);
        self.emojis.write().remove(&gid);
        self.stickers.write().remove(&gid);
        self.sounds.write().remove(&gid);
        self.voice_states.write().remove(&gid);
        self.presences.write().remove(&gid);
        self.stage_instances.write().remove(&gid);
        self.scheduled_events.write().remove(&gid);
    }

    fn insert_member(&self, gid: GuildId, m: &Member, c: &CacheConfig) {
        let Some(user) = m.user.as_ref() else {
            return;
        };
        let uid = user.id;
        // Users are reference-counted through member entries, so without
        // members only the current user is kept.
        if !c.has(ResourceType::MEMBERS) {
            return;
        }
        {
            let mut members = self.members.write();
            let guild = members.entry(gid).or_default();
            let known = guild.contains_key(&uid);
            if !known && c.member_cap().is_some_and(|cap| guild.len() >= cap) {
                return;
            }
            let mut member = m.clone();
            member.guild_id = Some(gid);
            strip::member(&mut member, c);
            guild.insert(uid, Arc::new(member));
        }
        if c.has(ResourceType::USERS) {
            let mut user = user.clone();
            strip::user(&mut user, c);
            self.users.write().insert(uid, Arc::new(user));
        }
    }

    fn member_update(&self, u: &GuildMemberUpdate, c: &CacheConfig) {
        if c.has(ResourceType::MEMBERS) {
            let mut members = self.members.write();
            if let Some(existing) = members
                .get_mut(&u.guild_id)
                .and_then(|g| g.get_mut(&u.user.id))
            {
                let m = Arc::make_mut(existing);
                m.roles.clone_from(&u.roles);
                m.nick.clone_from(&u.nick);
                m.avatar.clone_from(&u.avatar);
                m.banner.clone_from(&u.banner);
                if u.joined_at.is_some() {
                    m.joined_at = u.joined_at;
                }
                m.premium_since = u.premium_since;
                if let Some(deaf) = u.deaf {
                    m.deaf = deaf;
                }
                if let Some(mute) = u.mute {
                    m.mute = mute;
                }
                if let Some(pending) = u.pending {
                    m.pending = pending;
                }
                m.communication_disabled_until = u.communication_disabled_until;
                m.avatar_decoration_data
                    .clone_from(&u.avatar_decoration_data);
                m.collectibles.clone_from(&u.collectibles);
                m.flags = u.flags;
                strip::member(m, c);
            }
        }
        if c.has(ResourceType::USERS) && self.users.read().contains_key(&u.user.id) {
            let mut user = u.user.clone();
            strip::user(&mut user, c);
            self.users.write().insert(user.id, Arc::new(user));
        }
    }

    /// Drop a user once no guild has a member entry for it any more.
    fn release_user(&self, uid: UserId) {
        if self
            .current_user
            .read()
            .as_ref()
            .is_some_and(|u| u.id == uid)
        {
            return;
        }
        let still_member = self.members.read().values().any(|g| g.contains_key(&uid));
        if !still_member {
            self.users.write().remove(&uid);
        }
    }

    fn insert_voice_state(&self, gid: GuildId, v: &VoiceState, c: &CacheConfig) {
        let mut voice = self.voice_states.write();
        if v.channel_id.is_none() {
            if let Some(g) = voice.get_mut(&gid) {
                g.remove(&v.user_id);
            }
            return;
        }
        let mut v = v.clone();
        v.guild_id = Some(gid);
        strip::voice_state(&mut v, c);
        voice.entry(gid).or_default().insert(v.user_id, Arc::new(v));
    }

    fn insert_presence(&self, gid: GuildId, p: &PresenceUpdate, c: &CacheConfig) {
        let Some(uid) = p.user.as_ref().map(|u| u.id) else {
            return;
        };
        let mut presences = self.presences.write();
        let guild = presences.entry(gid).or_default();
        if !guild.contains_key(&uid) && c.presence_cap().is_some_and(|cap| guild.len() >= cap) {
            return;
        }
        let mut p = p.clone();
        p.guild_id = Some(gid);
        strip::presence(&mut p, c);
        guild.insert(uid, Arc::new(p));
    }

    fn message_create(&self, m: &Message, c: &CacheConfig) {
        let size = c.messages_per_channel();
        if !c.has(ResourceType::MESSAGES) || size == 0 {
            return;
        }
        let mut m = m.clone();
        strip::message(&mut m, c);
        let mut messages = self.messages.write();
        let ring = messages.entry(m.channel_id).or_default();
        ring.push_back(Arc::new(m));
        while ring.len() > size {
            ring.pop_front();
        }
    }

    fn message_update(&self, m: &Message, c: &CacheConfig) {
        if !c.has(ResourceType::MESSAGES) {
            return;
        }
        let mut messages = self.messages.write();
        if let Some(slot) = messages
            .get_mut(&m.channel_id)
            .and_then(|ring| ring.iter_mut().find(|x| x.id == m.id))
        {
            let mut m = m.clone();
            strip::message(&mut m, c);
            *slot = Arc::new(m);
        }
    }

    // ---- explicit inserts (backfill) ------------------------------------

    /// Insert messages fetched elsewhere, e.g. over REST at startup (the
    /// cache never fetches by itself). The same switches apply as for live
    /// events: nothing is stored unless [`ResourceType::MESSAGES`] is on and
    /// the per-channel size is above 0; parts that are off are cleared.
    ///
    /// Messages are merged by id with what is cached; an entry that is
    /// already cached wins (live events are newer). The channel keeps the
    /// newest `message_cache_size` messages, oldest first. Messages of other
    /// channels are ignored. Returns how many new messages were stored.
    pub fn insert_messages(
        &self,
        channel: ChannelId,
        messages: impl IntoIterator<Item = Message>,
    ) -> usize {
        let guard = self.config.read();
        let c: &CacheConfig = &guard;
        let size = c.messages_per_channel();
        if !c.has(ResourceType::MESSAGES) || size == 0 {
            return 0;
        }
        let mut store = self.messages.write();
        let ring = store.entry(channel).or_default();
        let mut merged: BTreeMap<Id<MessageMarker>, Arc<Message>> =
            ring.drain(..).map(|m| (m.id, m)).collect();
        let mut added = Vec::new();
        for mut m in messages {
            if m.channel_id != channel || merged.contains_key(&m.id) {
                continue;
            }
            strip::message(&mut m, c);
            added.push(m.id);
            merged.insert(m.id, Arc::new(m));
        }
        let skip = merged.len().saturating_sub(size);
        ring.extend(merged.into_values().skip(skip));
        let kept: HashSet<_> = ring.iter().map(|m| m.id).collect();
        added.iter().filter(|id| kept.contains(id)).count()
    }

    /// Insert members fetched elsewhere (member chunks, REST). Same rules as
    /// live events: needs [`ResourceType::MEMBERS`] (users also need
    /// [`ResourceType::USERS`]), honours the member cap and part switches.
    /// Members without a `user` are skipped. Returns how many members the
    /// guild has in the cache afterwards.
    pub fn insert_members(
        &self,
        guild: GuildId,
        members: impl IntoIterator<Item = Member>,
    ) -> usize {
        let guard = self.config.read();
        let c: &CacheConfig = &guard;
        if !c.has(ResourceType::MEMBERS) {
            return 0;
        }
        for m in members {
            self.insert_member(guild, &m, c);
        }
        self.members.read().get(&guild).map_or(0, HashMap::len)
    }

    // ---- lookups -------------------------------------------------------

    /// The bot user (from `READY` / `USER_UPDATE`; needs [`ResourceType::USERS`]).
    pub fn current_user(&self) -> Option<Arc<User>> {
        self.current_user.read().clone()
    }

    /// Guild by id.
    pub fn guild(&self, id: GuildId) -> Option<Arc<Guild>> {
        self.guilds.read().get(&id).cloned()
    }

    /// Ids of all cached (available) guilds.
    pub fn guild_ids(&self) -> Vec<GuildId> {
        self.guilds.read().keys().copied().collect()
    }

    /// Whether the guild is known to be unavailable (outage or not yet loaded).
    pub fn is_unavailable(&self, id: GuildId) -> bool {
        self.unavailable.read().contains(&id)
    }

    /// Channel (not thread) by id.
    pub fn channel(&self, id: ChannelId) -> Option<Arc<Channel>> {
        self.channels.read().get(&id).cloned()
    }

    /// Channels of a guild.
    pub fn guild_channels(&self, guild: GuildId) -> Vec<Arc<Channel>> {
        self.channels
            .read()
            .values()
            .filter(|c| c.guild_id == Some(guild))
            .cloned()
            .collect()
    }

    /// Thread by id.
    pub fn thread(&self, id: ChannelId) -> Option<Arc<Channel>> {
        self.threads.read().get(&id).cloned()
    }

    /// Threads of a guild.
    pub fn guild_threads(&self, guild: GuildId) -> Vec<Arc<Channel>> {
        self.threads
            .read()
            .values()
            .filter(|t| t.guild_id == Some(guild))
            .cloned()
            .collect()
    }

    /// User by id.
    pub fn user(&self, id: UserId) -> Option<Arc<User>> {
        self.users.read().get(&id).cloned()
    }

    /// Member of a guild (without `user`; use [`Self::user`]).
    pub fn member(&self, guild: GuildId, user: UserId) -> Option<Arc<Member>> {
        get_by_guild(&self.members, guild, &user)
    }

    /// All cached members of a guild.
    pub fn guild_members(&self, guild: GuildId) -> Vec<Arc<Member>> {
        all_by_guild(&self.members, guild)
    }

    /// Role by guild and id.
    pub fn role(&self, guild: GuildId, role: Id<RoleMarker>) -> Option<Arc<Role>> {
        get_by_guild(&self.roles, guild, &role)
    }

    /// Roles of a guild.
    pub fn guild_roles(&self, guild: GuildId) -> Vec<Arc<Role>> {
        all_by_guild(&self.roles, guild)
    }

    /// Channel permissions for `user` in `channel` (or a thread). Needs the
    /// guild, member, roles and channel in cache. Names the missing resource
    /// when something is off.
    pub fn permissions(
        &self,
        guild: GuildId,
        user: UserId,
        channel: ChannelId,
    ) -> Result<cordex_model::Permissions, PermissionCacheError> {
        use cordex_model::permission_calc;
        use cordex_model::timestamp::Timestamp;

        {
            let config = self.config.read();
            if !config.has_part(CacheParts::MEMBER_ROLES) {
                return Err(PermissionCacheError::MemberRolesOff);
            }
            if !config.has_part(CacheParts::CHANNEL_OVERWRITES) {
                return Err(PermissionCacheError::OverwritesOff);
            }
        }
        let g = self.guild(guild).ok_or(PermissionCacheError::Guild)?;
        let member = self
            .member(guild, user)
            .ok_or(PermissionCacheError::Member)?;
        let mut ch = self
            .channel(channel)
            .or_else(|| self.thread(channel))
            .ok_or(PermissionCacheError::Channel)?;
        // Threads have no overwrites; Discord uses the parent channel's.
        if ch.is_thread() {
            let parent = ch.parent_id.ok_or(PermissionCacheError::Channel)?;
            ch = self.channel(parent).ok_or(PermissionCacheError::Channel)?;
        }
        let everyone_role = self
            .role(guild, guild.cast())
            .ok_or(PermissionCacheError::Roles)?;
        let role_perms = member
            .roles
            .iter()
            .map(|id| self.role(guild, *id).map(|r| r.permissions))
            .collect::<Option<Vec<_>>>()
            .ok_or(PermissionCacheError::Roles)?;
        let owner_id = g.owner_id.ok_or(PermissionCacheError::Guild)?;
        let now_ms = match std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
            Ok(d) => i64::try_from(d.as_millis()).unwrap_or(i64::MAX),
            Err(_) => 0,
        };
        let timed_out = member.is_timed_out(Timestamp::from_millis(now_ms));
        Ok(permission_calc::channel_permissions(
            owner_id == user,
            everyone_role.permissions,
            role_perms,
            &ch.permission_overwrites,
            guild.cast(),
            user,
            &member.roles,
            timed_out,
        ))
    }

    /// Emoji by guild and id.
    pub fn emoji(&self, guild: GuildId, emoji: Id<EmojiMarker>) -> Option<Arc<Emoji>> {
        get_by_guild(&self.emojis, guild, &emoji)
    }

    /// Emojis of a guild.
    pub fn guild_emojis(&self, guild: GuildId) -> Vec<Arc<Emoji>> {
        all_by_guild(&self.emojis, guild)
    }

    /// Sticker by guild and id.
    pub fn sticker(&self, guild: GuildId, sticker: Id<StickerMarker>) -> Option<Arc<Sticker>> {
        get_by_guild(&self.stickers, guild, &sticker)
    }

    /// Stickers of a guild.
    pub fn guild_stickers(&self, guild: GuildId) -> Vec<Arc<Sticker>> {
        all_by_guild(&self.stickers, guild)
    }

    /// Soundboard sound by guild and id.
    pub fn soundboard_sound(
        &self,
        guild: GuildId,
        sound: Id<SoundMarker>,
    ) -> Option<Arc<SoundboardSound>> {
        get_by_guild(&self.sounds, guild, &sound)
    }

    /// Soundboard sounds of a guild.
    pub fn guild_soundboard_sounds(&self, guild: GuildId) -> Vec<Arc<SoundboardSound>> {
        all_by_guild(&self.sounds, guild)
    }

    /// Voice state of a user in a guild.
    pub fn voice_state(&self, guild: GuildId, user: UserId) -> Option<Arc<VoiceState>> {
        get_by_guild(&self.voice_states, guild, &user)
    }

    /// Voice states of everyone connected to `channel`.
    pub fn channel_voice_states(&self, guild: GuildId, channel: ChannelId) -> Vec<Arc<VoiceState>> {
        all_by_guild(&self.voice_states, guild)
            .into_iter()
            .filter(|v| v.channel_id == Some(channel))
            .collect()
    }

    /// Presence of a user in a guild.
    pub fn presence(&self, guild: GuildId, user: UserId) -> Option<Arc<PresenceUpdate>> {
        get_by_guild(&self.presences, guild, &user)
    }

    /// Stage instance by guild and id.
    pub fn stage_instance(
        &self,
        guild: GuildId,
        id: Id<StageMarker>,
    ) -> Option<Arc<StageInstance>> {
        get_by_guild(&self.stage_instances, guild, &id)
    }

    /// Scheduled event by guild and id.
    pub fn scheduled_event(
        &self,
        guild: GuildId,
        id: Id<ScheduledEventMarker>,
    ) -> Option<Arc<ScheduledEvent>> {
        get_by_guild(&self.scheduled_events, guild, &id)
    }

    /// Scheduled events of a guild.
    pub fn guild_scheduled_events(&self, guild: GuildId) -> Vec<Arc<ScheduledEvent>> {
        all_by_guild(&self.scheduled_events, guild)
    }

    /// Cached message by channel and id.
    pub fn message(&self, channel: ChannelId, id: Id<MessageMarker>) -> Option<Arc<Message>> {
        self.messages
            .read()
            .get(&channel)
            .and_then(|ring| ring.iter().find(|m| m.id == id).cloned())
    }

    /// Cached messages of a channel, oldest first.
    pub fn channel_messages(&self, channel: ChannelId) -> Vec<Arc<Message>> {
        self.messages
            .read()
            .get(&channel)
            .map(|ring| ring.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// Entry counts per resource.
    pub fn stats(&self) -> CacheStats {
        fn count<K, V>(map: &ByGuild<K, V>) -> usize {
            map.read().values().map(HashMap::len).sum()
        }
        CacheStats {
            guilds: self.guilds.read().len(),
            unavailable_guilds: self.unavailable.read().len(),
            channels: self.channels.read().len(),
            threads: self.threads.read().len(),
            users: self.users.read().len(),
            members: count(&self.members),
            roles: count(&self.roles),
            emojis: count(&self.emojis),
            stickers: count(&self.stickers),
            soundboard_sounds: count(&self.sounds),
            voice_states: count(&self.voice_states),
            presences: count(&self.presences),
            stage_instances: count(&self.stage_instances),
            scheduled_events: count(&self.scheduled_events),
            messages: self.messages.read().values().map(VecDeque::len).sum(),
        }
    }
}

fn insert_by_guild<K: Eq + Hash, V>(map: &ByGuild<K, V>, gid: GuildId, key: K, value: V) {
    map.write()
        .entry(gid)
        .or_default()
        .insert(key, Arc::new(value));
}

fn replace_guild<K: Eq + Hash, V>(
    map: &ByGuild<K, V>,
    gid: GuildId,
    items: impl Iterator<Item = (K, V)>,
) {
    let entries: HashMap<K, Arc<V>> = items.map(|(k, v)| (k, Arc::new(v))).collect();
    map.write().insert(gid, entries);
}

fn get_by_guild<K: Eq + Hash, V>(map: &ByGuild<K, V>, gid: GuildId, key: &K) -> Option<Arc<V>> {
    map.read().get(&gid).and_then(|g| g.get(key)).cloned()
}

fn all_by_guild<K, V>(map: &ByGuild<K, V>, gid: GuildId) -> Vec<Arc<V>> {
    map.read()
        .get(&gid)
        .map(|g| g.values().cloned().collect())
        .unwrap_or_default()
}

fn clear_unless<K, V>(enabled: bool, map: &ByGuild<K, V>) {
    if !enabled {
        map.write().clear();
    }
}

fn restrip_flat<K, V: Clone>(map: &Flat<K, V>, f: impl Fn(&mut V)) {
    for v in map.write().values_mut() {
        f(Arc::make_mut(v));
    }
}

fn restrip_by_guild<K, V: Clone>(map: &ByGuild<K, V>, f: impl Fn(&mut V)) {
    for g in map.write().values_mut() {
        for v in g.values_mut() {
            f(Arc::make_mut(v));
        }
    }
}

fn trim_by_guild<K: Clone + Eq + Hash, V>(map: &ByGuild<K, V>, cap: usize) {
    for g in map.write().values_mut() {
        if g.len() > cap {
            let extra: Vec<K> = g.keys().skip(cap).cloned().collect();
            for k in extra {
                g.remove(&k);
            }
        }
    }
}
