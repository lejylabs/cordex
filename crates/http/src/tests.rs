//! Mock REST tests (PLAN.md §17.3).

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

use bytes::Bytes;
use http::{HeaderMap, HeaderValue, Request, StatusCode};
use parking_lot::Mutex;
use serde_json::json;

use crate::client::{Client, USER_AGENT};
use crate::error::HttpError;
use crate::route::Route;
use crate::transport::{FnTransport, RawResponse};

fn ok_json(body: &str) -> RawResponse {
    let mut headers = HeaderMap::new();
    headers.insert("x-ratelimit-limit", HeaderValue::from_static("5"));
    headers.insert("x-ratelimit-remaining", HeaderValue::from_static("4"));
    headers.insert("x-ratelimit-reset-after", HeaderValue::from_static("1"));
    headers.insert("x-ratelimit-bucket", HeaderValue::from_static("test"));
    RawResponse {
        status: StatusCode::OK,
        headers,
        body: Bytes::from(body.to_owned()),
    }
}

fn client_with(f: impl Fn(Request<Bytes>) -> RawResponse + Send + Sync + 'static) -> Client {
    Client::builder("test.token.secret")
        .transport(Arc::new(FnTransport::new(move |req: Request<Bytes>| {
            let resp = f(req);
            async move { Ok(resp) }
        })))
        .build()
        .unwrap()
}

#[tokio::test]
async fn sends_bot_authorization_and_user_agent() {
    let seen = Arc::new(Mutex::new(None::<(String, String)>));
    let flag = seen.clone();
    let client = client_with(move |req| {
        let auth = req
            .headers()
            .get("authorization")
            .unwrap()
            .to_str()
            .unwrap()
            .to_owned();
        let ua = req
            .headers()
            .get("user-agent")
            .unwrap()
            .to_str()
            .unwrap()
            .to_owned();
        *flag.lock() = Some((auth, ua));
        ok_json("{}")
    });

    client.get(Route::new("/users/@me")).send().await.unwrap();

    let (auth, ua) = seen.lock().clone().unwrap();
    assert_eq!(auth, "Bot test.token.secret");
    assert_eq!(ua, USER_AGENT);
    assert!(ua.starts_with("DiscordBot (https://github.com/lejylabs/discord-rs, "));
    assert!(ua.contains("0.1.0"));
}

#[tokio::test]
async fn percent_encodes_audit_log_reason() {
    let seen = Arc::new(Mutex::new(None::<String>));
    let flag = seen.clone();
    let client = client_with(move |req| {
        *flag.lock() = req
            .headers()
            .get("x-audit-log-reason")
            .map(|v| v.to_str().unwrap().to_owned());
        ok_json("{}")
    });

    client
        .delete(Route::raw("/channels/{channel_id}", 1u64))
        .reason("banned for spam & trolling")
        .unwrap()
        .send()
        .await
        .unwrap();

    let encoded = seen.lock().clone().unwrap();
    assert!(encoded.contains("spam"));
    assert!(encoded.contains("%26"));
    assert!(!encoded.contains('&'));
}

#[test]
fn audit_log_reason_rejects_over_512_chars() {
    let too_long: String = "é".repeat(513);
    let err = crate::encode_audit_log_reason(&too_long).unwrap_err();
    assert!(matches!(err, HttpError::Validation(_)));
    let ok: String = "é".repeat(512);
    crate::encode_audit_log_reason(&ok).unwrap();
}

#[test]
fn client_debug_redacts_token() {
    let client = client_with(|_| ok_json("{}"));
    let debug = format!("{client:?}");
    assert!(!debug.contains("test.token.secret"));
    assert!(debug.contains("Token(***)"));
}

#[tokio::test]
async fn json_success_and_discord_error() {
    let n = AtomicU32::new(0);
    let client = client_with(move |_| {
        if n.fetch_add(1, Ordering::SeqCst) == 0 {
            ok_json(r#"{"id":"1"}"#)
        } else {
            RawResponse {
                status: StatusCode::NOT_FOUND,
                headers: HeaderMap::new(),
                body: Bytes::from(r#"{"message":"Unknown Message","code":10008}"#.to_owned()),
            }
        }
    });

    let v: serde_json::Value = client
        .get(Route::new("/users/@me"))
        .send()
        .await
        .unwrap()
        .json()
        .unwrap();
    assert_eq!(v["id"], "1");

    let err = client
        .get(Route::raw(
            "/channels/{channel_id}/messages/{message_id}",
            (1u64, 2u64),
        ))
        .send()
        .await
        .unwrap_err();
    assert_eq!(
        err.discord_code(),
        Some(crate::DiscordErrorCode::UnknownMessage)
    );
}

#[tokio::test]
async fn flatten_nested_errors_on_http_error() {
    let client = client_with(|_| RawResponse {
        status: StatusCode::BAD_REQUEST,
        headers: HeaderMap::new(),
        body: Bytes::from(
            json!({
                "message": "Invalid Form Body",
                "code": 50035,
                "errors": {
                    "content": { "_errors": [{ "code": "BASE", "message": "required" }] }
                }
            })
            .to_string(),
        ),
    });
    let err = client
        .post(Route::raw("/channels/{channel_id}/messages", 1u64))
        .json(&json!({"content": ""}))
        .unwrap()
        .send()
        .await
        .unwrap_err();
    assert_eq!(
        err.discord_code(),
        Some(crate::DiscordErrorCode::InvalidFormBody)
    );
    assert_eq!(
        err.flatten_errors(),
        vec![("content".into(), "required".into())]
    );
}

#[tokio::test(start_paused = true)]
async fn retries_429_when_wait_under_cap() {
    let n = Arc::new(AtomicU32::new(0));
    let hits = n.clone();
    let client = Client::builder("t")
        .transport(Arc::new(FnTransport::new(move |_| {
            let i = hits.fetch_add(1, Ordering::SeqCst);
            async move {
                if i == 0 {
                    let mut headers = HeaderMap::new();
                    headers.insert("x-ratelimit-bucket", HeaderValue::from_static("b"));
                    headers.insert("x-ratelimit-limit", HeaderValue::from_static("1"));
                    headers.insert("x-ratelimit-remaining", HeaderValue::from_static("0"));
                    headers.insert("x-ratelimit-reset-after", HeaderValue::from_static("0.05"));
                    headers.insert("retry-after", HeaderValue::from_static("0.05"));
                    Ok(RawResponse {
                        status: StatusCode::TOO_MANY_REQUESTS,
                        headers,
                        body: Bytes::from(r#"{"retry_after":0.05,"global":false}"#),
                    })
                } else {
                    Ok(ok_json("{}"))
                }
            }
        })))
        .build()
        .unwrap();

    client.get(Route::new("/users/@me")).send().await.unwrap();
    assert_eq!(n.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn long_429_returns_rate_limited() {
    let client = Client::builder("t")
        .retry_after_cap(Duration::from_secs(1))
        .transport(Arc::new(FnTransport::new(|_| async {
            let mut headers = HeaderMap::new();
            headers.insert("retry-after", HeaderValue::from_static("30"));
            Ok(RawResponse {
                status: StatusCode::TOO_MANY_REQUESTS,
                headers,
                body: Bytes::from(r#"{"retry_after":30.0,"global":true}"#),
            })
        })))
        .build()
        .unwrap();

    let err = client
        .get(Route::new("/users/@me"))
        .send()
        .await
        .unwrap_err();
    match err {
        HttpError::RateLimited {
            retry_after,
            global,
            ..
        } => {
            assert_eq!(retry_after, Duration::from_secs(30));
            assert!(global);
        }
        other => panic!("unexpected {other:?}"),
    }
}

#[tokio::test(start_paused = true)]
async fn retries_get_on_503_not_post() {
    let gets = Arc::new(AtomicU32::new(0));
    let g = gets.clone();
    let get_client = Client::builder("t")
        .transport(Arc::new(FnTransport::new(move |_| {
            let i = g.fetch_add(1, Ordering::SeqCst);
            async move {
                if i < 2 {
                    Ok(RawResponse {
                        status: StatusCode::SERVICE_UNAVAILABLE,
                        headers: HeaderMap::new(),
                        body: Bytes::new(),
                    })
                } else {
                    Ok(ok_json("{}"))
                }
            }
        })))
        .build()
        .unwrap();
    get_client
        .get(Route::new("/users/@me"))
        .send()
        .await
        .unwrap();
    assert_eq!(gets.load(Ordering::SeqCst), 3);

    let posts = Arc::new(AtomicU32::new(0));
    let p = posts.clone();
    let post_client = Client::builder("t")
        .transport(Arc::new(FnTransport::new(move |_| {
            p.fetch_add(1, Ordering::SeqCst);
            async {
                Ok(RawResponse {
                    status: StatusCode::SERVICE_UNAVAILABLE,
                    headers: HeaderMap::new(),
                    body: Bytes::new(),
                })
            }
        })))
        .build()
        .unwrap();
    let err = post_client
        .post(Route::raw("/channels/{channel_id}/messages", 1u64))
        .json(&json!({"content": "hi"}))
        .unwrap()
        .send()
        .await
        .unwrap_err();
    assert!(matches!(err, HttpError::Discord { status, .. } if status.as_u16() == 503));
    assert_eq!(posts.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn does_not_retry_500() {
    let n = Arc::new(AtomicU32::new(0));
    let c = n.clone();
    let client = Client::builder("t")
        .transport(Arc::new(FnTransport::new(move |_| {
            c.fetch_add(1, Ordering::SeqCst);
            async {
                Ok(RawResponse {
                    status: StatusCode::INTERNAL_SERVER_ERROR,
                    headers: HeaderMap::new(),
                    body: Bytes::new(),
                })
            }
        })))
        .build()
        .unwrap();
    let _ = client
        .get(Route::new("/users/@me"))
        .send()
        .await
        .unwrap_err();
    assert_eq!(n.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn unauthorized_poisons_client() {
    let n = Arc::new(AtomicU32::new(0));
    let c = n.clone();
    let client = Client::builder("t")
        .transport(Arc::new(FnTransport::new(move |_| {
            c.fetch_add(1, Ordering::SeqCst);
            async {
                Ok(RawResponse {
                    status: StatusCode::UNAUTHORIZED,
                    headers: HeaderMap::new(),
                    body: Bytes::from(r#"{"message":"401: Unauthorized","code":0}"#),
                })
            }
        })))
        .build()
        .unwrap();

    let err = client
        .get(Route::new("/users/@me"))
        .send()
        .await
        .unwrap_err();
    assert!(matches!(err, HttpError::Unauthorized));
    assert!(client.is_poisoned());
    let err = client
        .get(Route::new("/users/@me"))
        .send()
        .await
        .unwrap_err();
    assert!(matches!(err, HttpError::Unauthorized));
    assert_eq!(n.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn invalid_request_guard_rejects_locally() {
    let client = Client::builder("t")
        .invalid_request_thresholds(2, 3)
        .transport(Arc::new(FnTransport::new(|_| async {
            Ok(RawResponse {
                status: StatusCode::FORBIDDEN,
                headers: HeaderMap::new(),
                body: Bytes::from(r#"{"message":"Missing Access","code":50001}"#),
            })
        })))
        .build()
        .unwrap();

    let _ = client.get(Route::new("/a")).send().await.unwrap_err();
    let _ = client.get(Route::new("/b")).send().await.unwrap_err();
    let _ = client.get(Route::new("/c")).send().await.unwrap_err();
    let err = client.get(Route::new("/d")).send().await.unwrap_err();
    assert!(matches!(err, HttpError::InvalidRequestLimit { .. }));
}

#[tokio::test]
async fn shared_429_does_not_trip_guard() {
    let client = Client::builder("t")
        .invalid_request_thresholds(1, 1)
        .retry_after_cap(Duration::from_millis(1))
        .transport(Arc::new(FnTransport::new(|_| async {
            let mut headers = HeaderMap::new();
            headers.insert("x-ratelimit-scope", HeaderValue::from_static("shared"));
            headers.insert("retry-after", HeaderValue::from_static("30"));
            Ok(RawResponse {
                status: StatusCode::TOO_MANY_REQUESTS,
                headers,
                body: Bytes::from(r#"{"retry_after":30.0,"global":false}"#),
            })
        })))
        .build()
        .unwrap();

    let err = client
        .get(Route::new("/invite/x"))
        .send()
        .await
        .unwrap_err();
    assert!(matches!(err, HttpError::RateLimited { .. }));
    let err = client
        .get(Route::new("/invite/y"))
        .send()
        .await
        .unwrap_err();
    assert!(
        matches!(err, HttpError::RateLimited { .. }),
        "shared 429s must not fill the invalid-request guard: {err:?}"
    );
}

#[tokio::test(start_paused = true)]
async fn request_timeout() {
    let client = Client::builder("t")
        .request_timeout(Duration::from_millis(50))
        .transport(Arc::new(FnTransport::new(|_| async {
            tokio::time::sleep(Duration::from_secs(5)).await;
            Ok(ok_json("{}"))
        })))
        .build()
        .unwrap();
    let err = client
        .get(Route::new("/users/@me"))
        .send()
        .await
        .unwrap_err();
    assert!(matches!(err, HttpError::Timeout));
}

struct ChannelBucket {
    remaining: u32,
    reset_at: Option<tokio::time::Instant>,
}

fn channel_id_from_path(path: &str) -> u64 {
    let mut parts = path.split('/');
    parts
        .find(|p| *p == "channels")
        .and_then(|_| parts.next())
        .and_then(|p| p.parse().ok())
        .unwrap_or(0)
}

fn secs_until(reset_at: Option<tokio::time::Instant>, now: tokio::time::Instant) -> f64 {
    reset_at.map_or(1.0, |r| r.saturating_duration_since(now).as_secs_f64())
}

fn rate_headers(remaining: u32, reset_after: f64) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert("x-ratelimit-limit", HeaderValue::from_static("5"));
    headers.insert(
        "x-ratelimit-remaining",
        HeaderValue::from_str(&remaining.to_string()).unwrap(),
    );
    headers.insert(
        "x-ratelimit-reset-after",
        HeaderValue::from_str(&reset_after.to_string()).unwrap(),
    );
    headers.insert("x-ratelimit-bucket", HeaderValue::from_static("msg"));
    headers
}

fn enforce_per_channel(
    path: &str,
    state: &Mutex<HashMap<u64, ChannelBucket>>,
    avoidable: &AtomicU32,
) -> RawResponse {
    let channel = channel_id_from_path(path);
    let now = tokio::time::Instant::now();
    let mut map = state.lock();
    let bucket = map.entry(channel).or_insert(ChannelBucket {
        remaining: 5,
        reset_at: Some(now + Duration::from_secs(1)),
    });
    if let Some(reset) = bucket.reset_at
        && now >= reset
    {
        bucket.remaining = 5;
        bucket.reset_at = Some(now + Duration::from_secs(1));
    }
    if bucket.remaining == 0 {
        avoidable.fetch_add(1, Ordering::SeqCst);
        let retry = secs_until(bucket.reset_at, now);
        let mut headers = rate_headers(0, retry);
        headers.insert(
            "retry-after",
            HeaderValue::from_str(&retry.to_string()).unwrap(),
        );
        return RawResponse {
            status: StatusCode::TOO_MANY_REQUESTS,
            headers,
            body: Bytes::from(format!(r#"{{"retry_after":{retry},"global":false}}"#)),
        };
    }
    bucket.remaining -= 1;
    let reset_after = secs_until(bucket.reset_at, now);
    RawResponse {
        status: StatusCode::OK,
        headers: rate_headers(bucket.remaining, reset_after),
        body: Bytes::from("{}"),
    }
}

#[tokio::test(start_paused = true)]
async fn load_test_zero_avoidable_429s() {
    let state = Arc::new(Mutex::new(HashMap::<u64, ChannelBucket>::new()));
    let avoidable = Arc::new(AtomicU32::new(0));
    let s = state.clone();
    let a = avoidable.clone();

    let client = Client::builder("t")
        .global_rps(10_000)
        .transport(Arc::new(FnTransport::new(move |req: Request<Bytes>| {
            let s = s.clone();
            let a = a.clone();
            async move { Ok(enforce_per_channel(req.uri().path(), &s, &a)) }
        })))
        .build()
        .unwrap();

    let mut tasks = Vec::new();
    for channel in 1..=100u64 {
        for _ in 0..20 {
            let http = client.clone();
            tasks.push(tokio::spawn(async move {
                http.post(Route::raw("/channels/{channel_id}/messages", channel))
                    .json(&json!({"content": "x"}))
                    .unwrap()
                    .send()
                    .await
            }));
        }
    }

    let mut ok = 0u32;
    for task in tasks {
        match task.await.unwrap() {
            Ok(_) => ok += 1,
            Err(HttpError::RateLimited { .. }) => {}
            Err(e) => panic!("unexpected error {e:?}"),
        }
    }

    assert_eq!(ok, 2000);
    assert_eq!(
        avoidable.load(Ordering::SeqCst),
        0,
        "limiter sent a request the mock had to 429"
    );
}

const MESSAGE_JSON: &str = r#"{
  "id": "1",
  "channel_id": "2",
  "author": {"id": "3", "username": "bot", "discriminator": "0", "avatar": null},
  "content": "hello",
  "timestamp": "2017-07-11T17:27:07.299000+00:00",
  "edited_timestamp": null,
  "tts": false,
  "mention_everyone": false,
  "mentions": [],
  "mention_roles": [],
  "attachments": [],
  "embeds": [],
  "pinned": false,
  "type": 0
}"#;

const USER_JSON: &str = r#"{"id":"1","username":"bot","discriminator":"0","avatar":null}"#;

#[tokio::test]
async fn create_message_sends_json_and_await() {
    use drs_model::id::Id;
    use drs_model::id::marker::ChannelMarker;

    let seen = Arc::new(Mutex::new(None::<(String, String)>));
    let flag = seen.clone();
    let client = client_with(move |req| {
        *flag.lock() = Some((
            req.uri().path().to_owned(),
            String::from_utf8(req.body().to_vec()).unwrap(),
        ));
        ok_json(MESSAGE_JSON)
    });
    let channel = Id::<ChannelMarker>::new(2).unwrap();
    let msg = client
        .create_message(channel)
        .content("hello")
        .unwrap()
        .await
        .unwrap();
    assert_eq!(&*msg.content, "hello");
    let (path, body) = seen.lock().clone().unwrap();
    assert!(path.ends_with("/channels/2/messages"));
    assert!(body.contains("\"content\":\"hello\""));
    assert!(!body.contains("files["));
}

#[tokio::test]
async fn create_message_multipart_includes_files() {
    use drs_model::id::Id;
    use drs_model::id::marker::ChannelMarker;

    let seen = Arc::new(Mutex::new(None::<(String, String)>));
    let flag = seen.clone();
    let client = client_with(move |req| {
        let ct = req
            .headers()
            .get("content-type")
            .unwrap()
            .to_str()
            .unwrap()
            .to_owned();
        *flag.lock() = Some((ct, String::from_utf8_lossy(req.body()).into_owned()));
        ok_json(MESSAGE_JSON)
    });
    let channel = Id::<ChannelMarker>::new(2).unwrap();
    client
        .create_message(channel)
        .content("hi")
        .unwrap()
        .attach(crate::File::bytes("a.txt", &b"data"[..]))
        .unwrap()
        .send()
        .await
        .unwrap();
    let (ct, body) = seen.lock().clone().unwrap();
    assert!(ct.starts_with("multipart/form-data; boundary="));
    assert!(body.contains("name=\"payload_json\""));
    assert!(body.contains("name=\"files[0]\""));
    assert!(body.contains("filename=\"a.txt\""));
    assert!(body.contains("data"));
}

#[test]
fn create_message_rejects_overlong_content() {
    use drs_model::id::Id;
    use drs_model::id::marker::ChannelMarker;

    let client = client_with(|_| ok_json("{}"));
    let channel = Id::<ChannelMarker>::new(1).unwrap();
    let result = client.create_message(channel).content("x".repeat(2001));
    assert!(matches!(result, Err(HttpError::Validation(_))));
}

#[tokio::test]
async fn current_user_and_get_channel() {
    use drs_model::id::Id;
    use drs_model::id::marker::ChannelMarker;

    let paths = Arc::new(Mutex::new(Vec::new()));
    let flag = paths.clone();
    let client = client_with(move |req| {
        flag.lock().push(req.uri().path().to_owned());
        if req.uri().path().contains("/users/@me") {
            ok_json(USER_JSON)
        } else {
            ok_json(
                r#"{"id":"2","type":0,"guild_id":"1","position":0,"permission_overwrites":[],"name":"general"}"#,
            )
        }
    });
    let user = client.current_user().await.unwrap();
    assert_eq!(&*user.username, "bot");
    let channel = client
        .get_channel(Id::<ChannelMarker>::new(2).unwrap())
        .await
        .unwrap();
    assert_eq!(channel.id.get(), 2);
    let paths = paths.lock().clone();
    assert!(paths.iter().any(|p| p.ends_with("/users/@me")));
    assert!(paths.iter().any(|p| p.ends_with("/channels/2")));
}

fn no_content() -> RawResponse {
    let mut resp = ok_json("");
    resp.status = StatusCode::NO_CONTENT;
    resp.body = Bytes::new();
    resp
}

#[tokio::test]
async fn delete_messages_rejects_short_list() {
    use drs_model::id::Id;
    use drs_model::id::marker::{ChannelMarker, MessageMarker};

    let client = client_with(|_| no_content());
    let channel = Id::<ChannelMarker>::new(2).unwrap();
    let err = client
        .delete_messages(channel, [Id::<MessageMarker>::new(1).unwrap()])
        .await
        .unwrap_err();
    assert!(matches!(err, HttpError::Validation(_)));
}

#[tokio::test]
async fn pin_and_bulk_delete_paths() {
    use drs_model::id::Id;
    use drs_model::id::marker::{ChannelMarker, MessageMarker};

    let paths = Arc::new(Mutex::new(Vec::new()));
    let flag = paths.clone();
    let client = client_with(move |req| {
        flag.lock().push((
            req.method().as_str().to_owned(),
            req.uri().path().to_owned(),
        ));
        no_content()
    });
    let channel = Id::<ChannelMarker>::new(2).unwrap();
    let a = Id::<MessageMarker>::new(10).unwrap();
    let b = Id::<MessageMarker>::new(11).unwrap();
    client.pin_message(channel, a).await.unwrap();
    client.delete_messages(channel, [a, b]).await.unwrap();
    let paths = paths.lock().clone();
    assert!(
        paths
            .iter()
            .any(|(m, p)| m == "PUT" && p.ends_with("/channels/2/messages/pins/10"))
    );
    assert!(
        paths
            .iter()
            .any(|(m, p)| m == "POST" && p.ends_with("/channels/2/messages/bulk-delete"))
    );
}

#[tokio::test]
async fn followup_is_global_exempt_and_uses_raw_token() {
    use drs_model::id::Id;
    use drs_model::id::marker::ApplicationMarker;
    use drs_model::token::InteractionToken;

    let seen = Arc::new(Mutex::new(None::<String>));
    let flag = seen.clone();
    let client = client_with(move |req| {
        *flag.lock() = Some(req.uri().path().to_owned());
        ok_json(MESSAGE_JSON)
    });
    let token = InteractionToken::new("secret-followup-token");
    client
        .create_followup_message(Id::<ApplicationMarker>::new(9).unwrap(), &token)
        .content("hi")
        .unwrap()
        .await
        .unwrap();
    let path = seen.lock().clone().unwrap();
    assert!(path.contains("secret-followup-token"));
    assert!(!path.contains("***"));
}

#[tokio::test]
async fn execute_webhook_fills_token_in_path() {
    use drs_model::id::Id;
    use drs_model::id::marker::WebhookMarker;
    use drs_model::token::WebhookToken;

    let seen = Arc::new(Mutex::new(None::<String>));
    let flag = seen.clone();
    let client = client_with(move |req| {
        *flag.lock() = Some(req.uri().to_string());
        ok_json(MESSAGE_JSON)
    });
    let token = WebhookToken::new("wh-secret");
    client
        .execute_webhook(Id::<WebhookMarker>::new(8).unwrap(), &token)
        .content("hi")
        .unwrap()
        .await
        .unwrap();
    let uri = seen.lock().clone().unwrap();
    assert!(uri.contains("/webhooks/8/wh-secret"));
    assert!(uri.contains("wait=true"));
}

#[tokio::test]
async fn bulk_overwrite_commands_sends_array() {
    use drs_model::id::Id;
    use drs_model::id::marker::ApplicationMarker;

    let seen = Arc::new(Mutex::new(None::<(String, String)>));
    let flag = seen.clone();
    let client = client_with(move |req| {
        *flag.lock() = Some((
            req.uri().path().to_owned(),
            String::from_utf8(req.body().to_vec()).unwrap(),
        ));
        ok_json("[]")
    });
    client
        .bulk_overwrite_global_commands(
            Id::<ApplicationMarker>::new(9).unwrap(),
            [crate::CommandPayload::chat_input("ping", "pong")],
        )
        .await
        .unwrap();
    let (path, body) = seen.lock().clone().unwrap();
    assert!(path.ends_with("/applications/9/commands"));
    assert!(body.contains("\"name\":\"ping\""));
    assert!(body.contains("\"description\":\"pong\""));
}

#[test]
fn builds_real_tls_client() {
    let client = Client::new("not-a-real-token");
    assert!(client.is_ok(), "{client:?}");
    let debug = format!("{:?}", client.unwrap());
    assert!(!debug.contains("not-a-real-token"));
}
