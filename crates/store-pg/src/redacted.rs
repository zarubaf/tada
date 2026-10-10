//! The tombstones of a legal redaction (ADR 0072) in the rows that the adapters read.

use tada_app::domain::redaction::Redactable;

/// The value of a content column that the schema keeps NOT NULL except after a legal redaction.
pub(crate) fn content<T>(value: Option<T>) -> Redactable<T> {
    value.map_or(Redactable::Redacted, Redactable::Present)
}
