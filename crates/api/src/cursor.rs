//! The envelope of an opaque list cursor (ADR 0044): URL-safe Base64 without padding.
//! Each list module keeps its own key format inside the envelope.

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use tada_app::problem::ProblemCode;

use crate::problem::ApiError;

/// Puts the bytes of a cursor key into the envelope.
pub(crate) fn encode(key: impl AsRef<[u8]>) -> String {
    URL_SAFE_NO_PAD.encode(key)
}

/// Takes the bytes of a cursor key from the envelope.
pub(crate) fn decode(text: &str) -> Result<Vec<u8>, ApiError> {
    URL_SAFE_NO_PAD.decode(text).map_err(|_| invalid())
}

/// Takes a UTF-8 cursor key from the envelope.
pub(crate) fn decode_text(text: &str) -> Result<String, ApiError> {
    String::from_utf8(decode(text)?).map_err(|_| invalid())
}

/// The problem for a cursor that the server did not make.
pub(crate) fn invalid() -> ApiError {
    ApiError::new(ProblemCode::MalformedRequest).with_detail("The cursor is not valid.")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_key_survives_its_round_trip() {
        assert_eq!(decode_text(&encode("7 abc")).unwrap(), "7 abc");
        assert_eq!(decode(&encode([0_u8, 255])).unwrap(), vec![0, 255]);
    }

    #[test]
    fn a_foreign_cursor_is_refused() {
        assert!(decode("not a cursor").is_err());
        assert!(decode_text(&encode([0xff_u8])).is_err());
    }
}
