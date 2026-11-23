//! Mock gateway scenarios (PLAN.md §17.2 items 1–12, 13–14).

use std::time::Duration;

use cordex_model::gateway::command::RequestGuildMembers;
use cordex_model::gateway::event::Event;
use cordex_model::gateway::presence::{Status, UpdatePresence};
use cordex_model::gateway::{CloseCode, GatewayBot, Intents, SessionStartLimit};
use cordex_model::id::Id;
use serde_json::Value;
use tokio::sync::mpsc;
use tokio::time::{Instant, timeout};

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use crate::connection::{Connector, Outgoing};
use crate::error::GatewayError;
use crate::identify::IDENTIFY_SPACING;
use crate::manager::ShardManager;
use crate::mock::{MockConnect, MockConnector, MockServer};
use crate::shard::Shard;
use crate::types::{
    Backpressure, CloseMode, HEARTBEAT_RESERVE, ManagerEvent, OUTBOUND_LIMIT, ShardState,
    USER_OUTBOUND_LIMIT,
};

const TOKEN: &str = "test.token.secret";

fn hello(interval_ms: u64) -> String {
    format!(r#"{{"op":10,"d":{{"heartbeat_interval":{interval_ms}}}}}"#)
}

fn ready_json(resume_url: &str) -> String {
    format!(
        r#"{{"op":0,"s":1,"t":"READY","d":{{"v":10,"user":{{"id":"1","username":"bot","discriminator":"0"}},"guilds":[{{"id":"2","unavailable":true}}],"session_id":"sess123","resume_gateway_url":"{resume_url}","application":{{"id":"3"}}}}}}"#
    )
}

fn message_create() -> &'static str {
    r#"{"op":0,"s":2,"t":"MESSAGE_CREATE","d":{"id":"10","channel_id":"20","author":{"id":"1","username":"bot","discriminator":"0"},"timestamp":"2017-07-11T17:27:07.299000+00:00","content":"hi","type":0}}"#
}

struct Harness {
    shard: Shard<MockConnector>,
    connects: mpsc::UnboundedReceiver<MockConnect>,
}

fn harness() -> Harness {
    harness_with(|b| b)
}

fn harness_with(
    f: impl FnOnce(crate::ShardBuilder<MockConnector>) -> crate::ShardBuilder<MockConnector>,
) -> Harness {
    let (connector, connects) = MockConnector::pair();
    let builder = Shard::builder(TOKEN, Intents::GUILDS)
        .gateway_url("wss://gateway.test")
        .connector(connector);
    Harness {
        shard: f(builder).build().expect("build"),
        connects,
    }
}

async fn accept(h: &mut Harness) -> (String, MockServer) {
    let conn = timeout(Duration::from_secs(2), h.connects.recv())
        .await
        .expect("connect timeout")
        .expect("connector closed");
    (conn.url, conn.server)
}

async fn handshake(server: &mut MockServer, interval_ms: u64, resume_url: &str) -> Value {
    server.send_json(hello(interval_ms));
    let raw = timeout(Duration::from_secs(2), server.recv_json())
        .await
        .expect("identify timeout")
        .expect("socket closed before identify");
    let json: Value = serde_json::from_str(&raw).unwrap();
    assert_eq!(json["op"], 2, "expected identify, got {raw}");
    assert_eq!(json["d"]["token"], TOKEN);
    assert_eq!(json["d"]["compress"], false);
    server.send_json(ready_json(resume_url));
    json
}

async fn next_ok(h: &Harness) -> Event {
    timeout(Duration::from_secs(5), h.shard.next_event())
        .await
        .expect("event timeout")
        .expect("shard stopped")
        .expect("gateway error")
}

#[test]
fn ready_fixture_deserializes() {
    let json = ready_json("wss://resume.test");
    let env = cordex_model::gateway::Envelope::parse(&json).unwrap();
    let event = Event::from_dispatch(env.t.as_deref().unwrap(), env.d).unwrap();
    assert!(matches!(event, Event::Ready(_)));
}

fn opcode(raw: &str) -> u64 {
    serde_json::from_str::<Value>(raw).unwrap()["op"]
        .as_u64()
        .unwrap_or(u64::MAX)
}

async fn recv_opcode(server: &mut MockServer, op: u64) -> String {
    loop {
        let raw = timeout(Duration::from_secs(5), server.recv_json())
            .await
            .expect("recv timeout")
            .expect("closed");
        if opcode(&raw) == op {
            return raw;
        }
    }
}

#[tokio::test]
async fn connect_identify_ready_events() {
    let mut h = harness();
    let (url, mut server) = accept(&mut h).await;
    assert!(url.contains("v=10"));
    assert!(url.contains("encoding=json"));
    #[cfg(feature = "zstd-stream")]
    assert!(url.contains("compress=zstd-stream"));
    #[cfg(not(feature = "zstd-stream"))]
    assert!(url.contains("compress=zlib-stream"));
    handshake(&mut server, 45_000, "wss://resume.test").await;
    let event = next_ok(&h).await;
    assert!(matches!(event, Event::Ready(_)));
    server.send_json(message_create());
    let event = next_ok(&h).await;
    assert!(matches!(event, Event::MessageCreate(_)));
    assert_eq!(h.shard.state(), ShardState::Active);

    let counters = h.shard.status().counters;
    assert_eq!((counters.identifies, counters.resumes), (1, 0));
    assert_eq!(counters.dispatches, 2);
    assert_eq!(counters.dropped_events, 0);
    assert_eq!(counters.bytes_received, counters.bytes_decompressed);
    let before = counters.bytes_received;
    assert!(before > 0);

    server
        .send_compressed(&ready_json("wss://resume.test"))
        .unwrap();
    let _ = next_ok(&h).await;
    let counters = h.shard.status().counters;
    assert_eq!(counters.dispatches, 3);
    let wire = counters.bytes_received - before;
    let json = counters.bytes_decompressed - before;
    assert!(wire > 0 && wire < json, "compressed {wire} vs json {json}");
}

#[tokio::test(start_paused = true)]
async fn resume_is_counted() {
    let mut h = harness();
    let (_, mut server) = accept(&mut h).await;
    handshake(&mut server, 45_000, "wss://resume.test").await;
    let _ = next_ok(&h).await;
    server.drop_tcp();
    let (_, mut server) = accept(&mut h).await;
    server.send_json(hello(45_000));
    let _ = recv_opcode(&mut server, 6).await;
    server.send_json(r#"{"op":0,"s":2,"t":"RESUMED","d":null}"#.to_owned());
    timeout(Duration::from_secs(5), async {
        while h.shard.status().counters.resumes == 0 {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("resume counted");
    assert_eq!(h.shard.status().counters.identifies, 1);
}

#[tokio::test(start_paused = true)]
async fn heartbeat_timing_with_jitter() {
    let mut h = harness();
    let (_, mut server) = accept(&mut h).await;
    let interval = Duration::from_millis(1_000);
    server.send_json(hello(1_000));
    let identify = recv_opcode(&mut server, 2).await;
    assert_eq!(opcode(&identify), 2);
    let t0 = Instant::now();
    tokio::time::advance(interval).await;
    let hb = recv_opcode(&mut server, 1).await;
    assert_eq!(opcode(&hb), 1);
    let first = t0.elapsed();
    assert!(first <= interval + Duration::from_millis(50));
    server.send_json(r#"{"op":11}"#);
    tokio::time::advance(interval).await;
    let hb2 = recv_opcode(&mut server, 1).await;
    assert_eq!(opcode(&hb2), 1);
}

#[tokio::test(start_paused = true)]
async fn missing_ack_zombie_then_resume() {
    let mut h = harness();
    let (_, mut server) = accept(&mut h).await;
    handshake(&mut server, 1_000, "wss://resume.test").await;
    let _ = next_ok(&h).await;
    tokio::time::advance(Duration::from_millis(2_500)).await;
    let mut saw_close = false;
    while let Ok(Some(msg)) = timeout(Duration::from_millis(10), server.recv()).await {
        if let Outgoing::Close { code, .. } = msg {
            assert_eq!(code, 4000);
            saw_close = true;
            break;
        }
    }
    drop(server);
    let (url, mut server) = accept(&mut h).await;
    assert!(url.contains("resume.test"), "resume url was {url}");
    server.send_json(hello(1_000));
    let raw = recv_opcode(&mut server, 6).await;
    let json: Value = serde_json::from_str(&raw).unwrap();
    assert_eq!(json["d"]["session_id"], "sess123");
    let _ = saw_close;
}

#[tokio::test]
async fn reconnect_opcode_resumes_to_resume_url() {
    let mut h = harness();
    let (_, mut server) = accept(&mut h).await;
    handshake(&mut server, 45_000, "wss://resume.test").await;
    let _ = next_ok(&h).await;
    server.send_json(r#"{"op":7}"#);
    let (url, mut server) = accept(&mut h).await;
    assert!(url.contains("resume.test"), "{url}");
    server.send_json(hello(45_000));
    let raw = recv_opcode(&mut server, 6).await;
    assert_eq!(opcode(&raw), 6);
}

#[tokio::test(start_paused = true)]
async fn invalid_session_true_resumes_false_waits_and_identifies() {
    let mut h = harness();
    let (_, mut server) = accept(&mut h).await;
    handshake(&mut server, 45_000, "wss://resume.test").await;
    let _ = next_ok(&h).await;

    server.send_json(r#"{"op":9,"d":true}"#);
    let raw = recv_opcode(&mut server, 6).await;
    assert_eq!(opcode(&raw), 6);

    server.send_json(r#"{"op":9,"d":false}"#);
    let t0 = Instant::now();
    let raw = recv_opcode(&mut server, 2).await;
    assert_eq!(opcode(&raw), 2);
    let elapsed = t0.elapsed();
    assert!(elapsed >= Duration::from_secs(1), "{elapsed:?}");
    assert!(
        elapsed <= Duration::from_secs(5) + Duration::from_millis(50),
        "{elapsed:?}"
    );
}

#[tokio::test]
async fn close_codes_map_to_actions() {
    async fn one(code: u16, expect_resume: Option<bool>) {
        let mut h = harness();
        let (_, mut server) = accept(&mut h).await;
        handshake(&mut server, 45_000, "wss://resume.test").await;
        let _ = next_ok(&h).await;
        server.send_close(code, "bye");
        match expect_resume {
            None => {
                let err = timeout(Duration::from_secs(2), h.shard.next_event())
                    .await
                    .unwrap()
                    .unwrap()
                    .unwrap_err();
                assert!(err.is_fatal(), "{err}");
                assert!(
                    timeout(Duration::from_millis(200), h.connects.recv())
                        .await
                        .ok()
                        .flatten()
                        .is_none()
                );
            }
            Some(resume) => {
                let (_, mut server) = accept(&mut h).await;
                server.send_json(hello(45_000));
                let raw = timeout(Duration::from_secs(2), server.recv_json())
                    .await
                    .unwrap()
                    .unwrap();
                let op = opcode(&raw);
                if resume {
                    assert_eq!(op, 6, "code {code}");
                } else {
                    assert_eq!(op, 2, "code {code}");
                }
            }
        }
    }

    for code in [4004, 4010, 4011, 4012, 4013, 4014] {
        one(code, None).await;
        assert!(!CloseCode::from(code).can_reconnect());
    }
    for code in [1000, 1001, 4003, 4007, 4009] {
        one(code, Some(false)).await;
    }
    for code in [4000, 4001, 4002, 4005, 4008, 4999] {
        one(code, Some(true)).await;
    }
}

#[tokio::test]
async fn tcp_drop_resumes() {
    let mut h = harness();
    let (_, server) = accept(&mut h).await;
    let mut server = server;
    handshake(&mut server, 45_000, "wss://resume.test").await;
    let _ = next_ok(&h).await;
    server.drop_tcp();
    let (url, mut server) = accept(&mut h).await;
    assert!(url.contains("resume.test"));
    server.send_json(hello(45_000));
    let raw = recv_opcode(&mut server, 6).await;
    assert_eq!(opcode(&raw), 6);
}

#[tokio::test(start_paused = true)]
async fn outbound_burst_keeps_heartbeats() {
    let mut h = harness();
    let (_, mut server) = accept(&mut h).await;
    handshake(&mut server, 10_000, "wss://resume.test").await;
    let _ = next_ok(&h).await;

    let shard = h.shard;
    let sender = tokio::spawn(async move {
        for _ in 0..200 {
            let _ = shard
                .update_presence(&UpdatePresence::new(Status::Online))
                .await;
        }
    });

    let mut sends = Vec::new();
    let mut heartbeats = 0u32;
    let mut presence = 0u32;
    let deadline = Instant::now() + Duration::from_secs(180);
    while Instant::now() < deadline && (presence < 200 || heartbeats == 0) {
        match timeout(Duration::from_secs(1), server.recv()).await {
            Ok(Some(Outgoing::Text(bytes))) => {
                let raw = String::from_utf8_lossy(&bytes).into_owned();
                let op = opcode(&raw);
                sends.push((Instant::now(), op));
                if op == 1 {
                    heartbeats += 1;
                    server.send_json(r#"{"op":11}"#);
                } else if op == 3 {
                    presence += 1;
                }
            }
            Ok(Some(Outgoing::Close { .. }) | None) | Err(_) => {
                tokio::time::advance(Duration::from_secs(1)).await;
            }
        }
    }
    let _ = sender.await;
    assert!(
        heartbeats >= 1,
        "heartbeats={heartbeats} presence={presence}"
    );
    for window in sends.windows(OUTBOUND_LIMIT as usize + 1) {
        if let (Some((first, _)), Some((last, _))) = (window.first(), window.last()) {
            assert!(
                last.duration_since(*first) >= Duration::from_secs(60) - Duration::from_millis(20),
                "more than {OUTBOUND_LIMIT} sends inside 60s"
            );
        }
    }
    assert!(presence <= 200);
    let _ = HEARTBEAT_RESERVE;
    let _ = USER_OUTBOUND_LIMIT;
}

#[tokio::test(start_paused = true)]
async fn slow_consumer_does_not_block_heartbeat() {
    let mut h = harness_with(|b| b.event_capacity(2).backpressure(Backpressure::DropNewest));
    let (_, mut server) = accept(&mut h).await;
    handshake(&mut server, 200, "wss://resume.test").await;
    let _ready = next_ok(&h).await;
    for i in 0..20 {
        server.send_json(format!(
            r#"{{"op":0,"s":{},"t":"FUTURE_EVENT","d":{{}}}}"#,
            i + 10
        ));
    }
    tokio::time::advance(Duration::from_millis(400)).await;
    let hb = recv_opcode(&mut server, 1).await;
    assert_eq!(opcode(&hb), 1);
    assert!(h.shard.dropped_events() > 0);
}

#[tokio::test]
async fn graceful_shutdown_both_modes() {
    let mut h = harness();
    let (_, mut server) = accept(&mut h).await;
    handshake(&mut server, 45_000, "wss://resume.test").await;
    let _ = next_ok(&h).await;
    let info = h.shard.close(CloseMode::Resumable).await.unwrap();
    let info = info.expect("resume info");
    assert_eq!(&*info.session_id, "sess123");
    loop {
        match timeout(Duration::from_secs(2), server.recv()).await {
            Ok(Some(Outgoing::Close { code, .. })) => {
                assert_eq!(code, 4000);
                break;
            }
            Ok(Some(Outgoing::Text(_))) => {}
            other => panic!("expected close, got {other:?}"),
        }
    }

    let mut h = harness();
    let (_, mut server) = accept(&mut h).await;
    handshake(&mut server, 45_000, "wss://resume.test").await;
    let _ = next_ok(&h).await;
    let info = h.shard.close(CloseMode::Terminate).await.unwrap();
    assert!(info.is_none());
    loop {
        match timeout(Duration::from_secs(2), server.recv()).await {
            Ok(Some(Outgoing::Close { code, .. })) => {
                assert_eq!(code, 1000);
                break;
            }
            Ok(Some(Outgoing::Text(_))) => {}
            other => panic!("expected close, got {other:?}"),
        }
    }
}

#[tokio::test]
async fn unknown_opcode_event_and_malformed_json_do_not_crash() {
    let mut h = harness();
    let (_, mut server) = accept(&mut h).await;
    handshake(&mut server, 45_000, "wss://resume.test").await;
    let _ = next_ok(&h).await;
    server.send_json(r#"{"op":77,"d":{}}"#);
    server.send_json("{");
    server.send_json(r#"{"op":0,"s":5,"t":"FUTURE_EVENT","d":{"x":1}}"#);
    let event = next_ok(&h).await;
    match event {
        Event::Unknown(u) => assert_eq!(&*u.name, "FUTURE_EVENT"),
        other => panic!("{other:?}"),
    }
    server.send_json(message_create());
    assert!(matches!(next_ok(&h).await, Event::MessageCreate(_)));
}

fn members_harness() -> Harness {
    let (connector, connects) = MockConnector::pair();
    Harness {
        shard: Shard::builder(TOKEN, Intents::GUILDS | Intents::GUILD_MEMBERS)
            .gateway_url("wss://gateway.test")
            .connector(connector)
            .build()
            .expect("build"),
        connects,
    }
}

#[tokio::test]
async fn request_guild_members_multiple_chunks() {
    let mut h = members_harness();
    let (_, mut server) = accept(&mut h).await;
    handshake(&mut server, 45_000, "wss://resume.test").await;
    let _ = next_ok(&h).await;

    let guild = Id::new(2).unwrap();
    let fetch = h
        .shard
        .request_guild_members(RequestGuildMembers::all(guild));
    let send = async {
        let raw = recv_opcode(&mut server, 8).await;
        let json: Value = serde_json::from_str(&raw).unwrap();
        let nonce = json["d"]["nonce"].as_str().unwrap().to_owned();
        assert_eq!(json["d"]["query"], "");
        assert_eq!(json["d"]["limit"], 0);
        server.send_json(format!(
            r#"{{"op":0,"s":2,"t":"GUILD_MEMBERS_CHUNK","d":{{"guild_id":"2","members":[{{"user":{{"id":"10","username":"a","discriminator":"0"}}}}],"chunk_index":0,"chunk_count":2,"nonce":"{nonce}"}}}}"#
        ));
        server.send_json(format!(
            r#"{{"op":0,"s":3,"t":"GUILD_MEMBERS_CHUNK","d":{{"guild_id":"2","members":[{{"user":{{"id":"11","username":"b","discriminator":"0"}}}}],"chunk_index":1,"chunk_count":2,"nonce":"{nonce}"}}}}"#
        ));
    };
    let (members, ()) = tokio::join!(fetch, send);
    let members = members.expect("chunks");
    assert_eq!(members.len(), 2);
}

fn gateway_bot(shards: u32, remaining: u32, reset_after: u64, max_concurrency: u32) -> GatewayBot {
    GatewayBot::new(
        "wss://gateway.test",
        shards,
        SessionStartLimit::new(1000, remaining, reset_after, max_concurrency),
    )
}

async fn manager_pair(
    bot: GatewayBot,
) -> (
    ShardManager<MockConnector>,
    mpsc::UnboundedReceiver<MockConnect>,
) {
    let (connector, connects) = MockConnector::pair();
    let manager = ShardManager::builder(TOKEN, Intents::GUILDS)
        .automatic()
        .connector(connector)
        .gateway_bot(bot)
        .build()
        .await
        .expect("manager");
    (manager, connects)
}

async fn accept_n(
    connects: &mut mpsc::UnboundedReceiver<MockConnect>,
    n: usize,
) -> Vec<MockServer> {
    let mut servers = Vec::with_capacity(n);
    for _ in 0..n {
        let conn = timeout(Duration::from_secs(2), connects.recv())
            .await
            .expect("connect timeout")
            .expect("connector closed");
        servers.push(conn.server);
    }
    servers
}

/// PLAN.md §17.2 scenario 11: `max_concurrency` 1 spaces identifies by 5.25 s.
#[tokio::test(start_paused = true)]
async fn multiple_shards_max_concurrency_one_identify_spacing() {
    let (manager, mut connects) = manager_pair(gateway_bot(2, 1000, 0, 1)).await;
    manager.start().await.unwrap();
    let mut servers = accept_n(&mut connects, 2).await;
    let mut s1 = servers.pop().unwrap();
    let mut s0 = servers.pop().unwrap();
    s0.send_json(hello(45_000));
    s1.send_json(hello(45_000));

    let first = tokio::select! {
        raw = recv_opcode(&mut s0, 2) => (0, raw),
        raw = recv_opcode(&mut s1, 2) => (1, raw),
    };
    assert_eq!(opcode(&first.1), 2);
    let second_server = if first.0 == 0 { &mut s1 } else { &mut s0 };
    let late = timeout(Duration::from_millis(1), recv_opcode(second_server, 2)).await;
    assert!(late.is_err(), "second identify must wait for the bucket");

    tokio::time::advance(IDENTIFY_SPACING).await;
    let second = recv_opcode(second_server, 2).await;
    assert_eq!(opcode(&second), 2);
}

/// PLAN.md §17.2 scenario 11: `max_concurrency` 16 lets different buckets
/// identify together.
#[tokio::test(start_paused = true)]
async fn multiple_shards_max_concurrency_sixteen_parallel() {
    let (manager, mut connects) = manager_pair(gateway_bot(2, 1000, 0, 16)).await;
    manager.start().await.unwrap();
    let mut servers = accept_n(&mut connects, 2).await;
    let mut s1 = servers.pop().unwrap();
    let mut s0 = servers.pop().unwrap();
    s0.send_json(hello(45_000));
    s1.send_json(hello(45_000));
    let t0 = Instant::now();
    let (a, b) = tokio::join!(recv_opcode(&mut s0, 2), recv_opcode(&mut s1, 2),);
    assert_eq!(opcode(&a), 2);
    assert_eq!(opcode(&b), 2);
    assert!(
        t0.elapsed() < Duration::from_millis(50),
        "parallel buckets waited {:?}",
        t0.elapsed()
    );
}

/// PLAN.md §17.2 scenario 12: `remaining` below the shard count waits for
/// `reset_after`.
#[tokio::test(start_paused = true)]
async fn session_start_limit_waits_for_reset_after() {
    let (manager, mut connects) = manager_pair(gateway_bot(2, 1, 2_000, 16)).await;
    manager.start().await.unwrap();
    let mut servers = accept_n(&mut connects, 2).await;
    let mut s1 = servers.pop().unwrap();
    let mut s0 = servers.pop().unwrap();
    s0.send_json(hello(45_000));
    s1.send_json(hello(45_000));

    let first = tokio::select! {
        raw = recv_opcode(&mut s0, 2) => (0, raw),
        raw = recv_opcode(&mut s1, 2) => (1, raw),
    };
    assert_eq!(opcode(&first.1), 2);
    let second_server = if first.0 == 0 { &mut s1 } else { &mut s0 };
    let late = timeout(Duration::from_millis(1), recv_opcode(second_server, 2)).await;
    assert!(late.is_err(), "second identify must wait for reset_after");

    tokio::time::advance(Duration::from_millis(2_000)).await;
    let second = recv_opcode(second_server, 2).await;
    assert_eq!(opcode(&second), 2);
}

#[tokio::test]
async fn manager_status_and_recommended_count() {
    let (connector, mut connects) = MockConnector::pair();
    let mut manager = ShardManager::builder(TOKEN, Intents::GUILDS)
        .shards(0..1, 1)
        .connector(connector)
        .gateway_bot(gateway_bot(4, 1000, 0, 16))
        .build()
        .await
        .unwrap();
    let mut events = manager.events();
    manager.start().await.unwrap();
    assert_eq!(manager.recommended_shards(), 4);
    assert_eq!(manager.total_shards(), 1);

    let mut saw_recommended = false;
    let deadline = Instant::now() + Duration::from_secs(2);
    while Instant::now() < deadline {
        match timeout(Duration::from_millis(200), events.recv()).await {
            Ok(Some(ManagerEvent::RecommendedShardCount {
                running,
                recommended,
            })) => {
                assert_eq!(running, 1);
                assert_eq!(recommended, 4);
                saw_recommended = true;
                break;
            }
            Ok(Some(_)) => {}
            _ => break,
        }
    }
    assert!(saw_recommended, "expected RecommendedShardCount");

    let mut server = accept_n(&mut connects, 1).await;
    handshake(&mut server[0], 45_000, "wss://resume.test").await;
    let deadline = Instant::now() + Duration::from_secs(2);
    while Instant::now() < deadline {
        if matches!(
            timeout(Duration::from_millis(200), events.recv()).await,
            Ok(Some(ManagerEvent::Dispatch { .. }))
        ) {
            break;
        }
    }
    let status = manager.status();
    assert_eq!(status.len(), 1);
    assert_eq!(status[0].guild_count, 1);
    assert_eq!(status[0].state, ShardState::Active);
}

/// Connector that panics on the first `panics` connects, then behaves like
/// the mock (PLAN.md §9.5 panic guard).
#[derive(Clone)]
struct PanickingConnector {
    inner: MockConnector,
    remaining: Arc<AtomicU32>,
}

impl Connector for PanickingConnector {
    type Connection = <MockConnector as Connector>::Connection;

    async fn connect(&self, url: &str) -> Result<Self::Connection, GatewayError> {
        if self.remaining.load(Ordering::SeqCst) > 0 {
            self.remaining.fetch_sub(1, Ordering::SeqCst);
            panic!("injected connect panic");
        }
        self.inner.connect(url).await
    }
}

fn panicking_shard(
    panics: u32,
) -> (
    Shard<PanickingConnector>,
    mpsc::UnboundedReceiver<MockConnect>,
) {
    let (inner, connects) = MockConnector::pair();
    let shard = Shard::builder(TOKEN, Intents::GUILDS)
        .gateway_url("wss://gateway.test")
        .connector(PanickingConnector {
            inner,
            remaining: Arc::new(AtomicU32::new(panics)),
        })
        .build()
        .expect("build");
    (shard, connects)
}

/// PLAN.md §22: a panicking shard run is reported and the shard reconnects.
#[tokio::test(start_paused = true)]
async fn panic_in_shard_run_reports_and_reconnects() {
    let (shard, mut connects) = panicking_shard(1);

    let first = timeout(Duration::from_secs(30), shard.next_event())
        .await
        .expect("panic report")
        .expect("shard stopped");
    match first {
        Err(GatewayError::Panic(msg)) => assert!(msg.contains("injected"), "{msg}"),
        other => panic!("expected Panic, got {other:?}"),
    }

    let conn = timeout(Duration::from_secs(30), connects.recv())
        .await
        .expect("reconnect timeout")
        .expect("connector closed");
    let mut server = conn.server;
    handshake(&mut server, 45_000, "wss://resume.test").await;
    let event = timeout(Duration::from_secs(5), shard.next_event())
        .await
        .expect("event timeout")
        .expect("shard stopped")
        .expect("gateway error");
    assert!(matches!(event, Event::Ready(_)));
    assert_eq!(shard.status().reconnect_count, 1);
}

/// §9.5: more than 5 panics in 10 minutes stop the shard; another shard on
/// the same runtime keeps running.
#[tokio::test(start_paused = true)]
async fn repeated_panics_stop_only_that_shard() {
    let mut healthy = harness();
    let (_, mut server) = accept(&mut healthy).await;
    // Long interval: the healthy shard must not hit a heartbeat timeout while
    // the broken one backs off (≈ 37 s of virtual time).
    handshake(&mut server, 600_000, "wss://resume.test").await;
    assert!(matches!(next_ok(&healthy).await, Event::Ready(_)));

    let (broken, mut broken_connects) = panicking_shard(u32::MAX);

    let mut soft = 0;
    let fatal = loop {
        let item = timeout(Duration::from_secs(600), broken.next_event())
            .await
            .expect("panic report")
            .expect("stream ended before the panic limit");
        match item {
            Err(GatewayError::Panic(_)) => soft += 1,
            Err(err) => break err,
            Ok(event) => panic!("unexpected event {event:?}"),
        }
    };
    assert!(matches!(fatal, GatewayError::PanicLimit(_)), "{fatal}");
    assert!(fatal.is_fatal());
    assert_eq!(soft, crate::types::PANIC_LIMIT);
    assert!(broken.next_event().await.is_none());
    assert_eq!(broken.state(), ShardState::Disconnected);
    assert!(broken_connects.try_recv().is_err(), "no connect after stop");

    server.send_json(message_create());
    assert!(matches!(next_ok(&healthy).await, Event::MessageCreate(_)));
    assert_eq!(healthy.shard.state(), ShardState::Active);
}

#[test]
fn build_outside_runtime_is_an_error() {
    let (connector, _connects) = MockConnector::pair();
    let result = Shard::builder(TOKEN, Intents::GUILDS)
        .connector(connector)
        .build();
    assert!(matches!(result, Err(GatewayError::NoRuntime)));
}

/// §16.4: a manager can place shards on explicit runtimes.
#[test]
fn manager_runs_shards_on_given_runtimes() {
    let shard_rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let spawner = Arc::new(crate::RuntimeHandles::new([shard_rt.handle().clone()]).unwrap());
    let (connector, mut connects) = MockConnector::pair();
    let control = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let manager = control.block_on(async {
        let manager = ShardManager::builder(TOKEN, Intents::GUILDS)
            .shards(0..1, 1)
            .connector(connector)
            .gateway_bot(gateway_bot(1, 1000, 0, 16))
            .spawn_with(spawner)
            .build()
            .await
            .unwrap();
        manager.start().await.unwrap();
        manager
    });
    // Nothing connects until the shard runtime is driven.
    assert!(connects.try_recv().is_err());
    let conn = shard_rt.block_on(async {
        timeout(Duration::from_secs(2), connects.recv())
            .await
            .expect("connect on shard runtime")
            .expect("connector closed")
    });
    assert!(conn.url.contains("v=10"));
    drop(manager);
}

#[test]
fn fatal_close_message_names_the_fix() {
    let err = GatewayError::Fatal {
        code: CloseCode::DisallowedIntents,
        reason: "Disallowed intent(s).".into(),
    };
    let text = err.to_string();
    assert!(text.contains("Disallowed intent(s)."), "{text}");
    assert!(text.contains("Privileged Gateway Intents"), "{text}");
}

#[tokio::test(start_paused = true)]
async fn decompressed_message_cap_is_configurable() {
    let mut h = harness_with(|b| b.max_message_bytes(64));
    let (_, mut server) = accept(&mut h).await;
    handshake(&mut server, 45_000, "wss://resume.test").await;
    let _ = next_ok(&h).await;
    // A compressed MESSAGE_CREATE inflates to more than 64 bytes: the
    // connection ends and the shard resumes on a new one.
    server.send_compressed(message_create()).unwrap();
    let (url, _server) = accept(&mut h).await;
    assert!(url.contains("resume.test"), "{url}");
}
