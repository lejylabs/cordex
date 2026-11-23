//! Mock REST tests.

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
    assert!(ua.starts_with("DiscordBot (https://github.com/lejylabs/cordex, "));
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
    let m = client.metrics();
    assert_eq!((m.requests, m.rate_limited_other), (2, 1));
    assert_eq!(m.invalid_requests_window, 1);
    // The route's pre-header bucket and the one for its hash `b`.
    assert_eq!(m.active_buckets, Some(2));
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
    assert_eq!(client.metrics().rate_limited_global, 1);
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
    let m = client.metrics();
    assert_eq!((m.rate_limited_shared, m.invalid_requests_window), (2, 0));
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
    let m = client.metrics();
    assert_eq!(m.requests, 2000);
    assert_eq!(
        m.rate_limited_user + m.rate_limited_global + m.rate_limited_other,
        0
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
    use cordex_model::id::Id;
    use cordex_model::id::marker::ChannelMarker;

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
async fn default_allowed_mentions_are_sent() {
    use cordex_model::id::Id;
    use cordex_model::id::marker::ChannelMarker;
    use cordex_model::message::AllowedMentions;

    let seen = Arc::new(Mutex::new(None::<String>));
    let flag = seen.clone();
    let client = Client::builder("test.token.secret")
        .default_allowed_mentions(AllowedMentions::none())
        .transport(Arc::new(FnTransport::new(move |req: Request<Bytes>| {
            *flag.lock() = Some(String::from_utf8(req.body().to_vec()).unwrap());
            async move { Ok(ok_json(MESSAGE_JSON)) }
        })))
        .build()
        .unwrap();
    let channel = Id::<ChannelMarker>::new(2).unwrap();
    client
        .create_message(channel)
        .content("hello")
        .unwrap()
        .await
        .unwrap();
    let body = seen.lock().clone().unwrap();
    assert!(
        body.contains("\"allowed_mentions\":{\"parse\":[]}"),
        "{body}"
    );
}

#[tokio::test]
async fn create_message_multipart_includes_files() {
    use cordex_model::id::Id;
    use cordex_model::id::marker::ChannelMarker;

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
    use cordex_model::id::Id;
    use cordex_model::id::marker::ChannelMarker;

    let client = client_with(|_| ok_json("{}"));
    let channel = Id::<ChannelMarker>::new(1).unwrap();
    let result = client.create_message(channel).content("x".repeat(2001));
    assert!(matches!(result, Err(HttpError::Validation(_))));
}

#[tokio::test]
async fn current_user_and_get_channel() {
    use cordex_model::id::Id;
    use cordex_model::id::marker::ChannelMarker;

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
    use cordex_model::id::Id;
    use cordex_model::id::marker::{ChannelMarker, MessageMarker};

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
    use cordex_model::id::Id;
    use cordex_model::id::marker::{ChannelMarker, MessageMarker};

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
    use cordex_model::id::Id;
    use cordex_model::id::marker::ApplicationMarker;
    use cordex_model::token::InteractionToken;

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
    use cordex_model::id::Id;
    use cordex_model::id::marker::WebhookMarker;
    use cordex_model::token::WebhookToken;

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
    use cordex_model::id::Id;
    use cordex_model::id::marker::ApplicationMarker;

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

/// `(METHOD path?query, body)` of every request.
type RequestLog = Arc<Mutex<Vec<(String, String)>>>;

/// Records `METHOD path?query` and the body of every request.
fn recording(body: &'static str) -> (Client, RequestLog) {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let log = Arc::clone(&seen);
    let client = client_with(move |req| {
        let line = format!(
            "{} {}",
            req.method(),
            req.uri().path_and_query().map_or("", |p| p.as_str())
        );
        let sent = String::from_utf8_lossy(req.body()).into_owned();
        log.lock().push((line, sent));
        ok_json(body)
    });
    (client, seen)
}

fn id<T>(n: u64) -> cordex_model::id::Id<T> {
    cordex_model::id::Id::new(n).unwrap()
}

#[tokio::test]
async fn query_requests_send_their_options() {
    let (client, seen) = recording(r#"{"threads":[],"members":[],"has_more":true}"#);
    let list = client
        .get_public_archived_threads(id(7))
        .limit(5)
        .before("2026-11-23T00:00:00Z".to_owned())
        .await
        .unwrap();
    assert!(list.has_more);
    let line = seen.lock()[0].0.clone();
    assert!(
        line.starts_with("GET /api/v10/channels/7/threads/archived/public?"),
        "{line}"
    );
    assert!(line.contains("limit=5"), "{line}");
    assert!(
        line.contains("before=2026%2D11%2D23T00%3A00%3A00Z"),
        "{line}"
    );
}

#[tokio::test]
async fn permissions_endpoint_returns_plain_permissions() {
    let (client, seen) = recording(r#"{"permissions":"1024"}"#);
    let perms = client
        .get_channel_member_permissions(id(1), id(2))
        .implicit(false)
        .await
        .unwrap();
    assert_eq!(
        perms.0,
        cordex_model::permissions::Permissions::VIEW_CHANNEL
    );
    assert_eq!(
        seen.lock()[0].0,
        "GET /api/v10/channels/1/permissions/member/2?implicit=false"
    );
}

#[tokio::test]
async fn simple_endpoints_use_the_documented_route() {
    let (client, seen) = recording("{}");
    client.trigger_typing(id(3)).await.unwrap();
    client.remove_thread_member(id(4), id(5)).await.unwrap();
    client
        .follow_announcement_channel(id(6), id(8))
        .await
        .unwrap_err(); // `{}` is not a FollowedChannel
    let _ = client.get_lobby(id(9)).await;
    let log = seen.lock();
    assert_eq!(log[0].0, "POST /api/v10/channels/3/typing");
    assert_eq!(log[1].0, "DELETE /api/v10/channels/4/thread-members/5");
    assert_eq!(log[2].0, "POST /api/v10/channels/6/followers");
    assert_eq!(log[2].1, r#"{"webhook_channel_id":"8"}"#);
    assert_eq!(log[3].0, "GET /api/v10/lobbies/9");
}

#[tokio::test]
async fn local_checks_run_before_sending() {
    let (client, seen) = recording("{}");
    let err = client
        .set_voice_channel_status(id(1), Some("x".repeat(501)))
        .await
        .unwrap_err();
    assert!(matches!(err, HttpError::Validation(_)), "{err}");
    let token = cordex_model::token::WebhookToken::new("tok");
    let err = client
        .update_webhook_with_token(id(2), &token)
        .channel_id(id(3))
        .await
        .unwrap_err();
    assert!(matches!(err, HttpError::Validation(_)), "{err}");
    assert!(seen.lock().is_empty());
}

#[tokio::test]
async fn json_requests_send_only_set_fields() {
    let (client, seen) = recording(r#"{"pruned":3}"#);
    let result = client
        .begin_guild_prune(id(9))
        .days(14)
        .compute_prune_count(true)
        .reason("cleanup")
        .unwrap()
        .await
        .unwrap();
    assert_eq!(result.pruned, Some(3));
    let (line, body) = seen.lock()[0].clone();
    assert_eq!(line, "POST /api/v10/guilds/9/prune");
    let body: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(body, json!({"days": 14, "compute_prune_count": true}));
}

#[tokio::test]
async fn nullable_fields_send_null() {
    let (client, seen) = recording("{}");
    let _ = client.update_current_member(id(1)).nick(None).await;
    assert_eq!(seen.lock()[0].1, r#"{"nick":null}"#);
}

#[tokio::test]
async fn guild_member_endpoints() {
    let (client, seen) = recording("[]");
    client
        .search_guild_members(id(2), "ze ny")
        .limit(5)
        .await
        .unwrap();
    let _ = client
        .add_guild_member(id(2), id(3), "oauth-token")
        .nick("n")
        .await;
    let log = seen.lock();
    assert_eq!(
        log[0].0,
        "GET /api/v10/guilds/2/members/search?query=ze%20ny&limit=5"
    );
    assert_eq!(log[1].0, "PUT /api/v10/guilds/2/members/3");
    let body: serde_json::Value = serde_json::from_str(&log[1].1).unwrap();
    assert_eq!(body, json!({"access_token": "oauth-token", "nick": "n"}));
}

#[tokio::test]
async fn bulk_ban_checks_limits_locally() {
    let (client, seen) = recording("{}");
    assert!(client.bulk_ban(id(1), vec![], 0).await.is_err());
    assert!(client.bulk_ban(id(1), vec![id(2)], 604_801).await.is_err());
    let users: Vec<_> = (1..=201).map(id).collect();
    assert!(client.bulk_ban(id(1), users, 0).await.is_err());
    assert!(seen.lock().is_empty());
}

#[tokio::test]
async fn sticker_upload_is_a_plain_form() {
    let (client, seen) = recording(
        r#"{"id":"1","name":"s","tags":"t","type":2,"format_type":1,"description":null}"#,
    );
    let file = crate::File::bytes("s.png", vec![1u8, 2, 3]).content_type("image/png");
    client
        .create_guild_sticker(id(5), "s", "desc", "t", &file)
        .await
        .unwrap();
    let (line, body) = seen.lock()[0].clone();
    assert_eq!(line, "POST /api/v10/guilds/5/stickers");
    assert!(body.contains("name=\"name\"\r\n\r\ns\r\n"), "{body}");
    assert!(body.contains("name=\"tags\"\r\n\r\nt\r\n"), "{body}");
    assert!(body.contains("name=\"file\"; filename=\"s.png\""), "{body}");
    assert!(!body.contains("payload_json"), "{body}");
}

#[tokio::test]
async fn asset_sizes_are_checked_locally() {
    let (client, seen) = recording("{}");
    let big = format!("data:image/png;base64,{}", "A".repeat(400_000));
    let huge = format!("data:audio/ogg;base64,{}", "A".repeat(800_000));
    assert!(
        client
            .create_guild_emoji(id(1), "e", &big, &[])
            .await
            .is_err()
    );
    assert!(
        client
            .create_application_emoji(id(1), "e", "not-a-data-uri")
            .await
            .is_err()
    );
    let file = crate::File::bytes("x.png", vec![0u8; 600 * 1024]);
    assert!(
        client
            .create_guild_sticker(id(1), "s", "", "t", &file)
            .await
            .is_err()
    );
    assert!(
        client
            .create_guild_soundboard_sound(id(1), "s", &huge)
            .await
            .is_err()
    );
    assert!(seen.lock().is_empty());
}

#[tokio::test]
async fn auto_moderation_rule_body() {
    use cordex_model::auto_moderation::{AutoModerationEventType, AutoModerationTriggerType};
    let (client, seen) = recording("{}");
    let _ = client
        .create_auto_moderation_rule(
            id(7),
            "no spam",
            AutoModerationEventType::MessageSend,
            AutoModerationTriggerType::Spam,
            &[],
        )
        .enabled(true)
        .await;
    let (line, body) = seen.lock()[0].clone();
    assert_eq!(line, "POST /api/v10/guilds/7/auto-moderation/rules");
    let body: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(
        body,
        json!({"name": "no spam", "event_type": 1, "trigger_type": 3, "actions": [], "enabled": true})
    );
}

#[tokio::test]
async fn search_reports_not_indexed_202() {
    let client = client_with(|_| {
        RawResponse {
        status: StatusCode::ACCEPTED,
        headers: HeaderMap::new(),
        body: Bytes::from_static(
            br#"{"message":"Index not yet available. Try again later","code":110000,"documents_indexed":0,"retry_after":2}"#,
        ),
    }
    });
    let result = client
        .search_guild_messages(id(1))
        .content("hi")
        .await
        .unwrap();
    assert_eq!(
        result,
        crate::endpoint::SearchResult::NotIndexed {
            retry_after: 2.0,
            documents_indexed: 0
        }
    );
}

#[tokio::test]
async fn search_sends_filters_and_parses_hits() {
    let (client, seen) = recording(
        r#"{"doing_deep_historical_index":false,"total_results":0,"messages":[],"threads":[]}"#,
    );
    let result = client
        .search_guild_messages(id(1))
        .content("a b")
        .channel_id(id(2))
        .channel_id(id(3))
        .limit(5)
        .await
        .unwrap();
    assert!(matches!(result, crate::endpoint::SearchResult::Found(r) if r.total_results == 0));
    assert_eq!(
        seen.lock()[0].0,
        "GET /api/v10/guilds/1/messages/search?content=a%20b&channel_id=2&channel_id=3&limit=5"
    );
}

#[tokio::test]
async fn invite_target_users_csv_and_bulk_limits() {
    let (client, seen) = recording("");
    client
        .update_invite_target_users("abc", &[id(1), id(2)])
        .await
        .unwrap();
    let (line, body) = seen.lock()[0].clone();
    assert_eq!(line, "PUT /api/v10/invites/abc/target-users");
    assert!(
        body.contains("name=\"target_users_file\"; filename=\"target_users.csv\""),
        "{body}"
    );
    assert!(body.contains("user_id\n1\n2\n"), "{body}");
    let too_many: Vec<_> = (1..=1001).map(id).collect();
    assert!(
        client
            .bulk_add_invite_target_users("abc", &too_many)
            .await
            .is_err()
    );
    assert!(
        client
            .bulk_remove_invite_target_users("abc", &[])
            .await
            .is_err()
    );
    assert_eq!(seen.lock().len(), 1);
}

#[tokio::test]
async fn components_v2_flag_is_set_automatically() {
    use cordex_model::component::{ActionRow, Button, Container, TextDisplay};
    let (client, seen) = recording(
        r#"{"id":"1","channel_id":"2","author":{"id":"3","username":"a","discriminator":"0","global_name":null,"avatar":null},"content":"","timestamp":"2026-11-23T00:00:00+00:00","edited_timestamp":null,"tts":false,"mention_everyone":false,"mentions":[],"mention_roles":[],"attachments":[],"embeds":[],"pinned":false,"type":0}"#,
    );
    let _ = client
        .create_message(id(2))
        .components(vec![Container::new([TextDisplay::new("hi")]).into()])
        .send()
        .await;
    let _ = client
        .create_message(id(2))
        .components(vec![ActionRow::new([Button::primary("a", "A")]).into()])
        .send()
        .await;
    let log = seen.lock();
    let v2: serde_json::Value = serde_json::from_str(&log[0].1).unwrap();
    assert_eq!(v2["flags"], 1 << 15);
    let classic: serde_json::Value = serde_json::from_str(&log[1].1).unwrap();
    assert!(classic.get("flags").is_none(), "{classic}");
}

#[tokio::test]
async fn interaction_client_fills_in_the_application_id() {
    let (client, seen) = recording("[]");
    let app = client.interaction(id(42));
    assert_eq!(app.application_id().get(), 42);
    app.global_commands().await.unwrap();
    app.guild_commands(id(7)).await.unwrap();
    let _ = app.skus().await;
    let token = cordex_model::token::InteractionToken::new("tok");
    let _ = app.delete_response(&token).await;
    let log = seen.lock();
    assert_eq!(log[0].0, "GET /api/v10/applications/42/commands");
    assert_eq!(log[1].0, "GET /api/v10/applications/42/guilds/7/commands");
    assert_eq!(log[2].0, "GET /api/v10/applications/42/skus");
    assert_eq!(
        log[3].0,
        "DELETE /api/v10/webhooks/42/tok/messages/@original"
    );
}

#[tokio::test(start_paused = true)]
async fn metrics_count_user_429_5xx_timeouts_and_transport_errors() {
    let n = Arc::new(AtomicU32::new(0));
    let hits = n.clone();
    let client = Client::builder("t")
        .max_retries(0)
        .request_timeout(Duration::from_secs(1))
        .transport(Arc::new(FnTransport::new(move |_| {
            let i = hits.fetch_add(1, Ordering::SeqCst);
            async move {
                match i {
                    0 => {
                        let mut headers = HeaderMap::new();
                        headers.insert("x-ratelimit-scope", HeaderValue::from_static("user"));
                        headers.insert("retry-after", HeaderValue::from_static("30"));
                        Ok(RawResponse {
                            status: StatusCode::TOO_MANY_REQUESTS,
                            headers,
                            body: Bytes::from(r#"{"retry_after":30.0,"global":false}"#),
                        })
                    }
                    1 => Ok(RawResponse {
                        status: StatusCode::INTERNAL_SERVER_ERROR,
                        headers: HeaderMap::new(),
                        body: Bytes::new(),
                    }),
                    2 => {
                        tokio::time::sleep(Duration::from_secs(5)).await;
                        Ok(ok_json("{}"))
                    }
                    _ => Err(HttpError::validation("connection reset")),
                }
            }
        })))
        .build()
        .unwrap();
    for _ in 0..4 {
        let _ = client.get(Route::new("/users/@me")).send().await;
    }
    let m = client.metrics();
    assert_eq!(m.requests, 4);
    assert_eq!(m.rate_limited_user, 1);
    assert_eq!(m.server_errors, 1);
    assert_eq!(m.timeouts, 1);
    assert_eq!(m.transport_errors, 1);
}
