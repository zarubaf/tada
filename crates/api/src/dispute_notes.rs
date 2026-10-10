//! `/api/v1/dispute-notes`: the objection of a person to immutable evidence (ADR 0073), and the notes that each
//! read of evidence or facts shows.

use axum::extract::State;
use axum::http::StatusCode;
use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use tada_app::access::Principal;
use tada_app::dispute_notes::{
    self as app, DisputeNote as AppDisputeNote, DisputeNoteCommandError, DisputeNotes,
    NewDisputeNote, NoteKey,
};
use tada_app::problem::ProblemCode;
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

use crate::ApiState;
use crate::contract::{AUTHENTICATED, JSON_BODY, codes};
use crate::extract::{Caller, Json};
use crate::problem::{ApiError, Problem};

pub(crate) fn routes() -> OpenApiRouter<ApiState> {
    OpenApiRouter::new().routes(routes!(create_dispute_note))
}

/// The problem codes of each operation (ADR 0037).
pub(crate) fn problem_codes() -> Vec<(&'static str, Vec<ProblemCode>)> {
    vec![(
        "create_dispute_note",
        codes(&[AUTHENTICATED, JSON_BODY, DisputeNoteCommandError::CODES]),
    )]
}

/// A dispute note as a reader of its target sees it (ADR 0073).
#[derive(Debug, Serialize, ToSchema)]
pub struct DisputeNote {
    /// The objection, 1 to 1000 characters.
    pub text: String,
    pub created_at: Timestamp,
}

impl From<&AppDisputeNote> for DisputeNote {
    fn from(note: &AppDisputeNote) -> Self {
        Self {
            text: note.text.as_str().to_owned(),
            created_at: note.created_at,
        }
    }
}

/// A response that shows evidence or facts, and so the dispute notes about them (ADR 0073).
/// Its conversion from the app view leaves the notes empty; `with_notes` fills them.
pub(crate) trait ShowsDisputeNotes {
    /// Adds the records whose notes the response shows.
    fn note_keys(&self, keys: &mut Vec<NoteKey>);
    /// Sets the notes of each record.
    fn set_notes(&mut self, notes: &DisputeNotes);
}

impl<T: ShowsDisputeNotes> ShowsDisputeNotes for Vec<T> {
    fn note_keys(&self, keys: &mut Vec<NoteKey>) {
        self.iter().for_each(|item| item.note_keys(keys));
    }

    fn set_notes(&mut self, notes: &DisputeNotes) {
        self.iter_mut().for_each(|item| item.set_notes(notes));
    }
}

/// The notes of `key` as a response shows them.
pub(crate) fn shown(notes: &DisputeNotes, key: &NoteKey) -> Vec<DisputeNote> {
    notes.of(key).iter().map(DisputeNote::from).collect()
}

/// The response with the dispute notes of its records. The caller read the records with their access checks.
pub(crate) async fn with_notes<T: ShowsDisputeNotes>(
    state: &ApiState,
    caller: &impl Principal,
    mut response: T,
) -> Result<T, ApiError> {
    let mut keys = Vec::new();
    response.note_keys(&mut keys);
    let notes = app::dispute_notes(caller, keys, state.dispute_notes.as_ref()).await?;
    response.set_notes(&notes);
    Ok(response)
}

/// The input of `CreateDisputeNote`.
#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateDisputeNoteRequest {
    /// `source-item`, `fact-version`, `document-version` or `proposal`.
    #[schema(example = "source-item")]
    pub target_kind: String,
    pub target_id: Uuid,
    /// The objection, 1 to 1000 characters.
    #[schema(example = "Die Person bestreitet diese Aussage.")]
    pub text: String,
}

/// A new dispute note.
#[derive(Debug, Serialize, ToSchema)]
pub struct CreatedDisputeNote {
    pub id: Uuid,
    pub target_kind: String,
    pub target_id: Uuid,
    pub text: String,
    pub created_at: Timestamp,
}

/// Adds a dispute note to immutable evidence: a source item, a fact version, a document version or a proposal.
/// Each reader of the target sees the note. Only an owner can do this, and the audit log records
/// `dispute_note.create`. A redacted target gets `invalid-transition`.
#[utoipa::path(
    post,
    path = "/dispute-notes",
    operation_id = "create_dispute_note",
    tag = "parties",
    request_body = CreateDisputeNoteRequest,
    responses(
        (status = CREATED, description = "The new note.", body = CreatedDisputeNote),
        (status = "default", description = "A problem (ADR 0037).", body = Problem, content_type = "application/problem+json"),
    ),
)]
async fn create_dispute_note(
    State(state): State<ApiState>,
    Caller(caller): Caller,
    Json(request): Json<CreateDisputeNoteRequest>,
) -> Result<(StatusCode, axum::Json<CreatedDisputeNote>), ApiError> {
    let note = app::create_dispute_note(
        &caller,
        NewDisputeNote {
            target_kind: request.target_kind,
            target_id: request.target_id,
            text: request.text,
        },
        state.dispute_notes.as_ref(),
        state.clock.as_ref(),
    )
    .await?;
    Ok((
        StatusCode::CREATED,
        axum::Json(CreatedDisputeNote {
            id: note.id.as_uuid(),
            target_kind: note.target.kind.as_str().to_owned(),
            target_id: note.target.id,
            text: note.text.as_str().to_owned(),
            created_at: note.created_at,
        }),
    ))
}
