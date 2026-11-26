//! Axum integration for `cordex`: Ed25519 [`VerifiedInteraction`] extractor,
//! [`interactions_router`], and Kubernetes-ready [`health_router`].

use std::future::Future;
use std::sync::Arc;

use axum::Router;
use axum::body::Bytes;
use axum::extract::{FromRequest, Request};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json, Response};
use axum::routing::{get, post};
use cordex::bot::HealthSnapshot;
use cordex_interactions::{PublicKey, verify_signature};
use cordex_model::interaction::{Interaction, InteractionType};
use serde_json::{Value, json};

/// Application state that provides a Discord Ed25519 [`PublicKey`] for
/// [`VerifiedInteraction`].
pub trait PublicKeyState {
    /// Return the application's Ed25519 public key.
    fn public_key(&self) -> &PublicKey;
}

impl PublicKeyState for PublicKey {
    fn public_key(&self) -> &PublicKey {
        self
    }
}

/// Axum extractor that verifies Discord's `X-Signature-Ed25519` and
/// `X-Signature-Timestamp` headers before deserializing the [`Interaction`].
///
/// Returns `401 Unauthorized` when the signature is missing or invalid, and
/// `400 Bad Request` when the JSON body cannot be parsed.
#[derive(Debug, Clone)]
pub struct VerifiedInteraction(pub Interaction);

impl<S> FromRequest<S> for VerifiedInteraction
where
    S: PublicKeyState + Send + Sync,
{
    type Rejection = StatusCode;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        let sig = req
            .headers()
            .get("x-signature-ed25519")
            .and_then(|v| v.to_str().ok())
            .ok_or(StatusCode::UNAUTHORIZED)?
            .to_owned();
        let ts = req
            .headers()
            .get("x-signature-timestamp")
            .and_then(|v| v.to_str().ok())
            .ok_or(StatusCode::UNAUTHORIZED)?
            .to_owned();

        let body = Bytes::from_request(req, state)
            .await
            .map_err(|_| StatusCode::BAD_REQUEST)?;

        if verify_signature(state.public_key(), &sig, &ts, &body).is_err() {
            return Err(StatusCode::UNAUTHORIZED);
        }

        let interaction: Interaction =
            serde_json::from_slice(&body).map_err(|_| StatusCode::BAD_REQUEST)?;
        Ok(Self(interaction))
    }
}

/// Build an Axum [`Router`] for Discord HTTP interactions (`POST /`).
///
/// Automatically verifies Ed25519 signatures and responds to `PING`
/// (`type = 1`) with `PONG` (`{"type": 1}`). All other interactions are
/// forwarded to `handler`.
pub fn interactions_router<F, Fut>(public_key: PublicKey, handler: F) -> Router
where
    F: Fn(Interaction) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Response> + Send + 'static,
{
    let handler = Arc::new(handler);
    Router::new()
        .route(
            "/",
            post(
                move |VerifiedInteraction(interaction): VerifiedInteraction| {
                    let handler = Arc::clone(&handler);
                    async move {
                        if interaction.kind == InteractionType::Ping {
                            return Json(json!({ "type": 1 })).into_response();
                        }
                        handler(interaction).await
                    }
                },
            ),
        )
        .with_state(public_key)
}

/// Build an Axum [`Router`] exposing `/healthz` and `/readyz` endpoints
/// backed by [`HealthSnapshot`].
///
/// - `GET /healthz` always returns `200 OK` with the JSON snapshot.
/// - `GET /readyz` returns `200 OK` when [`HealthSnapshot::is_ready`] is `true`,
///   otherwise `503 Service Unavailable`.
pub fn health_router<F>(snapshot: F) -> Router
where
    F: Fn() -> HealthSnapshot + Send + Sync + 'static,
{
    let snapshot: Arc<dyn Fn() -> HealthSnapshot + Send + Sync> = Arc::new(snapshot);
    let snap_health = Arc::clone(&snapshot);
    let snap_ready = snapshot;

    Router::new()
        .route(
            "/healthz",
            get(move || {
                let snap = (snap_health)();
                async move { (StatusCode::OK, Json(snap.to_json())) }
            }),
        )
        .route(
            "/readyz",
            get(move || {
                let snap = (snap_ready)();
                async move {
                    let status = if snap.is_ready() {
                        StatusCode::OK
                    } else {
                        StatusCode::SERVICE_UNAVAILABLE
                    };
                    (status, Json::<Value>(snap.to_json()))
                }
            }),
        )
}

#[cfg(test)]
mod tests {
    use std::fmt::Write;

    use super::*;
    use axum::body::Body;
    use axum::http::Request as HttpRequest;
    use ed25519_dalek::{Signer, SigningKey};
    use tower_service::Service;

    fn to_hex(bytes: &[u8]) -> String {
        let mut out = String::with_capacity(bytes.len() * 2);
        for b in bytes {
            let _ = write!(out, "{b:02x}");
        }
        out
    }

    #[tokio::test]
    async fn interactions_router_rejects_unsigned_and_answers_signed_ping() {
        let signing = SigningKey::from_bytes(&[7u8; 32]);
        let hex_key = to_hex(&signing.verifying_key().to_bytes());
        let Ok(pk) = PublicKey::from_hex(&hex_key) else {
            panic!("valid public key");
        };

        let mut app = interactions_router(pk, |_interaction| async move {
            StatusCode::NO_CONTENT.into_response()
        });

        // Missing signature -> 401
        let Ok(req) = HttpRequest::builder()
            .method("POST")
            .uri("/")
            .body(Body::from(
                r#"{"id":"1","application_id":"2","type":1,"token":"t","version":1}"#,
            ))
        else {
            panic!("request");
        };
        let Ok(resp) = app.call(req).await;
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

        // Signed PING -> 200 PONG
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs())
            .to_string();
        let body = br#"{"id":"1","application_id":"2","type":1,"token":"t","version":1}"#;
        let mut msg = Vec::from(ts.as_bytes());
        msg.extend_from_slice(body);
        let sig_hex = to_hex(&signing.sign(&msg).to_bytes());

        let Ok(req) = HttpRequest::builder()
            .method("POST")
            .uri("/")
            .header("x-signature-timestamp", &ts)
            .header("x-signature-ed25519", sig_hex)
            .body(Body::from(&body[..]))
        else {
            panic!("request");
        };
        let Ok(resp) = app.call(req).await;
        assert_eq!(resp.status(), StatusCode::OK);
    }
}
