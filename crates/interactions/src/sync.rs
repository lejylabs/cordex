//! Smart command sync diffing (`SyncReport`) and `DISCORD_DEV_GUILD_ID` helper.

use cordex_model::id::GuildId;
use serde_json::Value;

/// Server-populated fields Discord includes on `GET /commands` responses that
/// do not represent local definition changes.
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

/// Summary of differences between local and remote Discord application commands.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SyncReport {
    /// Commands present locally that do not exist remotely.
    pub added: Vec<String>,
    /// Commands present on both sides whose definitions differ.
    pub updated: Vec<String>,
    /// Commands present remotely that are no longer defined locally.
    pub removed: Vec<String>,
    /// Commands whose remote definition already matches the local one.
    pub unchanged: Vec<String>,
}

impl SyncReport {
    /// `true` when no commands need to be added, updated, or removed.
    pub fn is_up_to_date(&self) -> bool {
        self.added.is_empty() && self.updated.is_empty() && self.removed.is_empty()
    }
}

/// Compare local command JSON definitions with Discord's remote `GET /commands`
/// payload and return a structured [`SyncReport`].
pub fn diff_commands(local: &[Value], remote: &[Value]) -> SyncReport {
    let mut report = SyncReport::default();

    for l in local {
        let l_key = command_key(l);
        let name = l_key.1.unwrap_or("<unnamed>").to_owned();
        match remote.iter().find(|r| command_key(r) == l_key) {
            None => report.added.push(name),
            Some(r) if object_matches(l, r) => report.unchanged.push(name),
            Some(_) => report.updated.push(name),
        }
    }

    for r in remote {
        let r_key = command_key(r);
        if !local.iter().any(|l| command_key(l) == r_key) {
            report
                .removed
                .push(r_key.1.unwrap_or("<unnamed>").to_owned());
        }
    }

    report
}

/// Read `DISCORD_DEV_GUILD_ID` from the environment, if set to a valid snowflake.
pub fn dev_guild_id_from_env() -> Option<GuildId> {
    let raw = std::env::var("DISCORD_DEV_GUILD_ID").ok()?;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    trimmed.parse::<u64>().ok().and_then(GuildId::new)
}

fn command_key(command: &Value) -> (u64, Option<&str>) {
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
            l.len() == r.len() && l.iter().zip(r).all(|(a, b)| values_match(a, b))
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

    #[test]
    fn diff_reports_added_updated_removed_and_unchanged() {
        let local = vec![
            json!({"type": 1, "name": "ping", "description": "Pong"}),
            json!({"type": 1, "name": "help", "description": "New help"}),
            json!({"type": 1, "name": "info", "description": "Bot info"}),
        ];
        let remote = vec![
            json!({"id": "1", "type": 1, "name": "ping", "description": "Pong", "nsfw": false}),
            json!({"id": "2", "type": 1, "name": "help", "description": "Old help"}),
            json!({"id": "3", "type": 1, "name": "legacy", "description": "Removed"}),
        ];
        let report = diff_commands(&local, &remote);
        assert!(!report.is_up_to_date());
        assert_eq!(report.unchanged, vec!["ping"]);
        assert_eq!(report.updated, vec!["help"]);
        assert_eq!(report.added, vec!["info"]);
        assert_eq!(report.removed, vec!["legacy"]);
    }
}
