//! Event access (ADR 0052): the only place that decides what a caller can do in an event.

use tada_domain::identity::{EventRole, OrganizationRole};
use tada_domain::ids::{EventId, UserId};

use crate::caller::OrgScope;
use crate::identity::IdentityStore;
use crate::problem::ProblemCode;
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

    pub fn code(&self) -> ProblemCode {
        match self {
            Self::NotFound => ProblemCode::NotFound,
            Self::Store(error) => error.code(),
        }
    }
}

/// True if the caller acts as event manager in each event of its organization (ADR 0052).
pub(crate) fn sees_all_events(caller: &impl Principal) -> bool {
    caller.organization_role().is_owner_or_admin()
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
    if sees_all_events(caller) {
        return if identity.event_exists(scope, event_id).await? {
            Ok(EventAccess::Manager)
        } else {
            Err(AccessError::NotFound)
        };
    }
    identity
        .event_role(scope, event_id, caller.user_id())
        .await?
        .map(EventAccess::from)
        .ok_or(AccessError::NotFound)
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::Mutex;

    use async_trait::async_trait;
    use tada_domain::identity::Email;
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
    }

    #[async_trait]
    impl IdentityStore for MemoryIdentity {
        async fn user(&self, _: UserId) -> Result<Option<UserRef>, StoreError> {
            unreachable!()
        }

        async fn user_by_email(&self, _: &Email) -> Result<Option<UserRef>, StoreError> {
            unreachable!()
        }

        async fn memberships_of(&self, _: UserId) -> Result<Vec<Membership>, StoreError> {
            unreachable!()
        }

        async fn membership(
            &self,
            _: OrgScope,
            _: UserId,
        ) -> Result<Option<OrganizationRole>, StoreError> {
            unreachable!()
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
