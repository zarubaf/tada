//! `/api/v1/members` and `/api/v1/invitations`: organization memberships and invitations (ADR 0056).

use axum::extract::State;
use axum::http::StatusCode;
use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use tada_app::domain::ids::{InvitationId, UserId};
use tada_app::members::{
    self as app, EndSessionsError, Invitation as AppInvitation, InviteMemberError, Invited,
    ListInvitationsError, ListMembersError, MemberCursor, NewInvitation, OrganizationMember,
    RemoveMemberError, RevokeInvitationError,
};
use tada_app::problem::ProblemCode;
use utoipa::{IntoParams, ToSchema};
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

use crate::ApiState;
use crate::contract::{AUTHENTICATED, JSON_BODY, PATH, QUERY, codes};
use crate::cursor;
use crate::extract::{Caller, Json, Path, Query, page_limit, record_version};
use crate::problem::{ApiError, Problem};
use crate::roles::OrganizationRole;

pub(crate) fn routes() -> OpenApiRouter<ApiState> {
    OpenApiRouter::new()
        .routes(routes!(list_members))
        .routes(routes!(remove_member))
        .routes(routes!(end_member_sessions))
        .routes(routes!(list_invitations, invite_member))
        .routes(routes!(revoke_invitation))
}

/// The problem codes of each operation (ADR 0037).
pub(crate) fn problem_codes() -> Vec<(&'static str, Vec<ProblemCode>)> {
    vec![
        (
            "list_members",
            codes(&[AUTHENTICATED, QUERY, ListMembersError::CODES]),
        ),
        (
            "remove_member",
            codes(&[AUTHENTICATED, PATH, JSON_BODY, RemoveMemberError::CODES]),
        ),
        (
            "end_member_sessions",
            codes(&[AUTHENTICATED, PATH, EndSessionsError::CODES]),
        ),
        (
            "invite_member",
            codes(&[AUTHENTICATED, JSON_BODY, InviteMemberError::CODES]),
        ),
        (
            "list_invitations",
            codes(&[AUTHENTICATED, ListInvitationsError::CODES]),
        ),
        (
            "revoke_invitation",
            codes(&[AUTHENTICATED, PATH, RevokeInvitationError::CODES]),
        ),
    ]
}

/// A member of the organization.
#[derive(Serialize, ToSchema)]
pub struct Member {
    pub user_id: Uuid,
    pub display_name: String,
    /// The email address. Only owners and admins get it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    pub role: OrganizationRole,
    /// The record version of the organization membership. `RemoveMember` needs it.
    pub version: i64,
}

/// The name and the email address are personal data, so `Debug` leaves them out (ADR 0035).
impl std::fmt::Debug for Member {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Member")
            .field("user_id", &self.user_id)
            .field("role", &self.role)
            .field("version", &self.version)
            .finish_non_exhaustive()
    }
}

impl From<OrganizationMember> for Member {
    fn from(member: OrganizationMember) -> Self {
        Self {
            user_id: member.user_id.as_uuid(),
            display_name: member.display_name.as_str().to_owned(),
            email: member.email.map(|email| email.as_str().to_owned()),
            role: member.role.into(),
            version: member.version.get(),
        }
    }
}

/// One page of members.
#[derive(Debug, Serialize, ToSchema)]
pub struct MemberPage {
    pub items: Vec<Member>,
    /// The cursor of the next page. It is absent on the last page.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}

/// The parameters of `ListMembers`.
#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ListMembersQuery {
    /// The page size: 1 to 200. The default is 50.
    #[param(minimum = 1, maximum = 200)]
    pub limit: Option<u32>,
    /// The `next_cursor` of the previous page.
    pub cursor: Option<String>,
}

/// The input of `RemoveMember`.
#[derive(Debug, Deserialize, ToSchema)]
pub struct RemoveMemberRequest {
    #[schema(minimum = 1)]
    pub expected_version: i64,
}

/// An invitation to the organization.
#[derive(Serialize, ToSchema)]
pub struct Invitation {
    pub id: Uuid,
    pub email: String,
    pub display_name: String,
    pub role: OrganizationRole,
    pub created_at: Timestamp,
}

/// The email address and the name are personal data, so `Debug` leaves them out (ADR 0035).
impl std::fmt::Debug for Invitation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Invitation")
            .field("id", &self.id)
            .field("role", &self.role)
            .field("created_at", &self.created_at)
            .finish_non_exhaustive()
    }
}

impl From<AppInvitation> for Invitation {
    fn from(invitation: AppInvitation) -> Self {
        Self {
            id: invitation.id.as_uuid(),
            email: invitation.email.as_str().to_owned(),
            display_name: invitation.display_name.as_str().to_owned(),
            role: invitation.role.into(),
            created_at: invitation.created_at,
        }
    }
}

/// The pending invitations. An organization has few, so the list has one page.
#[derive(Debug, Serialize, ToSchema)]
pub struct InvitationPage {
    pub items: Vec<Invitation>,
}

/// The input of `InviteMember`.
#[derive(Deserialize, ToSchema)]
pub struct InviteMemberRequest {
    /// The UUIDv7 of the new invitation. A client that sends it can retry the request safely.
    pub id: Option<Uuid>,
    #[schema(example = "anna@example.org")]
    pub email: String,
    /// 1 to 100 characters.
    #[schema(example = "Anna Muster")]
    pub display_name: String,
    /// An owner invites with any role; an admin with admin or member.
    pub role: OrganizationRole,
}

/// The email address and the name are personal data, so `Debug` leaves them out (ADR 0035).
impl std::fmt::Debug for InviteMemberRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InviteMemberRequest")
            .field("id", &self.id)
            .field("role", &self.role)
            .finish_non_exhaustive()
    }
}

/// Lists the members of the organization, in the order of their display names.
/// Each member can read the list. Only owners and admins get the email addresses.
#[utoipa::path(
    get,
    path = "/members",
    operation_id = "list_members",
    tag = "members",
    params(ListMembersQuery),
    responses(
        (status = OK, description = "One page of members.", body = MemberPage),
        (status = "default", description = "A problem (ADR 0037).", body = Problem, content_type = "application/problem+json"),
    ),
)]
async fn list_members(
    State(state): State<ApiState>,
    Caller(caller): Caller,
    Query(query): Query<ListMembersQuery>,
) -> Result<axum::Json<MemberPage>, ApiError> {
    let limit = page_limit(query.limit)?;
    let after = query.cursor.as_deref().map(decode_cursor).transpose()?;
    let page = app::list_members(&caller, after, limit, state.members.as_ref()).await?;
    Ok(axum::Json(MemberPage {
        items: page.items.into_iter().map(Member::from).collect(),
        next_cursor: page.next.map(encode_cursor),
    }))
}

/// Removes a member from the organization, with all event memberships and API tokens of the member.
///
/// Owners and admins remove members; only an owner removes an owner. Each member can leave.
/// The last owner and the only event manager of an event stay: this gives `invalid-transition`.
/// The removal ends all sessions and the Telegram link of the member, in each organization, and
/// revokes the pending invitations that the member created.
#[utoipa::path(
    post,
    path = "/members/{user_id}/remove",
    operation_id = "remove_member",
    tag = "members",
    params(("user_id" = Uuid, Path, description = "The ID of the member.")),
    request_body = RemoveMemberRequest,
    responses(
        (status = NO_CONTENT, description = "The membership is removed."),
        (status = "default", description = "A problem (ADR 0037).", body = Problem, content_type = "application/problem+json"),
    ),
)]
async fn remove_member(
    State(state): State<ApiState>,
    Caller(caller): Caller,
    Path(user_id): Path<Uuid>,
    Json(request): Json<RemoveMemberRequest>,
) -> Result<StatusCode, ApiError> {
    app::remove_member(
        &caller,
        UserId::from_uuid(user_id),
        record_version(request.expected_version)?,
        state.members.as_ref(),
        state.clock.as_ref(),
    )
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Invites a person into the organization. The worker sends the invitation mail.
///
/// A pending invitation of the same email address is revoked. If an invitation with the same `id`
/// and the same content exists, the response is this invitation with the status 200.
#[utoipa::path(
    post,
    path = "/invitations",
    operation_id = "invite_member",
    tag = "members",
    request_body = InviteMemberRequest,
    responses(
        (status = CREATED, description = "The new invitation.", body = Invitation),
        (status = OK, description = "The invitation exists with this ID and the same content.", body = Invitation),
        (status = "default", description = "A problem (ADR 0037).", body = Problem, content_type = "application/problem+json"),
    ),
)]
async fn invite_member(
    State(state): State<ApiState>,
    Caller(caller): Caller,
    Json(request): Json<InviteMemberRequest>,
) -> Result<(StatusCode, axum::Json<Invitation>), ApiError> {
    let input = NewInvitation {
        id: request.id,
        email: request.email,
        display_name: request.display_name,
        role: request.role.into(),
    };
    match app::invite_member(&caller, input, state.members.as_ref(), state.clock.as_ref()).await? {
        Invited::New(invitation) => Ok((StatusCode::CREATED, axum::Json(invitation.into()))),
        Invited::Existing(invitation) => Ok((StatusCode::OK, axum::Json(invitation.into()))),
    }
}

/// Lists the pending invitations, the oldest first. Only owners and admins see them.
#[utoipa::path(
    get,
    path = "/invitations",
    operation_id = "list_invitations",
    tag = "members",
    responses(
        (status = OK, description = "All pending invitations.", body = InvitationPage),
        (status = "default", description = "A problem (ADR 0037).", body = Problem, content_type = "application/problem+json"),
    ),
)]
async fn list_invitations(
    State(state): State<ApiState>,
    Caller(caller): Caller,
) -> Result<axum::Json<InvitationPage>, ApiError> {
    let invitations = app::list_invitations(&caller, state.members.as_ref()).await?;
    Ok(axum::Json(InvitationPage {
        items: invitations.into_iter().map(Invitation::from).collect(),
    }))
}

/// Revokes a pending invitation. Its links stop working at once.
/// Only owners and admins can do this.
#[utoipa::path(
    post,
    path = "/invitations/{invitation_id}/revoke",
    operation_id = "revoke_invitation",
    tag = "members",
    params(("invitation_id" = Uuid, Path, description = "The ID of the invitation.")),
    responses(
        (status = NO_CONTENT, description = "The invitation is revoked."),
        (status = "default", description = "A problem (ADR 0037).", body = Problem, content_type = "application/problem+json"),
    ),
)]
async fn revoke_invitation(
    State(state): State<ApiState>,
    Caller(caller): Caller,
    Path(invitation_id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    app::revoke_invitation(
        &caller,
        InvitationId::from_uuid(invitation_id),
        state.members.as_ref(),
        state.clock.as_ref(),
    )
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

/// The cursor is opaque for clients (ADR 0044): the user ID of the last member, in Base64.
fn encode_cursor(cursor: MemberCursor) -> String {
    cursor::encode(cursor.0.as_uuid().as_bytes())
}

fn decode_cursor(text: &str) -> Result<MemberCursor, ApiError> {
    let bytes = cursor::decode(text)?;
    let id = Uuid::from_slice(&bytes).map_err(|_| cursor::invalid())?;
    Ok(MemberCursor(UserId::from_uuid(id)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_debug_output_has_no_address_and_no_name() {
        let member = Member {
            user_id: Uuid::nil(),
            display_name: "Anna Muster".to_owned(),
            email: Some("anna@example.org".to_owned()),
            role: OrganizationRole::Member,
            version: 1,
        };
        let invitation = Invitation {
            id: Uuid::nil(),
            email: "anna@example.org".to_owned(),
            display_name: "Anna Muster".to_owned(),
            role: OrganizationRole::Member,
            created_at: Timestamp::UNIX_EPOCH,
        };
        let request = InviteMemberRequest {
            id: None,
            email: "anna@example.org".to_owned(),
            display_name: "Anna Muster".to_owned(),
            role: OrganizationRole::Member,
        };
        for debug in [
            format!("{member:?}"),
            format!("{invitation:?}"),
            format!("{request:?}"),
        ] {
            assert!(
                !debug.contains("anna@example.org") && !debug.contains("Anna Muster"),
                "{debug}"
            );
        }
    }
}

/// Ends all sessions of a member, in each organization, without a removal: for example after a
/// session theft. The membership stays, and the member signs in again with a magic link.
///
/// Owners and admins end the sessions of members up to their own role; only an owner ends the
/// sessions of an owner. A member ends the own sessions with `/session/sign-out-everywhere`.
#[utoipa::path(
    post,
    path = "/members/{user_id}/sessions/end",
    operation_id = "end_member_sessions",
    tag = "members",
    params(("user_id" = Uuid, Path, description = "The ID of the member.")),
    responses(
        (status = NO_CONTENT, description = "The member has no session any more."),
        (status = "default", description = "A problem (ADR 0037).", body = Problem, content_type = "application/problem+json"),
    ),
)]
async fn end_member_sessions(
    State(state): State<ApiState>,
    Caller(caller): Caller,
    Path(user_id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    app::end_sessions(
        &caller,
        UserId::from_uuid(user_id),
        state.members.as_ref(),
        state.identity.as_ref(),
    )
    .await?;
    Ok(StatusCode::NO_CONTENT)
}
