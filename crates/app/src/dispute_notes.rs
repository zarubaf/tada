//! Dispute notes (ADR 0073): the objection of a person to immutable evidence.
//!
//! Evidence cannot change, so a correction request about it becomes a note next to it (Art. 32(3)).
//! Only an owner adds a note. Each reader of the target sees the notes of the target.

use std::collections::HashMap;
use std::fmt::Debug;

use async_trait::async_trait;
use jiff::Timestamp;
use tada_domain::RecordVersion;
use tada_domain::ids::{DisputeNoteId, FactId, SourceVersionId, UserId};
use tada_domain::redaction::{DisputeNoteError, DisputeNoteText, DisputeTarget, DisputeTargetKind};
use uuid::Uuid;

use crate::access::{self, Principal};
use crate::audit::{AuditAction, AuditEvent};
use crate::caller::{MemberCaller, OrgScope};
use crate::clock::Clock;
use crate::problem::{CommandError, FieldError, ProblemCode};
use crate::store::StoreError;

/// One dispute note. Its text can name a person, so `Debug` of the text shows its length only.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DisputeNote {
    pub id: DisputeNoteId,
    pub target: DisputeTarget,
    pub text: DisputeNoteText,
    pub created_at: Timestamp,
}

/// The records whose notes a read shows. A read names the record in the form that it has.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NoteKey {
    /// The notes of the target.
    Target(DisputeTarget),
    /// The notes of the source item of the source version: evidence cites a source version.
    SourceVersion(SourceVersionId),
    /// The notes of the version `number` of the fact: the profile shows a fact with its version number.
    FactVersion { fact: FactId, number: RecordVersion },
}

/// The notes of the records of one read, oldest first.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DisputeNotes(HashMap<NoteKey, Vec<DisputeNote>>);

impl DisputeNotes {
    pub fn of(&self, key: &NoteKey) -> &[DisputeNote] {
        self.0.get(key).map_or(&[], Vec::as_slice)
    }
}

/// What a create of a note found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NoteCreated {
    Created(DisputeNote),
    /// The target does not exist in the organization.
    NotFound,
    /// A legal redaction removed the content of the target.
    Redacted,
}

/// The repository port of dispute notes. Each method stays inside `scope`.
#[async_trait]
pub trait DisputeNoteStore: Debug + Send + Sync {
    /// True if the target exists in the organization.
    async fn target_exists(
        &self,
        scope: OrgScope,
        target: DisputeTarget,
    ) -> Result<bool, StoreError>;

    /// Adds the note and records `audit` in one transaction. It locks the target, so a redaction that runs
    /// at the same time redacts the note too.
    async fn create(
        &self,
        scope: OrgScope,
        note: &DisputeNote,
        author: UserId,
        audit: &AuditEvent,
    ) -> Result<NoteCreated, StoreError>;

    /// The notes of the records `keys` that a redaction did not remove, oldest first.
    async fn notes(
        &self,
        scope: OrgScope,
        keys: &[NoteKey],
    ) -> Result<Vec<(NoteKey, DisputeNote)>, StoreError>;
}

#[derive(Debug, thiserror::Error)]
pub enum DisputeNoteCommandError {
    /// Only an owner adds a note.
    #[error("the caller cannot do this")]
    Forbidden,
    #[error("the target does not exist")]
    NotFound,
    /// A legal redaction removed the content of the target.
    #[error("the target is redacted")]
    InvalidTransition,
    #[error("invalid values")]
    Invalid(Vec<FieldError>),
    #[error(transparent)]
    Store(#[from] StoreError),
}

impl DisputeNoteCommandError {
    /// All codes of the command, for the API contract (ADR 0037).
    pub const CODES: &[ProblemCode] = &[
        ProblemCode::Forbidden,
        ProblemCode::NotFound,
        ProblemCode::InvalidTransition,
        ProblemCode::ValidationFailed,
        ProblemCode::Unavailable,
        ProblemCode::Internal,
    ];
}

impl CommandError for DisputeNoteCommandError {
    fn code(&self) -> ProblemCode {
        match self {
            Self::Forbidden => ProblemCode::Forbidden,
            Self::NotFound => ProblemCode::NotFound,
            Self::InvalidTransition => ProblemCode::InvalidTransition,
            Self::Invalid(_) => ProblemCode::ValidationFailed,
            Self::Store(error) => error.code(),
        }
    }

    fn store_error(&self) -> Option<&StoreError> {
        match self {
            Self::Store(error) => Some(error),
            _ => None,
        }
    }

    fn field_errors(&self) -> &[FieldError] {
        match self {
            Self::Invalid(errors) => errors,
            _ => &[],
        }
    }
}

/// The input of a new note. The text can name a person, so `Debug` leaves it out.
#[derive(Clone)]
pub struct NewDisputeNote {
    pub target_kind: String,
    pub target_id: Uuid,
    pub text: String,
}

impl Debug for NewDisputeNote {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NewDisputeNote")
            .field("target_kind", &self.target_kind)
            .field("target_id", &self.target_id)
            .finish_non_exhaustive()
    }
}

/// Adds a dispute note to a source item, a fact version, a document version or a proposal.
/// Only an owner can do this; an AI client cannot, because the command takes a member only.
/// A caller who is not an owner gets `NotFound` for a target outside its organization and `Forbidden` otherwise.
/// A note is also possible in a closed event: it records an objection and changes no evidence.
pub async fn create_dispute_note(
    caller: &MemberCaller,
    input: NewDisputeNote,
    store: &dyn DisputeNoteStore,
    clock: &dyn Clock,
) -> Result<DisputeNote, DisputeNoteCommandError> {
    let kind: DisputeTargetKind = input.target_kind.parse().map_err(|_| {
        DisputeNoteCommandError::Invalid(vec![FieldError::new("target_kind", "unknown")])
    })?;
    let target = DisputeTarget {
        kind,
        id: input.target_id,
    };
    let scope = caller.scope();
    if !access::is_owner(caller) {
        return Err(if store.target_exists(scope, target).await? {
            DisputeNoteCommandError::Forbidden
        } else {
            DisputeNoteCommandError::NotFound
        });
    }
    let text = DisputeNoteText::parse(&input.text).map_err(|error| {
        let code = match error {
            DisputeNoteError::Empty => "empty",
            DisputeNoteError::TooLong => "too-long",
        };
        DisputeNoteCommandError::Invalid(vec![FieldError::new("text", code)])
    })?;
    let note = DisputeNote {
        id: DisputeNoteId::from_uuid(Uuid::now_v7()),
        target,
        text,
        created_at: clock.now(),
    };
    // The audit event names the note; its text stays out of the log (ADR 0045).
    let audit = AuditEvent::new(
        caller.actor(),
        AuditAction::DisputeNoteCreate,
        Some(note.id.as_uuid()),
        Some(scope),
    );
    match store.create(scope, &note, caller.user_id(), &audit).await? {
        NoteCreated::Created(note) => Ok(note),
        NoteCreated::NotFound => Err(DisputeNoteCommandError::NotFound),
        NoteCreated::Redacted => Err(DisputeNoteCommandError::InvalidTransition),
    }
}

/// The notes of the records `keys`, for a read that shows them.
/// The caller read the records with the access checks of that read; the notes add no record to it.
pub async fn dispute_notes(
    caller: &impl Principal,
    keys: Vec<NoteKey>,
    store: &dyn DisputeNoteStore,
) -> Result<DisputeNotes, StoreError> {
    if keys.is_empty() {
        return Ok(DisputeNotes::default());
    }
    let mut notes = DisputeNotes::default();
    for (key, note) in store.notes(caller.scope(), &keys).await? {
        notes.0.entry(key).or_default().push(note);
    }
    Ok(notes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_error_gives_a_code_of_its_list() {
        for error in [
            DisputeNoteCommandError::Forbidden,
            DisputeNoteCommandError::NotFound,
            DisputeNoteCommandError::InvalidTransition,
            DisputeNoteCommandError::Invalid(Vec::new()),
            DisputeNoteCommandError::Store(StoreError::Unavailable("test".into())),
            DisputeNoteCommandError::Store(StoreError::Internal("test".into())),
        ] {
            assert!(
                DisputeNoteCommandError::CODES.contains(&error.code()),
                "{error:?}"
            );
        }
    }

    #[test]
    fn debug_hides_the_text() {
        let input = NewDisputeNote {
            target_kind: "proposal".into(),
            target_id: Uuid::nil(),
            text: "Beat Muster widerspricht".into(),
        };
        assert!(!format!("{input:?}").contains("Muster"));
    }
}
