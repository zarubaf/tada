//! Problem details (RFC 9457) for all error responses (ADR 0037).

use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use tada_app::problem::{FieldError, ProblemCode};
use tada_app::store::StoreError;
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
        ProblemCode::Forbidden => StatusCode::FORBIDDEN,
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
}

impl ApiError {
    pub fn new(code: ProblemCode) -> Self {
        Self {
            code,
            detail: None,
            errors: Vec::new(),
        }
    }

    pub fn with_detail(mut self, detail: &'static str) -> Self {
        self.detail = Some(detail);
        self
    }

    /// A `validation-failed` problem. Each field of the command input is a member of the request body.
    pub fn invalid(errors: Vec<FieldError>) -> Self {
        Self {
            code: ProblemCode::ValidationFailed,
            detail: None,
            errors: errors
                .into_iter()
                .map(|error| ProblemError {
                    pointer: format!("/{}", error.field),
                    code: error.code.to_owned(),
                })
                .collect(),
        }
    }

    /// A store failure. The log gets the cause; the response does not.
    pub fn store(error: &StoreError) -> Self {
        let code = match error {
            StoreError::Unavailable(_) => ProblemCode::Unavailable,
            StoreError::Internal(_) => ProblemCode::Internal,
        };
        tracing::error!(error = %error_chain(error), "the store failed");
        Self::new(code)
    }
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
        response
    }
}
