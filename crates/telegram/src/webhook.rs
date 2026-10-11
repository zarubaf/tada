//! `POST /telegram/webhook`: the updates of the Bot API in webhook mode (ADR 0076).

use std::sync::Arc;

use axum::Router;
use axum::body::to_bytes;
use axum::extract::{Request, State};
use axum::http::{HeaderValue, StatusCode};
use axum::routing::post;
use frankenstein::updates::Update;
use secrecy::{ExposeSecret, SecretString};
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;

use crate::{Gateway, UpdateSource};

/// The path that `setWebhook` registers below `TADA_PUBLIC_URL`.
pub(crate) const WEBHOOK_PATH: &str = "/telegram/webhook";
/// Telegram sends the secret of `setWebhook` in this header.
const SECRET_HEADER: &str = "x-telegram-bot-api-secret-token";
/// An update is far smaller; a larger body is not from Telegram.
const BODY_LIMIT: usize = 1024 * 1024;

#[derive(Clone)]
struct Webhook {
    gateway: Arc<Gateway>,
    secret: SecretString,
}

/// The webhook route in webhook mode, and no route in polling mode.
pub fn routes(gateway: Arc<Gateway>, source: &UpdateSource) -> Router {
    match source {
        UpdateSource::Polling => Router::new(),
        UpdateSource::Webhook { secret, .. } => Router::new()
            .route(WEBHOOK_PATH, post(receive))
            .with_state(Webhook {
                gateway,
                secret: secret.clone(),
            }),
    }
}

/// Checks the secret before it reads the body, then handles the update like a polled one.
/// The answer has no detail: a caller without the secret learns nothing.
async fn receive(State(webhook): State<Webhook>, request: Request) -> StatusCode {
    let given = request
        .headers()
        .get(SECRET_HEADER)
        .map_or(&[][..], HeaderValue::as_bytes);
    // The digests have one length, so the comparison does not show the length of the secret.
    let expected = Sha256::digest(webhook.secret.expose_secret().as_bytes());
    if !bool::from(Sha256::digest(given).ct_eq(&expected)) {
        tracing::warn!("a webhook request has no valid secret");
        return StatusCode::UNAUTHORIZED;
    }
    let Ok(body) = to_bytes(request.into_body(), BODY_LIMIT).await else {
        tracing::warn!("a webhook request body is too large or incomplete");
        return StatusCode::PAYLOAD_TOO_LARGE;
    };
    let Ok(update) = serde_json::from_slice::<Update>(&body) else {
        // Telegram would send the update again after an error, so the gateway skips it as in polling
        // mode (see `bot_error`).
        tracing::warn!("the gateway skips an update that does not decode");
        return StatusCode::OK;
    };
    // The dedupe makes a new delivery safe, so Telegram can try again after an error.
    match webhook.gateway.handle(update).await {
        Ok(()) => StatusCode::OK,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}
