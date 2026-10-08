//! Problem details (RFC 9457) for all error responses (ADR 0037).

use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use jiff::SignedDuration;
use serde::Serialize;
use tada_app::problem::{CommandError, ProblemCode};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::request_id;

/// The base of the `type` URL: the public catalog of problem codes.
const CATALOG: &str = "https://github.com/zarubaf/tada/blob/main/doc/problems.md";

/// The HTTP status of each code. A new code without a status does not compile.
pub const fn status(code: ProblemCode) -> StatusCode {
    match code {
        ProblemCode::MalformedRequest => StatusCode::BAD_REQUEST,
        ProblemCode::Unauthenticated => StatusCode::UNAUTHORIZED,
        // The member is signed in but must choose an organization first.
        ProblemCode::Forbidden | ProblemCode::OrganizationRequired => StatusCode::FORBIDDEN,
        ProblemCode::NotFound => StatusCode::NOT_FOUND,
        ProblemCode::RecordVersionConflict | ProblemCode::InvalidTransition => StatusCode::CONFLICT,
        ProblemCode::PayloadTooLarge => StatusCode::PAYLOAD_TOO_LARGE,
        ProblemCode::UnsupportedMediaType => StatusCode::UNSUPPORTED_MEDIA_TYPE,
        ProblemCode::ValidationFailed => StatusCode::UNPROCESSABLE_ENTITY,
        ProblemCode::RateLimited => StatusCode::TOO_MANY_REQUESTS,
        ProblemCode::Unavailable => StatusCode::SERVICE_UNAVAILABLE,
        ProblemCode::Internal => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

/// An error response. `detail` never repeats input values.
#[derive(Debug, Serialize, ToSchema)]
pub struct Problem {
    /// The URL of the code in the public catalog.
    #[serde(rename = "type")]
    pub type_url: String,
    /// The stable problem code. Clients react to this field. The list of codes is open.
    pub code: String,
    /// A short, stable English text for developers.
    pub title: String,
    pub status: u16,
    /// An English text about this case, for developers. Clients never show it to members.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    /// `urn:uuid:` and the request ID.
    pub instance: String,
    pub request_id: Uuid,
    /// The invalid values of a `validation-failed` problem.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub errors: Vec<ProblemError>,
}

/// One invalid value.
#[derive(Debug, Serialize, ToSchema)]
pub struct ProblemError {
    /// A JSON pointer to the value in the request body, for example `/key`.
    pub pointer: String,
    /// The reason, for example `taken`. The list of codes is open.
    pub code: String,
}

/// An error of a handler. It becomes a problem response.
#[derive(Debug)]
pub struct ApiError {
    code: ProblemCode,
    detail: Option<&'static str>,
    errors: Vec<ProblemError>,
    /// The seconds of the `Retry-After` header.
    retry_after: Option<i64>,
}

impl ApiError {
    pub fn new(code: ProblemCode) -> Self {
        Self {
            code,
            detail: None,
            errors: Vec::new(),
            retry_after: None,
        }
    }

    pub fn with_detail(mut self, detail: &'static str) -> Self {
        self.detail = Some(detail);
        self
    }
}

/// Each error of a command or query becomes a problem with the code of the error (ADR 0037).
/// A store failure goes to the log with its cause; the response does not show the cause.
impl<E: CommandError> From<E> for ApiError {
    fn from(error: E) -> Self {
        if let Some(store_error) = error.store_error() {
            tracing::error!(error = %error_chain(store_error), "the store failed");
        }
        Self {
            code: error.code(),
            detail: None,
            errors: error
                .field_errors()
                .iter()
                .map(|error| ProblemError {
                    pointer: error.pointer(),
                    code: error.code.to_owned(),
                })
                .collect(),
            retry_after: error.retry_after().map(retry_after_seconds),
        }
    }
}

/// The wait in whole seconds for `Retry-After`, rounded up, and at least one second.
fn retry_after_seconds(wait: SignedDuration) -> i64 {
    let seconds = wait.as_secs() + i64::from(wait.subsec_nanos() > 0);
    seconds.max(1)
}

/// The error and all its sources in one line, for logs.
fn error_chain(error: &dyn std::error::Error) -> String {
    let mut text = error.to_string();
    let mut source = error.source();
    while let Some(cause) = source {
        text.push_str(": ");
        text.push_str(&cause.to_string());
        source = cause.source();
    }
    text
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = status(self.code);
        let request_id = request_id::current();
        let problem = Problem {
            type_url: format!("{CATALOG}#{}", self.code.as_str()),
            code: self.code.as_str().to_owned(),
            title: self.code.meaning().to_owned(),
            status: status.as_u16(),
            detail: self.detail.map(str::to_owned),
            instance: format!("urn:uuid:{request_id}"),
            request_id,
            errors: self.errors,
        };
        let mut response = (status, axum::Json(problem)).into_response();
        response.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/problem+json"),
        );
        if let Some(seconds) = self.retry_after {
            response
                .headers_mut()
                .insert(header::RETRY_AFTER, HeaderValue::from(seconds));
        }
        response
    }
}
