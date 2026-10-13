//! `/api/v1/persons/{id}/mentions` and `/api/v1/persons/{id}/export`: the data about one person (ADR 0073).

use axum::extract::State;
use axum::http::{HeaderValue, header};
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use tada_app::domain::ids::{EventId, PersonId};
use tada_app::person_data::{self as app, Mention as AppMention, PersonDataError};
use tada_app::problem::ProblemCode;
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

use crate::ApiState;
use crate::contract::{AUTHENTICATED, PATH, RECENT_SIGN_IN, codes};
use crate::extract::{Caller, Path};
use crate::problem::{ApiError, Problem};

pub(crate) fn routes() -> OpenApiRouter<ApiState> {
    OpenApiRouter::new()
        .routes(routes!(list_person_mentions))
        .routes(routes!(export_person))
}

/// The problem codes of each operation (ADR 0037).
pub(crate) fn problem_codes() -> Vec<(&'static str, Vec<ProblemCode>)> {
    vec![
        (
            "list_person_mentions",
            codes(&[AUTHENTICATED, PATH, PersonDataError::CODES]),
        ),
        (
            "export_person",
            codes(&[AUTHENTICATED, PATH, PersonDataError::CODES, RECENT_SIGN_IN]),
        ),
    ]
}

/// Where a text mentions a person, or a record refers to it by ID (ADR 0073).
#[derive(Debug, Serialize, ToSchema)]
pub struct PersonMention {
    /// `source-item`, `fact-version`, `proposal`, `open-question`, `action`, `commitment`, `workstream`,
    /// `document-version`, `milestone`, `decision`, `promisor` for a commitment whose promisor is the person, or
    /// `decision-party` for a decision that the person took.
    /// The list of kinds is open.
    pub kind: String,
    /// The ID of the record of the kind. A redaction or a dispute note of a `source-item`, `fact-version`,
    /// `document-version` or `proposal` takes it as the target.
    pub id: Uuid,
    /// The event of the record. It is absent for a source of the organization.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event_id: Option<Uuid>,
    /// At most 200 characters of the text around the first match. It is absent after a legal redaction.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub excerpt: Option<String>,
    /// True after a legal redaction (ADR 0072).
    pub redacted: bool,
    /// True if a dispute note is about the record.
    pub disputed: bool,
}

impl From<AppMention> for PersonMention {
    fn from(mention: AppMention) -> Self {
        Self {
            kind: mention.kind.as_str().to_owned(),
            id: mention.id,
            event_id: mention.event_id.map(EventId::as_uuid),
            redacted: mention.excerpt.as_present().is_none(),
            excerpt: mention.excerpt.as_present().cloned(),
            disputed: mention.disputed,
        }
    }
}

/// The mentions of a person.
#[derive(Debug, Serialize, ToSchema)]
pub struct PersonMentionList {
    /// The text matches in the order of their kinds and times, then the references by ID. Each record comes once.
    pub items: Vec<PersonMention>,
}

/// Lists where tada mentions a person: each text that contains its name, email address or phone number, whatever
/// the case, and each record that refers to it by ID. A term with fewer than 3 characters is not searched.
/// Only an owner can do this.
#[utoipa::path(
    get,
    path = "/persons/{person_id}/mentions",
    operation_id = "list_person_mentions",
    tag = "parties",
    params(("person_id" = Uuid, Path, description = "The ID of the person.")),
    responses(
        (status = OK, description = "The mentions of the person.", body = PersonMentionList),
        (status = "default", description = "A problem (ADR 0037).", body = Problem, content_type = "application/problem+json"),
    ),
)]
async fn list_person_mentions(
    State(state): State<ApiState>,
    Caller(caller): Caller,
    Path(person_id): Path<Uuid>,
) -> Result<axum::Json<PersonMentionList>, ApiError> {
    let mentions = app::list_mentions(
        &caller,
        PersonId::from_uuid(person_id),
        state.parties.as_ref(),
        state.person_data.as_ref(),
    )
    .await?;
    Ok(axum::Json(PersonMentionList {
        items: mentions.into_iter().map(PersonMention::from).collect(),
    }))
}

/// Exports all data about a person as a JSON file with `format_version` 1 (ADR 0073): the organization, the
/// person, the references by ID, the mentions with their full text, the dispute notes and, for a person with an
/// account, the account data. The file holds no secret and no token hash. Only an owner can do this, and the audit
/// log records `person.export`.
#[utoipa::path(
    get,
    path = "/persons/{person_id}/export",
    operation_id = "export_person",
    tag = "parties",
    params(("person_id" = Uuid, Path, description = "The ID of the person.")),
    responses(
        (status = OK, description = "The file `person-PER-001.json` as an attachment.", content_type = "application/json", body = Object),
        (status = "default", description = "A problem (ADR 0037).", body = Problem, content_type = "application/problem+json"),
    ),
)]
async fn export_person(
    State(state): State<ApiState>,
    Caller(caller): Caller,
    Path(person_id): Path<Uuid>,
) -> Result<Response, ApiError> {
    let export = app::export_person(
        &caller,
        PersonId::from_uuid(person_id),
        state.parties.as_ref(),
        state.person_data.as_ref(),
        state.clock.as_ref(),
    )
    .await?;
    let body =
        serde_json::to_vec_pretty(&export).map_err(|_| ApiError::new(ProblemCode::Internal))?;
    // The readable ID has letters, digits and `-` only, so the name needs no quoting rules.
    let disposition =
        HeaderValue::from_str(&format!("attachment; filename=\"{}\"", export.file_name()))
            .map_err(|_| ApiError::new(ProblemCode::Internal))?;
    let headers = [
        (
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/json"),
        ),
        (header::CONTENT_DISPOSITION, disposition),
        (
            header::CACHE_CONTROL,
            HeaderValue::from_static("private, no-store"),
        ),
    ];
    Ok((headers, body).into_response())
}
