//! The guard of each MCP request: the `Origin` check and the token authentication (ADR 0039, ADR 0040).

use axum::Json;
use axum::extract::{Request, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use tada_app::auth::{Authenticated, AuthenticationError, Authenticator, Credential};
use tada_app::caller::RequestId;
use tada_app::problem::{CommandError, ProblemCode};
use uuid::Uuid;

use crate::McpState;

/// Rejects a request with a foreign `Origin` (403) or without a valid personal API token (401).
/// For a valid token, it gives the `AiCaller` of the request to the tools in the extensions of the request.
pub(crate) async fn check(
    State(state): State<McpState>,
    mut request: Request,
    next: Next,
) -> Response {
    let request_id = request_id(request.extensions());
    let problem = |code| problem(code, request_id);
    if !origin_allowed(request.headers(), state.public_url.origin()) {
        return problem(ProblemCode::Forbidden);
    }
    let credential = bearer_token(request.headers()).map(Credential::ApiToken);
    let authenticated = match state.authenticator.authenticate(credential).await {
        Ok(authenticated) => authenticated,
        Err(error) => {
            if let AuthenticationError::Store(store_error) = &error {
                tracing::error!(error = %error_chain(store_error), "the store failed");
            }
            return problem(error.code());
        }
    };
    match authenticated {
        Authenticated::Ai(caller) => {
            let caller = caller.with_request(request_id);
            request.extensions_mut().insert(caller);
            next.run(request).await
        }
        // The token authenticator never gives a member: only AI clients use this server (ADR 0039).
        Authenticated::Member(_member) => problem(ProblemCode::Unauthenticated),
    }
}

/// The ID that the HTTP server gave the request (ADR 0035).
pub(crate) fn request_id(extensions: &axum::http::Extensions) -> Option<Uuid> {
    extensions.get::<RequestId>().map(|id| id.as_uuid())
}

/// The error and all its sources in one line, for logs.
pub(crate) fn error_chain(error: &dyn std::error::Error) -> String {
    let mut text = error.to_string();
    let mut source = error.source();
    while let Some(cause) = source {
        text.push_str(": ");
        text.push_str(&cause.to_string());
        source = cause.source();
    }
    text
}

/// A browser sends `Origin`; an MCP client usually does not. A request with `Origin` must come from tada itself.
fn origin_allowed(headers: &HeaderMap, origin: &str) -> bool {
    let mut values = headers.get_all(header::ORIGIN).iter();
    match (values.next(), values.next()) {
        (None, _) => true,
        (Some(value), None) => value.as_bytes() == origin.as_bytes(),
        (Some(_), Some(_)) => false,
    }
}

/// The token of the only `Authorization: Bearer` header of the request.
fn bearer_token(headers: &HeaderMap) -> Option<&str> {
    let mut values = headers.get_all(header::AUTHORIZATION).iter();
    let (Some(value), None) = (values.next(), values.next()) else {
        return None;
    };
    let (scheme, token) = value.to_str().ok()?.split_once(' ')?;
    let token = token.trim();
    (scheme.eq_ignore_ascii_case("bearer") && !token.is_empty()).then_some(token)
}

/// The body of an error response: the problem code, its meaning and the request ID (ADR 0037).
#[derive(Serialize)]
struct Problem {
    /// The URL of the code in the public catalog.
    #[serde(rename = "type")]
    type_url: String,
    code: &'static str,
    title: &'static str,
    status: u16,
    /// `urn:uuid:` and the request ID.
    #[serde(skip_serializing_if = "Option::is_none")]
    instance: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    request_id: Option<Uuid>,
}

fn problem(code: ProblemCode, request_id: Option<Uuid>) -> Response {
    // Each status of `ProblemCode` is valid; a test checks it.
    let status =
        StatusCode::from_u16(code.http_status()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    let body = Problem {
        type_url: code.type_url(),
        code: code.as_str(),
        title: code.meaning(),
        status: status.as_u16(),
        instance: request_id.map(|id| format!("urn:uuid:{id}")),
        request_id,
    };
    let mut response = (status, Json(body)).into_response();
    let headers = response.headers_mut();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/problem+json"),
    );
    if status == StatusCode::UNAUTHORIZED {
        // RFC 6750: the client must show a bearer token.
        headers.insert(header::WWW_AUTHENTICATE, HeaderValue::from_static("Bearer"));
    }
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    fn headers(pairs: &[(header::HeaderName, &str)]) -> HeaderMap {
        let mut headers = HeaderMap::new();
        for (name, value) in pairs {
            headers.append(name.clone(), HeaderValue::from_str(value).unwrap());
        }
        headers
    }

    /// Each code gets the status of the catalog, so a code that the authenticator adds later
    /// never becomes a silent 500 (ADR 0066).
    #[test]
    fn each_problem_code_gets_its_status_of_the_catalog() {
        for code in ProblemCode::ALL {
            let response = problem(code, None);
            assert_eq!(response.status().as_u16(), code.http_status(), "{code:?}");
        }
    }

    #[test]
    fn reads_only_one_bearer_token() {
        let token =
            |pairs: &[(header::HeaderName, &str)]| bearer_token(&headers(pairs)).map(str::to_owned);
        assert_eq!(
            token(&[(header::AUTHORIZATION, "Bearer tada_pat_x")]).as_deref(),
            Some("tada_pat_x")
        );
        assert_eq!(
            token(&[(header::AUTHORIZATION, "bearer tada_pat_x")]).as_deref(),
            Some("tada_pat_x")
        );
        assert_eq!(token(&[(header::AUTHORIZATION, "Basic dGFkYQ==")]), None);
        assert_eq!(token(&[(header::AUTHORIZATION, "Bearer ")]), None);
        assert_eq!(token(&[]), None);
        let two = [
            (header::AUTHORIZATION, "Bearer a"),
            (header::AUTHORIZATION, "Bearer b"),
        ];
        assert_eq!(token(&two), None);
    }

    #[test]
    fn allows_no_origin_or_the_origin_of_tada_only() {
        let origin = "https://tada.example.org";
        assert!(origin_allowed(&headers(&[]), origin));
        assert!(origin_allowed(
            &headers(&[(header::ORIGIN, origin)]),
            origin
        ));
        assert!(!origin_allowed(
            &headers(&[(header::ORIGIN, "https://evil.example")]),
            origin
        ));
        assert!(!origin_allowed(
            &headers(&[(header::ORIGIN, "null")]),
            origin
        ));
        let two = [(header::ORIGIN, origin), (header::ORIGIN, origin)];
        assert!(!origin_allowed(&headers(&two), origin));
    }
}
