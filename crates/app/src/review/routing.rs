//! Review routing (ADR 0067): who reviews each proposal.
//!
//! tada computes the reviewers at each read and each apply from the current state: the current owner of a record
//! and the current lead of a workstream. It stores no reviewers, so a replaced lead or a member who lost the
//! event role stops seeing and applying the proposals at once.
//!
//! The decision is pure. `RoutingFacts::load` reads the current state that it needs through the ports.

use std::collections::{BTreeSet, HashMap, HashSet};

use tada_domain::ids::{ActionId, CommitmentId, EventId, ProposalId, UserId, WorkstreamId};
use tada_domain::proposals::Operation;

use super::selection::closure;
use crate::access::EventAccess;
use crate::caller::OrgScope;
use crate::identity::IdentityStore;
use crate::store::StoreError;
use crate::work::{WorkStore, is_possible_owner};
use crate::workstreams::WorkstreamStore;

/// The reviewers of one proposal by rules 1 to 4 of ADR 0067.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reviewers {
    /// The owner of the record that the proposal changes, or the lead of the workstream of a new record.
    Users(BTreeSet<UserId>),
    /// The event managers of the event of the changeset.
    EventManagers,
    /// The owners and admins of the organization: a changeset of the organization, for example a new event.
    OrganizationAdmins,
}

impl Reviewers {
    fn user(user: UserId) -> Self {
        Self::Users(BTreeSet::from([user]))
    }

    /// True if the proposal goes to `user` as a member, not as a manager.
    fn names(&self, user: UserId) -> bool {
        matches!(self, Self::Users(users) if users.contains(&user))
    }

    /// True if the proposal goes to members, not only to the managers.
    fn has_members(&self) -> bool {
        matches!(self, Self::Users(users) if !users.is_empty())
    }
}

/// The current state that the routing reads: the owner of each action and commitment and the lead of each
/// workstream of the event. `None` means that the event has no such record.
pub trait RoutingLookup {
    fn action_owner(&self, id: ActionId) -> Option<UserId>;
    fn commitment_owner(&self, id: CommitmentId) -> Option<UserId>;
    fn workstream_lead(&self, id: WorkstreamId) -> Option<UserId>;
}

/// The reviewers of a proposal with `operation` in a changeset of the event `event` (ADR 0067, rules 1 to 4).
/// A changeset of the organization (`None`) goes to the owners and admins.
/// A record or a workstream that the event does not have goes to the event managers: the apply refuses it anyway.
pub fn reviewers(
    operation: &Operation,
    event: Option<EventId>,
    lookup: &impl RoutingLookup,
) -> Reviewers {
    if event.is_none() {
        return Reviewers::OrganizationAdmins;
    }
    let user = match operation {
        Operation::ChangeActionStatus { action_id, .. }
        | Operation::ChangeActionDue { action_id, .. } => lookup.action_owner(*action_id),
        Operation::ChangeCommitmentStatus { commitment_id, .. } => {
            lookup.commitment_owner(*commitment_id)
        }
        Operation::CreateAction { workstream, .. }
        | Operation::CreateCommitment { workstream, .. } => {
            workstream.and_then(|id| lookup.workstream_lead(id))
        }
        _ => None,
    };
    user.map_or(Reviewers::EventManagers, Reviewers::user)
}

/// True if the proposal goes to the caller: the caller is a member reviewer of the proposal or of a proposal that
/// depends on it (rule 5), or the caller is an event manager and the proposal goes to no member.
/// A member reviewer must still have the contributor or manager role (rule 8).
pub fn routed_to(
    caller: UserId,
    access: EventAccess,
    own: &Reviewers,
    dependents: &[&Reviewers],
) -> bool {
    let all = || std::iter::once(own).chain(dependents.iter().copied());
    if access.can_propose() && all().any(|reviewers| reviewers.names(caller)) {
        return true;
    }
    access.can_review() && !all().any(Reviewers::has_members)
}

/// True if the caller can accept or reject the proposal: it goes to the caller (`routed_to`),
/// or the caller is an event manager, who keeps the right to review each proposal of the event (rule 6).
pub fn may_review(
    caller: UserId,
    access: EventAccess,
    own: &Reviewers,
    dependents: &[&Reviewers],
) -> bool {
    access.can_review() || routed_to(caller, access, own, dependents)
}

/// True if the Review Inbox of the caller shows the open proposal: it goes to the caller, or the caller is an event
/// manager and the proposal is overdue (rule 6).
pub fn in_inbox_of(
    caller: UserId,
    access: EventAccess,
    own: &Reviewers,
    dependents: &[&Reviewers],
    overdue: bool,
) -> bool {
    routed_to(caller, access, own, dependents) || (access.can_review() && overdue)
}

/// One proposal of a changeset as the routing sees it.
#[derive(Debug, Clone, Copy)]
pub struct RoutedProposal<'a> {
    pub id: ProposalId,
    pub operation: &'a Operation,
    pub depends_on: &'a [ProposalId],
    /// Only an open proposal can still be selected, so only an open proposal gives its reviewers to its dependencies.
    pub open: bool,
}

/// The reviewers of each proposal of one changeset, and the open proposals that depend on each (rule 5).
#[derive(Debug, Clone)]
pub struct ChangesetRoutes {
    own: HashMap<ProposalId, Reviewers>,
    /// The open proposals that depend on each proposal, directly or through other open proposals.
    dependents: HashMap<ProposalId, Vec<ProposalId>>,
}

impl ChangesetRoutes {
    pub fn new<'a>(
        proposals: impl IntoIterator<Item = RoutedProposal<'a>>,
        event: Option<EventId>,
        lookup: &impl RoutingLookup,
    ) -> Self {
        let proposals: Vec<RoutedProposal<'a>> = proposals.into_iter().collect();
        let own = proposals
            .iter()
            .map(|proposal| (proposal.id, reviewers(proposal.operation, event, lookup)))
            .collect();
        let mut direct: HashMap<ProposalId, Vec<ProposalId>> = HashMap::new();
        for proposal in proposals.iter().filter(|proposal| proposal.open) {
            for dependency in proposal.depends_on {
                direct.entry(*dependency).or_default().push(proposal.id);
            }
        }
        let dependents = proposals
            .iter()
            .map(|proposal| {
                let start = HashSet::from([proposal.id]);
                let mut all: Vec<ProposalId> =
                    closure(&start, |id| direct.get(&id).cloned().unwrap_or_default())
                        .into_iter()
                        .filter(|id| *id != proposal.id)
                        .collect();
                all.sort();
                (proposal.id, all)
            })
            .collect();
        Self { own, dependents }
    }

    /// The reviewers of the proposal and of its open dependents. A proposal of another changeset has none.
    fn of(&self, id: ProposalId) -> Option<(&Reviewers, Vec<&Reviewers>)> {
        let own = self.own.get(&id)?;
        let dependents = self
            .dependents
            .get(&id)
            .into_iter()
            .flatten()
            .filter_map(|dependent| self.own.get(dependent))
            .collect();
        Some((own, dependents))
    }

    pub fn routed_to(&self, caller: UserId, access: EventAccess, id: ProposalId) -> bool {
        self.of(id)
            .is_some_and(|(own, dependents)| routed_to(caller, access, own, &dependents))
    }

    pub fn may_review(&self, caller: UserId, access: EventAccess, id: ProposalId) -> bool {
        self.of(id)
            .is_some_and(|(own, dependents)| may_review(caller, access, own, &dependents))
    }

    pub fn in_inbox_of(
        &self,
        caller: UserId,
        access: EventAccess,
        id: ProposalId,
        overdue: bool,
    ) -> bool {
        self.of(id)
            .is_some_and(|(own, dependents)| in_inbox_of(caller, access, own, &dependents, overdue))
    }
}

/// The current owners and leads that the routing of some proposals of one event reads.
#[derive(Debug, Clone, Default)]
pub(crate) struct RoutingFacts {
    action_owners: HashMap<ActionId, UserId>,
    commitment_owners: HashMap<CommitmentId, UserId>,
    leads: HashMap<WorkstreamId, UserId>,
}

impl RoutingFacts {
    /// Reads the owners of the records that `operations` change and, if a new record names a workstream,
    /// the leads of the workstreams of `event`: at most two queries, and the access of each owner and lead.
    /// An owner or a lead without the contributor or manager role in the event now is no reviewer (rule 8),
    /// so the proposal goes to the event managers at once.
    pub(crate) async fn load<'a>(
        scope: OrgScope,
        event: EventId,
        operations: impl IntoIterator<Item = &'a Operation>,
        ports: RoutingPorts<'_>,
    ) -> Result<Self, StoreError> {
        let mut actions = Vec::new();
        let mut commitments = Vec::new();
        let mut needs_leads = false;
        for operation in operations {
            match operation {
                Operation::ChangeActionStatus { action_id, .. }
                | Operation::ChangeActionDue { action_id, .. } => actions.push(*action_id),
                Operation::ChangeCommitmentStatus { commitment_id, .. } => {
                    commitments.push(*commitment_id);
                }
                Operation::CreateAction { workstream, .. }
                | Operation::CreateCommitment { workstream, .. } => {
                    needs_leads |= workstream.is_some();
                }
                _ => {}
            }
        }
        let mut facts = Self::default();
        if !actions.is_empty() || !commitments.is_empty() {
            let owners = ports
                .work
                .owners(scope, event, &actions, &commitments)
                .await?;
            facts.action_owners = owners.actions.into_iter().collect();
            facts.commitment_owners = owners.commitments.into_iter().collect();
        }
        if needs_leads {
            facts.leads = ports
                .workstreams
                .list(scope, event)
                .await?
                .into_iter()
                .map(|workstream| (workstream.id, workstream.lead))
                .collect();
        }
        facts.keep_reviewers(scope, event, ports.identity).await?;
        Ok(facts)
    }

    /// Drops each owner and lead who cannot review in the event now: one access check for each user.
    async fn keep_reviewers(
        &mut self,
        scope: OrgScope,
        event: EventId,
        identity: &dyn IdentityStore,
    ) -> Result<(), StoreError> {
        let users: BTreeSet<UserId> = self
            .action_owners
            .values()
            .chain(self.commitment_owners.values())
            .chain(self.leads.values())
            .copied()
            .collect();
        let mut reviewers = HashSet::new();
        for user in users {
            if is_possible_owner(scope, event, user, identity).await? {
                reviewers.insert(user);
            }
        }
        self.action_owners
            .retain(|_, user| reviewers.contains(user));
        self.commitment_owners
            .retain(|_, user| reviewers.contains(user));
        self.leads.retain(|_, user| reviewers.contains(user));
        Ok(())
    }
}

/// The ports that `RoutingFacts::load` reads.
#[derive(Debug, Clone, Copy)]
pub(crate) struct RoutingPorts<'a> {
    pub(crate) identity: &'a dyn IdentityStore,
    pub(crate) work: &'a dyn WorkStore,
    pub(crate) workstreams: &'a dyn WorkstreamStore,
}

impl RoutingLookup for RoutingFacts {
    fn action_owner(&self, id: ActionId) -> Option<UserId> {
        self.action_owners.get(&id).copied()
    }

    fn commitment_owner(&self, id: CommitmentId) -> Option<UserId> {
        self.commitment_owners.get(&id).copied()
    }

    fn workstream_lead(&self, id: WorkstreamId) -> Option<UserId> {
        self.leads.get(&id).copied()
    }
}

#[cfg(test)]
mod tests {
    use tada_domain::RecordVersion;
    use tada_domain::ids::{FieldDefinitionId, PersonId};
    use tada_domain::parties::{Party, PartyName};
    use tada_domain::work::{ActionStatus, ActionTitle, CommitmentText};
    use uuid::Uuid;

    use super::*;

    fn user(n: u128) -> UserId {
        UserId::from_uuid(Uuid::from_u128(n))
    }

    fn proposal_id(n: u128) -> ProposalId {
        ProposalId::from_uuid(Uuid::from_u128(n))
    }

    fn event() -> EventId {
        EventId::from_uuid(Uuid::from_u128(20))
    }

    fn action() -> ActionId {
        ActionId::from_uuid(Uuid::from_u128(30))
    }

    fn workstream() -> WorkstreamId {
        WorkstreamId::from_uuid(Uuid::from_u128(40))
    }

    const OWNER: u128 = 1;
    const LEAD: u128 = 2;
    const MANAGER: u128 = 3;
    const OTHER: u128 = 4;

    /// The action `action()` with the owner `OWNER` and the workstream `workstream()` with the lead `LEAD`.
    fn facts() -> RoutingFacts {
        RoutingFacts {
            action_owners: HashMap::from([(action(), user(OWNER))]),
            commitment_owners: HashMap::new(),
            leads: HashMap::from([(workstream(), user(LEAD))]),
        }
    }

    fn change_action() -> Operation {
        Operation::ChangeActionStatus {
            event_id: event(),
            action_id: action(),
            status: ActionStatus::InProgress,
            expected_version: RecordVersion::new(1).unwrap(),
        }
    }

    fn new_action(workstream: Option<WorkstreamId>) -> Operation {
        Operation::CreateAction {
            id: ActionId::from_uuid(Uuid::from_u128(31)),
            event_id: event(),
            title: ActionTitle::parse("Generator bestellen").unwrap(),
            description: None,
            owner: user(OTHER),
            workstream,
            due_date: None,
        }
    }

    fn new_commitment(person: PersonId, workstream: Option<WorkstreamId>) -> Operation {
        Operation::CreateCommitment {
            id: CommitmentId::from_uuid(Uuid::from_u128(32)),
            event_id: event(),
            text: CommitmentText::parse("Liefert den Generator").unwrap(),
            promisor: Party::Person(person),
            owner: user(OTHER),
            workstream,
            due_date: None,
            condition: None,
        }
    }

    fn new_person(person: PersonId) -> Operation {
        Operation::CreatePerson {
            id: person,
            name: PartyName::parse("Moritz Muster").unwrap(),
            email: None,
            phone: None,
        }
    }

    fn deprecation() -> Operation {
        Operation::DeprecateField {
            event_id: event(),
            field_id: FieldDefinitionId::from_uuid(Uuid::from_u128(50)),
        }
    }

    #[test]
    fn each_operation_goes_to_its_reviewers() {
        let facts = facts();
        let owner = Reviewers::user(user(OWNER));
        let lead = Reviewers::user(user(LEAD));
        assert_eq!(reviewers(&change_action(), Some(event()), &facts), owner);
        assert_eq!(
            reviewers(&new_action(Some(workstream())), Some(event()), &facts),
            lead
        );
        assert_eq!(
            reviewers(&new_action(None), Some(event()), &facts),
            Reviewers::EventManagers
        );
        assert_eq!(
            reviewers(&deprecation(), Some(event()), &facts),
            Reviewers::EventManagers
        );
        assert_eq!(
            reviewers(&new_action(Some(workstream())), None, &facts),
            Reviewers::OrganizationAdmins
        );
        // A record or a workstream that the event does not have goes to the managers.
        let unknown = WorkstreamId::from_uuid(Uuid::from_u128(41));
        assert_eq!(
            reviewers(&new_action(Some(unknown)), Some(event()), &facts),
            Reviewers::EventManagers
        );
        assert_eq!(
            reviewers(&change_action(), Some(event()), &RoutingFacts::default()),
            Reviewers::EventManagers
        );
    }

    #[test]
    fn a_member_reviewer_needs_the_contributor_or_manager_role() {
        let owner = Reviewers::user(user(OWNER));
        assert!(routed_to(
            user(OWNER),
            EventAccess::Contributor,
            &owner,
            &[]
        ));
        assert!(may_review(
            user(OWNER),
            EventAccess::Contributor,
            &owner,
            &[]
        ));
        assert!(!routed_to(user(OWNER), EventAccess::Viewer, &owner, &[]));
        assert!(!may_review(user(OWNER), EventAccess::Viewer, &owner, &[]));
        assert!(!may_review(
            user(OTHER),
            EventAccess::Contributor,
            &owner,
            &[]
        ));
    }

    #[test]
    fn a_manager_reviews_each_proposal_but_sees_only_unrouted_or_overdue_ones() {
        let lead = Reviewers::user(user(LEAD));
        let manager = (user(MANAGER), EventAccess::Manager);
        assert!(may_review(manager.0, manager.1, &lead, &[]));
        assert!(!routed_to(manager.0, manager.1, &lead, &[]));
        assert!(!in_inbox_of(manager.0, manager.1, &lead, &[], false));
        assert!(in_inbox_of(manager.0, manager.1, &lead, &[], true));
        for reviewers in [Reviewers::EventManagers, Reviewers::OrganizationAdmins] {
            assert!(in_inbox_of(manager.0, manager.1, &reviewers, &[], false));
            assert!(!in_inbox_of(
                user(OTHER),
                EventAccess::Contributor,
                &reviewers,
                &[],
                true
            ));
        }
        // A manager who is the lead gets the proposal as the lead.
        assert!(routed_to(user(LEAD), EventAccess::Manager, &lead, &[]));
    }

    #[test]
    fn a_dependency_goes_to_the_reviewers_of_its_open_dependents() {
        let person = PersonId::from_uuid(Uuid::from_u128(60));
        let supplier = new_person(person);
        let promise = new_commitment(person, Some(workstream()));
        let follow_up = new_action(None);
        let (s, p, f) = (proposal_id(1), proposal_id(2), proposal_id(3));
        let depends_on_supplier = [s];
        let depends_on_promise = [p];
        let routes = |promise_open: bool| {
            ChangesetRoutes::new(
                [
                    RoutedProposal {
                        id: s,
                        operation: &supplier,
                        depends_on: &[],
                        open: true,
                    },
                    RoutedProposal {
                        id: p,
                        operation: &promise,
                        depends_on: &depends_on_supplier,
                        open: promise_open,
                    },
                    RoutedProposal {
                        id: f,
                        operation: &follow_up,
                        depends_on: &depends_on_promise,
                        open: true,
                    },
                ],
                Some(event()),
                &facts(),
            )
        };
        let open = routes(true);
        let lead = (user(LEAD), EventAccess::Contributor);
        assert!(open.routed_to(lead.0, lead.1, s));
        assert!(open.may_review(lead.0, lead.1, p));
        assert!(!open.may_review(lead.0, lead.1, f));
        // The supplier has a member reviewer now, so it leaves the inbox of the managers.
        assert!(!open.in_inbox_of(user(MANAGER), EventAccess::Manager, s, false));
        // The new action goes to the managers, and the commitment that it depends on stays with the lead.
        assert!(open.routed_to(user(MANAGER), EventAccess::Manager, f));
        assert!(!open.routed_to(user(MANAGER), EventAccess::Manager, p));

        // A closed dependent passes nothing on.
        let closed = routes(false);
        assert!(!closed.may_review(lead.0, lead.1, s));
        assert!(closed.in_inbox_of(user(MANAGER), EventAccess::Manager, s, false));
        assert!(!closed.may_review(lead.0, lead.1, proposal_id(9)));
    }
}
