//! Problem codes (ADR 0037). The `app` crate owns them, so that the API and the Telegram gateway use the same codes.

use std::borrow::Cow;

use jiff::SignedDuration;

use crate::store::StoreError;

/// The base of the `type` URL of a problem: the public catalog of problem codes (ADR 0037).
const CATALOG: &str = "https://github.com/zarubaf/tada/blob/main/doc/problems.md";

/// A stable problem code. A code never changes its meaning; a code that is no longer used stays reserved.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ProblemCode {
    MalformedRequest,
    Unauthenticated,
    OrganizationRequired,
    Forbidden,
    NotFound,
    RecordVersionConflict,
    InvalidTransition,
    PayloadTooLarge,
    UnsupportedMediaType,
    ValidationFailed,
    RateLimited,
    Unavailable,
    Internal,
}

impl ProblemCode {
    pub const ALL: [Self; 13] = [
        Self::MalformedRequest,
        Self::Unauthenticated,
        Self::OrganizationRequired,
        Self::Forbidden,
        Self::NotFound,
        Self::RecordVersionConflict,
        Self::InvalidTransition,
        Self::PayloadTooLarge,
        Self::UnsupportedMediaType,
        Self::ValidationFailed,
        Self::RateLimited,
        Self::Unavailable,
        Self::Internal,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::MalformedRequest => "malformed-request",
            Self::Unauthenticated => "unauthenticated",
            Self::OrganizationRequired => "organization-required",
            Self::Forbidden => "forbidden",
            Self::NotFound => "not-found",
            Self::RecordVersionConflict => "record-version-conflict",
            Self::InvalidTransition => "invalid-transition",
            Self::PayloadTooLarge => "payload-too-large",
            Self::UnsupportedMediaType => "unsupported-media-type",
            Self::ValidationFailed => "validation-failed",
            Self::RateLimited => "rate-limited",
            Self::Unavailable => "unavailable",
            Self::Internal => "internal",
        }
    }

    /// The URL of the code in the public catalog: the `type` of a problem (ADR 0037).
    pub fn type_url(self) -> String {
        format!("{CATALOG}#{}", self.as_str())
    }

    /// The HTTP status of the code (ADR 0037, ADR 0066). Each adapter that answers over HTTP uses it.
    /// A new code without a status does not compile.
    pub const fn http_status(self) -> u16 {
        match self {
            Self::MalformedRequest => 400,
            Self::Unauthenticated => 401,
            // The member is signed in but must choose an organization first.
            Self::Forbidden | Self::OrganizationRequired => 403,
            Self::NotFound => 404,
            Self::RecordVersionConflict | Self::InvalidTransition => 409,
            Self::PayloadTooLarge => 413,
            Self::UnsupportedMediaType => 415,
            Self::ValidationFailed => 422,
            Self::RateLimited => 429,
            Self::Unavailable => 503,
            Self::Internal => 500,
        }
    }

    /// The meaning of the code: a short, stable English text for developers.
    pub const fn meaning(self) -> &'static str {
        match self {
            Self::MalformedRequest => "The body is not valid JSON or does not match the schema.",
            Self::Unauthenticated => "No valid session or token.",
            Self::OrganizationRequired => {
                "The session has no organization. The client lets the member choose one."
            }
            Self::Forbidden => {
                "The caller can see the record but lacks the permission for this action."
            }
            Self::NotFound => {
                "The record does not exist, or it is in a scope the caller cannot see."
            }
            Self::RecordVersionConflict => "The record changed after the caller read it.",
            Self::InvalidTransition => "The current state does not allow this change.",
            Self::PayloadTooLarge => "The body is larger than the limit.",
            Self::UnsupportedMediaType => "The media type is not supported.",
            Self::ValidationFailed => "Values break a domain rule. The `errors` list names them.",
            Self::RateLimited => "Too many requests. The response has `Retry-After`.",
            Self::Unavailable => "A dependency is unavailable. The client can retry.",
            Self::Internal => "An unexpected error. The response contains no other detail.",
        }
    }
}

/// One invalid input value of a command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldError {
    /// The path of the input field, without a leading `/`, for example `key` or `proposals/0/evidence`.
    /// Each adapter shows it as the JSON pointer of `pointer` (ADR 0037).
    pub field: Cow<'static, str>,
    pub code: &'static str,
}

impl FieldError {
    pub fn new(field: impl Into<Cow<'static, str>>, code: &'static str) -> Self {
        Self {
            field: field.into(),
            code,
        }
    }

    /// The JSON pointer of the field in the request, for example `/proposals/0/evidence` (ADR 0037).
    pub fn pointer(&self) -> String {
        format!("/{}", self.field)
    }
}

/// An error of a command or query (ADR 0037).
/// The API builds its problem response from these methods only.
pub trait CommandError {
    /// The problem code. It is one of the `CODES` of the error enum.
    fn code(&self) -> ProblemCode;

    /// The store failure, if this is one. The API logs it.
    fn store_error(&self) -> Option<&StoreError> {
        None
    }

    /// The invalid fields of a `validation-failed` error.
    fn field_errors(&self) -> &[FieldError] {
        &[]
    }

    /// The wait before the next try of a `rate-limited` error.
    fn retry_after(&self) -> Option<SignedDuration> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_field_error_names_its_field_by_a_json_pointer() {
        assert_eq!(FieldError::new("key", "empty").pointer(), "/key");
        assert_eq!(
            FieldError::new("proposals/0/evidence", "evidence-missing").pointer(),
            "/proposals/0/evidence"
        );
    }
}
