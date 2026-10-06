//! Problem codes (ADR 0037). The `app` crate owns them, so that the API and the Telegram gateway use the same codes.

/// A stable problem code. A code never changes its meaning; a code that is no longer used stays reserved.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ProblemCode {
    MalformedRequest,
    Unauthenticated,
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
    pub const ALL: [Self; 12] = [
        Self::MalformedRequest,
        Self::Unauthenticated,
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

    /// The meaning of the code: a short, stable English text for developers.
    pub const fn meaning(self) -> &'static str {
        match self {
            Self::MalformedRequest => "The body is not valid JSON or does not match the schema.",
            Self::Unauthenticated => "No valid session or token.",
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

/// One invalid input value of a command. `field` is the name of the input field, for example `key`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldError {
    pub field: &'static str,
    pub code: &'static str,
}
