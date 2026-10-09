//! Failed Bot API requests: their log form (ADR 0035) and the updates that the gateway skips.
//!
//! The `Display` of `frankenstein::Error` is not safe for the log: a decode error holds the whole answer of
//! the Bot API, with message texts, names and Telegram user IDs. The log gets the kind of the error and
//! the status code only.
//!
//! A batch of updates that does not decode does not decode on the next request either, so a retry stops
//! the gateway forever. The gateway then decodes each update of the answer on its own, handles the ones
//! that decode and skips the others: it asks for the updates after the highest update ID in the answer.
//! The senders of skipped updates get no reply, and the log shows only the number of skipped updates.

use std::fmt;

use frankenstein::Error;
use frankenstein::updates::Update;
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

/// The updates of a `getUpdates` answer that does not decode as a whole.
#[derive(Debug)]
pub(crate) struct Salvaged {
    /// The updates that decode on their own, in the order of the answer.
    pub(crate) updates: Vec<Update>,
    /// The number of updates that do not decode.
    pub(crate) skipped: usize,
    /// The offset after the highest update ID in the answer.
    pub(crate) next_offset: i64,
}

/// Decodes each update of a `getUpdates` answer that does not decode as a whole. Returns `None` if the
/// error is of another kind or the answer has no valid update ID.
pub(crate) fn salvage(error: &Error) -> Option<Salvaged> {
    let Error::JsonDecode { input, .. } = error else {
        return None;
    };
    let answer: Value = serde_json::from_str(input).ok()?;
    let results = answer.get("result")?.as_array()?;
    // Telegram update IDs are `u32`; a larger number is not an update ID.
    let highest = results
        .iter()
        .filter_map(|update| update.get("update_id")?.as_u64())
        .filter_map(|id| u32::try_from(id).ok())
        .max()?;
    let updates: Vec<Update> = results
        .iter()
        .filter_map(|update| serde_json::from_value(update.clone()).ok())
        .collect();
    Some(Salvaged {
        skipped: results.len() - updates.len(),
        updates,
        next_offset: i64::from(highest) + 1,
    })
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

    /// A private message with the update ID `id`.
    fn message(id: u64) -> String {
        format!(
            r#"{{"update_id":{id},"message":{{"message_id":1,"date":0,"text":"x","chat":{{"id":1,"type":"private"}}}}}}"#
        )
    }

    #[test]
    fn keeps_the_updates_that_decode_and_skips_past_the_highest_update_id() {
        let bad = r#"{"update_id":7,"message":{"date":"gestern"}}"#;
        let input = format!(
            r#"{{"ok":true,"result":[{bad},{},{bad_late}]}}"#,
            message(8),
            bad_late = bad.replace('7', "9")
        );
        let salvaged = salvage(&decode_error(&input)).unwrap();
        let ids: Vec<u32> = salvaged
            .updates
            .iter()
            .map(|update| update.update_id)
            .collect();
        assert_eq!(ids, [8]);
        assert_eq!(salvaged.skipped, 2);
        assert_eq!(salvaged.next_offset, 10);
    }

    #[test]
    fn an_update_id_above_u32_is_no_update_id() {
        let input = format!(
            r#"{{"ok":true,"result":[{{"update_id":{}}},{{"update_id":{}}}]}}"#,
            u64::from(u32::MAX) + 1,
            i64::MAX
        );
        assert!(salvage(&decode_error(&input)).is_none());
        let input = format!(r#"{{"ok":true,"result":[{{"update_id":{}}}]}}"#, u32::MAX);
        assert_eq!(
            salvage(&decode_error(&input)).unwrap().next_offset,
            i64::from(u32::MAX) + 1
        );
    }

    #[test]
    fn retries_an_answer_without_an_update_id() {
        assert!(salvage(&decode_error("<html>Bad Gateway</html>")).is_none());
        assert!(salvage(&decode_error(r#"{"ok":true,"result":[]}"#)).is_none());
    }

    #[test]
    fn the_log_form_holds_no_part_of_the_answer() {
        let error = decode_error(r#"{"result":[{"message":{"text":"Hangar 3"}}]}"#);
        assert!(!BotFailure(&error).to_string().contains("Hangar"));
    }
}
