//! Pure permission calculator (topics/permissions.mdx).

use crate::channel::{OverwriteType, PermissionOverwrite};
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
    let with_overwrites = apply_overwrites(guild, overwrites, everyone_id, member_id, member_roles);
    apply_implicit(with_overwrites, timed_out)
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
}
