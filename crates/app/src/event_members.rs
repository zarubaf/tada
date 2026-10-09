//! Event memberships (ADR 0052): the event managers give, change and remove event roles.

use std::fmt::Debug;

use async_trait::async_trait;
use jiff::Timestamp;
use tada_domain::RecordVersion;
use tada_domain::identity::{DisplayName, EventRole};
use tada_domain::ids::{EventId, UserId};

use crate::access::{self, AccessError};
use crate::audit::{AuditAction, AuditEvent, AuditRole};
use crate::caller::{MemberCaller, OrgScope};
use crate::clock::Clock;
use crate::identity::IdentityStore;
use crate::problem::{CommandError, FieldError, ProblemCode};
use crate::store::StoreError;

/// The event role of one member in one event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventMember {
    pub user_id: UserId,
    pub display_name: DisplayName,
    pub event_role: EventRole,
    pub version: RecordVersion,
    pub created_at: Timestamp,
}

/// The result of `EventMemberStore::add`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Added {
    Added(EventMember),
    /// The user is not a member of the organization.
    NotMember,
    /// The user has an event role in this event.
    Taken,
}

/// The result of `EventMemberStore::change_role` and `EventMemberStore::remove`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Changed<T> {
    Changed(T),
    /// The user has no event role in this event.
    NotFound,
    /// The membership has another version.
    VersionConflict,
    /// The change would demote or remove the only event manager of the event (ADR 0052).
    LastManager,
}

/// The repository port for event memberships. Each method stays inside `scope`.
/// Each change records its audit event in the same transaction.
#[async_trait]
pub trait EventMemberStore: Debug + Send + Sync {
    /// The members of the event with an event role, in the order of their display names.
    async fn list(&self, scope: OrgScope, event: EventId) -> Result<Vec<EventMember>, StoreError>;

    async fn add(
        &self,
        scope: OrgScope,
        event: EventId,
        user: UserId,
        role: EventRole,
        now: Timestamp,
        audit: &AuditEvent,
    ) -> Result<Added, StoreError>;

    async fn change_role(
        &self,
        scope: OrgScope,
        event: EventId,
        user: UserId,
        role: EventRole,
        expected_version: RecordVersion,
        audit: &AuditEvent,
    ) -> Result<Changed<EventMember>, StoreError>;

    async fn remove(
        &self,
        scope: OrgScope,
        event: EventId,
        user: UserId,
        expected_version: RecordVersion,
        audit: &AuditEvent,
    ) -> Result<Changed<()>, StoreError>;
}

#[derive(Debug, thiserror::Error)]
pub enum ListEventMembersError {
    #[error("the event does not exist or the caller cannot see it")]
    NotFound,
    #[error(transparent)]
    Store(#[from] StoreError),
}

impl ListEventMembersError {
    /// All codes that this query can return, for the API contract (ADR 0037).
    pub const CODES: &[ProblemCode] = &[
        ProblemCode::NotFound,
        ProblemCode::Unavailable,
        ProblemCode::Internal,
    ];
}

impl CommandError for ListEventMembersError {
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

#[derive(Debug, thiserror::Error)]
pub enum AddEventMemberError {
    #[error("the event does not exist or the caller cannot see it")]
    NotFound,
    #[error("only event managers give event roles")]
    Forbidden,
    #[error("invalid values")]
    Invalid(Vec<FieldError>),
    #[error(transparent)]
    Store(#[from] StoreError),
}

impl AddEventMemberError {
    /// All codes that this command can return, for the API contract (ADR 0037).
    pub const CODES: &[ProblemCode] = &[
        ProblemCode::NotFound,
        ProblemCode::Forbidden,
        ProblemCode::ValidationFailed,
        ProblemCode::Unavailable,
        ProblemCode::Internal,
    ];
}

impl CommandError for AddEventMemberError {
    fn code(&self) -> ProblemCode {
        match self {
            Self::NotFound => ProblemCode::NotFound,
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

/// The error of `change_event_role` and `remove_event_member`.
#[derive(Debug, thiserror::Error)]
pub enum ChangeEventMemberError {
    /// The event or the event membership does not exist, or the caller cannot see it.
    #[error("the event membership does not exist or the caller cannot see it")]
    NotFound,
    #[error("only event managers change event roles")]
    Forbidden,
    #[error("the event membership changed after the caller read it")]
    VersionConflict,
    /// An event has at least one event manager (ADR 0052).
    #[error("the event needs another event manager first")]
    LastManager,
    #[error(transparent)]
    Store(#[from] StoreError),
}

impl ChangeEventMemberError {
    /// All codes that these commands can return, for the API contract (ADR 0037).
    pub const CODES: &[ProblemCode] = &[
        ProblemCode::NotFound,
        ProblemCode::Forbidden,
        ProblemCode::RecordVersionConflict,
        ProblemCode::InvalidTransition,
        ProblemCode::Unavailable,
        ProblemCode::Internal,
    ];
}

impl CommandError for ChangeEventMemberError {
    fn code(&self) -> ProblemCode {
        match self {
            Self::NotFound => ProblemCode::NotFound,
            Self::Forbidden => ProblemCode::Forbidden,
            Self::VersionConflict => ProblemCode::RecordVersionConflict,
            Self::LastManager => ProblemCode::InvalidTransition,
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

/// The access check of each command here: the caller must manage the event memberships.
/// The result is `None` if the event is not found, and `Some(false)` if the caller cannot manage it.
async fn can_manage_members(
    caller: &MemberCaller,
    event: EventId,
    identity: &dyn IdentityStore,
) -> Result<Option<bool>, StoreError> {
    match access::event_access(caller, event, identity).await {
        Ok(access) => Ok(Some(access.can_manage_members())),
        Err(AccessError::NotFound) => Ok(None),
        Err(AccessError::Store(error)) => Err(error),
    }
}

/// The audit event of a change of the event membership of `user`. The store adds the old role.
fn audit(caller: &MemberCaller, action: AuditAction, event: EventId, user: UserId) -> AuditEvent {
    AuditEvent::new(
        caller.actor(),
        action,
        Some(event.as_uuid()),
        Some(caller.scope()),
    )
    .about(user)
}

/// True if the change takes away the only event manager of an event (ADR 0052).
///
/// `role` is the current event role of the member, `new` the role after the change (`None` is a
/// removal), and `managers` the number of event manager rows of the event, the member included.
/// Owners and admins act as event manager, but they do not count. The stores call this rule on the
/// rows that they locked.
pub fn takes_last_manager(role: EventRole, new: Option<EventRole>, managers: usize) -> bool {
    role == EventRole::EventManager && new != Some(EventRole::EventManager) && managers == 1
}

/// The event memberships of an event: names and roles. Each member who can see the event reads
/// them, so that a contributor can give work to another member. Only managers change them.
pub async fn list_event_members(
    caller: &MemberCaller,
    event: EventId,
    identity: &dyn IdentityStore,
    store: &dyn EventMemberStore,
) -> Result<Vec<EventMember>, ListEventMembersError> {
    match access::event_access(caller, event, identity).await {
        Ok(_) => Ok(store.list(caller.scope(), event).await?),
        Err(AccessError::NotFound) => Err(ListEventMembersError::NotFound),
        Err(AccessError::Store(error)) => Err(error.into()),
    }
}

/// Gives a member of the organization an event role in the event.
pub async fn add_event_member(
    caller: &MemberCaller,
    event: EventId,
    user: UserId,
    role: EventRole,
    identity: &dyn IdentityStore,
    store: &dyn EventMemberStore,
    clock: &dyn Clock,
) -> Result<EventMember, AddEventMemberError> {
    match can_manage_members(caller, event, identity).await? {
        None => return Err(AddEventMemberError::NotFound),
        Some(false) => return Err(AddEventMemberError::Forbidden),
        Some(true) => {}
    }
    let audit = audit(caller, AuditAction::EventMembershipAdd, event, user)
        .with_roles(None, Some(AuditRole::Event(role)));
    let invalid = |code| AddEventMemberError::Invalid(vec![FieldError::new("user_id", code)]);
    match store
        .add(caller.scope(), event, user, role, clock.now(), &audit)
        .await?
    {
        Added::Added(member) => Ok(member),
        Added::NotMember => Err(invalid("not-a-member")),
        Added::Taken => Err(invalid("taken")),
    }
}

/// Changes the event role of a member in the event.
pub async fn change_event_role(
    caller: &MemberCaller,
    event: EventId,
    user: UserId,
    role: EventRole,
    expected_version: RecordVersion,
    identity: &dyn IdentityStore,
    store: &dyn EventMemberStore,
) -> Result<EventMember, ChangeEventMemberError> {
    check_change(caller, event, identity).await?;
    let audit = audit(caller, AuditAction::EventMembershipChangeRole, event, user);
    let changed = store
        .change_role(caller.scope(), event, user, role, expected_version, &audit)
        .await?;
    changed_or_error(changed)
}

/// Removes the event role of a member in the event. The member loses access with the next request.
pub async fn remove_event_member(
    caller: &MemberCaller,
    event: EventId,
    user: UserId,
    expected_version: RecordVersion,
    identity: &dyn IdentityStore,
    store: &dyn EventMemberStore,
) -> Result<(), ChangeEventMemberError> {
    check_change(caller, event, identity).await?;
    let audit = audit(caller, AuditAction::EventMembershipRemove, event, user);
    let removed = store
        .remove(caller.scope(), event, user, expected_version, &audit)
        .await?;
    changed_or_error(removed)
}

async fn check_change(
    caller: &MemberCaller,
    event: EventId,
    identity: &dyn IdentityStore,
) -> Result<(), ChangeEventMemberError> {
    match can_manage_members(caller, event, identity).await? {
        None => Err(ChangeEventMemberError::NotFound),
        Some(false) => Err(ChangeEventMemberError::Forbidden),
        Some(true) => Ok(()),
    }
}

fn changed_or_error<T>(changed: Changed<T>) -> Result<T, ChangeEventMemberError> {
    match changed {
        Changed::Changed(value) => Ok(value),
        Changed::NotFound => Err(ChangeEventMemberError::NotFound),
        Changed::VersionConflict => Err(ChangeEventMemberError::VersionConflict),
        Changed::LastManager => Err(ChangeEventMemberError::LastManager),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_change_of_the_only_event_manager_takes_the_last_manager() {
        let manager = EventRole::EventManager;
        assert!(!takes_last_manager(manager, Some(manager), 1));
        assert!(takes_last_manager(
            manager,
            Some(EventRole::EventContributor),
            1
        ));
        assert!(takes_last_manager(manager, None, 1));
        assert!(!takes_last_manager(manager, None, 2));
        assert!(!takes_last_manager(EventRole::EventContributor, None, 1));
    }

    #[test]
    fn each_error_gives_a_code_of_its_list() {
        let stores = || {
            [
                StoreError::Internal("test".into()),
                StoreError::Unavailable("test".into()),
            ]
        };
        let list = [ListEventMembersError::NotFound]
            .into_iter()
            .chain(stores().map(ListEventMembersError::Store));
        for error in list {
            assert!(
                ListEventMembersError::CODES.contains(&error.code()),
                "{error:?}"
            );
        }
        let add = [
            AddEventMemberError::NotFound,
            AddEventMemberError::Forbidden,
            AddEventMemberError::Invalid(Vec::new()),
        ]
        .into_iter()
        .chain(stores().map(AddEventMemberError::Store));
        for error in add {
            assert!(
                AddEventMemberError::CODES.contains(&error.code()),
                "{error:?}"
            );
        }
        let change = [
            ChangeEventMemberError::NotFound,
            ChangeEventMemberError::Forbidden,
            ChangeEventMemberError::VersionConflict,
            ChangeEventMemberError::LastManager,
        ]
        .into_iter()
        .chain(stores().map(ChangeEventMemberError::Store));
        for error in change {
            assert!(
                ChangeEventMemberError::CODES.contains(&error.code()),
                "{error:?}"
            );
        }
    }
}
