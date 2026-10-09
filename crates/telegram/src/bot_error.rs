//! The log form of a failed Bot API request (ADR 0035).
//!
//! The `Display` of `frankenstein::Error` is not safe for the log: a decode error holds the whole answer of
//! the Bot API, with message texts, names and Telegram user IDs. The log gets the kind of the error and
//! the status code only.

use std::fmt;

use frankenstein::Error;

/// The kind and the status code of a failed Bot API request, never its body.
pub(crate) struct BotFailure<'error>(pub(crate) &'error Error);

impl fmt::Display for BotFailure<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            Error::Api(response) => write!(
                f,
                "the Bot API refused the request: code {}",
                response.error_code
            ),
            Error::JsonDecode { .. } => f.write_str("the answer of the Bot API does not decode"),
            Error::JsonEncode { .. } => f.write_str("the request does not encode"),
            Error::HttpReqwest(error) => match error.status() {
                Some(status) => write!(f, "HTTP status {}", status.as_u16()),
                None if error.is_timeout() => f.write_str("the HTTP request timed out"),
                None if error.is_connect() => f.write_str("no connection to the Bot API"),
                None => f.write_str("the HTTP request failed"),
            },
            _ => f.write_str("the Bot API client failed"),
        }
    }
}
