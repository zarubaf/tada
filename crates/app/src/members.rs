//! Organization memberships and invitations (ADR 0056): owners and admins invite members, revoke
//! invitations and remove memberships.

use std::fmt::Debug;

use async_trait::async_trait;
use jiff::Timestamp;
use tada_domain::RecordVersion;
use tada_domain::identity::{DisplayName, DisplayNameError, Email, EmailError, OrganizationRole};
use tada_domain::ids::{self, InvitationId, UserId};
use uuid::Uuid;

use crate::access::Principal;
use crate::audit::{AuditAction, AuditEvent, AuditRole};
use crate::caller::{MemberCaller, OrgScope};
use crate::clock::Clock;
use crate::paging::{Page, PageLimit};
use crate::problem::{CommandError, FieldError, ProblemCode};
use crate::store::StoreError;

/// A member of the organization.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrganizationMember {
    pub user_id: UserId,
    pub display_name: DisplayName,
    /// Only owners and admins see the email address of a member.
    pub email: Option<Email>,
    pub role: OrganizationRole,
    pub version: RecordVersion,
}

/// An invitation to the organization.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Invitation {
    pub id: InvitationId,
    pub email: Email,
    pub display_name: DisplayName,
    pub role: OrganizationRole,
    pub created_at: Timestamp,
}

impl Invitation {
    fn same_content(&self, other: &Self) -> bool {
        self.email == other.email
            && self.display_name == other.display_name
            && self.role == other.role
    }
}

/// The input of `InviteMember`, as the caller gives it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewInvitation {
    /// The ID of the new invitation. A client that sends it can retry safely (ADR 0038).
    pub id: Option<Uuid>,
    pub email: String,
    pub display_name: String,
    pub role: OrganizationRole,
}

/// The position after the last member of a page (ADR 0044).
/// The members are in the order of their display names. The cursor holds the user ID only, and
/// the store reads the display name from it, so that no name goes into a URL.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemberCursor(pub UserId);

/// The result of `MemberStore::invite`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InvitationInsert {
    Inserted,
    /// An invitation with this ID exists, in this organization or in another one.
    IdTaken,
    /// The email address belongs to a member of the organization.
    AlreadyMember,
}

/// The result of a successful `InviteMember`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Invited {
    New(Invitation),
    /// A retry: the invitation with this ID and the same content exists, and nothing changed.
    Existing(Invitation),
}

/// Why a removal did not happen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    /// The user is not a member of the organization.
    NotFound,
    Forbidden,
    /// The membership has another version.
    VersionConflict,
    /// The member is the last owner of the organization (ADR 0056).
    LastOwner,
    /// The member is the only event manager of an event of the organization (ADR 0052).
    LastManager,
}

/// The membership to remove and its surroundings, locked in the transaction of the removal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LockedMembership {
    pub role: OrganizationRole,
    pub version: RecordVersion,
    /// The number of owners of the organization, the member included.
    pub owners: usize,
    /// True if the member is the only event manager of at least one event.
    pub only_manager: bool,
    /// The role of the remover, locked in the same transaction. `None` if the remover is no
    /// longer a member. The role of the session can be out of date.
    pub remover_role: Option<OrganizationRole>,
}

/// The member who removes an organization membership.
/// It holds the role rules of a removal (ADR 0056), so the store can apply them to the locked rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Remover {
    user_id: UserId,
}

impl Remover {
    pub fn of(caller: &MemberCaller) -> Self {
        Self {
            user_id: caller.user_id(),
        }
    }

    /// The store locks the membership of this user too, for `LockedMembership::remover_role`.
    pub fn user_id(self) -> UserId {
        self.user_id
    }

    /// `None` if this member can remove the locked membership of `member`.
    ///
    /// Each member can leave. Owners and admins remove members up to their own locked role.
    /// The last owner and the only event manager of an event stay.
    pub fn refusal(
        self,
        member: UserId,
        expected_version: RecordVersion,
        locked: &LockedMembership,
    ) -> Option<Refusal> {
        let Some(remover_role) = locked.remover_role else {
            return Some(Refusal::Forbidden);
        };
        if member != self.user_id && !manages(remover_role, locked.role) {
            Some(Refusal::Forbidden)
        } else if locked.version != expected_version {
            Some(Refusal::VersionConflict)
        } else if locked.role == OrganizationRole::Owner && locked.owners <= 1 {
            Some(Refusal::LastOwner)
        } else if locked.only_manager {
            Some(Refusal::LastManager)
        } else {
            None
        }
    }
}

/// True if a member with the role `manager` can give or take the role `role`:
/// owners and admins manage the roles up to their own role (ADR 0056).
fn manages(manager: OrganizationRole, role: OrganizationRole) -> bool {
    manager.is_owner_or_admin() && role <= manager
}

/// The repository port for organization memberships and invitations. Each method stays inside
/// `scope`. Each change records its audit events in the same transaction.
#[async_trait]
pub trait MemberStore: Debug + Send + Sync {
    /// At most `limit` members with their email addresses, in the order of the display names and
    /// the user IDs, after `after` if it is given.
    async fn list(
        &self,
        scope: OrgScope,
        after: Option<MemberCursor>,
        limit: u32,
    ) -> Result<Vec<OrganizationMember>, StoreError>;

    /// Inserts the invitation and queues its mail (ADR 0042), all in one transaction.
    /// It first revokes each pending invitation of the same email address and records an
    /// `InvitationReplace` event of the actor of `audit` for each one. Then it records `audit`.
    async fn invite(
        &self,
        scope: OrgScope,
        invitation: &Invitation,
        invited_by: UserId,
        audit: &AuditEvent,
    ) -> Result<InvitationInsert, StoreError>;

    /// The invitation `id` in any status.
    async fn invitation(
        &self,
        scope: OrgScope,
        id: InvitationId,
    ) -> Result<Option<Invitation>, StoreError>;

    /// The pending invitations, the oldest first.
    async fn pending_invitations(&self, scope: OrgScope) -> Result<Vec<Invitation>, StoreError>;

    /// Revokes the pending invitation `id` and deletes its tokens.
    /// Returns false if no pending invitation has this ID.
    async fn revoke(
        &self,
        scope: OrgScope,
        id: InvitationId,
        now: Timestamp,
        audit: &AuditEvent,
    ) -> Result<bool, StoreError>;

    /// Removes the organization membership of `member` with its event memberships, if
    /// `remover` allows it for the locked rows. The store adds the old role to `audit`.
    async fn remove(
        &self,
        scope: OrgScope,
        remover: Remover,
        member: UserId,
        expected_version: RecordVersion,
        audit: &AuditEvent,
    ) -> Result<Option<Refusal>, StoreError>;
}

#[derive(Debug, thiserror::Error)]
pub enum ListMembersError {
    #[error(transparent)]
    Store(#[from] StoreError),
}

impl ListMembersError {
    /// All codes that this query can return, for the API contract (ADR 0037).
    pub const CODES: &[ProblemCode] = &[ProblemCode::Unavailable, ProblemCode::Internal];
}

impl CommandError for ListMembersError {
    fn code(&self) -> ProblemCode {
        let Self::Store(error) = self;
        error.code()
    }

    fn store_error(&self) -> Option<&StoreError> {
        let Self::Store(error) = self;
        Some(error)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum InviteMemberError {
    #[error("the caller cannot invite with this role")]
    Forbidden,
    #[error("invalid values")]
    Invalid(Vec<FieldError>),
    #[error(transparent)]
    Store(#[from] StoreError),
}

impl InviteMemberError {
    /// All codes that this command can return, for the API contract (ADR 0037).
    pub const CODES: &[ProblemCode] = &[
        ProblemCode::Forbidden,
        ProblemCode::ValidationFailed,
        ProblemCode::Unavailable,
        ProblemCode::Internal,
    ];

    fn invalid(field: &'static str, code: &'static str) -> Self {
        Self::Invalid(vec![FieldError { field, code }])
    }
}

impl CommandError for InviteMemberError {
    fn code(&self) -> ProblemCode {
        match self {
            Self::Forbidden => ProblemCode::Forbidden,
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

/// The error of `list_invitations`.
#[derive(Debug, thiserror::Error)]
pub enum ListInvitationsError {
    #[error("only owners and admins see the invitations")]
    Forbidden,
    #[error(transparent)]
    Store(#[from] StoreError),
}

impl ListInvitationsError {
    /// All codes that this query can return, for the API contract (ADR 0037).
    pub const CODES: &[ProblemCode] = &[
        ProblemCode::Forbidden,
        ProblemCode::Unavailable,
        ProblemCode::Internal,
    ];
}

impl CommandError for ListInvitationsError {
    fn code(&self) -> ProblemCode {
        match self {
            Self::Forbidden => ProblemCode::Forbidden,
            Self::Store(error) => error.code(),
        }
    }

    fn store_error(&self) -> Option<&StoreError> {
        match self {
            Self::Store(error) => Some(error),
            Self::Forbidden => None,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum RevokeInvitationError {
    /// No pending invitation of the organization has this ID.
    #[error("the pending invitation does not exist")]
    NotFound,
    #[error("only owners and admins revoke invitations")]
    Forbidden,
    #[error(transparent)]
    Store(#[from] StoreError),
}

impl RevokeInvitationError {
    /// All codes that this command can return, for the API contract (ADR 0037).
    pub const CODES: &[ProblemCode] = &[
        ProblemCode::NotFound,
        ProblemCode::Forbidden,
        ProblemCode::Unavailable,
        ProblemCode::Internal,
    ];
}

impl CommandError for RevokeInvitationError {
    fn code(&self) -> ProblemCode {
        match self {
            Self::NotFound => ProblemCode::NotFound,
            Self::Forbidden => ProblemCode::Forbidden,
            Self::Store(error) => error.code(),
        }
    }

    fn store_error(&self) -> Option<&StoreError> {
        match self {
            Self::Store(error) => Some(error),
            _ => None,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum RemoveMemberError {
    #[error("the user is not a member of the organization")]
    NotFound,
    #[error("the caller cannot remove this member")]
    Forbidden,
    #[error("the membership changed after the caller read it")]
    VersionConflict,
    /// An organization has at least one owner (ADR 0056).
    #[error("the organization needs another owner first")]
    LastOwner,
    /// An event has at least one event manager (ADR 0052).
    #[error("an event of the member needs another event manager first")]
    LastManager,
    #[error(transparent)]
    Store(#[from] StoreError),
}

impl RemoveMemberError {
    /// All codes that this command can return, for the API contract (ADR 0037).
    pub const CODES: &[ProblemCode] = &[
        ProblemCode::NotFound,
        ProblemCode::Forbidden,
        ProblemCode::RecordVersionConflict,
        ProblemCode::InvalidTransition,
        ProblemCode::Unavailable,
        ProblemCode::Internal,
    ];
}

impl From<Refusal> for RemoveMemberError {
    fn from(refusal: Refusal) -> Self {
        match refusal {
            Refusal::NotFound => Self::NotFound,
            Refusal::Forbidden => Self::Forbidden,
            Refusal::VersionConflict => Self::VersionConflict,
            Refusal::LastOwner => Self::LastOwner,
            Refusal::LastManager => Self::LastManager,
        }
    }
}

impl CommandError for RemoveMemberError {
    fn code(&self) -> ProblemCode {
        match self {
            Self::NotFound => ProblemCode::NotFound,
            Self::Forbidden => ProblemCode::Forbidden,
            Self::VersionConflict => ProblemCode::RecordVersionConflict,
            Self::LastOwner | Self::LastManager => ProblemCode::InvalidTransition,
            Self::Store(error) => error.code(),
        }
    }

    fn store_error(&self) -> Option<&StoreError> {
        match self {
            Self::Store(error) => Some(error),
            _ => None,
        }
    }
}

/// Lists the members of the organization, in the order of their display names.
/// Each member can read the list. Only owners and admins see the email addresses.
pub async fn list_members(
    caller: &MemberCaller,
    after: Option<MemberCursor>,
    limit: PageLimit,
    store: &dyn MemberStore,
) -> Result<Page<OrganizationMember, MemberCursor>, ListMembersError> {
    // One more than the limit shows if a next page exists.
    let mut items = store.list(caller.scope(), after, limit.get() + 1).await?;
    let more = items.len() > limit.get() as usize;
    items.truncate(limit.get() as usize);
    if !caller.organization_role().is_owner_or_admin() {
        for member in &mut items {
            member.email = None;
        }
    }
    let next = more
        .then(|| items.last())
        .flatten()
        .map(|last| MemberCursor(last.user_id));
    Ok(Page { items, next })
}

/// Invites a person into the organization and queues the invitation mail (ADR 0056).
///
/// An owner invites with the role owner, admin or member; an admin with admin or member.
/// A new invitation replaces a pending invitation of the same email address.
pub async fn invite_member(
    caller: &MemberCaller,
    input: NewInvitation,
    store: &dyn MemberStore,
    clock: &dyn Clock,
) -> Result<Invited, InviteMemberError> {
    if !manages(caller.organization_role(), input.role) {
        return Err(InviteMemberError::Forbidden);
    }
    let scope = caller.scope();
    let invitation = validate(input, clock)?;
    let audit = AuditEvent::new(
        caller.actor(),
        AuditAction::InvitationCreate,
        Some(invitation.id.as_uuid()),
        Some(scope),
    )
    .with_roles(None, Some(AuditRole::Organization(invitation.role)));
    match store
        .invite(scope, &invitation, caller.user_id(), &audit)
        .await?
    {
        InvitationInsert::Inserted => Ok(Invited::New(invitation)),
        InvitationInsert::AlreadyMember => {
            Err(InviteMemberError::invalid("email", "already-member"))
        }
        InvitationInsert::IdTaken => match store.invitation(scope, invitation.id).await? {
            Some(existing) if existing.same_content(&invitation) => Ok(Invited::Existing(existing)),
            _ => Err(InviteMemberError::invalid("id", "taken")),
        },
    }
}

fn validate(input: NewInvitation, clock: &dyn Clock) -> Result<Invitation, InviteMemberError> {
    let mut errors = Vec::new();
    let mut error = |field, code| errors.push(FieldError { field, code });
    let id = match input.id {
        Some(id) if !ids::is_record_id(id) => {
            error("id", "not-uuid-v7");
            None
        }
        Some(id) => Some(InvitationId::from_uuid(id)),
        None => Some(InvitationId::from_uuid(Uuid::now_v7())),
    };
    let email = Email::parse(&input.email)
        .map_err(|e| {
            error(
                "email",
                match e {
                    EmailError::Shape => "shape",
                    EmailError::TooLong => "too-long",
                    EmailError::ControlCharacter => "control-character",
                },
            );
        })
        .ok();
    let display_name = DisplayName::parse(&input.display_name)
        .map_err(|e| {
            error(
                "display_name",
                match e {
                    DisplayNameError::Empty => "empty",
                    DisplayNameError::TooLong => "too-long",
                    DisplayNameError::ControlCharacter => "control-character",
                },
            );
        })
        .ok();
    match (id, email, display_name) {
        (Some(id), Some(email), Some(display_name)) => Ok(Invitation {
            id,
            email,
            display_name,
            role: input.role,
            created_at: clock.now(),
        }),
        _ => Err(InviteMemberError::Invalid(errors)),
    }
}

/// The pending invitations of the organization. Only owners and admins see them.
pub async fn list_invitations(
    caller: &MemberCaller,
    store: &dyn MemberStore,
) -> Result<Vec<Invitation>, ListInvitationsError> {
    if !caller.organization_role().is_owner_or_admin() {
        return Err(ListInvitationsError::Forbidden);
    }
    Ok(store.pending_invitations(caller.scope()).await?)
}

/// Revokes a pending invitation. Its links stop working at once.
pub async fn revoke_invitation(
    caller: &MemberCaller,
    id: InvitationId,
    store: &dyn MemberStore,
    clock: &dyn Clock,
) -> Result<(), RevokeInvitationError> {
    if !caller.organization_role().is_owner_or_admin() {
        return Err(RevokeInvitationError::Forbidden);
    }
    let audit = AuditEvent::new(
        caller.actor(),
        AuditAction::InvitationRevoke,
        Some(id.as_uuid()),
        Some(caller.scope()),
    );
    if store
        .revoke(caller.scope(), id, clock.now(), &audit)
        .await?
    {
        Ok(())
    } else {
        Err(RevokeInvitationError::NotFound)
    }
}

/// Removes the organization membership of `member` and its event memberships.
///
/// The sessions of the member stay: the authenticator clears their organization with the next
/// request, so the member loses access with the next request (ADR 0056).
pub async fn remove_member(
    caller: &MemberCaller,
    member: UserId,
    expected_version: RecordVersion,
    store: &dyn MemberStore,
) -> Result<(), RemoveMemberError> {
    let scope = caller.scope();
    // An organization membership has no ID of its own: the record ID is its organization.
    let audit = AuditEvent::new(
        caller.actor(),
        AuditAction::OrganizationMembershipRemove,
        Some(scope.organization_id().as_uuid()),
        Some(scope),
    )
    .about(member);
    match store
        .remove(scope, Remover::of(caller), member, expected_version, &audit)
        .await?
    {
        None => Ok(()),
        Some(refusal) => Err(refusal.into()),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use tada_domain::ids::OrganizationId;

    use super::*;

    use OrganizationRole::{Admin, Member, Owner};

    const NOW: Timestamp = Timestamp::constant(1_900_000_000, 0);

    #[derive(Debug)]
    struct FixedClock;

    impl Clock for FixedClock {
        fn now(&self) -> Timestamp {
            NOW
        }
    }

    fn user(n: u128) -> UserId {
        UserId::from_uuid(Uuid::from_u128(n))
    }

    fn caller(role: OrganizationRole) -> MemberCaller {
        MemberCaller::new(
            user(1),
            OrganizationId::from_uuid(Uuid::from_u128(100)),
            role,
        )
    }

    /// The locked membership with the role `target`, removed by a member with the role `remover`.
    fn locked(remover: OrganizationRole, target: OrganizationRole) -> LockedMembership {
        LockedMembership {
            role: target,
            version: RecordVersion::FIRST,
            owners: 2,
            only_manager: false,
            remover_role: Some(remover),
        }
    }

    /// The refusal of a removal of the member `2` with the role `target` by `remover`.
    fn removal(remover: OrganizationRole, target: OrganizationRole) -> Option<Refusal> {
        Remover::of(&caller(remover)).refusal(
            user(2),
            RecordVersion::FIRST,
            &locked(remover, target),
        )
    }

    #[test]
    fn owners_and_admins_remove_members_up_to_their_own_role() {
        let roles = [Owner, Admin, Member];
        let allowed = |remover, target| removal(remover, target).is_none();
        for target in roles {
            assert!(allowed(Owner, target), "owner removes {target:?}");
            assert!(!allowed(Member, target), "member removes {target:?}");
        }
        assert!(allowed(Admin, Admin) && allowed(Admin, Member));
        assert_eq!(removal(Admin, Owner), Some(Refusal::Forbidden));
    }

    #[test]
    fn each_member_can_leave_but_the_last_owner_cannot() {
        for role in [Owner, Admin, Member] {
            let remover = Remover::of(&caller(role));
            assert_eq!(
                remover.refusal(user(1), RecordVersion::FIRST, &locked(role, role)),
                None,
                "{role:?} leaves"
            );
        }
        let last = LockedMembership {
            owners: 1,
            ..locked(Owner, Owner)
        };
        let owner = Remover::of(&caller(Owner));
        assert_eq!(
            owner.refusal(user(1), RecordVersion::FIRST, &last),
            Some(Refusal::LastOwner)
        );
        assert_eq!(
            owner.refusal(user(2), RecordVersion::FIRST, &last),
            Some(Refusal::LastOwner)
        );
        // The last owner rule is about owners only.
        let admin = LockedMembership {
            owners: 1,
            ..locked(Owner, Admin)
        };
        assert_eq!(owner.refusal(user(2), RecordVersion::FIRST, &admin), None);
    }

    #[test]
    fn a_removal_checks_the_version_and_the_event_managers() {
        let owner = Remover::of(&caller(Owner));
        let second = RecordVersion::new(2).unwrap();
        assert_eq!(
            owner.refusal(user(2), second, &locked(Owner, Member)),
            Some(Refusal::VersionConflict)
        );
        let manager = LockedMembership {
            only_manager: true,
            ..locked(Owner, Member)
        };
        assert_eq!(
            owner.refusal(user(2), RecordVersion::FIRST, &manager),
            Some(Refusal::LastManager)
        );
        // A forbidden removal does not tell the version or the events of the member.
        let admin = Remover::of(&caller(Admin));
        let owner_target = LockedMembership {
            only_manager: true,
            ..locked(Admin, Owner)
        };
        assert_eq!(
            admin.refusal(user(2), second, &owner_target),
            Some(Refusal::Forbidden)
        );
    }

    /// The locked role of the remover decides, not the role of the session.
    #[test]
    fn a_demoted_or_removed_remover_cannot_remove() {
        let admin = Remover::of(&caller(Admin));
        let demoted = locked(Member, Member);
        assert_eq!(
            admin.refusal(user(2), RecordVersion::FIRST, &demoted),
            Some(Refusal::Forbidden)
        );
        let gone = LockedMembership {
            remover_role: None,
            ..locked(Admin, Member)
        };
        assert_eq!(
            admin.refusal(user(2), RecordVersion::FIRST, &gone),
            Some(Refusal::Forbidden)
        );
    }

    /// Records the calls of the commands; `invite` answers with `insert`.
    #[derive(Debug)]
    struct MemoryStore {
        insert: InvitationInsert,
        existing: Option<Invitation>,
        members: Vec<OrganizationMember>,
        invited: Mutex<Vec<(Invitation, UserId, AuditEvent)>>,
    }

    impl MemoryStore {
        fn answering(insert: InvitationInsert) -> Self {
            Self {
                insert,
                existing: None,
                members: Vec::new(),
                invited: Mutex::default(),
            }
        }
    }

    #[async_trait]
    impl MemberStore for MemoryStore {
        async fn list(
            &self,
            _: OrgScope,
            _: Option<MemberCursor>,
            limit: u32,
        ) -> Result<Vec<OrganizationMember>, StoreError> {
            Ok(self.members.iter().take(limit as usize).cloned().collect())
        }

        async fn invite(
            &self,
            _: OrgScope,
            invitation: &Invitation,
            invited_by: UserId,
            audit: &AuditEvent,
        ) -> Result<InvitationInsert, StoreError> {
            self.invited
                .lock()
                .unwrap()
                .push((invitation.clone(), invited_by, audit.clone()));
            Ok(self.insert)
        }

        async fn invitation(
            &self,
            _: OrgScope,
            _: InvitationId,
        ) -> Result<Option<Invitation>, StoreError> {
            Ok(self.existing.clone())
        }

        async fn pending_invitations(&self, _: OrgScope) -> Result<Vec<Invitation>, StoreError> {
            unreachable!()
        }

        async fn revoke(
            &self,
            _: OrgScope,
            _: InvitationId,
            _: Timestamp,
            _: &AuditEvent,
        ) -> Result<bool, StoreError> {
            unreachable!()
        }

        async fn remove(
            &self,
            _: OrgScope,
            _: Remover,
            _: UserId,
            _: RecordVersion,
            _: &AuditEvent,
        ) -> Result<Option<Refusal>, StoreError> {
            unreachable!()
        }
    }

    fn input(role: OrganizationRole) -> NewInvitation {
        NewInvitation {
            id: None,
            email: " Anna@Example.org ".into(),
            display_name: "Anna Muster".into(),
            role,
        }
    }

    async fn invite(
        role: OrganizationRole,
        invited: OrganizationRole,
    ) -> Result<Invited, InviteMemberError> {
        let store = MemoryStore::answering(InvitationInsert::Inserted);
        invite_member(&caller(role), input(invited), &store, &FixedClock).await
    }

    #[tokio::test]
    async fn owners_and_admins_invite_up_to_their_own_role() {
        for invited in [Owner, Admin, Member] {
            assert!(
                invite(Owner, invited).await.is_ok(),
                "owner gives {invited:?}"
            );
            let member = invite(Member, invited).await;
            assert!(
                matches!(member, Err(InviteMemberError::Forbidden)),
                "member gives {invited:?}"
            );
        }
        assert!(invite(Admin, Admin).await.is_ok());
        assert!(invite(Admin, Member).await.is_ok());
        assert!(matches!(
            invite(Admin, Owner).await,
            Err(InviteMemberError::Forbidden)
        ));
    }

    #[tokio::test]
    async fn an_invitation_is_normalized_and_audited_with_its_role() {
        let store = MemoryStore::answering(InvitationInsert::Inserted);
        let Invited::New(invitation) =
            invite_member(&caller(Admin), input(Member), &store, &FixedClock)
                .await
                .unwrap()
        else {
            panic!("not new");
        };
        assert_eq!(invitation.email.as_str(), "anna@example.org");
        assert_eq!(invitation.created_at, NOW);
        let (stored, invited_by, audit) = store.invited.lock().unwrap().pop().unwrap();
        assert_eq!(stored, invitation);
        assert_eq!(invited_by, user(1));
        assert_eq!(audit.action(), AuditAction::InvitationCreate);
        assert_eq!(audit.record_id(), Some(invitation.id.as_uuid()));
        assert_eq!(
            audit.roles().unwrap().new,
            Some(AuditRole::Organization(Member))
        );
    }

    #[tokio::test]
    async fn rejects_invalid_values_and_an_existing_member() {
        let store = MemoryStore::answering(InvitationInsert::Inserted);
        let invalid = NewInvitation {
            id: Some(Uuid::from_u128(5)),
            email: "anna".into(),
            display_name: " ".into(),
            role: Member,
        };
        let Err(error) = invite_member(&caller(Owner), invalid, &store, &FixedClock).await else {
            panic!("accepted");
        };
        let fields: Vec<_> = error
            .field_errors()
            .iter()
            .map(|e| (e.field, e.code))
            .collect();
        assert_eq!(
            fields,
            [
                ("id", "not-uuid-v7"),
                ("email", "shape"),
                ("display_name", "empty")
            ]
        );
        assert!(store.invited.lock().unwrap().is_empty());

        let store = MemoryStore::answering(InvitationInsert::AlreadyMember);
        let Err(error) = invite_member(&caller(Owner), input(Member), &store, &FixedClock).await
        else {
            panic!("accepted");
        };
        assert_eq!(error.code(), ProblemCode::ValidationFailed);
        assert_eq!(
            error.field_errors(),
            [FieldError {
                field: "email",
                code: "already-member"
            }]
        );
    }

    #[tokio::test]
    async fn a_retry_with_the_same_id_and_content_returns_the_invitation() {
        let id = Uuid::now_v7();
        let first = invite(Owner, Member).await.unwrap();
        let Invited::New(mut existing) = first else {
            panic!("not new");
        };
        existing.id = InvitationId::from_uuid(id);
        let store = MemoryStore {
            existing: Some(existing.clone()),
            ..MemoryStore::answering(InvitationInsert::IdTaken)
        };
        let retry = NewInvitation {
            id: Some(id),
            ..input(Member)
        };
        assert_eq!(
            invite_member(&caller(Owner), retry.clone(), &store, &FixedClock)
                .await
                .unwrap(),
            Invited::Existing(existing)
        );
        let other = NewInvitation {
            role: Admin,
            ..retry
        };
        let Err(error) = invite_member(&caller(Owner), other, &store, &FixedClock).await else {
            panic!("accepted");
        };
        assert_eq!(
            error.field_errors(),
            [FieldError {
                field: "id",
                code: "taken"
            }]
        );
    }

    #[tokio::test]
    async fn only_owners_and_admins_see_email_addresses() {
        let member = |n| OrganizationMember {
            user_id: user(n),
            display_name: DisplayName::parse("Anna Muster").unwrap(),
            email: Some(Email::parse("anna@example.org").unwrap()),
            role: Member,
            version: RecordVersion::FIRST,
        };
        let store = MemoryStore {
            members: vec![member(2), member(3)],
            ..MemoryStore::answering(InvitationInsert::Inserted)
        };
        let limit = PageLimit::new(1).unwrap();
        let page = list_members(&caller(Admin), None, limit, &store)
            .await
            .unwrap();
        assert!(page.items[0].email.is_some());
        assert_eq!(page.next, Some(MemberCursor(user(2))));
        let page = list_members(&caller(Member), None, PageLimit::DEFAULT, &store)
            .await
            .unwrap();
        assert!(page.items.iter().all(|m| m.email.is_none()));
        assert_eq!(page.next, None);
    }

    #[tokio::test]
    async fn a_member_cannot_list_or_revoke_invitations() {
        let store = MemoryStore::answering(InvitationInsert::Inserted);
        assert!(matches!(
            list_invitations(&caller(Member), &store).await,
            Err(ListInvitationsError::Forbidden)
        ));
        let id = InvitationId::from_uuid(Uuid::now_v7());
        assert!(matches!(
            revoke_invitation(&caller(Member), id, &store, &FixedClock).await,
            Err(RevokeInvitationError::Forbidden)
        ));
    }

    #[test]
    fn each_error_gives_a_code_of_its_list() {
        let stores = || {
            [
                StoreError::Internal("test".into()),
                StoreError::Unavailable("test".into()),
            ]
        };
        for error in stores().map(ListMembersError::Store) {
            assert!(ListMembersError::CODES.contains(&error.code()));
        }
        let invite = [
            InviteMemberError::Forbidden,
            InviteMemberError::Invalid(Vec::new()),
        ]
        .into_iter()
        .chain(stores().map(InviteMemberError::Store));
        for error in invite {
            assert!(
                InviteMemberError::CODES.contains(&error.code()),
                "{error:?}"
            );
        }
        let list = [ListInvitationsError::Forbidden]
            .into_iter()
            .chain(stores().map(ListInvitationsError::Store));
        for error in list {
            assert!(
                ListInvitationsError::CODES.contains(&error.code()),
                "{error:?}"
            );
        }
        let revoke = [
            RevokeInvitationError::NotFound,
            RevokeInvitationError::Forbidden,
        ]
        .into_iter()
        .chain(stores().map(RevokeInvitationError::Store));
        for error in revoke {
            assert!(
                RevokeInvitationError::CODES.contains(&error.code()),
                "{error:?}"
            );
        }
        let remove = [
            Refusal::NotFound,
            Refusal::Forbidden,
            Refusal::VersionConflict,
            Refusal::LastOwner,
            Refusal::LastManager,
        ]
        .map(RemoveMemberError::from)
        .into_iter()
        .chain(stores().map(RemoveMemberError::Store));
        for error in remove {
            assert!(
                RemoveMemberError::CODES.contains(&error.code()),
                "{error:?}"
            );
        }
        assert_eq!(
            RemoveMemberError::LastOwner.code(),
            ProblemCode::InvalidTransition
        );
    }
}
