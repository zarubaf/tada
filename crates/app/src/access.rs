//! Event access (ADR 0052): the only place that decides what a caller can do in an event.

use tada_domain::identity::{EventRole, OrganizationRole};
use tada_domain::ids::{EventId, UserId};

use crate::caller::OrgScope;
use crate::identity::IdentityStore;
use crate::problem::{CommandError, ProblemCode};
use crate::store::StoreError;

/// A caller that acts for a member: the member itself, or an AI client of the member.
/// An AI caller never has more rights than its principal (ADR 0052), so access uses the member.
pub trait Principal {
    /// The member for whom the caller acts.
    fn user_id(&self) -> UserId;
    /// The organization that all reads and writes of the caller stay in.
    fn scope(&self) -> OrgScope;
    /// The organization role of the member.
    fn organization_role(&self) -> OrganizationRole;
}

/// What a caller can do in one event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventAccess {
    Manager,
    Contributor,
    Viewer,
}

impl EventAccess {
    pub fn can_read(self) -> bool {
        true
    }

    pub fn can_propose(self) -> bool {
        matches!(self, Self::Manager | Self::Contributor)
    }

    pub fn can_review(self) -> bool {
        self == Self::Manager
    }

    pub fn can_manage_members(self) -> bool {
        self == Self::Manager
    }

    pub fn can_approve_documents(self) -> bool {
        self == Self::Manager
    }
}

impl From<EventRole> for EventAccess {
    fn from(role: EventRole) -> Self {
        match role {
            EventRole::EventManager => Self::Manager,
            EventRole::EventContributor => Self::Contributor,
            EventRole::EventViewer => Self::Viewer,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum AccessError {
    /// The event is not in the caller's organization, or the caller has no event role in it.
    /// The two cases look the same, so a caller cannot find out which events exist.
    #[error("the event does not exist or the caller cannot see it")]
    NotFound,
    #[error(transparent)]
    Store(#[from] StoreError),
}

impl AccessError {
    /// All codes that the access check can return, for the API contract (ADR 0037).
    pub const CODES: &[ProblemCode] = &[
        ProblemCode::NotFound,
        ProblemCode::Unavailable,
        ProblemCode::Internal,
    ];
}

impl CommandError for AccessError {
    fn code(&self) -> ProblemCode {
        match self {
            Self::NotFound => ProblemCode::NotFound,
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

/// True if the caller acts as event manager in each event of its organization (ADR 0052).
pub(crate) fn sees_all_events(caller: &impl Principal) -> bool {
    manages_all_events(caller.organization_role())
}

/// The one statement of the rule of ADR 0052: owners and admins act as event manager in each event of their organization.
fn manages_all_events(role: OrganizationRole) -> bool {
    role.is_owner_or_admin()
}

/// The access that the organization role `role` and the event role of `user` give in the event `event`.
/// It does not check that the event exists.
async fn role_access(
    scope: OrgScope,
    event: EventId,
    user: UserId,
    role: OrganizationRole,
    identity: &dyn IdentityStore,
) -> Result<Option<EventAccess>, StoreError> {
    if manages_all_events(role) {
        return Ok(Some(EventAccess::Manager));
    }
    Ok(identity
        .event_role(scope, event, user)
        .await?
        .map(EventAccess::from))
}

/// The access of `caller` to the event `event_id` at the time of the call (ADR 0039, ADR 0052).
///
/// Owners and admins get `Manager` in each event of their organization.
/// Other members get the access of their event role. Without an event role, the event is not found.
pub async fn event_access(
    caller: &impl Principal,
    event_id: EventId,
    identity: &dyn IdentityStore,
) -> Result<EventAccess, AccessError> {
    let scope = caller.scope();
    if sees_all_events(caller) && !identity.event_exists(scope, event_id).await? {
        return Err(AccessError::NotFound);
    }
    role_access(
        scope,
        event_id,
        caller.user_id(),
        caller.organization_role(),
        identity,
    )
    .await?
    .ok_or(AccessError::NotFound)
}

/// The access of the member `user`, who is not the caller, in the event `event`, or `None` if the user has none.
/// It also serves an event that a changeset creates and that does not exist yet: there, only owners and admins have access.
/// For example, the owner of a work record must be a member of its event (ADR 0052).
pub async fn member_access(
    scope: OrgScope,
    event: EventId,
    user: UserId,
    identity: &dyn IdentityStore,
) -> Result<Option<EventAccess>, StoreError> {
    match identity.membership(scope, user).await? {
        None => Ok(None),
        Some(role) => role_access(scope, event, user, role, identity).await,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::Mutex;

    use async_trait::async_trait;
    use tada_domain::ids::OrganizationId;
    use uuid::Uuid;

    use super::*;
    use crate::caller::MemberCaller;
    use crate::identity::{Membership, UserRef};

    /// The events of each organization and the event roles of each user.
    #[derive(Debug, Default)]
    struct MemoryIdentity {
        events: Vec<(OrganizationId, EventId)>,
        roles: Mutex<HashMap<(EventId, UserId), EventRole>>,
        /// The organization role of each member of Testwil.
        members: Mutex<HashMap<UserId, OrganizationRole>>,
    }

    #[async_trait]
    impl IdentityStore for MemoryIdentity {
        async fn user(&self, _: UserId) -> Result<Option<UserRef>, StoreError> {
            unreachable!()
        }

        async fn memberships_of(&self, _: UserId) -> Result<Vec<Membership>, StoreError> {
            unreachable!()
        }

        async fn membership(
            &self,
            scope: OrgScope,
            user: UserId,
        ) -> Result<Option<OrganizationRole>, StoreError> {
            if scope.organization_id() != testwil() {
                return Ok(None);
            }
            Ok(self.members.lock().unwrap().get(&user).copied())
        }

        async fn event_exists(&self, scope: OrgScope, event: EventId) -> Result<bool, StoreError> {
            Ok(self.events.contains(&(scope.organization_id(), event)))
        }

        async fn event_role(
            &self,
            scope: OrgScope,
            event: EventId,
            user: UserId,
        ) -> Result<Option<EventRole>, StoreError> {
            if !self.events.contains(&(scope.organization_id(), event)) {
                return Ok(None);
            }
            Ok(self.roles.lock().unwrap().get(&(event, user)).copied())
        }
    }

    fn testwil() -> OrganizationId {
        OrganizationId::from_uuid(Uuid::from_u128(10))
    }

    fn musterhausen() -> OrganizationId {
        OrganizationId::from_uuid(Uuid::from_u128(11))
    }

    fn open_day() -> EventId {
        EventId::from_uuid(Uuid::from_u128(20))
    }

    fn anna() -> UserId {
        UserId::from_uuid(Uuid::from_u128(1))
    }

    fn identity() -> MemoryIdentity {
        MemoryIdentity {
            events: vec![(testwil(), open_day())],
            roles: Mutex::default(),
            members: Mutex::default(),
        }
    }

    fn caller(organization: OrganizationId, role: OrganizationRole) -> MemberCaller {
        MemberCaller::new(anna(), organization, role)
    }

    #[tokio::test]
    async fn owners_and_admins_manage_each_event_of_their_organization() {
        let identity = identity();
        for role in [OrganizationRole::Owner, OrganizationRole::Admin] {
            let access = event_access(&caller(testwil(), role), open_day(), &identity).await;
            assert_eq!(access.unwrap(), EventAccess::Manager);
        }
    }

    #[tokio::test]
    async fn an_owner_does_not_find_an_event_of_another_organization() {
        let identity = identity();
        let owner = caller(musterhausen(), OrganizationRole::Owner);
        let access = event_access(&owner, open_day(), &identity).await;
        assert!(matches!(access, Err(AccessError::NotFound)));
    }

    #[tokio::test]
    async fn a_member_gets_the_access_of_the_event_role() {
        let identity = identity();
        let member = caller(testwil(), OrganizationRole::Member);
        for (role, expected) in [
            (EventRole::EventManager, EventAccess::Manager),
            (EventRole::EventContributor, EventAccess::Contributor),
            (EventRole::EventViewer, EventAccess::Viewer),
        ] {
            identity
                .roles
                .lock()
                .unwrap()
                .insert((open_day(), anna()), role);
            let access = event_access(&member, open_day(), &identity).await;
            assert_eq!(access.unwrap(), expected);
        }
    }

    #[tokio::test]
    async fn a_member_without_an_event_role_does_not_find_the_event() {
        let identity = identity();
        let member = caller(testwil(), OrganizationRole::Member);
        let access = event_access(&member, open_day(), &identity).await;
        assert!(matches!(access, Err(AccessError::NotFound)));
    }

    #[tokio::test]
    async fn an_event_role_gives_no_access_with_a_caller_of_another_organization() {
        let identity = identity();
        identity
            .roles
            .lock()
            .unwrap()
            .insert((open_day(), anna()), EventRole::EventManager);
        let member = caller(musterhausen(), OrganizationRole::Member);
        let access = event_access(&member, open_day(), &identity).await;
        assert!(matches!(access, Err(AccessError::NotFound)));
    }

    #[tokio::test]
    async fn the_access_of_another_member_follows_the_same_rule() {
        let identity = identity();
        let scope = caller(testwil(), OrganizationRole::Member).scope();
        let bruno = UserId::from_uuid(Uuid::from_u128(2));
        let new_event = EventId::from_uuid(Uuid::from_u128(21));
        let access = |event, user| member_access(scope, event, user, &identity);
        assert_eq!(
            access(open_day(), anna()).await.unwrap(),
            None,
            "no membership"
        );

        identity
            .members
            .lock()
            .unwrap()
            .insert(anna(), OrganizationRole::Member);
        assert_eq!(
            access(open_day(), anna()).await.unwrap(),
            None,
            "no event role"
        );
        identity
            .roles
            .lock()
            .unwrap()
            .insert((open_day(), anna()), EventRole::EventViewer);
        assert_eq!(
            access(open_day(), anna()).await.unwrap(),
            Some(EventAccess::Viewer)
        );
        assert_eq!(access(new_event, anna()).await.unwrap(), None);

        // An admin manages each event, also one that a changeset creates and that does not exist yet.
        identity
            .members
            .lock()
            .unwrap()
            .insert(bruno, OrganizationRole::Admin);
        for event in [open_day(), new_event] {
            assert_eq!(
                access(event, bruno).await.unwrap(),
                Some(EventAccess::Manager)
            );
        }
        let elsewhere = caller(musterhausen(), OrganizationRole::Member).scope();
        let other = member_access(elsewhere, open_day(), bruno, &identity).await;
        assert_eq!(other.unwrap(), None);
    }

    #[test]
    fn each_access_has_the_rights_of_its_event_role() {
        let rights = |access: EventAccess| {
            [
                access.can_read(),
                access.can_propose(),
                access.can_review(),
                access.can_manage_members(),
                access.can_approve_documents(),
            ]
        };
        assert_eq!(rights(EventAccess::Manager), [true; 5]);
        assert_eq!(
            rights(EventAccess::Contributor),
            [true, true, false, false, false]
        );
        assert_eq!(
            rights(EventAccess::Viewer),
            [true, false, false, false, false]
        );
    }

    #[test]
    fn each_error_gives_a_code_of_its_list() {
        for error in [
            AccessError::NotFound,
            AccessError::Store(StoreError::Internal("test".into())),
            AccessError::Store(StoreError::Unavailable("test".into())),
        ] {
            assert!(AccessError::CODES.contains(&error.code()), "{error:?}");
        }
    }
}
