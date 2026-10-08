//! The audit log (ADR 0039): who did what to which record. It holds no personal data and no free text.

use tada_domain::identity::{EventRole, OrganizationRole};
use tada_domain::ids::{InvitationId, OrganizationId, UserId};
use uuid::Uuid;

use crate::caller::{Actor, Bootstrap, Channel, OrgScope, ServiceCaller};

/// What an audit event records. The set is closed, so each action has one spelling.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuditAction {
    OrganizationCreate,
    InvitationCreate,
    InvitationRevoke,
    InvitationAccept,
    /// A new invitation of the same email address revoked a pending invitation (ADR 0008).
    InvitationReplace,
    OrganizationMembershipRemove,
    EventCreate,
    EventMembershipAdd,
    EventMembershipChangeRole,
    EventMembershipRemove,
    ChangesetCreate,
    ProposalAccept,
    ProposalReject,
    /// An apply found that the target of the proposal changed after the proposal (ADR 0050).
    ProposalConflict,
}

impl AuditAction {
    /// The name in the `action` column of the audit log.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::OrganizationCreate => "organization.create",
            Self::InvitationCreate => "invitation.create",
            Self::InvitationRevoke => "invitation.revoke",
            Self::InvitationAccept => "invitation.accept",
            Self::InvitationReplace => "invitation.replace",
            Self::OrganizationMembershipRemove => "organization_membership.remove",
            Self::EventCreate => "event.create",
            Self::EventMembershipAdd => "event_membership.add",
            Self::EventMembershipChangeRole => "event_membership.change_role",
            Self::EventMembershipRemove => "event_membership.remove",
            Self::ChangesetCreate => "changeset.create",
            Self::ProposalAccept => "proposal.accept",
            Self::ProposalReject => "proposal.reject",
            Self::ProposalConflict => "proposal.conflict",
        }
    }

    /// The kind of the record that `record_id` identifies.
    pub fn record_kind(self) -> &'static str {
        match self {
            Self::OrganizationCreate => "organization",
            Self::InvitationCreate
            | Self::InvitationRevoke
            | Self::InvitationAccept
            | Self::InvitationReplace => "invitation",
            // An organization membership has no ID of its own: the record ID is its organization,
            // and the subject is its member.
            Self::OrganizationMembershipRemove => "organization_membership",
            Self::EventCreate => "event",
            // An event membership has no ID of its own: the record ID is its event,
            // and the subject is its member.
            Self::EventMembershipAdd
            | Self::EventMembershipChangeRole
            | Self::EventMembershipRemove => "event_membership",
            Self::ChangesetCreate => "changeset",
            Self::ProposalAccept | Self::ProposalReject | Self::ProposalConflict => "proposal",
        }
    }
}

/// A role name in an audit event. Only role names can enter the detail of an event (ADR 0061).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuditRole {
    Organization(OrganizationRole),
    Event(EventRole),
}

impl AuditRole {
    /// The kebab-case name of the role.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Organization(role) => role.as_str(),
            Self::Event(role) => role.as_str(),
        }
    }
}

/// The role of the subject before and after the change. At least one of the two is set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RoleChange {
    pub old: Option<AuditRole>,
    pub new: Option<AuditRole>,
}

/// One entry of the audit log. A command records it in its own transaction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditEvent {
    actor: Actor,
    action: AuditAction,
    record_id: Option<Uuid>,
    organization_id: Option<OrganizationId>,
    subject: Option<UserId>,
    roles: Option<RoleChange>,
}

impl AuditEvent {
    /// `scope` is the organization of the record, if it has one. Only a caller gives a scope,
    /// so an audit event cannot name an organization that its caller has no scope for.
    /// The two exceptions are `by_bootstrap` and `by_invitee`.
    pub fn new(
        actor: Actor,
        action: AuditAction,
        record_id: Option<Uuid>,
        scope: Option<OrgScope>,
    ) -> Self {
        Self {
            actor,
            action,
            record_id,
            organization_id: scope.map(OrgScope::organization_id),
            subject: None,
            roles: None,
        }
    }

    /// An event of `bootstrap`, the only service identity without an organization (ADR 0039).
    /// Its store finds the organization by slug inside its transaction, so the event names the
    /// organization by ID. This gives the store the right to write an audit row as `bootstrap`,
    /// not a scope for other reads or writes.
    pub fn by_bootstrap(
        caller: &ServiceCaller<Bootstrap>,
        action: AuditAction,
        record_id: Uuid,
        organization_id: OrganizationId,
    ) -> Self {
        Self::new(
            caller.actor(),
            action,
            Some(record_id),
            Some(caller.scope(organization_id)),
        )
    }

    /// The acceptance of an invitation through the web (ADR 0056). The invitee has no session and
    /// no caller yet: the invitation token is the credential, and the store finds or creates the
    /// user inside its transaction. So the event names the new member as actor and the organization
    /// of the invitation by ID. This gives the store the right to write this one audit row, not a
    /// scope for other reads or writes.
    ///
    /// The invitee is also the subject. The event records the role change only if the membership is
    /// new or its role changed: `existing` is the role before, `accepted` the role after.
    pub fn by_invitee(
        user_id: UserId,
        organization_id: OrganizationId,
        invitation_id: InvitationId,
        request_id: Option<Uuid>,
        existing: Option<OrganizationRole>,
        accepted: OrganizationRole,
    ) -> Self {
        let event = Self {
            actor: Actor::member(user_id, Channel::Web, request_id),
            action: AuditAction::InvitationAccept,
            record_id: Some(invitation_id.as_uuid()),
            organization_id: Some(organization_id),
            subject: None,
            roles: None,
        }
        .about(user_id);
        if existing == Some(accepted) {
            event
        } else {
            event.with_roles(
                existing.map(AuditRole::Organization),
                Some(AuditRole::Organization(accepted)),
            )
        }
    }

    /// An event of the same actor in the same organization about another record, without a
    /// subject and without roles. A store uses it for the records that it finds in its transaction.
    #[must_use]
    pub fn for_record(&self, action: AuditAction, record_id: Uuid) -> Self {
        Self {
            actor: self.actor,
            action,
            record_id: Some(record_id),
            organization_id: self.organization_id,
            subject: None,
            roles: None,
        }
    }

    /// The member whom the event is about.
    #[must_use]
    pub fn about(self, subject: UserId) -> Self {
        Self {
            subject: Some(subject),
            ..self
        }
    }

    /// The role of the subject before and after the change. Two `None` values record no change.
    /// Neither changes the actor, the organization or the subject.
    #[must_use]
    pub fn with_roles(self, old: Option<AuditRole>, new: Option<AuditRole>) -> Self {
        let roles = (old.is_some() || new.is_some()).then_some(RoleChange { old, new });
        Self { roles, ..self }
    }

    pub fn subject(&self) -> Option<UserId> {
        self.subject
    }

    pub fn roles(&self) -> Option<RoleChange> {
        self.roles
    }

    pub fn actor(&self) -> &Actor {
        &self.actor
    }

    pub fn action(&self) -> AuditAction {
        self.action
    }

    pub fn record_kind(&self) -> &'static str {
        self.action.record_kind()
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
        let event = AuditEvent::by_invitee(
            user,
            organization,
            invitation,
            Some(request),
            Some(OrganizationRole::Member),
            OrganizationRole::Admin,
        );
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
                AuditAction::InvitationAccept,
                "invitation",
                Some(invitation.as_uuid())
            )
        );
        assert_eq!(event.organization_id(), Some(organization));
        assert_eq!(event.subject(), Some(user));
        assert_eq!(
            event.roles(),
            Some(RoleChange {
                old: Some(AuditRole::Organization(OrganizationRole::Member)),
                new: Some(AuditRole::Organization(OrganizationRole::Admin)),
            })
        );
    }

    #[test]
    fn an_acceptance_without_a_role_change_records_no_roles() {
        let id = |n| Uuid::from_u128(n);
        let event = AuditEvent::by_invitee(
            UserId::from_uuid(id(1)),
            OrganizationId::from_uuid(id(2)),
            InvitationId::from_uuid(id(3)),
            None,
            Some(OrganizationRole::Owner),
            OrganizationRole::Owner,
        );
        assert_eq!(event.roles(), None);
        let new = AuditEvent::by_invitee(
            UserId::from_uuid(id(1)),
            OrganizationId::from_uuid(id(2)),
            InvitationId::from_uuid(id(3)),
            None,
            None,
            OrganizationRole::Member,
        );
        assert_eq!(
            new.roles(),
            Some(RoleChange {
                old: None,
                new: Some(AuditRole::Organization(OrganizationRole::Member)),
            })
        );
    }
}
