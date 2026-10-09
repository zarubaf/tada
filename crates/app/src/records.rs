//! The conventions that the record kinds share: workstreams, actions, commitments, persons and institutions
//! (ADR 0067, ADR 0068, ADR 0069).
//!
//! - A create takes an optional ID that the client chooses: a UUIDv7 (`record_id`). A taken ID is the field code `taken`.
//! - A change needs at least one field. A change without a field is `validation-failed` without field errors.
//! - A change compares the expected version before it checks the values.
//! - The app clock gives the time of each write. The store keeps the time that it gets.
//! - A read takes a `Principal`, so that an AI client reads what its member reads.
//! - A list pages by the local number of the records (`NumberCursor`).

use tada_domain::ids;
use uuid::Uuid;

use crate::audit::{AuditAction, AuditEvent};
use crate::caller::MemberCaller;
use crate::paging::{Page, PageLimit};
use crate::problem::FieldError;

/// Collects the field errors of one input, so that the caller sees all of them at once.
#[derive(Debug, Default)]
pub(crate) struct Checker(Vec<FieldError>);

impl Checker {
    /// The parsed value, or `None` after it records the error of `field`.
    pub(crate) fn parse<T, E>(
        &mut self,
        field: &'static str,
        input: &str,
        parse: impl FnOnce(&str) -> Result<T, E>,
        code: impl FnOnce(E) -> &'static str,
    ) -> Option<T> {
        parse(input)
            .map_err(|error| self.push(field, code(error)))
            .ok()
    }

    pub(crate) fn push(&mut self, field: &'static str, code: &'static str) {
        self.0.push(FieldError::new(field, code));
    }

    pub(crate) fn add(&mut self, error: FieldError) {
        self.0.push(error);
    }

    /// The value if no field has an error. A value without errors is never `None`.
    pub(crate) fn finish<T>(self, value: Option<T>) -> Result<T, Vec<FieldError>> {
        match value {
            Some(value) if self.0.is_empty() => Ok(value),
            _ => Err(self.0),
        }
    }
}

/// The result of a create in a store.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Created<T> {
    Created(T),
    /// A record with this ID exists, in this organization or in another one.
    IdTaken,
}

/// The result of a change in a store.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Changed<T> {
    Changed(T),
    /// The scope has no such record.
    NotFound,
    /// The record has another version.
    VersionConflict,
}

/// The position after the last record of a page: its local number (ADR 0038, ADR 0044).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NumberCursor(pub u64);

/// The page of `items`, which hold one record more than `limit` if a next page exists.
pub(crate) fn page<T>(
    mut items: Vec<T>,
    limit: PageLimit,
    number: impl Fn(&T) -> u64,
) -> Page<T, NumberCursor> {
    let more = items.len() > limit.get() as usize;
    items.truncate(limit.get() as usize);
    let next = more
        .then(|| items.last())
        .flatten()
        .map(|last| NumberCursor(number(last)));
    Page { items, next }
}

/// The ID of a new record: the UUIDv7 of the client, or a new one (ADR 0038).
pub(crate) fn record_id(id: Option<Uuid>) -> Result<Uuid, FieldError> {
    match id {
        Some(id) if !ids::is_record_id(id) => Err(FieldError::new("id", "not-uuid-v7")),
        Some(id) => Ok(id),
        None => Ok(Uuid::now_v7()),
    }
}

/// The audit event of a direct command on the record `record`.
pub(crate) fn audit(caller: &MemberCaller, action: AuditAction, record: Uuid) -> AuditEvent {
    AuditEvent::new(caller.actor(), action, Some(record), Some(caller.scope()))
}
