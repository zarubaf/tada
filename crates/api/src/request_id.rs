//! The request ID and the request log (ADR 0035).

use std::net::SocketAddr;
use std::time::Instant;

use axum::extract::{ConnectInfo, MatchedPath, Request, State};
use axum::http::HeaderValue;
use axum::middleware::Next;
use axum::response::Response;
use tracing::Instrument;
use uuid::Uuid;

use crate::ApiState;

pub const HEADER: &str = "x-request-id";

tokio::task_local! {
    static REQUEST_ID: Uuid;
}

/// The ID of the current request. Outside a request, for example in a test of a single handler, it is
/// the nil UUID.
pub fn current() -> Uuid {
    REQUEST_ID.try_with(|id| *id).unwrap_or(Uuid::nil())
}

/// Gives each request an ID, a log span with this ID, one log line at the end and the response header.
pub async fn track(State(state): State<ApiState>, request: Request, next: Next) -> Response {
    let request_id = forwarded_id(&state, &request).unwrap_or_else(Uuid::now_v7);
    // The route template, never the raw path: a path can contain a token (ADR 0008).
    let route = request
        .extensions()
        .get::<MatchedPath>()
        .map_or("unmatched", |path| path.as_str())
        .to_owned();
    let method = request.method().clone();
    let span = tracing::info_span!("request", request_id = %request_id);
    let started = Instant::now();

    let mut response = REQUEST_ID
        .scope(request_id, next.run(request))
        .instrument(span.clone())
        .await;

    span.in_scope(|| {
        tracing::info!(
            method = %method,
            route,
            status = response.status().as_u16(),
            duration_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
            "request completed"
        );
    });
    if let Ok(value) = HeaderValue::from_str(&request_id.to_string()) {
        response.headers_mut().insert(HEADER, value);
    }
    response
}

/// The `X-Request-Id` of a trusted proxy, if it is a UUID.
fn forwarded_id(state: &ApiState, request: &Request) -> Option<Uuid> {
    let peer = request
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()?
        .0
        .ip();
    if !state
        .trusted_proxies
        .iter()
        .any(|range| range.contains(&peer))
    {
        return None;
    }
    request.headers().get(HEADER)?.to_str().ok()?.parse().ok()
}
