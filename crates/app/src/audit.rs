//! The audit log (ADR 0039): who did what to which record. It holds no personal data and no free text.

use tada_domain::ids::{InvitationId, OrganizationId, UserId};
use uuid::Uuid;

use crate::caller::{Actor, Bootstrap, Channel, OrgScope, ServiceCaller};

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
    /// The two exceptions are `by_bootstrap` and `by_invitee`.
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

    /// The acceptance of an invitation through the web (ADR 0056). The invitee has no session and
    /// no caller yet: the invitation token is the credential, and the store finds or creates the
    /// user inside its transaction. So the event names the new member as actor and the organization
    /// of the invitation by ID. This gives the store the right to write this one audit row, not a
    /// scope for other reads or writes.
    pub fn by_invitee(
        user_id: UserId,
        organization_id: OrganizationId,
        invitation_id: InvitationId,
        request_id: Option<Uuid>,
    ) -> Self {
        Self {
            actor: Actor::member(user_id, Channel::Web, request_id),
            action: "invitation.accept",
            record_kind: "invitation",
            record_id: Some(invitation_id.as_uuid()),
            organization_id: Some(organization_id),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::caller::ActorKind;

    #[test]
    fn an_acceptance_names_the_invitee_the_invitation_and_its_organization() {
        let user = UserId::from_uuid(Uuid::from_u128(1));
        let organization = OrganizationId::from_uuid(Uuid::from_u128(2));
        let invitation = InvitationId::from_uuid(Uuid::from_u128(3));
        let request = Uuid::from_u128(4);
        let event = AuditEvent::by_invitee(user, organization, invitation, Some(request));
        let actor = event.actor();
        assert_eq!(
            (actor.kind(), actor.id(), actor.principal()),
            (ActorKind::Member, user.as_uuid(), None)
        );
        assert_eq!(
            (actor.channel(), actor.request_id()),
            (Channel::Web, Some(request))
        );
        assert_eq!(
            (event.action(), event.record_kind(), event.record_id()),
            (
                "invitation.accept",
                "invitation",
                Some(invitation.as_uuid())
            )
        );
        assert_eq!(event.organization_id(), Some(organization));
    }
}
