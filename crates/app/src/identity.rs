//! The port for users and memberships (ADR 0008, ADR 0052).

use std::fmt::Debug;

use async_trait::async_trait;
use tada_domain::identity::{DisplayName, EventRole, OrganizationRole};
use tada_domain::ids::{EventId, OrganizationId, UserId};

use crate::caller::OrgScope;
use crate::store::StoreError;

/// A user as the sign-in flow needs it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserRef {
    pub id: UserId,
    pub display_name: DisplayName,
    pub locale: String,
}

/// The membership of a user in one organization.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Membership {
    pub organization_id: OrganizationId,
    pub organization_name: String,
    pub role: OrganizationRole,
}

#[async_trait]
pub trait IdentityStore: Debug + Send + Sync {
    /// Infrastructure query (ADR 0039): a session names a user, not an organization.
    async fn user(&self, id: UserId) -> Result<Option<UserRef>, StoreError>;

    /// All memberships of a user, in the order of the organization names.
    /// Infrastructure query (ADR 0039): it lists the organizations of one user.
    async fn memberships_of(&self, user: UserId) -> Result<Vec<Membership>, StoreError>;

    /// The role of a user in the organization of `scope`, or `None` for a non-member.
    async fn membership(
        &self,
        scope: OrgScope,
        user: UserId,
    ) -> Result<Option<OrganizationRole>, StoreError>;

    /// True if the event `event` is in the organization of `scope`.
    async fn event_exists(&self, scope: OrgScope, event: EventId) -> Result<bool, StoreError>;

    /// The event role of a user in one event, or `None` if the user has none.
    async fn event_role(
        &self,
        scope: OrgScope,
        event: EventId,
        user: UserId,
    ) -> Result<Option<EventRole>, StoreError>;
}
