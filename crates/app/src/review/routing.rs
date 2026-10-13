//! Review routing (ADR 0067): who reviews each proposal.
//!
//! tada computes the reviewers at each read and each apply from the current state: the current owner of a record
//! and the current lead of a workstream. It stores no reviewers, so a replaced lead or a member who lost the
//! event role stops seeing and applying the proposals at once.
//!
//! The decision is pure. `RoutingFacts::load` reads the current state that it needs through the ports.

use std::collections::{BTreeSet, HashMap, HashSet};

use tada_domain::ids::{EventId, ProposalId, UserId, WorkstreamId};
use tada_domain::proposals::Operation;
use tada_domain::records::RecordRef;

use super::selection::closure;
use crate::access::EventLevel;
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

/// The current state that the routing reads: the owner of each record and the lead of each workstream of the event.
/// `None` means that the event has no such record, or that the record has no owner who can review.
pub trait RoutingLookup {
    fn owner_of(&self, record: RecordRef) -> Option<UserId>;
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
    if operation.managers_only() {
        return Reviewers::EventManagers;
    }
    let user = match operation.changed_record() {
        Some(record) => lookup.owner_of(record),
        None => operation
            .workstream()
            .and_then(|id| lookup.workstream_lead(id)),
    };
    user.map_or(Reviewers::EventManagers, Reviewers::user)
}

/// True if the proposal goes to the caller: the caller is a member reviewer of the proposal or of an open proposal
/// that refers to the record that it creates (rule 5), or the caller is an event manager and the proposal goes to no member.
/// A member reviewer must still have the contributor or manager role (rule 8).
pub fn routed_to(
    caller: UserId,
    access: EventLevel,
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
    access: EventLevel,
    own: &Reviewers,
    dependents: &[&Reviewers],
) -> bool {
    access.can_review() || routed_to(caller, access, own, dependents)
}

/// True if the Review Inbox of the caller shows the open proposal: it goes to the caller, or the caller is an event
/// manager and the proposal is overdue (rule 6).
pub fn in_inbox_of(
    caller: UserId,
    access: EventLevel,
    own: &Reviewers,
    dependents: &[&Reviewers],
    overdue: bool,
) -> bool {
    routed_to(caller, access, own, dependents) || (access.can_review() && overdue)
}

/// True if `dependent` refers to the record that `dependency` creates (rule 5): `Operation::references`, for example
/// a new commitment whose promisor is a new person or a new institution of the changeset.
/// Only then the reviewers of the dependent decide on the dependency too.
/// Any other dependency keeps its own reviewers, so a `depends_on` entry cannot move a proposal to another reviewer.
/// A dependency that only the event managers review, for example a new workstream, gets no reviewers from its
/// dependents (ADR 0080).
fn refers_to_new_record(dependent: &Operation, dependency: &Operation) -> bool {
    !dependency.managers_only()
        && dependency
            .new_record()
            .is_some_and(|record| dependent.references().contains(&record))
}

/// One proposal of a changeset as the routing sees it.
#[derive(Debug, Clone, Copy)]
pub struct RoutedProposal<'a> {
    pub id: ProposalId,
    pub operation: &'a Operation,
    pub depends_on: &'a [ProposalId],
    /// Only an open proposal can still be selected, so only an open proposal gives its reviewers to the proposal of
    /// the record that it refers to.
    pub open: bool,
}

/// The reviewers of each proposal of one changeset, and the open proposals that refer to the record of each (rule 5).
#[derive(Debug, Clone)]
pub struct ChangesetRoutes {
    own: HashMap<ProposalId, Reviewers>,
    /// The open proposals that refer to the new record of each proposal, directly or through other open proposals.
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
        let operations: HashMap<ProposalId, &Operation> = proposals
            .iter()
            .map(|proposal| (proposal.id, proposal.operation))
            .collect();
        let mut direct: HashMap<ProposalId, Vec<ProposalId>> = HashMap::new();
        for proposal in proposals.iter().filter(|proposal| proposal.open) {
            let needed = proposal.depends_on.iter().filter(|dependency| {
                operations
                    .get(dependency)
                    .is_some_and(|operation| refers_to_new_record(proposal.operation, operation))
            });
            for dependency in needed {
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

    /// The reviewers of the proposal and of the open proposals that refer to its new record.
    /// A proposal of another changeset has none.
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

    pub fn routed_to(&self, caller: UserId, access: EventLevel, id: ProposalId) -> bool {
        self.of(id)
            .is_some_and(|(own, dependents)| routed_to(caller, access, own, &dependents))
    }

    pub fn may_review(&self, caller: UserId, access: EventLevel, id: ProposalId) -> bool {
        self.of(id)
            .is_some_and(|(own, dependents)| may_review(caller, access, own, &dependents))
    }

    pub fn in_inbox_of(
        &self,
        caller: UserId,
        access: EventLevel,
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
    owners: HashMap<RecordRef, UserId>,
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
        let mut records = Vec::new();
        let mut needs_leads = false;
        for operation in operations
            .into_iter()
            .filter(|operation| !operation.managers_only())
        {
            match operation.changed_record() {
                Some(record) => records.push(record),
                None => needs_leads |= operation.workstream().is_some(),
            }
        }
        let mut facts = Self::default();
        if !records.is_empty() {
            facts.owners = ports.work.owners(scope, event, &records).await?;
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
            .owners
            .values()
            .chain(self.leads.values())
            .copied()
            .collect();
        let mut reviewers = HashSet::new();
        for user in users {
            if is_possible_owner(scope, event, user, identity).await? {
                reviewers.insert(user);
            }
        }
        self.owners.retain(|_, user| reviewers.contains(user));
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
    fn owner_of(&self, record: RecordRef) -> Option<UserId> {
        self.owners.get(&record).copied()
    }

    fn workstream_lead(&self, id: WorkstreamId) -> Option<UserId> {
        self.leads.get(&id).copied()
    }
}

#[cfg(test)]
mod tests {
    use tada_domain::RecordVersion;
    use tada_domain::ids::{
        ActionId, CommitmentId, DecisionId, FieldDefinitionId, InstitutionId, MilestoneId,
        PersonId, RiskId,
    };
    use tada_domain::parties::{InstitutionKind, Party, PartyName};
    use tada_domain::records::{
        DecisionParties, DecisionWording, Level, MilestoneStatus, MilestoneTitle, RecordReason,
        RiskRating, RiskStatus, RiskTitle,
    };
    use tada_domain::work::{ActionStatus, ActionTitle, CommitmentText, WorkstreamName};
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
            owners: HashMap::from([(RecordRef::Action(action()), user(OWNER))]),
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
            start_date: None,
            due_date: None,
            milestone: None,
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

    fn new_institution(institution: InstitutionId) -> Operation {
        Operation::CreateInstitution {
            id: institution,
            name: PartyName::parse("Gemeinde Testwil").unwrap(),
            kind: InstitutionKind::Authority,
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

    fn new_workstream() -> Operation {
        Operation::CreateWorkstream {
            id: workstream(),
            event_id: event(),
            name: WorkstreamName::parse("Programm").unwrap(),
            lead: user(LEAD),
        }
    }

    #[test]
    fn new_workstream_goes_to_the_managers() {
        // Not to its future lead: the lead reviews the records of the workstream only after the managers created it.
        let facts = facts();
        let create = new_workstream();
        assert_eq!(
            reviewers(&create, Some(event()), &facts),
            Reviewers::EventManagers
        );
        let routes = ChangesetRoutes::new(
            [
                RoutedProposal {
                    id: proposal_id(1),
                    operation: &create,
                    depends_on: &[],
                    open: true,
                },
                RoutedProposal {
                    id: proposal_id(2),
                    operation: &new_action(Some(workstream())),
                    depends_on: &[proposal_id(1)],
                    open: true,
                },
            ],
            Some(event()),
            &RoutingFacts::default(),
        );
        for id in [proposal_id(1), proposal_id(2)] {
            assert!(!routes.routed_to(user(LEAD), EventLevel::Contributor, id));
            assert!(routes.routed_to(user(MANAGER), EventLevel::Manager, id));
        }
        // With the lead of the existing workstream, the action alone would go to the lead.
        assert!(
            ChangesetRoutes::new(
                [RoutedProposal {
                    id: proposal_id(2),
                    operation: &new_action(Some(workstream())),
                    depends_on: &[],
                    open: true,
                }],
                Some(event()),
                &facts,
            )
            .routed_to(user(LEAD), EventLevel::Contributor, proposal_id(2))
        );
    }

    fn decision(n: u128) -> DecisionId {
        DecisionId::from_uuid(Uuid::from_u128(n))
    }

    #[test]
    fn decision_proposals_go_to_managers_even_in_a_workstream() {
        // The lead of the workstream and the owner of the decision are contributors: neither reviews a decision.
        let person = PersonId::from_uuid(Uuid::from_u128(61));
        let facts = RoutingFacts {
            owners: HashMap::from([(RecordRef::Decision(decision(80)), user(OWNER))]),
            leads: HashMap::from([(workstream(), user(LEAD))]),
        };
        let create = Operation::CreateDecision {
            id: decision(81),
            event_id: event(),
            wording: DecisionWording::parse("Das Fest findet bei jedem Wetter statt.").unwrap(),
            decided_on: jiff::civil::date(2030, 3, 1),
            decided_by: DecisionParties::new(vec![Party::Person(person)]).unwrap(),
            owner: user(OWNER),
            workstream: Some(workstream()),
        };
        let supersede = Operation::SupersedeDecision {
            event_id: event(),
            decision_id: decision(80),
            successor: decision(81),
            expected_version: RecordVersion::FIRST,
        };
        let revoke = Operation::RevokeDecision {
            event_id: event(),
            decision_id: decision(80),
            reason: RecordReason::parse("Die Gemeinde verbietet es.").unwrap(),
            expected_version: RecordVersion::FIRST,
        };
        for operation in [&create, &supersede, &revoke] {
            assert_eq!(
                reviewers(operation, Some(event()), &facts),
                Reviewers::EventManagers
            );
        }
        // The new party of the decision and its successor stay with the managers too (rule 5).
        let person_proposal = new_person(person);
        let routes = ChangesetRoutes::new(
            [
                RoutedProposal {
                    id: proposal_id(1),
                    operation: &person_proposal,
                    depends_on: &[],
                    open: true,
                },
                RoutedProposal {
                    id: proposal_id(2),
                    operation: &create,
                    depends_on: &[proposal_id(1)],
                    open: true,
                },
                RoutedProposal {
                    id: proposal_id(3),
                    operation: &supersede,
                    depends_on: &[proposal_id(2)],
                    open: true,
                },
                RoutedProposal {
                    id: proposal_id(4),
                    operation: &revoke,
                    depends_on: &[],
                    open: true,
                },
            ],
            Some(event()),
            &facts,
        );
        for id in (1..=4).map(proposal_id) {
            for member in [LEAD, OWNER] {
                assert!(!routes.routed_to(user(member), EventLevel::Contributor, id));
                assert!(!routes.may_review(user(member), EventLevel::Contributor, id));
                assert!(!routes.in_inbox_of(user(member), EventLevel::Contributor, id, true));
            }
            assert!(routes.routed_to(user(MANAGER), EventLevel::Manager, id));
            assert!(routes.may_review(user(MANAGER), EventLevel::Manager, id));
        }
    }

    fn risk() -> RiskId {
        RiskId::from_uuid(Uuid::from_u128(36))
    }

    fn new_risk(workstream: Option<WorkstreamId>) -> Operation {
        Operation::CreateRisk {
            id: RiskId::from_uuid(Uuid::from_u128(37)),
            event_id: event(),
            title: RiskTitle::parse("Starkregen am Festtag").unwrap(),
            description: None,
            rating: RiskRating {
                likelihood: Level::Medium,
                impact: Level::High,
            },
            mitigation: None,
            owner: user(OTHER),
            workstream,
        }
    }

    #[test]
    fn rating_change_goes_to_the_owner() {
        let facts = RoutingFacts {
            owners: HashMap::from([(RecordRef::Risk(risk()), user(OWNER))]),
            leads: HashMap::from([(workstream(), user(LEAD))]),
        };
        let rating = Operation::ChangeRiskRating {
            event_id: event(),
            risk_id: risk(),
            rating: RiskRating {
                likelihood: Level::High,
                impact: Level::High,
            },
            expected_version: RecordVersion::new(1).unwrap(),
        };
        let status = Operation::ChangeRiskStatus {
            event_id: event(),
            risk_id: risk(),
            status: RiskStatus::Mitigated,
            expected_version: RecordVersion::new(1).unwrap(),
        };
        for change in [&rating, &status] {
            assert_eq!(
                reviewers(change, Some(event()), &facts),
                Reviewers::user(user(OWNER))
            );
        }
        // A risk without a known owner goes to the event managers.
        let no_owner = RoutingFacts {
            owners: HashMap::new(),
            leads: HashMap::new(),
        };
        assert_eq!(
            reviewers(&rating, Some(event()), &no_owner),
            Reviewers::EventManagers
        );
        // A new risk in a workstream goes to the lead (rule 2); without one, to the managers (rule 3).
        assert_eq!(
            reviewers(&new_risk(Some(workstream())), Some(event()), &facts),
            Reviewers::user(user(LEAD))
        );
        assert_eq!(
            reviewers(&new_risk(None), Some(event()), &facts),
            Reviewers::EventManagers
        );
    }

    fn milestone() -> MilestoneId {
        MilestoneId::from_uuid(Uuid::from_u128(33))
    }

    fn new_milestone(workstream: Option<WorkstreamId>) -> Operation {
        Operation::CreateMilestone {
            id: milestone(),
            event_id: event(),
            title: MilestoneTitle::parse("Bewilligung liegt vor").unwrap(),
            description: None,
            date: jiff::civil::date(2030, 5, 1),
            owner: user(OTHER),
            workstream,
        }
    }

    #[test]
    fn change_of_a_milestone_goes_to_its_owner() {
        let facts = RoutingFacts {
            owners: HashMap::from([(RecordRef::Milestone(milestone()), user(OWNER))]),
            leads: HashMap::new(),
        };
        let reached = Operation::ChangeMilestoneStatus {
            event_id: event(),
            milestone_id: milestone(),
            status: MilestoneStatus::Reached,
            expected_version: RecordVersion::new(1).unwrap(),
        };
        assert_eq!(
            reviewers(&reached, Some(event()), &facts),
            Reviewers::user(user(OWNER))
        );
        // An action with the same UUID is another record.
        let same_uuid = RoutingFacts {
            owners: HashMap::from([(
                RecordRef::Action(ActionId::from_uuid(milestone().as_uuid())),
                user(OWNER),
            )]),
            leads: HashMap::new(),
        };
        assert_eq!(
            reviewers(&reached, Some(event()), &same_uuid),
            Reviewers::EventManagers
        );
    }

    #[test]
    fn new_milestone_in_a_workstream_goes_to_the_lead() {
        let facts = facts();
        assert_eq!(
            reviewers(&new_milestone(Some(workstream())), Some(event()), &facts),
            Reviewers::user(user(LEAD))
        );
        assert_eq!(
            reviewers(&new_milestone(None), Some(event()), &facts),
            Reviewers::EventManagers
        );
    }

    /// Rule 5 of ADR 0067 (ADR 0080): a new action that names a new milestone gives its reviewers to the milestone.
    #[test]
    fn new_action_with_a_new_milestone_brings_the_milestone_to_the_lead() {
        let milestone_op = new_milestone(None);
        let action_op = Operation::CreateAction {
            id: ActionId::from_uuid(Uuid::from_u128(34)),
            event_id: event(),
            title: ActionTitle::parse("Gesuch einreichen").unwrap(),
            description: None,
            owner: user(OTHER),
            workstream: Some(workstream()),
            start_date: None,
            due_date: None,
            milestone: Some(milestone()),
        };
        let other_op = new_action(Some(workstream()));
        let (m, a, o) = (proposal_id(1), proposal_id(2), proposal_id(3));
        let depends_on_milestone = [m];
        let routes = ChangesetRoutes::new(
            [
                RoutedProposal {
                    id: m,
                    operation: &milestone_op,
                    depends_on: &[],
                    open: true,
                },
                RoutedProposal {
                    id: a,
                    operation: &action_op,
                    depends_on: &depends_on_milestone,
                    open: true,
                },
                RoutedProposal {
                    id: o,
                    operation: &other_op,
                    depends_on: &depends_on_milestone,
                    open: true,
                },
            ],
            Some(event()),
            &facts(),
        );
        let lead = (user(LEAD), EventLevel::Contributor);
        // The milestone has no workstream, so alone it would go to the managers.
        assert!(routes.routed_to(lead.0, lead.1, m));
        assert!(routes.may_review(lead.0, lead.1, a));
        assert!(!routes.in_inbox_of(user(MANAGER), EventLevel::Manager, m, false));
        // A dependency that does not name the milestone does not pass it on.
        assert!(
            !other_op
                .references()
                .contains(&tada_domain::proposals::NewRecord::Milestone(milestone()))
        );
    }

    /// Rule 5 gives no member reviewer to a proposal that only the event managers review (ADR 0080), also through
    /// a chain: a new action of an existing workstream names a new milestone, and the milestone names a new workstream.
    #[test]
    fn a_chain_of_new_records_keeps_a_new_workstream_with_the_managers() {
        let new_stream = WorkstreamId::from_uuid(Uuid::from_u128(42));
        let workstream_op = Operation::CreateWorkstream {
            id: new_stream,
            event_id: event(),
            name: WorkstreamName::parse("Logistik").unwrap(),
            lead: user(OTHER),
        };
        let milestone_op = new_milestone(Some(new_stream));
        let action_op = Operation::CreateAction {
            id: ActionId::from_uuid(Uuid::from_u128(34)),
            event_id: event(),
            title: ActionTitle::parse("Gesuch einreichen").unwrap(),
            description: None,
            owner: user(OTHER),
            workstream: Some(workstream()),
            start_date: None,
            due_date: None,
            milestone: Some(milestone()),
        };
        let (w, m, a) = (proposal_id(1), proposal_id(2), proposal_id(3));
        let depends_on_workstream = [w];
        let depends_on_milestone = [m];
        let routes = ChangesetRoutes::new(
            [
                RoutedProposal {
                    id: w,
                    operation: &workstream_op,
                    depends_on: &[],
                    open: true,
                },
                RoutedProposal {
                    id: m,
                    operation: &milestone_op,
                    depends_on: &depends_on_workstream,
                    open: true,
                },
                RoutedProposal {
                    id: a,
                    operation: &action_op,
                    depends_on: &depends_on_milestone,
                    open: true,
                },
            ],
            Some(event()),
            &facts(),
        );
        let lead = (user(LEAD), EventLevel::Contributor);
        assert!(!routes.routed_to(lead.0, lead.1, w));
        assert!(!routes.may_review(lead.0, lead.1, w));
        assert!(routes.in_inbox_of(user(MANAGER), EventLevel::Manager, w, false));
        // The milestone still goes to the lead of the action, as before.
        assert!(routes.routed_to(lead.0, lead.1, m));
        assert!(routes.routed_to(lead.0, lead.1, a));
        assert!(!routes.in_inbox_of(user(MANAGER), EventLevel::Manager, m, false));
    }

    /// A new workstream with the ID of an existing one does not go to the lead of the existing one through a new
    /// action in it.
    #[test]
    fn a_new_workstream_with_an_existing_id_stays_with_the_managers() {
        let create = new_workstream();
        let action_op = new_action(Some(workstream()));
        let (w, a) = (proposal_id(1), proposal_id(2));
        let depends_on_workstream = [w];
        let routes = ChangesetRoutes::new(
            [
                RoutedProposal {
                    id: w,
                    operation: &create,
                    depends_on: &[],
                    open: true,
                },
                RoutedProposal {
                    id: a,
                    operation: &action_op,
                    depends_on: &depends_on_workstream,
                    open: true,
                },
            ],
            Some(event()),
            &facts(),
        );
        let lead = (user(LEAD), EventLevel::Contributor);
        assert!(!routes.routed_to(lead.0, lead.1, w));
        assert!(!routes.may_review(lead.0, lead.1, w));
        assert!(routes.in_inbox_of(user(MANAGER), EventLevel::Manager, w, false));
    }

    #[test]
    fn a_member_reviewer_needs_the_contributor_or_manager_role() {
        let owner = Reviewers::user(user(OWNER));
        assert!(routed_to(user(OWNER), EventLevel::Contributor, &owner, &[]));
        assert!(may_review(
            user(OWNER),
            EventLevel::Contributor,
            &owner,
            &[]
        ));
        assert!(!routed_to(user(OWNER), EventLevel::Viewer, &owner, &[]));
        assert!(!may_review(user(OWNER), EventLevel::Viewer, &owner, &[]));
        assert!(!may_review(
            user(OTHER),
            EventLevel::Contributor,
            &owner,
            &[]
        ));
    }

    #[test]
    fn a_manager_reviews_each_proposal_but_sees_only_unrouted_or_overdue_ones() {
        let lead = Reviewers::user(user(LEAD));
        let manager = (user(MANAGER), EventLevel::Manager);
        assert!(may_review(manager.0, manager.1, &lead, &[]));
        assert!(!routed_to(manager.0, manager.1, &lead, &[]));
        assert!(!in_inbox_of(manager.0, manager.1, &lead, &[], false));
        assert!(in_inbox_of(manager.0, manager.1, &lead, &[], true));
        for reviewers in [Reviewers::EventManagers, Reviewers::OrganizationAdmins] {
            assert!(in_inbox_of(manager.0, manager.1, &reviewers, &[], false));
            assert!(!in_inbox_of(
                user(OTHER),
                EventLevel::Contributor,
                &reviewers,
                &[],
                true
            ));
        }
        // A manager who is the lead gets the proposal as the lead.
        assert!(routed_to(user(LEAD), EventLevel::Manager, &lead, &[]));
    }

    #[test]
    fn refers_to_new_record_needs_same_kind() {
        let uuid = Uuid::from_u128(60);
        let person = PersonId::from_uuid(uuid);
        let institution = InstitutionId::from_uuid(uuid);
        let promise = new_commitment(person, None);
        assert!(refers_to_new_record(&promise, &new_person(person)));
        // The same UUID with another kind is another record.
        assert!(!refers_to_new_record(
            &promise,
            &new_institution(institution)
        ));
    }

    #[test]
    fn a_new_promisor_goes_to_the_reviewers_of_its_open_commitments() {
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
        let lead = (user(LEAD), EventLevel::Contributor);
        assert!(open.routed_to(lead.0, lead.1, s));
        assert!(open.may_review(lead.0, lead.1, p));
        assert!(!open.may_review(lead.0, lead.1, f));
        // The supplier has a member reviewer now, so it leaves the inbox of the managers.
        assert!(!open.in_inbox_of(user(MANAGER), EventLevel::Manager, s, false));
        // The new action goes to the managers, and the commitment that it depends on stays with the lead.
        assert!(open.routed_to(user(MANAGER), EventLevel::Manager, f));
        assert!(!open.routed_to(user(MANAGER), EventLevel::Manager, p));

        // A closed dependent passes nothing on.
        let closed = routes(false);
        assert!(!closed.may_review(lead.0, lead.1, s));
        assert!(closed.in_inbox_of(user(MANAGER), EventLevel::Manager, s, false));
        assert!(!closed.may_review(lead.0, lead.1, proposal_id(9)));
    }

    #[test]
    fn a_dependency_that_the_dependent_does_not_refer_to_keeps_its_own_reviewers() {
        let deprecation = deprecation();
        let change = change_action();
        let (d, c) = (proposal_id(1), proposal_id(2));
        let depends_on_deprecation = [d];
        let routes = ChangesetRoutes::new(
            [
                RoutedProposal {
                    id: d,
                    operation: &deprecation,
                    depends_on: &[],
                    open: true,
                },
                RoutedProposal {
                    id: c,
                    operation: &change,
                    depends_on: &depends_on_deprecation,
                    open: true,
                },
            ],
            Some(event()),
            &facts(),
        );
        let owner = (user(OWNER), EventLevel::Contributor);
        assert!(routes.may_review(owner.0, owner.1, c));
        assert!(!routes.may_review(owner.0, owner.1, d));
        assert!(!routes.routed_to(owner.0, owner.1, d));
        assert!(routes.in_inbox_of(user(MANAGER), EventLevel::Manager, d, false));
    }
}
