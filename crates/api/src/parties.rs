//! `/api/v1/persons` and `/api/v1/institutions` (ADR 0069).

use axum::extract::State;
use axum::http::StatusCode;
use serde::{Deserialize, Deserializer, Serialize};
use tada_app::domain::ids::{InstitutionId, PersonId, UserId};
use tada_app::parties::{
    self as app, InstitutionChange, InstitutionView, NewInstitution, NewPerson, PartyError,
    PartyReadError, PersonChange, PersonView,
};
use tada_app::problem::ProblemCode;
use tada_app::records::{NumberCursor, Shown};
use utoipa::{IntoParams, ToSchema};
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

use crate::ApiState;
use crate::contract::{AUTHENTICATED, JSON_BODY, PATH, QUERY, codes};
use crate::cursor;
use crate::extract::{Caller, Json, Path, Query, page_limit, record_version};
use crate::problem::{ApiError, Problem};
use crate::work::RecordEvidence;

pub(crate) fn routes() -> OpenApiRouter<ApiState> {
    OpenApiRouter::new()
        .routes(routes!(list_persons, create_person))
        .routes(routes!(get_person, change_person))
        .routes(routes!(list_institutions, create_institution))
        .routes(routes!(get_institution, change_institution))
}

/// The problem codes of each operation (ADR 0037).
pub(crate) fn problem_codes() -> Vec<(&'static str, Vec<ProblemCode>)> {
    vec![
        (
            "list_persons",
            codes(&[AUTHENTICATED, QUERY, PartyReadError::CODES]),
        ),
        (
            "create_person",
            codes(&[AUTHENTICATED, JSON_BODY, PartyError::CODES]),
        ),
        (
            "get_person",
            codes(&[AUTHENTICATED, PATH, PartyReadError::CODES]),
        ),
        (
            "change_person",
            codes(&[AUTHENTICATED, PATH, JSON_BODY, PartyError::CODES]),
        ),
        (
            "list_institutions",
            codes(&[AUTHENTICATED, QUERY, PartyReadError::CODES]),
        ),
        (
            "create_institution",
            codes(&[AUTHENTICATED, JSON_BODY, PartyError::CODES]),
        ),
        (
            "get_institution",
            codes(&[AUTHENTICATED, PATH, PartyReadError::CODES]),
        ),
        (
            "change_institution",
            codes(&[AUTHENTICATED, PATH, JSON_BODY, PartyError::CODES]),
        ),
    ]
}

/// A person of the organization.
#[derive(Debug, Serialize, ToSchema)]
pub struct Person {
    pub id: Uuid,
    /// The readable ID, for example `PER-001`.
    pub local_id: String,
    pub name: String,
    pub email: Option<String>,
    pub phone: Option<String>,
    /// The account of the person, if the person is a member.
    pub user_id: Option<Uuid>,
    /// The record version. A change needs it.
    pub version: i64,
    /// The evidence of the accepted proposals that created or changed the person,
    /// from the sources that the caller can read.
    pub evidence: Vec<RecordEvidence>,
    /// True if the caller can change the person: an owner or an admin.
    pub can_change: bool,
}

impl From<Shown<PersonView>> for Person {
    fn from(shown: Shown<PersonView>) -> Self {
        let Shown {
            record: person,
            evidence,
            can_change,
        } = shown;
        Self {
            id: person.id.as_uuid(),
            local_id: person.local_id(),
            name: person.name.as_str().to_owned(),
            email: person.email.map(|email| email.as_str().to_owned()),
            phone: person.phone.map(|phone| phone.as_str().to_owned()),
            user_id: person.user_id.map(UserId::as_uuid),
            version: person.version.get(),
            evidence: evidence.into_iter().map(RecordEvidence::from).collect(),
            can_change,
        }
    }
}

/// An institution of the organization.
#[derive(Debug, Serialize, ToSchema)]
pub struct Institution {
    pub id: Uuid,
    /// The readable ID, for example `INS-001`.
    pub local_id: String,
    pub name: String,
    /// `authority`, `company`, `club` or `other`.
    pub kind: String,
    pub email: Option<String>,
    pub phone: Option<String>,
    /// The record version. A change needs it.
    pub version: i64,
    /// The evidence of the accepted proposals that created or changed the institution,
    /// from the sources that the caller can read.
    pub evidence: Vec<RecordEvidence>,
    /// True if the caller can change the institution: an owner or an admin.
    pub can_change: bool,
}

impl From<Shown<InstitutionView>> for Institution {
    fn from(shown: Shown<InstitutionView>) -> Self {
        let Shown {
            record: institution,
            evidence,
            can_change,
        } = shown;
        Self {
            id: institution.id.as_uuid(),
            local_id: institution.local_id(),
            name: institution.name.as_str().to_owned(),
            kind: institution.kind.as_str().to_owned(),
            email: institution.email.map(|email| email.as_str().to_owned()),
            phone: institution.phone.map(|phone| phone.as_str().to_owned()),
            version: institution.version.get(),
            evidence: evidence.into_iter().map(RecordEvidence::from).collect(),
            can_change,
        }
    }
}

/// One page of persons.
#[derive(Debug, Serialize, ToSchema)]
pub struct PersonPage {
    pub items: Vec<Person>,
    /// The cursor of the next page. It is absent on the last page.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}

/// One page of institutions.
#[derive(Debug, Serialize, ToSchema)]
pub struct InstitutionPage {
    pub items: Vec<Institution>,
    /// The cursor of the next page. It is absent on the last page.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}

/// The parameters of a list.
#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ListPartiesQuery {
    /// The page size: 1 to 200. The default is 50.
    #[param(minimum = 1, maximum = 200)]
    pub limit: Option<u32>,
    /// The `next_cursor` of the previous page.
    pub cursor: Option<String>,
    /// Only records whose name contains this text. Case, accents and extra spaces do not matter.
    pub q: Option<String>,
}

/// The input of `CreatePerson`.
#[derive(Debug, Deserialize, ToSchema)]
pub struct CreatePersonRequest {
    /// The UUIDv7 of the new person. Without it, the server chooses one. An ID that a record holds gives the field code `taken`, also on a retry.
    pub id: Option<Uuid>,
    /// 1 to 200 characters.
    #[schema(example = "Beat Muster")]
    pub name: String,
    pub email: Option<String>,
    /// 1 to 50 characters, as written.
    pub phone: Option<String>,
    /// A member of the caller's organization.
    pub user_id: Option<Uuid>,
}

/// The input of `CreateInstitution`.
#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateInstitutionRequest {
    /// The UUIDv7 of the new institution. Without it, the server chooses one. An ID that a record holds gives the field code `taken`, also on a retry.
    pub id: Option<Uuid>,
    /// 1 to 200 characters.
    #[schema(example = "Testwil Generatoren AG")]
    pub name: String,
    /// `authority`, `company`, `club` or `other`.
    #[schema(example = "company")]
    pub kind: String,
    pub email: Option<String>,
    /// 1 to 50 characters, as written.
    pub phone: Option<String>,
}

/// Tells an absent field from a field that is `null`: the first keeps the value, the second clears it.
fn present<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<Option<String>>, D::Error> {
    Option::<String>::deserialize(deserializer).map(Some)
}

/// The input of `ChangePerson`. An absent field stays as it is. `null` clears email or phone.
/// A request needs at least one field to change.
#[derive(Debug, Deserialize, ToSchema)]
pub struct ChangePersonRequest {
    pub name: Option<String>,
    #[serde(default, deserialize_with = "present")]
    #[schema(value_type = Option<String>)]
    pub email: Option<Option<String>>,
    #[serde(default, deserialize_with = "present")]
    #[schema(value_type = Option<String>)]
    pub phone: Option<Option<String>>,
    #[schema(minimum = 1)]
    pub expected_version: i64,
}

/// The input of `ChangeInstitution`. An absent field stays as it is. `null` clears email or phone.
/// A request needs at least one field to change.
#[derive(Debug, Deserialize, ToSchema)]
pub struct ChangeInstitutionRequest {
    pub name: Option<String>,
    pub kind: Option<String>,
    #[serde(default, deserialize_with = "present")]
    #[schema(value_type = Option<String>)]
    pub email: Option<Option<String>>,
    #[serde(default, deserialize_with = "present")]
    #[schema(value_type = Option<String>)]
    pub phone: Option<Option<String>>,
    #[schema(minimum = 1)]
    pub expected_version: i64,
}

fn encode_cursor(cursor: &NumberCursor) -> String {
    cursor::encode(cursor.0.to_string())
}

fn decode_cursor(text: &str) -> Result<NumberCursor, ApiError> {
    cursor::decode_text(text)?
        .parse()
        .map(NumberCursor)
        .map_err(|_| cursor::invalid())
}

/// Lists the persons of the organization in the order of their numbers.
/// The caller needs an event role in some event, or the organization role owner or admin.
#[utoipa::path(
    get,
    path = "/persons",
    operation_id = "list_persons",
    tag = "parties",
    params(ListPartiesQuery),
    responses(
        (status = OK, description = "One page of persons.", body = PersonPage),
        (status = "default", description = "A problem (ADR 0037).", body = Problem, content_type = "application/problem+json"),
    ),
)]
async fn list_persons(
    State(state): State<ApiState>,
    Caller(caller): Caller,
    Query(query): Query<ListPartiesQuery>,
) -> Result<axum::Json<PersonPage>, ApiError> {
    let limit = page_limit(query.limit)?;
    let after = query.cursor.as_deref().map(decode_cursor).transpose()?;
    let page = app::list_persons(
        &caller,
        query.q.as_deref(),
        after,
        limit,
        state.identity.as_ref(),
        state.parties.as_ref(),
    )
    .await?;
    Ok(axum::Json(PersonPage {
        items: page.items.into_iter().map(Person::from).collect(),
        next_cursor: page.next.as_ref().map(encode_cursor),
    }))
}

/// Creates a person. The caller needs the contributor or manager role in some event, or the
/// organization role owner or admin.
#[utoipa::path(
    post,
    path = "/persons",
    operation_id = "create_person",
    tag = "parties",
    request_body = CreatePersonRequest,
    responses(
        (status = CREATED, description = "The new person.", body = Person),
        (status = "default", description = "A problem (ADR 0037).", body = Problem, content_type = "application/problem+json"),
    ),
)]
async fn create_person(
    State(state): State<ApiState>,
    Caller(caller): Caller,
    Json(request): Json<CreatePersonRequest>,
) -> Result<(StatusCode, axum::Json<Person>), ApiError> {
    let input = NewPerson {
        id: request.id,
        name: request.name,
        email: request.email,
        phone: request.phone,
        user_id: request.user_id.map(UserId::from_uuid),
    };
    let person = app::create_person(
        &caller,
        input,
        state.identity.as_ref(),
        state.parties.as_ref(),
        state.clock.as_ref(),
    )
    .await?;
    Ok((StatusCode::CREATED, axum::Json(person.into())))
}

/// Reads one person.
#[utoipa::path(
    get,
    path = "/persons/{person_id}",
    operation_id = "get_person",
    tag = "parties",
    params(("person_id" = Uuid, Path, description = "The ID of the person.")),
    responses(
        (status = OK, description = "The person.", body = Person),
        (status = "default", description = "A problem (ADR 0037).", body = Problem, content_type = "application/problem+json"),
    ),
)]
async fn get_person(
    State(state): State<ApiState>,
    Caller(caller): Caller,
    Path(person_id): Path<Uuid>,
) -> Result<axum::Json<Person>, ApiError> {
    let person = app::get_person(
        &caller,
        PersonId::from_uuid(person_id),
        state.identity.as_ref(),
        state.parties.as_ref(),
    )
    .await?;
    Ok(axum::Json(person.into()))
}

/// Changes a person. Only an owner or an admin can do it.
/// A request with an old `expected_version` gets `record-version-conflict`.
#[utoipa::path(
    patch,
    path = "/persons/{person_id}",
    operation_id = "change_person",
    tag = "parties",
    params(("person_id" = Uuid, Path, description = "The ID of the person.")),
    request_body = ChangePersonRequest,
    responses(
        (status = OK, description = "The changed person.", body = Person),
        (status = "default", description = "A problem (ADR 0037).", body = Problem, content_type = "application/problem+json"),
    ),
)]
async fn change_person(
    State(state): State<ApiState>,
    Caller(caller): Caller,
    Path(person_id): Path<Uuid>,
    Json(request): Json<ChangePersonRequest>,
) -> Result<axum::Json<Person>, ApiError> {
    let change = PersonChange {
        name: request.name,
        email: request.email,
        phone: request.phone,
        expected_version: record_version(request.expected_version)?,
    };
    let person = app::change_person(
        &caller,
        PersonId::from_uuid(person_id),
        change,
        state.identity.as_ref(),
        state.parties.as_ref(),
        state.clock.as_ref(),
    )
    .await?;
    Ok(axum::Json(person.into()))
}

/// Lists the institutions of the organization in the order of their numbers.
/// The caller needs an event role in some event, or the organization role owner or admin.
#[utoipa::path(
    get,
    path = "/institutions",
    operation_id = "list_institutions",
    tag = "parties",
    params(ListPartiesQuery),
    responses(
        (status = OK, description = "One page of institutions.", body = InstitutionPage),
        (status = "default", description = "A problem (ADR 0037).", body = Problem, content_type = "application/problem+json"),
    ),
)]
async fn list_institutions(
    State(state): State<ApiState>,
    Caller(caller): Caller,
    Query(query): Query<ListPartiesQuery>,
) -> Result<axum::Json<InstitutionPage>, ApiError> {
    let limit = page_limit(query.limit)?;
    let after = query.cursor.as_deref().map(decode_cursor).transpose()?;
    let page = app::list_institutions(
        &caller,
        query.q.as_deref(),
        after,
        limit,
        state.identity.as_ref(),
        state.parties.as_ref(),
    )
    .await?;
    Ok(axum::Json(InstitutionPage {
        items: page.items.into_iter().map(Institution::from).collect(),
        next_cursor: page.next.as_ref().map(encode_cursor),
    }))
}

/// Creates an institution. The caller needs the contributor or manager role in some event, or the
/// organization role owner or admin.
#[utoipa::path(
    post,
    path = "/institutions",
    operation_id = "create_institution",
    tag = "parties",
    request_body = CreateInstitutionRequest,
    responses(
        (status = CREATED, description = "The new institution.", body = Institution),
        (status = "default", description = "A problem (ADR 0037).", body = Problem, content_type = "application/problem+json"),
    ),
)]
async fn create_institution(
    State(state): State<ApiState>,
    Caller(caller): Caller,
    Json(request): Json<CreateInstitutionRequest>,
) -> Result<(StatusCode, axum::Json<Institution>), ApiError> {
    let input = NewInstitution {
        id: request.id,
        name: request.name,
        kind: request.kind,
        email: request.email,
        phone: request.phone,
    };
    let institution = app::create_institution(
        &caller,
        input,
        state.identity.as_ref(),
        state.parties.as_ref(),
        state.clock.as_ref(),
    )
    .await?;
    Ok((StatusCode::CREATED, axum::Json(institution.into())))
}

/// Reads one institution.
#[utoipa::path(
    get,
    path = "/institutions/{institution_id}",
    operation_id = "get_institution",
    tag = "parties",
    params(("institution_id" = Uuid, Path, description = "The ID of the institution.")),
    responses(
        (status = OK, description = "The institution.", body = Institution),
        (status = "default", description = "A problem (ADR 0037).", body = Problem, content_type = "application/problem+json"),
    ),
)]
async fn get_institution(
    State(state): State<ApiState>,
    Caller(caller): Caller,
    Path(institution_id): Path<Uuid>,
) -> Result<axum::Json<Institution>, ApiError> {
    let institution = app::get_institution(
        &caller,
        InstitutionId::from_uuid(institution_id),
        state.identity.as_ref(),
        state.parties.as_ref(),
    )
    .await?;
    Ok(axum::Json(institution.into()))
}

/// Changes an institution. Only an owner or an admin can do it.
/// A request with an old `expected_version` gets `record-version-conflict`.
#[utoipa::path(
    patch,
    path = "/institutions/{institution_id}",
    operation_id = "change_institution",
    tag = "parties",
    params(("institution_id" = Uuid, Path, description = "The ID of the institution.")),
    request_body = ChangeInstitutionRequest,
    responses(
        (status = OK, description = "The changed institution.", body = Institution),
        (status = "default", description = "A problem (ADR 0037).", body = Problem, content_type = "application/problem+json"),
    ),
)]
async fn change_institution(
    State(state): State<ApiState>,
    Caller(caller): Caller,
    Path(institution_id): Path<Uuid>,
    Json(request): Json<ChangeInstitutionRequest>,
) -> Result<axum::Json<Institution>, ApiError> {
    let change = InstitutionChange {
        name: request.name,
        kind: request.kind,
        email: request.email,
        phone: request.phone,
        expected_version: record_version(request.expected_version)?,
    };
    let institution = app::change_institution(
        &caller,
        InstitutionId::from_uuid(institution_id),
        change,
        state.identity.as_ref(),
        state.parties.as_ref(),
        state.clock.as_ref(),
    )
    .await?;
    Ok(axum::Json(institution.into()))
}
