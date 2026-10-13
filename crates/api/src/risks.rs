//! `/api/v1/events/{event_id}/risks` (ADR 0080): what can go wrong in an event, rated by likelihood and impact.

use axum::extract::State;
use axum::http::StatusCode;
use serde::{Deserialize, Serialize};
use tada_app::domain::ids::{EventId, RiskId, UserId, WorkstreamId};
use tada_app::domain::records::{Level as DomainLevel, RiskRating, RiskStatus as DomainStatus};
use tada_app::problem::ProblemCode;
use tada_app::records::Shown;
use tada_app::work::{
    self as app, NewRisk, RiskChange, RiskCursor, RiskView, WorkError, WorkQuery,
};
use utoipa::{IntoParams, ToSchema};
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

use crate::ApiState;
use crate::contract::{AUTHENTICATED, JSON_BODY, PATH, QUERY, codes};
use crate::cursor;
use crate::extract::{Caller, Json, Path, Query, page_limit, record_version};
use crate::problem::{ApiError, Problem};
use crate::records::{RecordEvidence, present};
use crate::work::{owner_filter, ports};

pub(crate) fn routes() -> OpenApiRouter<ApiState> {
    OpenApiRouter::new()
        .routes(routes!(list_risks, create_risk))
        .routes(routes!(get_risk, change_risk))
}

/// The problem codes of each operation (ADR 0037).
pub(crate) fn problem_codes() -> Vec<(&'static str, Vec<ProblemCode>)> {
    let write = || codes(&[AUTHENTICATED, PATH, JSON_BODY, WorkError::CODES]);
    vec![
        (
            "list_risks",
            codes(&[AUTHENTICATED, PATH, QUERY, WorkError::READ_CODES]),
        ),
        ("create_risk", write()),
        (
            "get_risk",
            codes(&[AUTHENTICATED, PATH, WorkError::READ_CODES]),
        ),
        ("change_risk", write()),
    ]
}

/// The status of a risk.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "kebab-case")]
pub enum RiskStatus {
    Open,
    Mitigated,
    Occurred,
    Closed,
}

impl From<RiskStatus> for DomainStatus {
    fn from(status: RiskStatus) -> Self {
        match status {
            RiskStatus::Open => Self::Open,
            RiskStatus::Mitigated => Self::Mitigated,
            RiskStatus::Occurred => Self::Occurred,
            RiskStatus::Closed => Self::Closed,
        }
    }
}

impl From<DomainStatus> for RiskStatus {
    fn from(status: DomainStatus) -> Self {
        match status {
            DomainStatus::Open => Self::Open,
            DomainStatus::Mitigated => Self::Mitigated,
            DomainStatus::Occurred => Self::Occurred,
            DomainStatus::Closed => Self::Closed,
        }
    }
}

/// A level of the likelihood or the impact of a risk.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Level {
    Low,
    Medium,
    High,
}

impl From<Level> for DomainLevel {
    fn from(level: Level) -> Self {
        match level {
            Level::Low => Self::Low,
            Level::Medium => Self::Medium,
            Level::High => Self::High,
        }
    }
}

impl From<DomainLevel> for Level {
    fn from(level: DomainLevel) -> Self {
        match level {
            DomainLevel::Low => Self::Low,
            DomainLevel::Medium => Self::Medium,
            DomainLevel::High => Self::High,
        }
    }
}

/// A risk of an event.
#[derive(Debug, Serialize, ToSchema)]
pub struct Risk {
    pub id: Uuid,
    /// The readable ID, for example `RSK-001`.
    pub local_id: String,
    pub event_id: Uuid,
    /// Empty after a legal redaction (ADR 0072).
    pub title: String,
    /// Absent after a legal redaction. It can have several lines.
    pub description: Option<String>,
    pub likelihood: Level,
    pub impact: Level,
    /// What the club does about the risk. Absent after a legal redaction. It can have several lines.
    pub mitigation: Option<String>,
    /// True after a legal redaction (ADR 0072): the texts are empty or absent.
    pub redacted: bool,
    /// The member who owns the risk: a contributor or manager of the event.
    pub owner_user_id: Uuid,
    pub workstream_id: Option<Uuid>,
    pub status: RiskStatus,
    /// The record version. A change needs it.
    pub version: i64,
    /// The evidence of the accepted proposals that created or changed the risk.
    pub evidence: Vec<RecordEvidence>,
    /// True if the caller can change the risk now: its owner, the lead of its workstream or an event manager.
    pub can_change: bool,
    /// The statuses that the caller can set with a change now. Empty without `can_change`.
    pub next_statuses: Vec<RiskStatus>,
}

impl From<Shown<RiskView>> for Risk {
    fn from(shown: Shown<RiskView>) -> Self {
        let next_statuses = shown.next_statuses().into_iter().map(Into::into).collect();
        let Shown {
            record: risk,
            evidence,
            can_change,
            dispute_notes,
        } = shown;
        let local_id = risk.local_id();
        let fields = risk.fields;
        let redacted = fields.title.as_present().is_none();
        Self {
            id: risk.id.as_uuid(),
            local_id,
            event_id: risk.event_id.as_uuid(),
            redacted,
            title: fields
                .title
                .as_present()
                .map(|title| title.as_str().to_owned())
                .unwrap_or_default(),
            description: fields.description.map(|text| text.as_str().to_owned()),
            likelihood: fields.rating.likelihood.into(),
            impact: fields.rating.impact.into(),
            mitigation: fields.mitigation.map(|text| text.as_str().to_owned()),
            owner_user_id: fields.owner.as_uuid(),
            workstream_id: fields.workstream_id.map(WorkstreamId::as_uuid),
            status: fields.status.into(),
            version: risk.version.get(),
            evidence: evidence
                .into_iter()
                .map(|evidence| RecordEvidence::new(evidence, &dispute_notes))
                .collect(),
            // A redacted risk cannot change (ADR 0072).
            can_change: can_change && !redacted,
            next_statuses: if redacted { Vec::new() } else { next_statuses },
        }
    }
}

/// One page of risks.
#[derive(Debug, Serialize, ToSchema)]
pub struct RiskPage {
    pub items: Vec<Risk>,
    /// The cursor of the next page. It is absent on the last page.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}

/// The cursor is opaque for clients (ADR 0044): the likelihood, the impact and the number of the last risk of the
/// page, as the page showed them, in Base64.
fn encode_cursor(cursor: &RiskCursor) -> String {
    cursor::encode(format!(
        "{} {} {}",
        cursor.rating.likelihood.as_str(),
        cursor.rating.impact.as_str(),
        cursor.number
    ))
}

fn decode_cursor(text: &str) -> Result<RiskCursor, ApiError> {
    let text = cursor::decode_text(text)?;
    let mut parts = text.split(' ');
    let (Some(likelihood), Some(impact), Some(number), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return Err(cursor::invalid());
    };
    let level = |text| DomainLevel::parse(text).ok_or_else(cursor::invalid);
    Ok(RiskCursor {
        rating: RiskRating {
            likelihood: level(likelihood)?,
            impact: level(impact)?,
        },
        number: number.parse().map_err(|_| cursor::invalid())?,
    })
}

/// The parameters of a list of risks.
#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ListRisksQuery {
    /// The page size: 1 to 200. The default is 50.
    #[param(minimum = 1, maximum = 200)]
    pub limit: Option<u32>,
    /// The `next_cursor` of the previous page. A risk that changes its rating between two pages can appear twice
    /// or move, but the change of a risk never makes the list skip another risk.
    pub cursor: Option<String>,
    /// `me`, or the ID of a member: only the risks that this member owns.
    pub owner: Option<String>,
    pub status: Option<RiskStatus>,
    /// Only the risks of this workstream.
    pub workstream: Option<Uuid>,
}

/// The input of `CreateRisk`.
#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateRiskRequest {
    /// The UUIDv7 of the new risk. Without it, the server chooses one. An ID that a record holds gives the field code `taken`, also on a retry.
    pub id: Option<Uuid>,
    /// 1 to 200 characters.
    #[schema(example = "Starkregen am Festtag")]
    pub title: String,
    /// 1 to 4000 characters. It can have several lines.
    pub description: Option<String>,
    pub likelihood: Level,
    pub impact: Level,
    /// 1 to 4000 characters. It can have several lines.
    pub mitigation: Option<String>,
    /// A member with the contributor or manager role in the event.
    pub owner_user_id: Uuid,
    /// An active workstream of the event.
    pub workstream_id: Option<Uuid>,
}

/// The input of `ChangeRisk`. An absent field stays as it is. `null` clears an optional field.
/// A request needs at least one field to change.
#[derive(Debug, Deserialize, ToSchema)]
pub struct ChangeRiskRequest {
    pub title: Option<String>,
    #[serde(default, deserialize_with = "present")]
    #[schema(value_type = Option<String>)]
    pub description: Option<Option<String>>,
    /// The likelihood and the impact change on their own: the one that is absent stays.
    pub likelihood: Option<Level>,
    pub impact: Option<Level>,
    #[serde(default, deserialize_with = "present")]
    #[schema(value_type = Option<String>)]
    pub mitigation: Option<Option<String>>,
    pub owner_user_id: Option<Uuid>,
    /// An active workstream of the event. A record keeps a workstream that closes later.
    #[serde(default, deserialize_with = "present")]
    #[schema(value_type = Option<Uuid>)]
    pub workstream_id: Option<Option<Uuid>>,
    pub status: Option<RiskStatus>,
    #[schema(minimum = 1)]
    pub expected_version: i64,
}

/// Lists the risks of an event, the worst first: likelihood times impact (low 1, medium 2, high 3), then impact,
/// then the number. Each reader of the event can do it.
#[utoipa::path(
    get,
    path = "/events/{event_id}/risks",
    operation_id = "list_risks",
    tag = "work",
    params(("event_id" = Uuid, Path, description = "The ID of the event."), ListRisksQuery),
    responses(
        (status = OK, description = "One page of risks.", body = RiskPage),
        (status = "default", description = "A problem (ADR 0037).", body = Problem, content_type = "application/problem+json"),
    ),
)]
async fn list_risks(
    State(state): State<ApiState>,
    Caller(caller): Caller,
    Path(event_id): Path<Uuid>,
    Query(query): Query<ListRisksQuery>,
) -> Result<axum::Json<RiskPage>, ApiError> {
    let query = WorkQuery {
        owner: owner_filter(&caller, query.owner.as_deref())?,
        status: query.status.map(DomainStatus::from),
        workstream: query.workstream.map(WorkstreamId::from_uuid),
        after: query.cursor.as_deref().map(decode_cursor).transpose()?,
        limit: page_limit(query.limit)?,
    };
    let page = app::list_risks(&caller, EventId::from_uuid(event_id), query, ports(&state)).await?;
    Ok(axum::Json(RiskPage {
        items: page.items.into_iter().map(Risk::from).collect(),
        next_cursor: page.next.as_ref().map(encode_cursor),
    }))
}

/// Creates a risk. A contributor or manager of the event can do it.
/// An owner without the contributor or manager role gives `validation-failed` with the field code `unknown-member`.
/// A closed workstream gives the field code `closed`.
#[utoipa::path(
    post,
    path = "/events/{event_id}/risks",
    operation_id = "create_risk",
    tag = "work",
    params(("event_id" = Uuid, Path, description = "The ID of the event.")),
    request_body = CreateRiskRequest,
    responses(
        (status = CREATED, description = "The new risk.", body = Risk),
        (status = "default", description = "A problem (ADR 0037).", body = Problem, content_type = "application/problem+json"),
    ),
)]
async fn create_risk(
    State(state): State<ApiState>,
    Caller(caller): Caller,
    Path(event_id): Path<Uuid>,
    Json(request): Json<CreateRiskRequest>,
) -> Result<(StatusCode, axum::Json<Risk>), ApiError> {
    let input = NewRisk {
        id: request.id,
        title: request.title,
        description: request.description,
        likelihood: request.likelihood.into(),
        impact: request.impact.into(),
        mitigation: request.mitigation,
        owner: UserId::from_uuid(request.owner_user_id),
        workstream: request.workstream_id.map(WorkstreamId::from_uuid),
    };
    let risk =
        app::create_risk(&caller, EventId::from_uuid(event_id), input, ports(&state)).await?;
    Ok((StatusCode::CREATED, axum::Json(risk.into())))
}

/// Reads one risk of the event, with its evidence.
#[utoipa::path(
    get,
    path = "/events/{event_id}/risks/{id}",
    operation_id = "get_risk",
    tag = "work",
    params(
        ("event_id" = Uuid, Path, description = "The ID of the event."),
        ("id" = Uuid, Path, description = "The ID of the risk."),
    ),
    responses(
        (status = OK, description = "The risk.", body = Risk),
        (status = "default", description = "A problem (ADR 0037).", body = Problem, content_type = "application/problem+json"),
    ),
)]
async fn get_risk(
    State(state): State<ApiState>,
    Caller(caller): Caller,
    Path((event_id, id)): Path<(Uuid, Uuid)>,
) -> Result<axum::Json<Risk>, ApiError> {
    let risk = app::get_risk(
        &caller,
        EventId::from_uuid(event_id),
        RiskId::from_uuid(id),
        ports(&state),
    )
    .await?;
    Ok(axum::Json(risk.into()))
}

/// Changes a risk. Its owner, the lead of its workstream or an event manager can do it.
/// A status that the current status cannot change to gives `invalid-transition`; so does the current status itself.
/// `open` and `mitigated` change into each other and to `occurred`; `open`, `mitigated` and `occurred` change to
/// `closed`; `closed` changes to `open`. A status change does not change the rating.
#[utoipa::path(
    patch,
    path = "/events/{event_id}/risks/{id}",
    operation_id = "change_risk",
    tag = "work",
    params(
        ("event_id" = Uuid, Path, description = "The ID of the event."),
        ("id" = Uuid, Path, description = "The ID of the risk."),
    ),
    request_body = ChangeRiskRequest,
    responses(
        (status = OK, description = "The changed risk.", body = Risk),
        (status = "default", description = "A problem (ADR 0037).", body = Problem, content_type = "application/problem+json"),
    ),
)]
async fn change_risk(
    State(state): State<ApiState>,
    Caller(caller): Caller,
    Path((event_id, id)): Path<(Uuid, Uuid)>,
    Json(request): Json<ChangeRiskRequest>,
) -> Result<axum::Json<Risk>, ApiError> {
    let change = RiskChange {
        title: request.title,
        description: request.description,
        likelihood: request.likelihood.map(DomainLevel::from),
        impact: request.impact.map(DomainLevel::from),
        mitigation: request.mitigation,
        owner: request.owner_user_id.map(UserId::from_uuid),
        workstream: request
            .workstream_id
            .map(|id| id.map(WorkstreamId::from_uuid)),
        status: request.status.map(DomainStatus::from),
        expected_version: record_version(request.expected_version)?,
    };
    let risk = app::change_risk(
        &caller,
        EventId::from_uuid(event_id),
        RiskId::from_uuid(id),
        change,
        ports(&state),
    )
    .await?;
    Ok(axum::Json(risk.into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_cursor_survives_its_round_trip() {
        let cursor = RiskCursor {
            rating: RiskRating {
                likelihood: DomainLevel::High,
                impact: DomainLevel::Low,
            },
            number: 42,
        };
        assert_eq!(decode_cursor(&encode_cursor(&cursor)).unwrap(), cursor);
    }

    #[test]
    fn a_forged_cursor_is_refused() {
        for forged in [
            "not a cursor".to_owned(),
            cursor::encode("7"),
            cursor::encode("high low"),
            cursor::encode("high low x"),
            cursor::encode("high huge 3"),
            cursor::encode("high low 3 4"),
            cursor::encode("high low -3"),
        ] {
            assert!(decode_cursor(&forged).is_err(), "{forged}");
        }
    }
}
