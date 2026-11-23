//! Decide whether the registered commands already match the local ones,
//! so a restart does not overwrite (and re-create) unchanged commands.
//!
//! The local builders serialize only what was set; Discord answers with
//! every field, filled with defaults and server data. Two sets match when
//! every command (by type and name) exists on both sides, every local
//! field has the same value remotely, and every field only Discord sends
//! is server data or a default. Anything unclear counts as a change: an
//! extra overwrite is harmless, a missed change is not.

use serde_json::Value;

/// Fields Discord adds that say nothing about the definition.
const SERVER_FIELDS: &[&str] = &[
    "id",
    "application_id",
    "version",
    "guild_id",
    "default_permission",
    "dm_permission",
    "name_localized",
    "description_localized",
];

/// `true` when `remote` (from `GET …/commands`) equals `local`.
pub(super) fn commands_match(local: &[Value], remote: &[Value]) -> bool {
    local.len() == remote.len()
        && local.iter().all(|l| {
            remote
                .iter()
                .find(|r| key(r) == key(l))
                .is_some_and(|r| object_matches(l, r))
        })
}

fn key(command: &Value) -> (u64, Option<&str>) {
    (
        command.get("type").and_then(Value::as_u64).unwrap_or(1),
        command.get("name").and_then(Value::as_str),
    )
}

fn object_matches(local: &Value, remote: &Value) -> bool {
    let (Some(l), Some(r)) = (local.as_object(), remote.as_object()) else {
        return values_match(local, remote);
    };
    l.iter()
        .all(|(k, lv)| r.get(k).is_some_and(|rv| values_match(lv, rv)))
        && r.iter()
            .filter(|(k, _)| !l.contains_key(*k))
            .all(|(k, rv)| is_default(k, rv))
}

fn values_match(local: &Value, remote: &Value) -> bool {
    match (local, remote) {
        (Value::Object(_), Value::Object(_)) => object_matches(local, remote),
        (Value::Array(l), Value::Array(r)) => {
            l.len() == r.len() && l.iter().zip(r).all(|(l, r)| values_match(l, r))
        }
        (Value::Number(l), Value::Number(r)) => l.as_f64() == r.as_f64(),
        _ => local == remote,
    }
}

fn is_default(key: &str, value: &Value) -> bool {
    if SERVER_FIELDS.contains(&key) {
        return true;
    }
    match value {
        Value::Null | Value::Bool(false) => true,
        Value::String(s) => s.is_empty(),
        Value::Array(a) => a.is_empty() || (key == "integration_types" && *a == [Value::from(0)]),
        Value::Object(o) => o.is_empty(),
        Value::Number(_) | Value::Bool(true) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn remote_ping() -> Value {
        json!({
            "id": "1", "application_id": "2", "version": "3", "guild_id": "4",
            "type": 1, "name": "ping", "description": "Replies",
            "default_member_permissions": null, "dm_permission": true,
            "nsfw": false, "integration_types": [0], "contexts": null,
            "name_localizations": null, "description_localizations": null,
            "options": [
                {"type": 3, "name": "text", "description": "What", "required": false,
                 "max_length": 100, "autocomplete": false}
            ]
        })
    }

    fn local_ping() -> Value {
        json!({
            "type": 1, "name": "ping", "description": "Replies",
            "options": [{"type": 3, "name": "text", "description": "What", "max_length": 100}]
        })
    }

    #[test]
    fn same_definition_matches() {
        assert!(commands_match(&[local_ping()], &[remote_ping()]));
    }

    #[test]
    fn changed_description_is_a_change() {
        let mut local = local_ping();
        local["description"] = json!("Other");
        assert!(!commands_match(&[local], &[remote_ping()]));
    }

    #[test]
    fn option_removed_locally_is_a_change() {
        let mut local = local_ping();
        local.as_object_mut().unwrap().remove("options");
        assert!(!commands_match(&[local], &[remote_ping()]));
    }

    #[test]
    fn remote_only_non_default_is_a_change() {
        let mut remote = remote_ping();
        remote["nsfw"] = json!(true);
        assert!(!commands_match(&[local_ping()], &[remote]));
    }

    #[test]
    fn count_and_names_must_match() {
        let user = json!({"type": 2, "name": "Info"});
        assert!(!commands_match(
            &[local_ping(), user.clone()],
            &[remote_ping()]
        ));
        let remote_user = json!({"type": 2, "name": "Info", "description": "", "id": "9"});
        assert!(commands_match(
            &[user, local_ping()],
            &[remote_ping(), remote_user]
        ));
        assert!(commands_match(&[], &[]));
    }

    #[test]
    fn numbers_compare_by_value() {
        assert!(values_match(&json!(1), &json!(1.0)));
    }
}
