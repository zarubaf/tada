//! The audit log (ADR 0039): who did what to which record. It holds no personal data and no free text.

use tada_domain::ids::OrganizationId;
use uuid::Uuid;

use crate::caller::{Actor, OrgScope};

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
