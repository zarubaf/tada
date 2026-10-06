//! Callers (ADR 0039): the typed values that commands and queries take for authorization.

use std::marker::PhantomData;

use tada_domain::ids::{OrganizationId, UserId};

/// The organization role of a member (glossary).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrganizationRole {
    Owner,
    Admin,
    Member,
}

/// A signed-in member. Only an `Authenticator` creates it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemberCaller {
    user_id: UserId,
    organization_id: OrganizationId,
    role: OrganizationRole,
}

impl MemberCaller {
    /// For `Authenticator` adapters and tests only. Other code gets a caller from an authenticator.
    pub fn new(user_id: UserId, organization_id: OrganizationId, role: OrganizationRole) -> Self {
        Self {
            user_id,
            organization_id,
            role,
        }
    }

    pub fn user_id(&self) -> UserId {
        self.user_id
    }

    /// The organization that all reads and writes of this caller stay in.
    pub fn scope(&self) -> OrgScope {
        OrgScope(self.organization_id)
    }

    /// Owners and admins can act as event manager in each event of the organization (ADR 0052).
    pub(crate) fn is_owner_or_admin(&self) -> bool {
        matches!(self.role, OrganizationRole::Owner | OrganizationRole::Admin)
    }
}

/// The organization boundary of a repository call (ADR 0006). Only a caller can give one,
/// so a repository method that takes it cannot run without a scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OrgScope(OrganizationId);

impl OrgScope {
    pub fn organization_id(self) -> OrganizationId {
        self.0
    }
}

/// A service identity of tada (ADR 0039). Each identity is its own type, so the set of commands
/// that it can call is fixed in code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ServiceCaller<S>(PhantomData<S>);

impl<S> ServiceCaller<S> {
    /// For the process role of the identity only, for example `tada telegram`.
    pub fn new() -> Self {
        Self(PhantomData)
    }
}

impl<S> Default for ServiceCaller<S> {
    fn default() -> Self {
        Self::new()
    }
}

/// The service identity `telegram-gateway`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TelegramGateway;
