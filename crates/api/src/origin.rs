//! The `Origin` check of state-changing requests (ADR 0008).
//!
//! A browser sends `Origin` with each POST, PUT, PATCH and DELETE request.
//! A request from a page of another site then changes nothing, even if the browser sends the
//! session cookie with it.

use axum::extract::{Request, State};
use axum::http::{Method, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use tada_app::problem::ProblemCode;

use crate::ApiState;
use crate::problem::ApiError;

/// Rejects a state-changing request to the API whose `Origin` is not the origin of
/// `TADA_PUBLIC_URL`. A request without `Origin` is rejected too. No handler runs for it.
pub(crate) async fn check(State(state): State<ApiState>, request: Request, next: Next) -> Response {
    if changes_state(request.method())
        && request.uri().path().starts_with("/api/")
        && !has_origin(&request, state.public_url.origin())
    {
        return ApiError::new(ProblemCode::Forbidden)
            .with_detail("The Origin header of the request is missing or not the origin of tada.")
            .into_response();
    }
    next.run(request).await
}

fn changes_state(method: &Method) -> bool {
    matches!(
        *method,
        Method::POST | Method::PUT | Method::PATCH | Method::DELETE
    )
}

/// True if the request has exactly one `Origin` header and its value is `origin`.
fn has_origin(request: &Request, origin: &str) -> bool {
    let mut values = request.headers().get_all(header::ORIGIN).iter();
    matches!(
        (values.next(), values.next()),
        (Some(value), None) if value.as_bytes() == origin.as_bytes()
    )
}
