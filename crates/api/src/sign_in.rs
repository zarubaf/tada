//! Sign-in with a magic link, the session and sign-out (ADR 0008, ADR 0056).
//!
//! The token of a magic link reaches the server only in the body of a POST request.
//! No GET request takes a token, because mail scanners open links (ADR 0008).

use axum::extract::State;
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};
use tada_app::domain::ids::OrganizationId;
use tada_app::identity::Membership;
use tada_app::problem::ProblemCode;
use tada_app::session::{self, ChooseOrganizationError, SessionError};
use tada_app::sign_in::{self as app, Accepted, RequestSignInError, SignInError};
use tada_app::store::StoreError;
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

use crate::ApiState;
use crate::client_ip::ClientIp;
use crate::contract::{JSON_BODY, codes};
use crate::extract::{Json, SessionToken, expired_session_cookie, request_id, session_cookie};
use crate::problem::{ApiError, Problem};
use crate::roles::OrganizationRole;

pub(crate) fn routes() -> OpenApiRouter<ApiState> {
    OpenApiRouter::new()
        .routes(routes!(request_sign_in))
        .routes(routes!(redeem_magic_link))
        .routes(routes!(preview_magic_link))
        .routes(routes!(get_session))
        .routes(routes!(choose_organization))
        .routes(routes!(sign_out))
        .routes(routes!(preview_invitation))
        .routes(routes!(accept_invitation))
}

pub(crate) fn problem_codes() -> Vec<(&'static str, Vec<ProblemCode>)> {
    vec![
        (
            "request_sign_in",
            codes(&[JSON_BODY, RequestSignInError::CODES]),
        ),
        (
            "redeem_magic_link",
            codes(&[JSON_BODY, SignInError::CODES, SessionError::CODES]),
        ),
        (
            "preview_magic_link",
            codes(&[JSON_BODY, SignInError::CODES]),
        ),
        ("get_session", codes(&[SessionError::CODES])),
        (
            "choose_organization",
            codes(&[
                JSON_BODY,
                ChooseOrganizationError::CODES,
                SessionError::CODES,
            ]),
        ),
        ("sign_out", codes(&[StoreError::CODES])),
        (
            "preview_invitation",
            codes(&[JSON_BODY, SignInError::CODES]),
        ),
        (
            "accept_invitation",
            codes(&[JSON_BODY, SignInError::CODES, SessionError::CODES]),
        ),
    ]
}

/// A membership of the signed-in user.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct MembershipSummary {
    pub organization_id: Uuid,
    /// The name of the organization.
    pub name: String,
    pub role: OrganizationRole,
}

impl From<Membership> for MembershipSummary {
    fn from(membership: Membership) -> Self {
        Self {
            organization_id: membership.organization_id.as_uuid(),
            name: membership.organization_name,
            role: membership.role.into(),
        }
    }
}

/// The signed-in user and the organization of the session.
#[derive(Debug, Serialize, ToSchema)]
pub struct SessionInfo {
    pub user_id: Uuid,
    pub display_name: String,
    /// The organization of the session. It is absent until the member chooses one (ADR 0056).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub organization: Option<MembershipSummary>,
    /// All memberships of the user, in the order of the organization names.
    pub memberships: Vec<MembershipSummary>,
}

impl From<session::SessionInfo> for SessionInfo {
    fn from(info: session::SessionInfo) -> Self {
        let memberships: Vec<MembershipSummary> = info
            .memberships
            .into_iter()
            .map(MembershipSummary::from)
            .collect();
        let organization = info.organization_id.and_then(|id| {
            memberships
                .iter()
                .find(|membership| membership.organization_id == id.as_uuid())
                .cloned()
        });
        Self {
            user_id: info.user_id.as_uuid(),
            display_name: info.display_name.as_str().to_owned(),
            organization,
            memberships,
        }
    }
}

/// The input of a sign-in request.
#[derive(Deserialize, ToSchema)]
pub struct SignInRequest {
    /// The email address of the member.
    pub email: String,
}

impl std::fmt::Debug for SignInRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // The address never goes to a log (ADR 0035).
        f.write_str("SignInRequest(redacted)")
    }
}

/// Asks for a magic link. The answer is the same for each address (ADR 0008).
/// Only a member gets a mail. Too many requests for one address or from one client get
/// `rate-limited` with `Retry-After` (ADR 0056).
#[utoipa::path(
    post,
    path = "/sign-in/requests",
    operation_id = "request_sign_in",
    tag = "sign-in",
    request_body = SignInRequest,
    responses(
        (status = ACCEPTED, description = "The request is accepted. The body is empty."),
        (status = "default", description = "A problem (ADR 0037).", body = Problem, content_type = "application/problem+json"),
    ),
)]
async fn request_sign_in(
    State(state): State<ApiState>,
    ClientIp(client_ip): ClientIp,
    Json(body): Json<SignInRequest>,
) -> Result<StatusCode, ApiError> {
    app::request_magic_link(
        &body.email,
        client_ip,
        request_id(),
        state.sign_in_requests.as_ref(),
        state.clock.as_ref(),
    )
    .await?;
    Ok(StatusCode::ACCEPTED)
}

/// The input of a sign-in with a magic link.
#[derive(Deserialize, ToSchema)]
pub struct RedeemMagicLinkRequest {
    /// The token from the fragment of the magic link.
    pub token: String,
}

impl std::fmt::Debug for RedeemMagicLinkRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // The token never goes to a log (ADR 0035).
        f.write_str("RedeemMagicLinkRequest(redacted)")
    }
}

/// Signs in with the token of a magic link. The token works once.
/// The response sets the session cookie. The new session ends the session that the request sends.
#[utoipa::path(
    post,
    path = "/sign-in/magic-link",
    operation_id = "redeem_magic_link",
    tag = "sign-in",
    request_body = RedeemMagicLinkRequest,
    responses(
        (status = OK, description = "The new session. The `Set-Cookie` header holds its token.", body = SessionInfo),
        (status = "default", description = "A problem (ADR 0037).", body = Problem, content_type = "application/problem+json"),
    ),
)]
async fn redeem_magic_link(
    State(state): State<ApiState>,
    replaced: Option<SessionToken>,
    headers: HeaderMap,
    Json(body): Json<RedeemMagicLinkRequest>,
) -> Result<Response, ApiError> {
    let result = app::redeem_magic_link(
        &body.token,
        replaced.as_ref().map(SessionToken::as_str),
        user_agent(&headers),
        state.sign_in.as_ref(),
        state.clock.as_ref(),
    )
    .await;
    new_session(&state, result).await
}

/// The account of a magic link, before the person signs in.
#[derive(Debug, Serialize, ToSchema)]
pub struct MagicLinkPreview {
    /// The masked address of the account, for example `a…@example.org`.
    pub email_hint: String,
}

/// Shows the masked address of the account of a magic link. It does not use the token. The person
/// sees which account the link signs in to, for example the account of someone else (login CSRF).
#[utoipa::path(
    post,
    path = "/sign-in/magic-link/preview",
    operation_id = "preview_magic_link",
    tag = "sign-in",
    request_body = RedeemMagicLinkRequest,
    responses(
        (status = OK, description = "The account of the link.", body = MagicLinkPreview),
        (status = "default", description = "A problem (ADR 0037).", body = Problem, content_type = "application/problem+json"),
    ),
)]
async fn preview_magic_link(
    State(state): State<ApiState>,
    Json(body): Json<RedeemMagicLinkRequest>,
) -> Result<Response, ApiError> {
    let email_hint =
        app::preview_magic_link(&body.token, state.sign_in.as_ref(), state.clock.as_ref()).await?;
    Ok(uncached(MagicLinkPreview { email_hint }))
}

/// The `User-Agent` of a request, for the list of sessions.
fn user_agent(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(header::USER_AGENT)
        .and_then(|value| value.to_str().ok())
}

/// The response to a sign-in that started a session: the session and the cookie of its token.
async fn new_session(
    state: &ApiState,
    result: Result<SecretString, SignInError>,
) -> Result<Response, ApiError> {
    let token = result?;
    let info = read_session(state, token.expose_secret()).await?;
    let mut response = uncached(info);
    response
        .headers_mut()
        .insert(header::SET_COOKIE, session_cookie(token.expose_secret())?);
    Ok(response)
}

/// A response that no cache keeps, because it holds personal data, for example the session.
fn uncached(body: impl Serialize) -> Response {
    let mut response = axum::Json(body).into_response();
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

async fn read_session(state: &ApiState, token: &str) -> Result<SessionInfo, ApiError> {
    let info = session::session_info(
        token,
        state.sessions.as_ref(),
        state.identity.as_ref(),
        state.clock.as_ref(),
    )
    .await?;
    Ok(info.into())
}

/// Reads the session of the request. A session without an organization also gets its information.
#[utoipa::path(
    get,
    path = "/session",
    operation_id = "get_session",
    tag = "sign-in",
    responses(
        (status = OK, description = "The session.", body = SessionInfo),
        (status = "default", description = "A problem (ADR 0037).", body = Problem, content_type = "application/problem+json"),
    ),
)]
async fn get_session(
    State(state): State<ApiState>,
    token: SessionToken,
) -> Result<Response, ApiError> {
    Ok(uncached(read_session(&state, token.as_str()).await?))
}

/// The input of `choose_organization`.
#[derive(Debug, Deserialize, ToSchema)]
pub struct ChooseOrganizationRequest {
    pub organization_id: Uuid,
}

/// Chooses or changes the organization of the session (ADR 0056). The token stays the same.
#[utoipa::path(
    post,
    path = "/session/organization",
    operation_id = "choose_organization",
    tag = "sign-in",
    request_body = ChooseOrganizationRequest,
    responses(
        (status = OK, description = "The changed session.", body = SessionInfo),
        (status = "default", description = "A problem (ADR 0037).", body = Problem, content_type = "application/problem+json"),
    ),
)]
async fn choose_organization(
    State(state): State<ApiState>,
    token: SessionToken,
    Json(body): Json<ChooseOrganizationRequest>,
) -> Result<Response, ApiError> {
    session::choose_organization(
        token.as_str(),
        OrganizationId::from_uuid(body.organization_id),
        state.sessions.as_ref(),
        state.identity.as_ref(),
        state.clock.as_ref(),
    )
    .await?;
    Ok(uncached(read_session(&state, token.as_str()).await?))
}

/// Signs out: ends the session and clears the cookie. It also succeeds without a session.
#[utoipa::path(
    post,
    path = "/sign-out",
    operation_id = "sign_out",
    tag = "sign-in",
    responses(
        (status = NO_CONTENT, description = "The session ended."),
        (status = "default", description = "A problem (ADR 0037).", body = Problem, content_type = "application/problem+json"),
    ),
)]
async fn sign_out(
    State(state): State<ApiState>,
    token: Option<SessionToken>,
) -> Result<Response, ApiError> {
    if let Some(token) = token {
        session::sign_out(token.as_str(), state.sessions.as_ref()).await?;
    }
    let mut response = StatusCode::NO_CONTENT.into_response();
    response
        .headers_mut()
        .insert(header::SET_COOKIE, expired_session_cookie()?);
    Ok(response)
}

/// The input of the invitation routes. Like the token of a magic link, the token of an invitation
/// reaches the server only in the body of a POST request (ADR 0008).
#[derive(Deserialize, ToSchema)]
pub struct InvitationTokenRequest {
    /// The token from the fragment of the invitation link.
    pub token: String,
}

impl std::fmt::Debug for InvitationTokenRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // The token never goes to a log (ADR 0035).
        f.write_str("InvitationTokenRequest(redacted)")
    }
}

/// What an invitation is for, before the invitee accepts it.
#[derive(Debug, Serialize, ToSchema)]
pub struct InvitationPreview {
    /// The name of the organization of the invitation.
    pub organization_name: String,
    /// The privacy notice of the organization in Markdown, or null if the template of the web client applies.
    /// The invitee reads it before the acceptance (ADR 0045).
    #[schema(required = true, nullable = true)]
    pub privacy_notice: Option<String>,
    /// The role that the invitation gives.
    pub role: OrganizationRole,
}

impl From<app::InvitationPreview> for InvitationPreview {
    fn from(preview: app::InvitationPreview) -> Self {
        Self {
            organization_name: preview.organization_name,
            privacy_notice: preview.privacy_notice,
            role: preview.role.into(),
        }
    }
}

/// Shows the organization and the role of an invitation. It does not use the token.
#[utoipa::path(
    post,
    path = "/invitations/preview",
    operation_id = "preview_invitation",
    tag = "sign-in",
    request_body = InvitationTokenRequest,
    responses(
        (status = OK, description = "What the invitation is for.", body = InvitationPreview),
        (status = "default", description = "A problem (ADR 0037).", body = Problem, content_type = "application/problem+json"),
    ),
)]
async fn preview_invitation(
    State(state): State<ApiState>,
    Json(body): Json<InvitationTokenRequest>,
) -> Result<Response, ApiError> {
    let preview =
        app::preview_invitation(&body.token, state.sign_in.as_ref(), state.clock.as_ref()).await?;
    Ok(uncached(InvitationPreview::from(preview)))
}

/// Accepts an invitation. The token works once. For a user without a membership in another
/// organization, the response sets the cookie of a new session in the organization of the
/// invitation, and the new session ends the session that the request sends. A member of another
/// organization gets `202` without a session and signs in with a magic link.
#[utoipa::path(
    post,
    path = "/invitations/accept",
    operation_id = "accept_invitation",
    tag = "sign-in",
    request_body = InvitationTokenRequest,
    responses(
        (status = OK, description = "The new session. The `Set-Cookie` header holds its token.", body = SessionInfo),
        (status = ACCEPTED, description = "The membership is added. The user is a member of another organization and signs in with a magic link. The body is empty."),
        (status = "default", description = "A problem (ADR 0037).", body = Problem, content_type = "application/problem+json"),
    ),
)]
async fn accept_invitation(
    State(state): State<ApiState>,
    replaced: Option<SessionToken>,
    headers: HeaderMap,
    Json(body): Json<InvitationTokenRequest>,
) -> Result<Response, ApiError> {
    let accepted = app::accept_invitation(
        &body.token,
        replaced.as_ref().map(SessionToken::as_str),
        user_agent(&headers),
        request_id(),
        state.sign_in.as_ref(),
        state.clock.as_ref(),
    )
    .await?;
    match accepted {
        Accepted::Session(token) => new_session(&state, Ok(token)).await,
        Accepted::SignInRequired => Ok(StatusCode::ACCEPTED.into_response()),
    }
}
