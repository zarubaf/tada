//! Failed Bot API requests: their log form (ADR 0035) and the updates that the gateway skips.
//!
//! The `Display` of `frankenstein::Error` is not safe for the log: a decode error holds the whole answer of
//! the Bot API, with message texts, names and Telegram user IDs. The log gets the kind of the error and
//! the status code only.
//!
//! A batch of updates that does not decode does not decode on the next request either, so a retry stops
//! the gateway forever. The gateway skips such a batch: it asks for the updates after the highest update
//! ID in the answer. The senders get no reply, and the log shows the number of skipped updates.

use std::fmt;

use frankenstein::Error;
use serde_json::Value;

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

/// The offset after the highest update ID of a `getUpdates` answer that does not decode, or `None` if the
/// error is of another kind or the answer has no update ID. Also returns the number of updates in it.
pub(crate) fn offset_past(error: &Error) -> Option<(i64, usize)> {
    let Error::JsonDecode { input, .. } = error else {
        return None;
    };
    let answer: Value = serde_json::from_str(input).ok()?;
    let updates = answer.get("result")?.as_array()?;
    let highest = updates
        .iter()
        .filter_map(|update| update.get("update_id")?.as_i64())
        .max()?;
    Some((highest + 1, updates.len()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn decode_error(input: &str) -> Error {
        Error::JsonDecode {
            source: serde_json::from_str::<u8>("x").unwrap_err(),
            input: input.to_owned(),
        }
    }

    #[test]
    fn skips_past_the_highest_update_id() {
        let error = decode_error(r#"{"ok":true,"result":[{"update_id":7},{"update_id":9}]}"#);
        assert_eq!(offset_past(&error), Some((10, 2)));
    }

    #[test]
    fn retries_an_answer_without_an_update_id() {
        assert_eq!(offset_past(&decode_error("<html>Bad Gateway</html>")), None);
        assert_eq!(
            offset_past(&decode_error(r#"{"ok":true,"result":[]}"#)),
            None
        );
    }

    #[test]
    fn the_log_form_holds_no_part_of_the_answer() {
        let error = decode_error(r#"{"result":[{"message":{"text":"Hangar 3"}}]}"#);
        assert!(!BotFailure(&error).to_string().contains("Hangar"));
    }
}
