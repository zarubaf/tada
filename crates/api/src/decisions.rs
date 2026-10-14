//! `/api/v1/events/{event_id}/decisions` (ADR 0081): what the club decided, when and by whom.
//! Each reader of the event reads them; only an event manager records, changes, supersedes or revokes one.

use axum::extract::State;
use axum::http::StatusCode;
use jiff::civil;
use serde::{Deserialize, Serialize};
use tada_app::domain::ids::{DecisionId, EventId, UserId, WorkstreamId};
use tada_app::domain::records::DecisionStatus as DomainStatus;
use tada_app::problem::ProblemCode;
use tada_app::records::Shown;
use tada_app::work::{
    self as app, DecisionState, DecisionUpdate, DecisionView, NewDecision, Revocation, WorkError,
    WorkQuery,
};
use utoipa::{IntoParams, ToSchema};
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

use crate::ApiState;
use crate::contract::{AUTHENTICATED, JSON_BODY, PATH, QUERY, codes};
use crate::extract::{Caller, Json, Path, Query, page_limit, record_version};
use crate::parties::PartyRef;
use crate::problem::{ApiError, Problem};
use crate::records::{RecordEvidence, decode_cursor, encode_cursor, present};
use crate::work::{PromisorInput, owner_filter, party, ports};

pub(crate) fn routes() -> OpenApiRouter<ApiState> {
    OpenApiRouter::new()
        .routes(routes!(list_decisions, create_decision))
        .routes(routes!(get_decision, change_decision))
        .routes(routes!(supersede_decision))
        .routes(routes!(revoke_decision))
}

/// The problem codes of each operation (ADR 0037).
pub(crate) fn problem_codes() -> Vec<(&'static str, Vec<ProblemCode>)> {
    let write = || codes(&[AUTHENTICATED, PATH, JSON_BODY, WorkError::CODES]);
    vec![
        (
            "list_decisions",
            codes(&[AUTHENTICATED, PATH, QUERY, WorkError::READ_CODES]),
        ),
        ("create_decision", write()),
        (
            "get_decision",
            codes(&[AUTHENTICATED, PATH, WorkError::READ_CODES]),
        ),
        ("change_decision", write()),
        ("supersede_decision", write()),
        ("revoke_decision", write()),
    ]
}

/// The status of a decision. Only a supersede or a revoke ends a decision in force; both ends are final.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "kebab-case")]
pub enum DecisionStatus {
    InForce,
    Superseded,
    Revoked,
}

impl From<DecisionStatus> for DomainStatus {
    fn from(status: DecisionStatus) -> Self {
        match status {
            DecisionStatus::InForce => Self::InForce,
            DecisionStatus::Superseded => Self::Superseded,
            DecisionStatus::Revoked => Self::Revoked,
        }
    }
}

impl From<DomainStatus> for DecisionStatus {
    fn from(status: DomainStatus) -> Self {
        match status {
            DomainStatus::InForce => Self::InForce,
            DomainStatus::Superseded => Self::Superseded,
            DomainStatus::Revoked => Self::Revoked,
        }
    }
}

/// A decision of an event.
#[derive(Debug, Serialize, ToSchema)]
pub struct Decision {
    pub id: Uuid,
    /// The readable ID, for example `DEC-001`.
    pub local_id: String,
    pub event_id: Uuid,
    /// The words of the decision. It can have line breaks. Empty after a legal redaction (ADR 0072).
    pub wording: String,
    /// True after a legal redaction (ADR 0072): the texts are empty or absent.
    pub redacted: bool,
    /// The date of the decision.
    pub decided_on: civil::Date,
    /// The persons and institutions that decided, in their order.
    pub decided_by: Vec<PartyRef>,
    /// The member who follows the decision up: a contributor or manager of the event.
    pub owner_user_id: Uuid,
    pub workstream_id: Option<Uuid>,
    pub status: DecisionStatus,
    /// The decision that replaced this one. Only a `superseded` decision has it.
    pub superseded_by: Option<Uuid>,
    /// Why the decision was revoked. Only a `revoked` decision has it; a legal redaction removes it.
    pub revoked_reason: Option<String>,
    /// The record version. A change, a supersede and a revoke need it.
    pub version: i64,
    /// The evidence of the accepted proposals that created or ended the decision.
    pub evidence: Vec<RecordEvidence>,
    /// True if the caller can change the owner and the workstream now: an event manager.
    pub can_change: bool,
    /// True if the caller can supersede or revoke the decision now: an event manager, and a decision in force.
    pub can_end: bool,
}

impl From<Shown<DecisionView>> for Decision {
    fn from(shown: Shown<DecisionView>) -> Self {
        let can_end = shown.can_end();
        let Shown {
            record: decision,
            evidence,
            can_change,
            dispute_notes,
        } = shown;
        let local_id = decision.local_id();
        let redacted = decision.wording.as_present().is_none();
        let status = decision.state.status().into();
        let (superseded_by, revoked_reason) = match decision.state {
            DecisionState::InForce => (None, None),
            DecisionState::Superseded { by } => (Some(by.as_uuid()), None),
            DecisionState::Revoked { reason } => (
                None,
                reason.as_present().map(|reason| reason.as_str().to_owned()),
            ),
        };
        Self {
            id: decision.id.as_uuid(),
            local_id,
            event_id: decision.event_id.as_uuid(),
            wording: decision
                .wording
                .as_present()
                .map(|wording| wording.as_str().to_owned())
                .unwrap_or_default(),
            redacted,
            decided_on: decision.decided_on,
            decided_by: decision.parties.into_iter().map(PartyRef::from).collect(),
            owner_user_id: decision.fields.owner.as_uuid(),
            workstream_id: decision.fields.workstream_id.map(WorkstreamId::as_uuid),
            status,
            superseded_by,
            revoked_reason,
            version: decision.version.get(),
            evidence: evidence
                .into_iter()
                .map(|evidence| RecordEvidence::new(evidence, &dispute_notes))
                .collect(),
            // A redacted decision cannot change (ADR 0072).
            can_change: can_change && !redacted,
            can_end: can_end && !redacted,
        }
    }
}

/// One page of decisions.
#[derive(Debug, Serialize, ToSchema)]
pub struct DecisionPage {
    pub items: Vec<Decision>,
    /// The cursor of the next page. It is absent on the last page.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}

/// The parameters of a list of decisions.
#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ListDecisionsQuery {
    /// The page size: 1 to 200. The default is 50.
    #[param(minimum = 1, maximum = 200)]
    pub limit: Option<u32>,
    /// The `next_cursor` of the previous page.
    pub cursor: Option<String>,
    /// `me`, or the ID of a member: only the decisions that this member owns.
    pub owner: Option<String>,
    pub status: Option<DecisionStatus>,
    /// Only the decisions of this workstream.
    pub workstream: Option<Uuid>,
}

/// The input of `CreateDecision`, and the new decision of `SupersedeDecision`.
#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateDecisionRequest {
    /// The UUIDv7 of the new decision. Without it, the server chooses one. An ID that a record holds gives the field code `taken`, also on a retry.
    pub id: Option<Uuid>,
    /// The words of the decision, 1 to 2000 characters. It can have several lines: line breaks and tabs are allowed,
    /// other control characters are not.
    #[schema(
        example = "Das Fest findet bei jedem Wetter statt.\nBei Sturm entscheidet das OK am Vortag."
    )]
    pub wording: String,
    /// The date of the decision.
    pub decided_on: civil::Date,
    /// The persons and institutions of the organization that decided: 1 to 50. A duplicate counts once.
    /// None gives the field code `required`, more than 50 `too-long`, and an unknown one `unknown-record`.
    pub decided_by: Vec<PromisorInput>,
    /// A member with the contributor or manager role in the event.
    pub owner_user_id: Uuid,
    /// An active workstream of the event.
    pub workstream_id: Option<Uuid>,
}

impl CreateDecisionRequest {
    fn input(&self) -> NewDecision {
        NewDecision {
            id: self.id,
            wording: self.wording.clone(),
            decided_on: self.decided_on,
            decided_by: self.decided_by.iter().map(party).collect(),
            owner: UserId::from_uuid(self.owner_user_id),
            workstream: self.workstream_id.map(WorkstreamId::from_uuid),
        }
    }
}

/// The input of `ChangeDecision`: only the owner and the workstream change. An absent field stays as it is.
/// `null` clears the workstream. A request needs at least one field to change.
#[derive(Debug, Deserialize, ToSchema)]
pub struct ChangeDecisionRequest {
    pub owner_user_id: Option<Uuid>,
    /// An active workstream of the event. A record keeps a workstream that closes later.
    #[serde(default, deserialize_with = "present")]
    #[schema(value_type = Option<Uuid>)]
    pub workstream_id: Option<Option<Uuid>>,
    #[schema(minimum = 1)]
    pub expected_version: i64,
}

/// The input of `SupersedeDecision`: the new decision, and the version of the decision that it replaces.
#[derive(Debug, Deserialize, ToSchema)]
pub struct SupersedeDecisionRequest {
    pub successor: CreateDecisionRequest,
    /// The current version of the decision that ends.
    #[schema(minimum = 1)]
    pub expected_version: i64,
}

/// The input of `RevokeDecision`.
#[derive(Debug, Deserialize, ToSchema)]
pub struct RevokeDecisionRequest {
    /// Why the decision ends: 1 to 500 characters. The decision keeps it.
    #[schema(example = "Die Gemeinde verbietet das Feuerwerk.")]
    pub reason: String,
    #[schema(minimum = 1)]
    pub expected_version: i64,
}

/// Lists the decisions of an event in the order of their numbers. Each reader of the event can do it.
#[utoipa::path(
    get,
    path = "/events/{event_id}/decisions",
    operation_id = "list_decisions",
    tag = "work",
    params(("event_id" = Uuid, Path, description = "The ID of the event."), ListDecisionsQuery),
    responses(
        (status = OK, description = "One page of decisions.", body = DecisionPage),
        (status = "default", description = "A problem (ADR 0037).", body = Problem, content_type = "application/problem+json"),
    ),
)]
async fn list_decisions(
    State(state): State<ApiState>,
    Caller(caller): Caller,
    Path(event_id): Path<Uuid>,
    Query(query): Query<ListDecisionsQuery>,
) -> Result<axum::Json<DecisionPage>, ApiError> {
    let query = WorkQuery {
        owner: owner_filter(&caller, query.owner.as_deref())?,
        status: query.status.map(DomainStatus::from),
        workstream: query.workstream.map(WorkstreamId::from_uuid),
        after: query.cursor.as_deref().map(decode_cursor).transpose()?,
        limit: page_limit(query.limit)?,
    };
    let page =
        app::list_decisions(&caller, EventId::from_uuid(event_id), query, ports(&state)).await?;
    Ok(axum::Json(DecisionPage {
        items: page.items.into_iter().map(Decision::from).collect(),
        next_cursor: page.next.as_ref().map(encode_cursor),
    }))
}

/// Records a decision. Only an event manager can do it; another member gets `forbidden`.
/// An owner without the contributor or manager role gives `validation-failed` with the field code `unknown-member`.
/// A closed workstream gives the field code `closed`.
#[utoipa::path(
    post,
    path = "/events/{event_id}/decisions",
    operation_id = "create_decision",
    tag = "work",
    params(("event_id" = Uuid, Path, description = "The ID of the event.")),
    request_body = CreateDecisionRequest,
    responses(
        (status = CREATED, description = "The new decision, in force.", body = Decision),
        (status = "default", description = "A problem (ADR 0037).", body = Problem, content_type = "application/problem+json"),
    ),
)]
async fn create_decision(
    State(state): State<ApiState>,
    Caller(caller): Caller,
    Path(event_id): Path<Uuid>,
    Json(request): Json<CreateDecisionRequest>,
) -> Result<(StatusCode, axum::Json<Decision>), ApiError> {
    let decision = app::create_decision(
        &caller,
        EventId::from_uuid(event_id),
        request.input(),
        ports(&state),
    )
    .await?;
    Ok((StatusCode::CREATED, axum::Json(decision.into())))
}

/// Reads one decision of the event, with its evidence.
#[utoipa::path(
    get,
    path = "/events/{event_id}/decisions/{id}",
    operation_id = "get_decision",
    tag = "work",
    params(
        ("event_id" = Uuid, Path, description = "The ID of the event."),
        ("id" = Uuid, Path, description = "The ID of the decision."),
    ),
    responses(
        (status = OK, description = "The decision.", body = Decision),
        (status = "default", description = "A problem (ADR 0037).", body = Problem, content_type = "application/problem+json"),
    ),
)]
async fn get_decision(
    State(state): State<ApiState>,
    Caller(caller): Caller,
    Path((event_id, id)): Path<(Uuid, Uuid)>,
) -> Result<axum::Json<Decision>, ApiError> {
    let decision = app::get_decision(
        &caller,
        EventId::from_uuid(event_id),
        DecisionId::from_uuid(id),
        ports(&state),
    )
    .await?;
    Ok(axum::Json(decision.into()))
}

/// Changes the owner or the workstream of a decision. Only an event manager can do it.
/// The wording, the date and the parties never change: supersede the decision instead.
#[utoipa::path(
    patch,
    path = "/events/{event_id}/decisions/{id}",
    operation_id = "change_decision",
    tag = "work",
    params(
        ("event_id" = Uuid, Path, description = "The ID of the event."),
        ("id" = Uuid, Path, description = "The ID of the decision."),
    ),
    request_body = ChangeDecisionRequest,
    responses(
        (status = OK, description = "The changed decision.", body = Decision),
        (status = "default", description = "A problem (ADR 0037).", body = Problem, content_type = "application/problem+json"),
    ),
)]
async fn change_decision(
    State(state): State<ApiState>,
    Caller(caller): Caller,
    Path((event_id, id)): Path<(Uuid, Uuid)>,
    Json(request): Json<ChangeDecisionRequest>,
) -> Result<axum::Json<Decision>, ApiError> {
    let update = DecisionUpdate {
        owner: request.owner_user_id.map(UserId::from_uuid),
        workstream: request
            .workstream_id
            .map(|id| id.map(WorkstreamId::from_uuid)),
        expected_version: record_version(request.expected_version)?,
    };
    let decision = app::change_decision(
        &caller,
        EventId::from_uuid(event_id),
        DecisionId::from_uuid(id),
        update,
        ports(&state),
    )
    .await?;
    Ok(axum::Json(decision.into()))
}

/// Records a new decision that replaces a decision in force, in one step. Only an event manager can do it.
/// The old decision becomes `superseded` and names the new one. A decision that is not in force gives
/// `invalid-transition`. The new decision follows the rules of `create_decision`.
#[utoipa::path(
    post,
    path = "/events/{event_id}/decisions/{id}/supersede",
    operation_id = "supersede_decision",
    tag = "work",
    params(
        ("event_id" = Uuid, Path, description = "The ID of the event."),
        ("id" = Uuid, Path, description = "The ID of the decision that ends."),
    ),
    request_body = SupersedeDecisionRequest,
    responses(
        (status = CREATED, description = "The new decision, in force.", body = Decision),
        (status = "default", description = "A problem (ADR 0037).", body = Problem, content_type = "application/problem+json"),
    ),
)]
async fn supersede_decision(
    State(state): State<ApiState>,
    Caller(caller): Caller,
    Path((event_id, id)): Path<(Uuid, Uuid)>,
    Json(request): Json<SupersedeDecisionRequest>,
) -> Result<(StatusCode, axum::Json<Decision>), ApiError> {
    let decision = app::supersede_decision(
        &caller,
        EventId::from_uuid(event_id),
        DecisionId::from_uuid(id),
        request.successor.input(),
        record_version(request.expected_version)?,
        ports(&state),
    )
    .await?;
    Ok((StatusCode::CREATED, axum::Json(decision.into())))
}

/// Revokes a decision in force for a reason. Only an event manager can do it.
/// A decision that is not in force gives `invalid-transition`.
#[utoipa::path(
    post,
    path = "/events/{event_id}/decisions/{id}/revoke",
    operation_id = "revoke_decision",
    tag = "work",
    params(
        ("event_id" = Uuid, Path, description = "The ID of the event."),
        ("id" = Uuid, Path, description = "The ID of the decision."),
    ),
    request_body = RevokeDecisionRequest,
    responses(
        (status = OK, description = "The revoked decision.", body = Decision),
        (status = "default", description = "A problem (ADR 0037).", body = Problem, content_type = "application/problem+json"),
    ),
)]
async fn revoke_decision(
    State(state): State<ApiState>,
    Caller(caller): Caller,
    Path((event_id, id)): Path<(Uuid, Uuid)>,
    Json(request): Json<RevokeDecisionRequest>,
) -> Result<axum::Json<Decision>, ApiError> {
    let revocation = Revocation {
        reason: request.reason,
        expected_version: record_version(request.expected_version)?,
    };
    let decision = app::revoke_decision(
        &caller,
        EventId::from_uuid(event_id),
        DecisionId::from_uuid(id),
        revocation,
        ports(&state),
    )
    .await?;
    Ok(axum::Json(decision.into()))
}
