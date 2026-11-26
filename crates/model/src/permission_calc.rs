//! Pure permission calculator (topics/permissions.mdx).

use crate::channel::{OverwriteType, PermissionOverwrite};
use crate::guild::{Member, Role};
use crate::id::Id;
use crate::id::marker::{GenericMarker, RoleMarker, UserMarker};
use crate::permissions::Permissions;

/// Permissions that require [`Permissions::SEND_MESSAGES`] (docs, implicit
/// permissions).
const SEND_DEPENDENT: Permissions = Permissions::from_bits_truncate(
    Permissions::SEND_TTS_MESSAGES.bits()
        | Permissions::MENTION_EVERYONE.bits()
        | Permissions::EMBED_LINKS.bits()
        | Permissions::ATTACH_FILES.bits(),
);

/// Highest role position held by `member` in `roles` (falls back to `0` if
/// no matching role is found).
///
/// Ties are broken by Discord's rule: higher `position` wins; equal positions
/// are ordered by lower role snowflake ID.
pub fn highest_role_position(member: &Member, roles: &[Role]) -> (i32, u64) {
    member
        .roles
        .iter()
        .filter_map(|rid| roles.iter().find(|r| r.id == *rid))
        .map(|r| (r.position, u64::MAX - r.id.get()))
        .max()
        .unwrap_or((0, 0))
}

/// Guild-level permissions: `@everyone` plus the member's roles. The owner
/// and anyone with [`Permissions::ADMINISTRATOR`] receive every bit.
pub fn guild_permissions(
    is_owner: bool,
    everyone: Permissions,
    role_permissions: impl IntoIterator<Item = Permissions>,
) -> Permissions {
    if is_owner {
        return Permissions::all();
    }
    let mut bits = everyone;
    for role in role_permissions {
        bits |= role;
    }
    if bits.contains(Permissions::ADMINISTRATOR) {
        Permissions::all()
    } else {
        bits
    }
}

/// Channel overwrites in Discord's order: `@everyone` deny/allow, then all
/// role denies, then all role allows, then the member deny/allow.
pub fn apply_overwrites(
    mut base: Permissions,
    overwrites: &[PermissionOverwrite],
    everyone_id: Id<GenericMarker>,
    member_id: Id<UserMarker>,
    member_roles: &[Id<RoleMarker>],
) -> Permissions {
    if base.contains(Permissions::ADMINISTRATOR) {
        return Permissions::all();
    }
    if let Some(ow) = overwrites
        .iter()
        .find(|o| o.kind == OverwriteType::Role && o.id == everyone_id)
    {
        base = (base - ow.deny) | ow.allow;
    }
    let mut role_deny = Permissions::empty();
    let mut role_allow = Permissions::empty();
    for ow in overwrites {
        if ow.kind != OverwriteType::Role || ow.id == everyone_id {
            continue;
        }
        if member_roles.iter().any(|r| r.get() == ow.id.get()) {
            role_deny |= ow.deny;
            role_allow |= ow.allow;
        }
    }
    base = (base - role_deny) | role_allow;
    if let Some(ow) = overwrites
        .iter()
        .find(|o| o.kind == OverwriteType::Member && o.id.get() == member_id.get())
    {
        base = (base - ow.deny) | ow.allow;
    }
    base
}

/// Implicit channel rules: no `VIEW_CHANNEL` → nothing; without
/// `SEND_MESSAGES` the send-dependent bits are stripped; a timeout leaves
/// only `VIEW_CHANNEL` and `READ_MESSAGE_HISTORY`.
pub fn apply_implicit(permissions: Permissions, timed_out: bool) -> Permissions {
    if !permissions.contains(Permissions::VIEW_CHANNEL) {
        return Permissions::empty();
    }
    let mut bits = permissions;
    if !bits.contains(Permissions::SEND_MESSAGES) {
        bits -= SEND_DEPENDENT;
    }
    if timed_out {
        bits &= Permissions::VIEW_CHANNEL | Permissions::READ_MESSAGE_HISTORY;
    }
    bits
}

/// Guild permissions plus channel overwrites and implicit rules.
///
/// For a thread pass the overwrites of its parent channel (threads have
/// none of their own). Owners and administrators get every bit and are
/// exempt from timeouts. Not modelled: the voice rule that a missing
/// `CONNECT` hides further permissions, and `SEND_MESSAGES_IN_THREADS`
/// taking the place of `SEND_MESSAGES` inside threads.
pub fn channel_permissions(
    is_owner: bool,
    everyone: Permissions,
    role_permissions: impl IntoIterator<Item = Permissions>,
    overwrites: &[PermissionOverwrite],
    everyone_id: Id<GenericMarker>,
    member_id: Id<UserMarker>,
    member_roles: &[Id<RoleMarker>],
    timed_out: bool,
) -> Permissions {
    let guild = guild_permissions(is_owner, everyone, role_permissions);
    if guild == Permissions::all() {
        // Owner or `ADMINISTRATOR`: overwrites, implicit rules and
        // timeouts do not apply (topics/permissions.mdx).
        return guild;
    }
    let with_overwrites = apply_overwrites(guild, overwrites, everyone_id, member_id, member_roles);
    apply_implicit(with_overwrites, timed_out)
}

/// Highest role in Discord's hierarchy order: greatest `position`, with the
/// lower role `id` winning ties.
pub fn highest_role<'a>(roles: impl IntoIterator<Item = &'a Role>) -> Option<&'a Role> {
    roles
        .into_iter()
        .max_by(|a, b| a.position.cmp(&b.position).then_with(|| b.id.cmp(&a.id)))
}

/// Whether `moderator_id` can moderate `target_id` according to Discord's role
/// hierarchy rules:
///
/// - The guild owner can never be moderated (not even by themselves).
/// - The guild owner can moderate every other member.
/// - A target holding [`Permissions::ADMINISTRATOR`] can only be moderated by
///   the guild owner.
/// - Otherwise the moderator's highest role must be strictly higher than the
///   target's highest role (higher `position`, or equal `position` with lower
///   role `id`).
pub fn can_moderate<'a>(
    owner_id: Id<UserMarker>,
    moderator_id: Id<UserMarker>,
    moderator_roles: impl IntoIterator<Item = &'a Role>,
    target_id: Id<UserMarker>,
    target_roles: impl IntoIterator<Item = &'a Role>,
) -> bool {
    if target_id == owner_id {
        return false;
    }
    if moderator_id == owner_id {
        return true;
    }
    let mut target_top: Option<&Role> = None;
    for role in target_roles {
        if role.permissions.contains(Permissions::ADMINISTRATOR) {
            return false;
        }
        if target_top.is_none_or(|top| {
            role.position
                .cmp(&top.position)
                .then_with(|| top.id.cmp(&role.id))
                .is_gt()
        }) {
            target_top = Some(role);
        }
    }
    let Some(mod_top) = highest_role(moderator_roles) else {
        return false;
    };
    let Some(target_top) = target_top else {
        return true;
    };
    mod_top
        .position
        .cmp(&target_top.position)
        .then_with(|| target_top.id.cmp(&mod_top.id))
        .is_gt()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::id::marker::RoleMarker;

    fn id_user(n: u64) -> Id<UserMarker> {
        Id::new(n).unwrap()
    }
    fn id_role(n: u64) -> Id<RoleMarker> {
        Id::new(n).unwrap()
    }
    fn id_generic(n: u64) -> Id<GenericMarker> {
        Id::new(n).unwrap()
    }

    fn ow(
        id: Id<GenericMarker>,
        kind: OverwriteType,
        allow: Permissions,
        deny: Permissions,
    ) -> PermissionOverwrite {
        PermissionOverwrite {
            id,
            kind,
            allow,
            deny,
        }
    }

    #[test]
    fn owner_and_admin_are_all() {
        assert_eq!(
            guild_permissions(true, Permissions::empty(), []),
            Permissions::all()
        );
        assert_eq!(
            guild_permissions(false, Permissions::ADMINISTRATOR, []),
            Permissions::all()
        );
    }

    #[test]
    fn overwrites_and_timeout() {
        let everyone = id_generic(1);
        let role = id_role(2);
        let member = id_user(3);
        let base = Permissions::VIEW_CHANNEL | Permissions::SEND_MESSAGES | Permissions::SPEAK;
        let overwrites = [
            ow(
                everyone,
                OverwriteType::Role,
                Permissions::empty(),
                Permissions::SEND_MESSAGES,
            ),
            ow(
                role.cast(),
                OverwriteType::Role,
                Permissions::SEND_MESSAGES,
                Permissions::empty(),
            ),
            ow(
                member.cast(),
                OverwriteType::Member,
                Permissions::empty(),
                Permissions::SPEAK,
            ),
        ];
        let bits = apply_overwrites(base, &overwrites, everyone, member, &[role]);
        assert!(bits.contains(Permissions::SEND_MESSAGES));
        assert!(!bits.contains(Permissions::SPEAK));

        let timed = apply_implicit(bits | Permissions::READ_MESSAGE_HISTORY, true);
        assert_eq!(
            timed,
            Permissions::VIEW_CHANNEL | Permissions::READ_MESSAGE_HISTORY
        );
        assert!(apply_implicit(Permissions::SEND_MESSAGES, false).is_empty());
    }

    #[test]
    fn admins_and_owners_ignore_timeouts_and_overwrites() {
        let everyone = id_generic(1);
        let member = id_user(3);
        let deny_all = [ow(
            member.cast(),
            OverwriteType::Member,
            Permissions::empty(),
            Permissions::all(),
        )];
        for (owner, base) in [
            (true, Permissions::empty()),
            (false, Permissions::ADMINISTRATOR),
        ] {
            let bits = channel_permissions(owner, base, [], &deny_all, everyone, member, &[], true);
            assert_eq!(bits, Permissions::all());
        }
        // A normal timed-out member keeps only view + history.
        let bits = channel_permissions(
            false,
            Permissions::VIEW_CHANNEL
                | Permissions::SEND_MESSAGES
                | Permissions::READ_MESSAGE_HISTORY,
            [],
            &[],
            everyone,
            member,
            &[],
            true,
        );
        assert_eq!(
            bits,
            Permissions::VIEW_CHANNEL | Permissions::READ_MESSAGE_HISTORY
        );
    }

    #[test]
    fn send_dependent_bits_need_send_messages() {
        let bits = apply_implicit(
            Permissions::VIEW_CHANNEL | Permissions::EMBED_LINKS | Permissions::ATTACH_FILES,
            false,
        );
        assert_eq!(bits, Permissions::VIEW_CHANNEL);
    }
}
