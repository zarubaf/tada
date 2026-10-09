//! `/api/v1/events/{event_id}/profile` and `/fields`: the event profile and the field catalog (ADR 0049).

use axum::extract::State;
use jiff::Timestamp;
use serde::Serialize;
use tada_app::access::AccessError;
use tada_app::domain::facts::{FieldDefinition, FieldScope, FieldStatus as DomainFieldStatus};
use tada_app::domain::ids::EventId;
use tada_app::facts::{
    self as app, EventProfile as AppProfile, OpenProposalRef, OpenQuestionRef, ProfileEntry,
};
use tada_app::problem::ProblemCode;
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

use crate::ApiState;
use crate::actors::Author;
use crate::contract::{AUTHENTICATED, PATH, codes};
use crate::extract::{Caller, Path};
use crate::problem::{ApiError, Problem};
use crate::values::{FactState, Label, Passage, Value, ValueType, state_parts};

pub(crate) fn routes() -> OpenApiRouter<ApiState> {
    OpenApiRouter::new()
        .routes(routes!(get_event_profile))
        .routes(routes!(list_fields))
}

/// The problem codes of each operation (ADR 0037).
pub(crate) fn problem_codes() -> Vec<(&'static str, Vec<ProblemCode>)> {
    vec![
        (
            "get_event_profile",
            codes(&[AUTHENTICATED, PATH, AccessError::CODES]),
        ),
        (
            "list_fields",
            codes(&[AUTHENTICATED, PATH, AccessError::CODES]),
        ),
    ]
}

/// The event profile: the current facts of the event, its open fact proposals and its open questions.
#[derive(Debug, Serialize, ToSchema)]
pub struct EventProfile {
    /// The current version of each fact of the event, in the order of the field keys.
    /// A field without a fact is absent. An unknown fact is present with the state `unknown`.
    pub facts: Vec<Fact>,
    /// The fact proposals of the event without a review result, oldest first. They are not accepted state.
    pub proposals: Vec<FactProposal>,
    /// The open questions of the event, in the order of their numbers.
    pub open_questions: Vec<OpenQuestion>,
}

/// The current version of one fact.
#[derive(Debug, Serialize, ToSchema)]
pub struct Fact {
    pub id: Uuid,
    pub field_id: Uuid,
    /// The key of the field, for example `date_window`.
    pub field_key: String,
    pub state: FactState,
    /// The value. It is absent if the state is `unknown`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<Value>,
    /// True for an approximate value, for example "about 20,000". It is absent if the state is `unknown`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub approximate: Option<bool>,
    /// The number of the current fact version. A proposal that changes the fact expects it.
    pub version: i64,
    /// The passages of source versions that support the current fact version.
    pub evidence: Vec<FactEvidence>,
    /// The reviewer who accepted the current fact version.
    pub accepted_by: Author,
    /// The time of the acceptance of the current fact version.
    pub accepted_at: Timestamp,
}

/// One evidence link: a passage of a source version.
#[derive(Debug, Serialize, ToSchema)]
pub struct FactEvidence {
    pub source_version_id: Uuid,
    pub passage: Passage,
    /// The time when tada captured the source version.
    pub captured_at: Timestamp,
}

/// An open proposal that sets the fact of a field.
#[derive(Debug, Serialize, ToSchema)]
pub struct FactProposal {
    pub id: Uuid,
    pub changeset_id: Uuid,
    pub field_id: Uuid,
    pub state: FactState,
    /// The proposed value. It is absent if the proposed state is `unknown`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<Value>,
    /// True for an approximate value, for example "about 20,000". It is absent if the state is `unknown`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub approximate: Option<bool>,
    /// The fact version that the proposal expects. It is absent if the field had no fact.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_version: Option<i64>,
    pub created_at: Timestamp,
}

/// An open question of the event.
#[derive(Debug, Serialize, ToSchema)]
pub struct OpenQuestion {
    pub id: Uuid,
    /// The event-local ID, for example `QST-001` (ADR 0038).
    pub local_id: String,
    pub text: String,
    /// The user ID of the member who owns the question.
    pub owner_id: Uuid,
    pub version: i64,
}

impl From<AppProfile> for EventProfile {
    fn from(profile: AppProfile) -> Self {
        Self {
            facts: profile.fields.into_iter().map(Fact::from).collect(),
            proposals: profile
                .proposals
                .into_iter()
                .map(FactProposal::from)
                .collect(),
            open_questions: profile
                .open_questions
                .into_iter()
                .map(OpenQuestion::from)
                .collect(),
        }
    }
}

impl From<ProfileEntry> for Fact {
    fn from(entry: ProfileEntry) -> Self {
        let (state, value, approximate) = state_parts(&entry.state);
        Self {
            id: entry.fact_id.as_uuid(),
            field_id: entry.field.id.as_uuid(),
            field_key: entry.field.key.as_str().to_owned(),
            state,
            value,
            approximate,
            version: entry.version.get(),
            accepted_by: entry.accepted_by.into(),
            accepted_at: entry.accepted_at,
            evidence: entry
                .evidence
                .iter()
                .map(|dated| FactEvidence {
                    source_version_id: dated.evidence.source_version_id.as_uuid(),
                    passage: (&dated.evidence.passage).into(),
                    captured_at: dated.captured_at,
                })
                .collect(),
        }
    }
}

impl From<OpenProposalRef> for FactProposal {
    fn from(proposal: OpenProposalRef) -> Self {
        let (state, value, approximate) = state_parts(&proposal.state);
        Self {
            id: proposal.proposal_id.as_uuid(),
            changeset_id: proposal.changeset_id.as_uuid(),
            field_id: proposal.field_id.as_uuid(),
            state,
            value,
            approximate,
            expected_version: proposal.expected_version.map(|version| version.get()),
            created_at: proposal.created_at,
        }
    }
}

impl From<OpenQuestionRef> for OpenQuestion {
    fn from(question: OpenQuestionRef) -> Self {
        Self {
            id: question.id.as_uuid(),
            local_id: question.readable_id(),
            text: question.text.as_str().to_owned(),
            owner_id: question.owner.as_uuid(),
            version: question.version.get(),
        }
    }
}

/// The field catalog of an event. It has few fields, so the list has one page.
#[derive(Debug, Serialize, ToSchema)]
pub struct FieldPage {
    pub items: Vec<Field>,
}

/// A field definition of the field catalog.
#[derive(Debug, Serialize, ToSchema)]
pub struct Field {
    pub id: Uuid,
    /// The stable `snake_case` key, for example `visitor_estimate`.
    pub key: String,
    pub label: Label,
    pub value_type: ValueType,
    /// The JSON Schema of a value of this field: the `value` of a proposal that sets its fact.
    #[schema(value_type = Object)]
    pub value_schema: serde_json::Value,
    /// The meaning of the field, in English.
    pub description: String,
    /// The module of the field, for example `core`.
    pub module: String,
    pub status: FieldStatus,
    /// The event of a field that the event added. It is absent for a shipped field.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event_id: Option<Uuid>,
}

/// A deprecated field is readable, but takes no new facts.
#[derive(Debug, Clone, Copy, Serialize, ToSchema)]
#[serde(rename_all = "kebab-case")]
pub enum FieldStatus {
    Active,
    Deprecated,
}

impl From<FieldDefinition> for Field {
    fn from(field: FieldDefinition) -> Self {
        Self {
            id: field.id.as_uuid(),
            key: field.key.as_str().to_owned(),
            label: (&field.label).into(),
            value_type: (&field.value_type).into(),
            value_schema: app::value_schema(&field.value_type),
            description: field.description.as_str().to_owned(),
            module: field.module.as_str().to_owned(),
            status: match field.status {
                DomainFieldStatus::Active => FieldStatus::Active,
                DomainFieldStatus::Deprecated => FieldStatus::Deprecated,
            },
            event_id: match field.scope {
                FieldScope::Shipped => None,
                FieldScope::Event(event) => Some(event.as_uuid()),
            },
        }
    }
}

/// Reads the event profile. Each member who can read the event can read it.
#[utoipa::path(
    get,
    path = "/events/{event_id}/profile",
    operation_id = "get_event_profile",
    tag = "facts",
    params(("event_id" = Uuid, Path, description = "The ID of the event.")),
    responses(
        (status = OK, description = "The event profile.", body = EventProfile),
        (status = "default", description = "A problem (ADR 0037).", body = Problem, content_type = "application/problem+json"),
    ),
)]
async fn get_event_profile(
    State(state): State<ApiState>,
    Caller(caller): Caller,
    Path(event_id): Path<Uuid>,
) -> Result<axum::Json<EventProfile>, ApiError> {
    let profile = app::get_event_profile(
        &caller,
        EventId::from_uuid(event_id),
        state.identity.as_ref(),
        state.facts.as_ref(),
    )
    .await?;
    Ok(axum::Json(profile.into()))
}

/// Lists the field catalog of an event: the shipped fields and the fields of the event, in the order of their keys.
/// Each field has the JSON Schema of its values.
#[utoipa::path(
    get,
    path = "/events/{event_id}/fields",
    operation_id = "list_fields",
    tag = "facts",
    params(("event_id" = Uuid, Path, description = "The ID of the event.")),
    responses(
        (status = OK, description = "The field catalog of the event.", body = FieldPage),
        (status = "default", description = "A problem (ADR 0037).", body = Problem, content_type = "application/problem+json"),
    ),
)]
async fn list_fields(
    State(state): State<ApiState>,
    Caller(caller): Caller,
    Path(event_id): Path<Uuid>,
) -> Result<axum::Json<FieldPage>, ApiError> {
    let fields = app::get_field_catalog(
        &caller,
        EventId::from_uuid(event_id),
        state.identity.as_ref(),
        state.facts.as_ref(),
    )
    .await?;
    Ok(axum::Json(FieldPage {
        items: fields.into_iter().map(Field::from).collect(),
    }))
}
