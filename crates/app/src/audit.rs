//! The audit log (ADR 0039): who did what to which record. It holds no personal data and no free text.

use tada_domain::ids::OrganizationId;
use uuid::Uuid;

use crate::caller::{Actor, Bootstrap, OrgScope, ServiceCaller};

/// One entry of the audit log. A command records it in its own transaction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditEvent {
    actor: Actor,
    action: &'static str,
    record_kind: &'static str,
    record_id: Option<Uuid>,
    organization_id: Option<OrganizationId>,
}

impl AuditEvent {
    /// `scope` is the organization of the record, if it has one. Only a caller gives a scope,
    /// so an audit event cannot name an organization that its caller has no scope for.
    /// The one exception is `by_bootstrap`.
    pub fn new(
        actor: Actor,
        action: &'static str,
        record_kind: &'static str,
        record_id: Option<Uuid>,
        scope: Option<OrgScope>,
    ) -> Self {
        Self {
            actor,
            action,
            record_kind,
            record_id,
            organization_id: scope.map(OrgScope::organization_id),
        }
    }

    /// An event of `bootstrap`, the only service identity without an organization (ADR 0039).
    /// Its store finds the organization by slug inside its transaction, so the event names the
    /// organization by ID. This gives the store the right to write an audit row as `bootstrap`,
    /// not a scope for other reads or writes.
    pub fn by_bootstrap(
        caller: &ServiceCaller<Bootstrap>,
        action: &'static str,
        record_kind: &'static str,
        record_id: Uuid,
        organization_id: OrganizationId,
    ) -> Self {
        Self::new(
            caller.actor(),
            action,
            record_kind,
            Some(record_id),
            Some(caller.scope(organization_id)),
        )
    }

    pub fn actor(&self) -> &Actor {
        &self.actor
    }

    pub fn action(&self) -> &'static str {
        self.action
    }

    pub fn record_kind(&self) -> &'static str {
        self.record_kind
    }

    pub fn record_id(&self) -> Option<Uuid> {
        self.record_id
    }

    pub fn organization_id(&self) -> Option<OrganizationId> {
        self.organization_id
    }
}
