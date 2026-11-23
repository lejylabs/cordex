//! Payloads shared by the model and cache benchmarks (built from the test
//! fixtures). Not every bench uses every payload.

#![allow(dead_code, reason = "shared by benches that use different subsets")]

use serde_json::{Value, json};

fn fixture(name: &str) -> Value {
    // Shared with the cache benches: resolve from the workspace's `crates/`.
    let crates = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crates dir");
    let path = crates
        .join("model/tests/fixtures")
        .join(format!("{name}.json"))
        .display()
        .to_string();
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("{path}: {e}"))
}

fn id(n: u64) -> Value {
    Value::String((175_928_847_299_117_063_u64 + n).to_string())
}

/// `MESSAGE_CREATE` data of the docs example message.
pub(crate) fn message_create() -> String {
    let mut m = fixture("message");
    m["guild_id"] = id(1);
    m.to_string()
}

/// `INTERACTION_CREATE` data of a slash command.
pub(crate) fn interaction_create() -> String {
    let mut i = fixture("interaction_slash");
    if i.get("application_id").is_none() {
        i["application_id"] = id(7);
    }
    if i.get("token").is_none() {
        i["token"] = json!("token");
    }
    i.to_string()
}

/// A `GUILD_CREATE` of a mid-sized guild: `members` members, 200 channels,
/// 50 roles and half as many presences as members.
pub(crate) fn guild_create(members: u64) -> String {
    let mut g = fixture("guild");
    g["id"] = id(1);
    g["joined_at"] = json!("2024-01-01T00:00:00.000000+00:00");
    g["large"] = json!(true);
    g["member_count"] = json!(members);
    let role = fixture("role");
    g["roles"] = Value::Array(
        (0..50)
            .map(|n| {
                let mut r = role.clone();
                r["id"] = id(10_000 + n);
                r
            })
            .collect(),
    );
    let channel = fixture("channel_text");
    g["channels"] = Value::Array(
        (0..200)
            .map(|n| {
                let mut c = channel.clone();
                c["id"] = id(20_000 + n);
                c.as_object_mut().map(|o| o.remove("guild_id"));
                c
            })
            .collect(),
    );
    let member = fixture("guild_member");
    let user = fixture("user");
    g["members"] = Value::Array(
        (0..members)
            .map(|n| {
                let mut u = user.clone();
                u["id"] = id(100_000 + n);
                let mut m = member.clone();
                m["user"] = u;
                m["roles"] = json!([id(10_000 + n % 50)]);
                m
            })
            .collect(),
    );
    g["presences"] = Value::Array(
        (0..members / 2)
            .map(|n| {
                json!({
                    "user": {"id": id(100_000 + n)},
                    "status": "online",
                    "activities": [],
                    "client_status": {"desktop": "online"}
                })
            })
            .collect(),
    );
    g.to_string()
}

/// A full gateway frame (`{"op":0,"t":..,"s":..,"d":..}`).
pub(crate) fn frame(name: &str, data: &str) -> String {
    format!(r#"{{"op":0,"t":"{name}","s":42,"d":{data}}}"#)
}
