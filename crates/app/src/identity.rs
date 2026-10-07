//! The port for users and memberships (ADR 0008, ADR 0052).

use async_trait::async_trait;
use tada_domain::identity::{DisplayName, Email, EventRole, OrganizationRole};
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

#[async_trait]
pub trait IdentityStore: Send + Sync {
    /// Finds the user of an email address. This is an infrastructure query without a scope:
    /// sign-in has no organization yet, and one address belongs to one user (ADR 0056).
    async fn user_by_email(&self, email: &Email) -> Result<Option<UserRef>, StoreError>;

    /// All organizations of a user, with the role in each. This query has no scope for the same reason.
    async fn memberships_of(
        &self,
        user: UserId,
    ) -> Result<Vec<(OrganizationId, OrganizationRole)>, StoreError>;

    /// The role of a user in the organization of `scope`, or `None` for a non-member.
    async fn membership(
        &self,
        scope: OrgScope,
        user: UserId,
    ) -> Result<Option<OrganizationRole>, StoreError>;

    /// The event role of a user in one event, or `None` if the user has none.
    async fn event_role(
        &self,
        scope: OrgScope,
        event: EventId,
        user: UserId,
    ) -> Result<Option<EventRole>, StoreError>;
}
