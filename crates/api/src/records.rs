//! The parts that the API of each record kind shares: the evidence of a record version,
//! the page cursor and the deserializer of optional fields.

use serde::{Deserialize, Deserializer, Serialize};
use tada_app::dispute_notes::{DisputeNotes, NoteKey};
use tada_app::records::{NumberCursor, RecordEvidenceView};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::cursor;
use crate::dispute_notes::{DisputeNote, shown};
use crate::problem::ApiError;

/// A passage that supports one version of a record.
#[derive(Debug, Serialize, ToSchema)]
pub struct RecordEvidence {
    /// The record version that the accepted change produced.
    pub record_version: i64,
    pub proposal_id: Uuid,
    pub source_version_id: Uuid,
    /// The capture time of the source version.
    pub captured_at: jiff::Timestamp,
    pub start_offset: u32,
    pub end_offset: u32,
    /// Empty after a legal redaction of the source (ADR 0072).
    pub quote: String,
    /// True after a legal redaction of the source: the quote is empty (ADR 0072).
    pub redacted: bool,
    pub page: Option<u32>,
    /// The dispute notes about the source of the passage (ADR 0073), oldest first.
    /// It is absent if there is none.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[schema(required = false)]
    pub dispute_notes: Vec<DisputeNote>,
}

impl RecordEvidence {
    /// The passage with the notes of its source from `notes`.
    pub(crate) fn new(evidence: RecordEvidenceView, notes: &DisputeNotes) -> Self {
        Self {
            dispute_notes: shown(notes, &NoteKey::source_version(evidence.source_version_id)),
            record_version: evidence.record_version.get(),
            proposal_id: evidence.proposal_id.as_uuid(),
            source_version_id: evidence.source_version_id.as_uuid(),
            captured_at: evidence.captured_at,
            start_offset: evidence.start_offset,
            end_offset: evidence.end_offset,
            redacted: evidence.quote.as_present().is_none(),
            quote: evidence.quote.as_present().cloned().unwrap_or_default(),
            page: evidence.page,
        }
    }
}

/// Tells an absent field from a field that is `null`: the first keeps the value, the second clears it.
pub(crate) fn present<'de, D: Deserializer<'de>, T: Deserialize<'de>>(
    deserializer: D,
) -> Result<Option<Option<T>>, D::Error> {
    Option::<T>::deserialize(deserializer).map(Some)
}

pub(crate) fn encode_cursor(cursor: &NumberCursor) -> String {
    cursor::encode(cursor.0.to_string())
}

pub(crate) fn decode_cursor(text: &str) -> Result<NumberCursor, ApiError> {
    cursor::decode_text(text)?
        .parse()
        .map(NumberCursor)
        .map_err(|_| cursor::invalid())
}
