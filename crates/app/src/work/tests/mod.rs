use std::collections::HashMap;
use std::sync::Mutex;

use tada_domain::identity::{EventRole, OrganizationRole};
use tada_domain::ids::{DecisionId, InstitutionId, MilestoneId, OrganizationId, PersonId, RiskId};
use tada_domain::parties::PartyName;
use tada_domain::records::{
    DecisionChange, DecisionStatus, MilestoneStatus, RecordReason, RiskStatus,
};
use tada_domain::work::{WorkstreamName, WorkstreamStatus};

use super::*;
use crate::access::SourceReach;
use crate::audit::AuditAction;
use crate::caller::MemberCaller;
use crate::identity::{Membership, UserRef};
use crate::parties::{InstitutionFields, InstitutionView, PersonFields, PersonView};
use crate::records::{Changed, Created, NumberCursor};
use crate::records::{EvidenceStore, RecordEvidenceView, RecordRef};
use crate::workstreams::{
    Changed as WorkstreamChanged, Created as WorkstreamCreated, Workstream, WorkstreamUpdate,
};
use uuid::Uuid;

fn testwil() -> OrganizationId {
    OrganizationId::from_uuid(Uuid::from_u128(10))
}

fn musterhausen() -> OrganizationId {
    OrganizationId::from_uuid(Uuid::from_u128(11))
}

fn open_day() -> EventId {
    EventId::from_uuid(Uuid::from_u128(20))
}

fn user(number: u128) -> UserId {
    UserId::from_uuid(Uuid::from_u128(number))
}

fn caller(number: u128) -> MemberCaller {
    MemberCaller::new(user(number), testwil(), OrganizationRole::Member)
}

const MANAGER: u128 = 1;
const OWNER: u128 = 2;
const VIEWER: u128 = 3;
const LEAD: u128 = 4;
const OTHER: u128 = 5;
const STRANGER: u128 = 99;

/// A person of Testwil and a person of Musterhausen.
fn supplier() -> PersonId {
    PersonId::from_uuid(Uuid::from_u128(100))
}

fn foreign_person() -> PersonId {
    PersonId::from_uuid(Uuid::from_u128(101))
}

fn ground() -> WorkstreamId {
    WorkstreamId::from_uuid(Uuid::from_u128(200))
}

fn kitchen() -> WorkstreamId {
    WorkstreamId::from_uuid(Uuid::from_u128(201))
}

/// Five members of Testwil with an event role in `open_day`; the lead leads the workstream „Gelände“.
#[derive(Debug)]
struct Memory {
    actions: Mutex<Vec<ActionView>>,
    commitments: Mutex<Vec<CommitmentView>>,
    milestones: Mutex<Vec<MilestoneView>>,
    decisions: Mutex<Vec<DecisionView>>,
    risks: Mutex<Vec<RiskView>>,
    workstreams: Mutex<Vec<Workstream>>,
    audit: Mutex<Vec<AuditAction>>,
}

impl Default for Memory {
    fn default() -> Self {
        let workstream = |id, name, status| Workstream {
            id,
            event_id: open_day(),
            name: tada_domain::redaction::Redactable::Present(WorkstreamName::parse(name).unwrap()),
            lead: user(LEAD),
            status,
            version: RecordVersion::FIRST,
        };
        Self {
            actions: Mutex::default(),
            commitments: Mutex::default(),
            milestones: Mutex::default(),
            decisions: Mutex::default(),
            risks: Mutex::default(),
            workstreams: Mutex::new(vec![
                workstream(ground(), "Gelände", WorkstreamStatus::Active),
                workstream(kitchen(), "Küche", WorkstreamStatus::Closed),
            ]),
            audit: Mutex::default(),
        }
    }
}

impl Memory {
    fn ports(&self) -> WorkPorts<'_> {
        WorkPorts {
            identity: self,
            work: self,
            milestones: self,
            decisions: self,
            risks: self,
            workstreams: self,
            parties: self,
            clock: &FixedClock,
        }
    }

    fn audit(&self) -> Vec<AuditAction> {
        self.audit.lock().unwrap().clone()
    }
}

fn roles() -> HashMap<UserId, EventRole> {
    HashMap::from([
        (user(MANAGER), EventRole::EventManager),
        (user(OWNER), EventRole::EventContributor),
        (user(VIEWER), EventRole::EventViewer),
        (user(LEAD), EventRole::EventContributor),
        (user(OTHER), EventRole::EventContributor),
    ])
}

#[derive(Debug)]
struct FixedClock;

impl Clock for FixedClock {
    fn now(&self) -> Timestamp {
        "2030-05-18T08:00:00Z".parse().unwrap()
    }
}

#[async_trait]
impl IdentityStore for Memory {
    async fn user(&self, _: UserId) -> Result<Option<UserRef>, StoreError> {
        unreachable!()
    }

    async fn memberships_of(&self, _: UserId) -> Result<Vec<Membership>, StoreError> {
        unreachable!()
    }

    async fn membership(
        &self,
        _: OrgScope,
        user: UserId,
    ) -> Result<Option<OrganizationRole>, StoreError> {
        Ok(roles()
            .contains_key(&user)
            .then_some(OrganizationRole::Member))
    }

    async fn event_status(
        &self,
        _: OrgScope,
        event: EventId,
    ) -> Result<Option<tada_domain::events::EventStatus>, StoreError> {
        Ok((event == open_day()).then_some(tada_domain::events::EventStatus::Active))
    }

    async fn event_role(
        &self,
        _: OrgScope,
        event: EventId,
        user: UserId,
    ) -> Result<Option<EventRole>, StoreError> {
        Ok((event == open_day())
            .then(|| roles().get(&user).copied())
            .flatten())
    }

    async fn event_roles_of(
        &self,
        _: OrgScope,
        user: UserId,
    ) -> Result<Vec<(EventId, EventRole)>, StoreError> {
        Ok(roles()
            .get(&user)
            .map(|role| (open_day(), *role))
            .into_iter()
            .collect())
    }
}

#[async_trait]
impl WorkstreamStore for Memory {
    async fn create(
        &self,
        _: OrgScope,
        _: &Workstream,
        _: Timestamp,
        _: &AuditEvent,
    ) -> Result<WorkstreamCreated, StoreError> {
        unreachable!()
    }

    async fn change(
        &self,
        _: OrgScope,
        _: EventId,
        _: WorkstreamId,
        _: &WorkstreamUpdate,
        _: RecordVersion,
        _: Timestamp,
        _: &AuditEvent,
    ) -> Result<WorkstreamChanged, StoreError> {
        unreachable!()
    }

    async fn get(
        &self,
        _: OrgScope,
        event: EventId,
        id: WorkstreamId,
    ) -> Result<Option<Workstream>, StoreError> {
        let all = self.workstreams.lock().unwrap();
        Ok(all
            .iter()
            .find(|known| known.id == id && known.event_id == event)
            .cloned())
    }

    async fn list(&self, _: OrgScope, event: EventId) -> Result<Vec<Workstream>, StoreError> {
        let all = self.workstreams.lock().unwrap();
        Ok(all
            .iter()
            .filter(|known| known.event_id == event)
            .cloned()
            .collect())
    }
}

fn person_view(id: PersonId) -> PersonView {
    PersonView {
        id,
        local_number: 1,
        name: tada_domain::redaction::Redactable::Present(PartyName::parse("Beat Muster").unwrap()),
        email: None,
        phone: None,
        user_id: None,
        version: RecordVersion::FIRST,
    }
}

#[async_trait::async_trait]
impl crate::dispute_notes::DisputeNoteReads for Memory {
    async fn notes(
        &self,
        _: crate::caller::OrgScope,
        _: &[crate::dispute_notes::NoteKey],
    ) -> Result<
        Vec<(
            crate::dispute_notes::NoteKey,
            crate::dispute_notes::DisputeNote,
        )>,
        crate::store::StoreError,
    > {
        Ok(Vec::new())
    }
}

#[async_trait]
impl EvidenceStore for Memory {
    async fn evidence_of(
        &self,
        _: OrgScope,
        _: &[RecordRef],
        _: &SourceReach,
    ) -> Result<Vec<(RecordRef, RecordEvidenceView)>, StoreError> {
        Ok(Vec::new())
    }
}

#[async_trait]
impl PartyStore for Memory {
    async fn create_person(
        &self,
        _: OrgScope,
        _: PersonId,
        _: &PersonFields,
        _: Timestamp,
        _: &AuditEvent,
    ) -> Result<Created<PersonView>, StoreError> {
        unreachable!()
    }

    async fn change_person(
        &self,
        _: OrgScope,
        _: PersonId,
        _: &PersonFields,
        _: RecordVersion,
        _: Timestamp,
        _: &AuditEvent,
    ) -> Result<Changed<PersonView>, StoreError> {
        unreachable!()
    }

    /// The supplier is a person of Testwil; the foreign person is one of Musterhausen.
    async fn person(
        &self,
        scope: OrgScope,
        id: PersonId,
    ) -> Result<Option<PersonView>, StoreError> {
        let organization = if id == supplier() {
            testwil()
        } else if id == foreign_person() {
            musterhausen()
        } else {
            return Ok(None);
        };
        Ok((scope.organization_id() == organization).then(|| person_view(id)))
    }

    async fn persons(
        &self,
        _: OrgScope,
        _: Option<&str>,
        _: Option<NumberCursor>,
        _: u32,
    ) -> Result<Vec<PersonView>, StoreError> {
        unreachable!()
    }

    async fn create_institution(
        &self,
        _: OrgScope,
        _: InstitutionId,
        _: &InstitutionFields,
        _: Timestamp,
        _: &AuditEvent,
    ) -> Result<Created<InstitutionView>, StoreError> {
        unreachable!()
    }

    async fn change_institution(
        &self,
        _: OrgScope,
        _: InstitutionId,
        _: &InstitutionFields,
        _: RecordVersion,
        _: Timestamp,
        _: &AuditEvent,
    ) -> Result<Changed<InstitutionView>, StoreError> {
        unreachable!()
    }

    async fn institution(
        &self,
        _: OrgScope,
        _: InstitutionId,
    ) -> Result<Option<InstitutionView>, StoreError> {
        Ok(None)
    }

    async fn institutions(
        &self,
        _: OrgScope,
        _: Option<&str>,
        _: Option<NumberCursor>,
        _: u32,
    ) -> Result<Vec<InstitutionView>, StoreError> {
        unreachable!()
    }

    async fn named_like(&self, _: OrgScope, _: &str) -> Result<Vec<PartyRef>, StoreError> {
        unreachable!()
    }
}

fn next(version: RecordVersion) -> RecordVersion {
    RecordVersion::new(version.get() + 1).unwrap()
}

/// The fake has no warnings: the store calculates them in its query (`store-pg`).
#[async_trait]
impl MilestoneStore for Memory {
    async fn create_milestone(
        &self,
        _: OrgScope,
        milestone: &NewMilestoneRecord,
        _: Timestamp,
        audit: &AuditEvent,
    ) -> Result<Created<MilestoneView>, StoreError> {
        let mut all = self.milestones.lock().unwrap();
        if all.iter().any(|known| known.id == milestone.id) {
            return Ok(Created::IdTaken);
        }
        let view = MilestoneView {
            id: milestone.id,
            local_number: all.len() as u64 + 1,
            event_id: milestone.event_id,
            fields: milestone.fields.clone(),
            version: RecordVersion::FIRST,
            warnings: Vec::new(),
        };
        all.push(view.clone());
        self.audit.lock().unwrap().push(audit.action());
        Ok(Created::Created(view))
    }

    async fn change_milestone(
        &self,
        _: OrgScope,
        event: EventId,
        id: MilestoneId,
        fields: &MilestoneFields,
        expected: RecordVersion,
        _: Timestamp,
        audit: &AuditEvent,
    ) -> Result<Changed<MilestoneView>, StoreError> {
        let mut all = self.milestones.lock().unwrap();
        let Some(known) = all
            .iter_mut()
            .find(|known| known.id == id && known.event_id == event)
        else {
            return Ok(Changed::NotFound);
        };
        if known.version != expected {
            return Ok(Changed::VersionConflict);
        }
        known.fields = fields.clone();
        known.version = next(known.version);
        self.audit.lock().unwrap().push(audit.action());
        Ok(Changed::Changed(known.clone()))
    }

    async fn milestone(
        &self,
        _: OrgScope,
        event: EventId,
        id: MilestoneId,
        _: Timestamp,
    ) -> Result<Option<MilestoneView>, StoreError> {
        let all = self.milestones.lock().unwrap();
        Ok(all
            .iter()
            .find(|known| known.id == id && known.event_id == event)
            .cloned())
    }

    async fn milestones(
        &self,
        _: OrgScope,
        event: EventId,
        filter: &WorkFilter<MilestoneStatus>,
        _: Timestamp,
    ) -> Result<Vec<MilestoneView>, StoreError> {
        let all = self.milestones.lock().unwrap();
        Ok(all
            .iter()
            .filter(|known| known.event_id == event)
            .filter(|known| {
                filter
                    .status
                    .is_none_or(|status| known.fields.status == status)
            })
            .take(filter.limit as usize)
            .cloned()
            .collect())
    }
}

#[async_trait]
impl RiskStore for Memory {
    async fn create_risk(
        &self,
        _: OrgScope,
        risk: &NewRiskRecord,
        _: Timestamp,
        audit: &AuditEvent,
    ) -> Result<Created<RiskView>, StoreError> {
        let mut all = self.risks.lock().unwrap();
        if all.iter().any(|known| known.id == risk.id) {
            return Ok(Created::IdTaken);
        }
        let view = RiskView {
            id: risk.id,
            local_number: all.len() as u64 + 1,
            event_id: risk.event_id,
            fields: risk.fields.clone(),
            version: RecordVersion::FIRST,
        };
        all.push(view.clone());
        self.audit.lock().unwrap().push(audit.action());
        Ok(Created::Created(view))
    }

    async fn change_risk(
        &self,
        _: OrgScope,
        event: EventId,
        id: RiskId,
        fields: &RiskFields,
        expected: RecordVersion,
        _: Timestamp,
        audit: &AuditEvent,
    ) -> Result<Changed<RiskView>, StoreError> {
        let mut all = self.risks.lock().unwrap();
        let Some(known) = all
            .iter_mut()
            .find(|known| known.id == id && known.event_id == event)
        else {
            return Ok(Changed::NotFound);
        };
        if known.version != expected {
            return Ok(Changed::VersionConflict);
        }
        known.fields = fields.clone();
        known.version = next(known.version);
        self.audit.lock().unwrap().push(audit.action());
        Ok(Changed::Changed(known.clone()))
    }

    async fn risk(
        &self,
        _: OrgScope,
        event: EventId,
        id: RiskId,
    ) -> Result<Option<RiskView>, StoreError> {
        let all = self.risks.lock().unwrap();
        Ok(all
            .iter()
            .find(|known| known.id == id && known.event_id == event)
            .cloned())
    }

    async fn risks(
        &self,
        _: OrgScope,
        event: EventId,
        filter: &WorkFilter<RiskStatus, RiskCursor>,
    ) -> Result<Vec<RiskView>, StoreError> {
        let mut all: Vec<RiskView> = self
            .risks
            .lock()
            .unwrap()
            .iter()
            .filter(|known| known.event_id == event)
            .filter(|known| {
                filter
                    .status
                    .is_none_or(|status| known.fields.status == status)
            })
            .cloned()
            .collect();
        all.sort_by_key(|risk| (risk.fields.rating.sort_key(), risk.local_number));
        Ok(all
            .into_iter()
            .filter(|risk| filter.after.is_none_or(|cursor| cursor.comes_before(risk)))
            .take(filter.limit as usize)
            .collect())
    }
}

impl Memory {
    fn new_decision_view(&self, decision: &NewDecisionRecord, number: u64) -> DecisionView {
        DecisionView {
            id: decision.id,
            local_number: number,
            event_id: decision.event_id,
            wording: tada_domain::redaction::Redactable::Present(decision.wording.clone()),
            decided_on: decision.decided_on,
            parties: decision
                .parties
                .as_slice()
                .iter()
                .map(|party| PartyRef {
                    party: *party,
                    local_id: "PER-001".to_owned(),
                    name: person_view(supplier()).name,
                })
                .collect(),
            fields: decision.fields,
            state: DecisionState::InForce,
            version: RecordVersion::FIRST,
        }
    }

    /// Ends the decision with `change` like the store: only a decision in force with the expected version.
    fn end(
        &self,
        event: EventId,
        id: DecisionId,
        change: &DecisionChange,
        expected: RecordVersion,
    ) -> Changed<DecisionView> {
        let mut all = self.decisions.lock().unwrap();
        let Some(known) = all
            .iter_mut()
            .find(|known| known.id == id && known.event_id == event)
        else {
            return Changed::NotFound;
        };
        if known.version != expected || known.state != DecisionState::InForce {
            return Changed::VersionConflict;
        }
        known.state = match change {
            DecisionChange::Supersede { successor } => DecisionState::Superseded { by: *successor },
            DecisionChange::Revoke { reason } => DecisionState::Revoked {
                reason: tada_domain::redaction::Redactable::Present(reason.clone()),
            },
        };
        known.version = next(known.version);
        Changed::Changed(known.clone())
    }
}

#[async_trait]
impl DecisionStore for Memory {
    async fn create_decision(
        &self,
        _: OrgScope,
        decision: &NewDecisionRecord,
        _: Timestamp,
        audit: &AuditEvent,
    ) -> Result<Created<DecisionView>, StoreError> {
        let mut all = self.decisions.lock().unwrap();
        if all.iter().any(|known| known.id == decision.id) {
            return Ok(Created::IdTaken);
        }
        let view = self.new_decision_view(decision, all.len() as u64 + 1);
        all.push(view.clone());
        self.audit.lock().unwrap().push(audit.action());
        Ok(Created::Created(view))
    }

    async fn change_decision(
        &self,
        _: OrgScope,
        event: EventId,
        id: DecisionId,
        fields: &DecisionFields,
        expected: RecordVersion,
        _: Timestamp,
        audit: &AuditEvent,
    ) -> Result<Changed<DecisionView>, StoreError> {
        let mut all = self.decisions.lock().unwrap();
        let Some(known) = all
            .iter_mut()
            .find(|known| known.id == id && known.event_id == event)
        else {
            return Ok(Changed::NotFound);
        };
        if known.version != expected {
            return Ok(Changed::VersionConflict);
        }
        known.fields = *fields;
        known.version = next(known.version);
        self.audit.lock().unwrap().push(audit.action());
        Ok(Changed::Changed(known.clone()))
    }

    async fn supersede_decision(
        &self,
        _: OrgScope,
        event: EventId,
        old: DecisionId,
        successor: &NewDecisionRecord,
        expected: RecordVersion,
        _: Timestamp,
        audit: &[AuditEvent],
    ) -> Result<Created<Changed<Superseded>>, StoreError> {
        let number = {
            let all = self.decisions.lock().unwrap();
            if all.iter().any(|known| known.id == successor.id) {
                return Ok(Created::IdTaken);
            }
            all.len() as u64 + 1
        };
        let change = DecisionChange::Supersede {
            successor: successor.id,
        };
        let old = match self.end(event, old, &change, expected) {
            Changed::Changed(old) => old,
            Changed::NotFound => return Ok(Created::Created(Changed::NotFound)),
            Changed::VersionConflict => return Ok(Created::Created(Changed::VersionConflict)),
        };
        let successor = self.new_decision_view(successor, number);
        self.decisions.lock().unwrap().push(successor.clone());
        self.audit
            .lock()
            .unwrap()
            .extend(audit.iter().map(AuditEvent::action));
        Ok(Created::Created(Changed::Changed(Superseded {
            old,
            successor,
        })))
    }

    async fn revoke_decision(
        &self,
        _: OrgScope,
        event: EventId,
        id: DecisionId,
        reason: &RecordReason,
        expected: RecordVersion,
        _: Timestamp,
        audit: &AuditEvent,
    ) -> Result<Changed<DecisionView>, StoreError> {
        let change = DecisionChange::Revoke {
            reason: reason.clone(),
        };
        let result = self.end(event, id, &change, expected);
        if matches!(result, Changed::Changed(_)) {
            self.audit.lock().unwrap().push(audit.action());
        }
        Ok(result)
    }

    async fn decision(
        &self,
        _: OrgScope,
        event: EventId,
        id: DecisionId,
    ) -> Result<Option<DecisionView>, StoreError> {
        let all = self.decisions.lock().unwrap();
        Ok(all
            .iter()
            .find(|known| known.id == id && known.event_id == event)
            .cloned())
    }

    async fn decisions(
        &self,
        _: OrgScope,
        event: EventId,
        filter: &WorkFilter<DecisionStatus>,
    ) -> Result<Vec<DecisionView>, StoreError> {
        let all = self.decisions.lock().unwrap();
        Ok(all
            .iter()
            .filter(|known| known.event_id == event)
            .filter(|known| {
                filter
                    .status
                    .is_none_or(|status| known.state.status() == status)
            })
            .take(filter.limit as usize)
            .cloned()
            .collect())
    }
}

#[async_trait]
impl WorkStore for Memory {
    async fn create_action(
        &self,
        _: OrgScope,
        action: &NewActionRecord,
        _: Timestamp,
        audit: &AuditEvent,
    ) -> Result<Created<ActionView>, StoreError> {
        let mut all = self.actions.lock().unwrap();
        if all.iter().any(|known| known.id == action.id) {
            return Ok(Created::IdTaken);
        }
        let view = ActionView {
            id: action.id,
            local_number: all.len() as u64 + 1,
            event_id: action.event_id,
            fields: action.fields.clone(),
            version: RecordVersion::FIRST,
        };
        all.push(view.clone());
        self.audit.lock().unwrap().push(audit.action());
        Ok(Created::Created(view))
    }

    async fn change_action(
        &self,
        _: OrgScope,
        event: EventId,
        id: ActionId,
        fields: &ActionFields,
        expected: RecordVersion,
        _: Timestamp,
        audit: &AuditEvent,
    ) -> Result<Changed<ActionView>, StoreError> {
        let mut all = self.actions.lock().unwrap();
        let Some(known) = all
            .iter_mut()
            .find(|known| known.id == id && known.event_id == event)
        else {
            return Ok(Changed::NotFound);
        };
        if known.version != expected {
            return Ok(Changed::VersionConflict);
        }
        known.fields = fields.clone();
        known.version = next(known.version);
        self.audit.lock().unwrap().push(audit.action());
        Ok(Changed::Changed(known.clone()))
    }

    async fn action(
        &self,
        _: OrgScope,
        event: EventId,
        id: ActionId,
    ) -> Result<Option<ActionView>, StoreError> {
        let all = self.actions.lock().unwrap();
        Ok(all
            .iter()
            .find(|known| known.id == id && known.event_id == event)
            .cloned())
    }

    async fn actions(
        &self,
        _: OrgScope,
        event: EventId,
        filter: &WorkFilter<ActionStatus>,
    ) -> Result<Vec<ActionView>, StoreError> {
        let all = self.actions.lock().unwrap();
        Ok(all
            .iter()
            .filter(|known| known.event_id == event)
            .filter(|known| filter.owner.is_none_or(|owner| known.fields.owner == owner))
            .filter(|known| {
                filter
                    .status
                    .is_none_or(|status| known.fields.status == status)
            })
            .filter(|known| known.local_number > filter.after.map_or(0, |cursor| cursor.0))
            .take(filter.limit as usize)
            .cloned()
            .collect())
    }

    async fn create_commitment(
        &self,
        _: OrgScope,
        commitment: &NewCommitmentRecord,
        _: Timestamp,
        audit: &AuditEvent,
    ) -> Result<Created<CommitmentView>, StoreError> {
        let mut all = self.commitments.lock().unwrap();
        let Party::Person(person) = commitment.promisor else {
            unreachable!()
        };
        let view = CommitmentView {
            id: commitment.id,
            local_number: all.len() as u64 + 1,
            event_id: commitment.event_id,
            condition: commitment.condition.clone(),
            promisor: PartyRef {
                party: commitment.promisor,
                local_id: "PER-001".to_owned(),
                name: person_view(person).name,
            },
            fields: commitment.fields.clone(),
            version: RecordVersion::FIRST,
        };
        all.push(view.clone());
        self.audit.lock().unwrap().push(audit.action());
        Ok(Created::Created(view))
    }

    async fn change_commitment(
        &self,
        _: OrgScope,
        event: EventId,
        id: CommitmentId,
        fields: &CommitmentFields,
        expected: RecordVersion,
        _: Timestamp,
        audit: &AuditEvent,
    ) -> Result<Changed<CommitmentView>, StoreError> {
        let mut all = self.commitments.lock().unwrap();
        let Some(known) = all
            .iter_mut()
            .find(|known| known.id == id && known.event_id == event)
        else {
            return Ok(Changed::NotFound);
        };
        if known.version != expected {
            return Ok(Changed::VersionConflict);
        }
        known.fields = fields.clone();
        known.version = next(known.version);
        self.audit.lock().unwrap().push(audit.action());
        Ok(Changed::Changed(known.clone()))
    }

    async fn commitment(
        &self,
        _: OrgScope,
        event: EventId,
        id: CommitmentId,
    ) -> Result<Option<CommitmentView>, StoreError> {
        let all = self.commitments.lock().unwrap();
        Ok(all
            .iter()
            .find(|known| known.id == id && known.event_id == event)
            .cloned())
    }

    async fn commitments(
        &self,
        _: OrgScope,
        _: EventId,
        _: &WorkFilter<CommitmentStatus>,
    ) -> Result<Vec<CommitmentView>, StoreError> {
        unreachable!()
    }

    async fn owners(
        &self,
        _: OrgScope,
        _: EventId,
        _: &[RecordRef],
    ) -> Result<WorkOwners, StoreError> {
        unreachable!()
    }

    async fn my_open_work(
        &self,
        _: OrgScope,
        _: UserId,
        _: &crate::access::EventReach,
    ) -> Result<MyWork, StoreError> {
        unreachable!()
    }
}

fn new_action(owner: u128, workstream: Option<WorkstreamId>) -> NewAction {
    NewAction {
        id: None,
        title: "Generator bestellen".to_owned(),
        description: None,
        owner: user(owner),
        workstream,
        start_date: None,
        due_date: None,
        milestone: None,
    }
}

fn no_change(expected_version: RecordVersion) -> ActionChange {
    ActionChange {
        title: None,
        description: None,
        owner: None,
        workstream: None,
        start_date: None,
        due_date: None,
        milestone: None,
        status: None,
        expected_version,
    }
}

fn new_commitment(condition: Option<&str>) -> NewCommitment {
    NewCommitment {
        id: None,
        text: "Generator delivery Friday 15:00".to_owned(),
        condition: condition.map(str::to_owned),
        promisor: Party::Person(supplier()),
        owner: user(OWNER),
        workstream: None,
        due_date: None,
    }
}

fn commitment_change(expected_version: RecordVersion) -> CommitmentChange {
    CommitmentChange {
        text: None,
        owner: None,
        workstream: None,
        due_date: None,
        status: None,
        expected_version,
    }
}

fn fields<T: Debug>(result: Result<T, WorkError>) -> Vec<(String, &'static str)> {
    match result {
        Err(WorkError::Invalid(errors)) => errors
            .into_iter()
            .map(|error| (error.field.into_owned(), error.code))
            .collect(),
        other => panic!("not invalid: {other:?}"),
    }
}

fn field(name: &str, code: &'static str) -> Vec<(String, &'static str)> {
    vec![(name.to_owned(), code)]
}

async fn action_by(memory: &Memory, by: u128, input: NewAction) -> Result<ActionView, WorkError> {
    create_action(&caller(by), open_day(), input, memory.ports())
        .await
        .map(|shown| shown.record)
}

async fn change_action_by(
    memory: &Memory,
    by: u128,
    id: ActionId,
    change: ActionChange,
) -> Result<ActionView, WorkError> {
    super::change_action(&caller(by), open_day(), id, change, memory.ports())
        .await
        .map(|shown| shown.record)
}

async fn change_commitment_by(
    memory: &Memory,
    by: u128,
    id: CommitmentId,
    change: CommitmentChange,
) -> Result<CommitmentView, WorkError> {
    super::change_commitment(&caller(by), open_day(), id, change, memory.ports())
        .await
        .map(|shown| shown.record)
}

async fn firm_by(
    memory: &Memory,
    by: u128,
    id: CommitmentId,
    reason: &str,
    expected_version: RecordVersion,
) -> Result<CommitmentView, WorkError> {
    let input = FirmInput {
        reason: reason.to_owned(),
        expected_version,
    };
    make_commitment_firm(&caller(by), open_day(), id, input, memory.ports())
        .await
        .map(|shown| shown.record)
}

mod actions;
mod commitments;
mod decisions;
mod milestones;
mod risks;
