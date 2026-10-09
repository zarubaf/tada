//! The request ID and the request log (ADR 0035).

use std::time::Instant;

use axum::extract::{MatchedPath, Request, State};
use axum::http::HeaderValue;
use axum::middleware::Next;
use axum::response::Response;
use tada_app::caller::RequestId;
use tracing::Instrument;
use uuid::Uuid;

use crate::ApiState;
use crate::client_ip;

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
///
/// The server always makes the ID. A proxy can pass the header of the client on, so a client could
/// otherwise give its request the ID of another request. The ID of a trusted proxy goes into the
/// log line only, as `proxy_request_id`, to join the log of the proxy.
pub async fn track(State(state): State<ApiState>, mut request: Request, next: Next) -> Response {
    let request_id = Uuid::now_v7();
    let proxy_request_id = forwarded_id(&state, &request);
    // The task-local does not reach a handler in another task; the extensions do.
    request.extensions_mut().insert(RequestId::new(request_id));
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
            proxy_request_id = proxy_request_id.as_ref().map(tracing::field::display),
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
    let peer = client_ip::peer(request.extensions())?;
    if !client_ip::is_trusted_proxy(&state.trusted_proxies, peer) {
        return None;
    }
    request.headers().get(HEADER)?.to_str().ok()?.parse().ok()
}
