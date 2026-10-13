use std::collections::HashMap;
use std::sync::Mutex;

use tada_domain::identity::{EventRole, OrganizationRole};
use tada_domain::ids::{InstitutionId, MilestoneId, OrganizationId, PersonId};
use tada_domain::parties::PartyName;
use tada_domain::records::MilestoneStatus;
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

#[tokio::test]
async fn a_contributor_creates_an_action_for_another_contributor() {
    let memory = Memory::default();
    let action = action_by(&memory, OTHER, new_action(OWNER, Some(ground())))
        .await
        .unwrap();
    assert_eq!(action.fields.owner, user(OWNER));
    assert_eq!(action.fields.status, ActionStatus::Open);
    assert_eq!(action.fields.workstream_id, Some(ground()));
    assert_eq!(action.version, RecordVersion::FIRST);
    assert_eq!(action.local_id(), "ACT-001");
    assert_eq!(memory.audit(), [AuditAction::ActionCreate]);

    // The owner must be a contributor or manager of the event.
    for owner in [VIEWER, STRANGER] {
        let result = action_by(&memory, OTHER, new_action(owner, None)).await;
        assert_eq!(fields(result), field("owner", "unknown-member"), "{owner}");
    }
    // A change of the owner obeys the same rule.
    let change = ActionChange {
        owner: Some(user(VIEWER)),
        ..no_change(action.version)
    };
    let result = change_action_by(&memory, OWNER, action.id, change).await;
    assert_eq!(fields(result), field("owner", "unknown-member"));
}

#[tokio::test]
async fn a_viewer_cannot_create_an_action() {
    let memory = Memory::default();
    let viewer = action_by(&memory, VIEWER, new_action(OWNER, None)).await;
    assert!(matches!(viewer, Err(WorkError::Forbidden)), "{viewer:?}");
    let stranger = action_by(&memory, STRANGER, new_action(OWNER, None)).await;
    assert!(matches!(stranger, Err(WorkError::NotFound)), "{stranger:?}");
    let commitment = create_commitment(
        &caller(VIEWER),
        open_day(),
        new_commitment(None),
        memory.ports(),
    )
    .await;
    assert!(matches!(commitment, Err(WorkError::Forbidden)));
    assert!(memory.audit().is_empty());
}

#[tokio::test]
async fn the_owner_the_lead_and_a_manager_change_an_action_and_nobody_else() {
    let memory = Memory::default();
    let action = action_by(&memory, OWNER, new_action(OWNER, Some(ground())))
        .await
        .unwrap();
    let mut version = action.version;
    for by in [OWNER, LEAD, MANAGER] {
        let change = ActionChange {
            title: Some(format!("Generator bestellen {by}")),
            ..no_change(version)
        };
        version = change_action_by(&memory, by, action.id, change)
            .await
            .unwrap()
            .version;
    }
    assert_eq!(version.get(), 4);
    for by in [OTHER, VIEWER] {
        let change = ActionChange {
            title: Some("X".to_owned()),
            ..no_change(version)
        };
        let result = change_action_by(&memory, by, action.id, change).await;
        assert!(
            matches!(result, Err(WorkError::Forbidden)),
            "{by}: {result:?}"
        );
    }
    let stranger = change_action_by(&memory, STRANGER, action.id, no_change(version)).await;
    assert!(matches!(stranger, Err(WorkError::NotFound)));

    // Without a workstream, the lead is not a lead of the action.
    let free = action_by(&memory, OWNER, new_action(OWNER, None))
        .await
        .unwrap();
    let change = ActionChange {
        title: Some("X".to_owned()),
        ..no_change(free.version)
    };
    let lead = change_action_by(&memory, LEAD, free.id, change).await;
    assert!(matches!(lead, Err(WorkError::Forbidden)));
}

/// The view tells each reader whether it can change the record and to which statuses (ADR 0067, ADR 0068),
/// so that a client does not repeat the rules.
#[tokio::test]
async fn the_view_tells_the_reader_its_right_and_the_next_statuses() {
    let memory = Memory::default();
    let action = action_by(&memory, OWNER, new_action(OWNER, Some(ground())))
        .await
        .unwrap();
    for (reader, can_change) in [
        (OWNER, true),
        (LEAD, true),
        (MANAGER, true),
        (OTHER, false),
        (VIEWER, false),
    ] {
        let shown = get_action(&caller(reader), open_day(), action.id, memory.ports())
            .await
            .unwrap();
        assert_eq!(shown.can_change, can_change, "{reader}");
        let next = if can_change {
            vec![
                ActionStatus::InProgress,
                ActionStatus::Blocked,
                ActionStatus::Done,
                ActionStatus::Canceled,
            ]
        } else {
            Vec::new()
        };
        assert_eq!(shown.next_statuses(), next, "{reader}");
    }

    let commitment = create_commitment(
        &caller(OWNER),
        open_day(),
        new_commitment(Some("subject to signed order")),
        memory.ports(),
    )
    .await
    .unwrap();
    assert!(commitment.can_change);
    // "Make firm" sets `firm`, so a change does not offer it.
    assert_eq!(
        commitment.next_statuses(),
        [
            CommitmentStatus::Fulfilled,
            CommitmentStatus::Broken,
            CommitmentStatus::Withdrawn
        ]
    );
    let by_viewer = get_commitment(
        &caller(VIEWER),
        open_day(),
        commitment.record.id,
        memory.ports(),
    )
    .await
    .unwrap();
    assert!(!by_viewer.can_change);
    assert!(by_viewer.next_statuses().is_empty());
    // Only a conditional commitment can become firm, and only for a caller who can change it.
    assert!(commitment.can_make_firm());
    assert!(!by_viewer.can_make_firm());
    let firm = create_commitment(
        &caller(OWNER),
        open_day(),
        new_commitment(None),
        memory.ports(),
    )
    .await
    .unwrap();
    assert!(firm.can_change);
    assert!(!firm.can_make_firm());
}

#[test]
fn may_change_work_follows_the_permission_table() {
    let (owner, lead, other) = (user(OWNER), user(LEAD), user(OTHER));
    assert!(may_change_work(EventLevel::Contributor, owner, owner, None));
    assert!(may_change_work(
        EventLevel::Contributor,
        lead,
        owner,
        Some(lead)
    ));
    assert!(may_change_work(EventLevel::Manager, other, owner, None));
    assert!(!may_change_work(
        EventLevel::Contributor,
        other,
        owner,
        Some(lead)
    ));
    // A viewer changes nothing, not even a record that it owned before.
    assert!(!may_change_work(
        EventLevel::Viewer,
        owner,
        owner,
        Some(owner)
    ));
}

#[tokio::test]
async fn an_unknown_record_is_not_found_before_its_fields_are_checked() {
    let memory = Memory::default();
    let missing = ActionId::from_uuid(Uuid::now_v7());
    let change = ActionChange {
        title: Some(String::new()),
        ..no_change(RecordVersion::FIRST)
    };
    let result = change_action_by(&memory, MANAGER, missing, change).await;
    assert!(matches!(result, Err(WorkError::NotFound)), "{result:?}");
}

#[tokio::test]
async fn a_change_without_a_field_is_invalid() {
    let memory = Memory::default();
    let action = action_by(&memory, OWNER, new_action(OWNER, None))
        .await
        .unwrap();
    let result = change_action_by(&memory, OWNER, action.id, no_change(action.version)).await;
    assert_eq!(fields(result), []);
    let commitment = create_commitment(
        &caller(OWNER),
        open_day(),
        new_commitment(None),
        memory.ports(),
    )
    .await
    .unwrap()
    .record;
    let result = change_commitment_by(
        &memory,
        OWNER,
        commitment.id,
        commitment_change(commitment.version),
    )
    .await;
    assert_eq!(fields(result), []);
    assert_eq!(
        memory.audit(),
        [AuditAction::ActionCreate, AuditAction::CommitmentCreate]
    );
}

#[tokio::test]
async fn an_invalid_status_change_is_refused() {
    let memory = Memory::default();
    let action = action_by(&memory, OWNER, new_action(OWNER, None))
        .await
        .unwrap();
    let status = |status, version| ActionChange {
        status: Some(status),
        ..no_change(version)
    };
    let canceled = change_action_by(
        &memory,
        OWNER,
        action.id,
        status(ActionStatus::Canceled, action.version),
    )
    .await
    .unwrap();
    let reopen = change_action_by(
        &memory,
        OWNER,
        action.id,
        status(ActionStatus::Open, canceled.version),
    )
    .await;
    assert!(
        matches!(reopen, Err(WorkError::InvalidTransition)),
        "{reopen:?}"
    );

    let commitment = create_commitment(
        &caller(OWNER),
        open_day(),
        new_commitment(None),
        memory.ports(),
    )
    .await
    .unwrap()
    .record;
    let back = change_commitment_by(
        &memory,
        OWNER,
        commitment.id,
        CommitmentChange {
            status: Some(CommitmentStatus::Conditional),
            ..commitment_change(commitment.version)
        },
    )
    .await;
    assert!(matches!(back, Err(WorkError::InvalidTransition)));
}

/// A change to the current status is no transition (ADR 0068), as for a proposal: it changes nothing.
#[tokio::test]
async fn a_change_to_the_same_status_is_refused() {
    let memory = Memory::default();
    let action = action_by(&memory, OWNER, new_action(OWNER, None))
        .await
        .unwrap();
    let same = change_action_by(
        &memory,
        OWNER,
        action.id,
        ActionChange {
            status: Some(ActionStatus::Open),
            ..no_change(action.version)
        },
    )
    .await;
    assert!(
        matches!(same, Err(WorkError::InvalidTransition)),
        "{same:?}"
    );

    let commitment = create_commitment(
        &caller(OWNER),
        open_day(),
        new_commitment(Some("subject to signed order")),
        memory.ports(),
    )
    .await
    .unwrap()
    .record;
    let same = change_commitment_by(
        &memory,
        OWNER,
        commitment.id,
        CommitmentChange {
            status: Some(CommitmentStatus::Conditional),
            ..commitment_change(commitment.version)
        },
    )
    .await;
    assert!(
        matches!(same, Err(WorkError::InvalidTransition)),
        "{same:?}"
    );
    assert_eq!(
        memory.audit(),
        [AuditAction::ActionCreate, AuditAction::CommitmentCreate]
    );
}

#[tokio::test]
async fn a_conditional_commitment_stays_conditional_after_a_change() {
    let memory = Memory::default();
    let commitment = create_commitment(
        &caller(OTHER),
        open_day(),
        new_commitment(Some("subject to signed order")),
        memory.ports(),
    )
    .await
    .unwrap()
    .record;
    assert_eq!(commitment.fields.status, CommitmentStatus::Conditional);
    assert_eq!(commitment.local_id(), "COM-001");
    let change = CommitmentChange {
        text: Some("Generator delivery Saturday 09:00".to_owned()),
        due_date: Some(Some(jiff::civil::date(2030, 6, 1))),
        owner: Some(user(LEAD)),
        ..commitment_change(commitment.version)
    };
    let changed = change_commitment_by(&memory, OWNER, commitment.id, change)
        .await
        .unwrap();
    assert_eq!(changed.fields.status, CommitmentStatus::Conditional);
    assert_eq!(changed.fields.owner, user(LEAD));
    assert_eq!(
        changed.condition.as_ref().map(ConditionText::as_str),
        Some("subject to signed order")
    );
    assert_eq!(changed.fields.firm_reason, None);
}

#[tokio::test]
async fn only_make_firm_makes_a_commitment_firm() {
    let memory = Memory::default();
    let commitment = create_commitment(
        &caller(OWNER),
        open_day(),
        new_commitment(Some("subject to signed order")),
        memory.ports(),
    )
    .await
    .unwrap()
    .record;
    let firm = CommitmentChange {
        status: Some(CommitmentStatus::Firm),
        ..commitment_change(commitment.version)
    };
    let refused = change_commitment_by(&memory, OWNER, commitment.id, firm).await;
    assert!(matches!(refused, Err(WorkError::InvalidTransition)));

    let made = firm_by(
        &memory,
        OWNER,
        commitment.id,
        "Order signed",
        commitment.version,
    )
    .await
    .unwrap();
    assert_eq!(made.fields.status, CommitmentStatus::Firm);
    assert_eq!(
        made.fields.firm_reason.as_ref().map(FirmReason::as_str),
        Some("Order signed")
    );
    assert_eq!(
        made.condition.as_ref().map(ConditionText::as_str),
        Some("subject to signed order")
    );
    let again = firm_by(&memory, OWNER, commitment.id, "Again", made.version).await;
    assert!(matches!(again, Err(WorkError::InvalidTransition)));
    assert_eq!(
        memory.audit(),
        [AuditAction::CommitmentCreate, AuditAction::CommitmentFirm]
    );

    // A commitment without a condition starts firm and has no reason.
    let plain = create_commitment(
        &caller(OWNER),
        open_day(),
        new_commitment(None),
        memory.ports(),
    )
    .await
    .unwrap()
    .record;
    assert_eq!(plain.fields.status, CommitmentStatus::Firm);
    assert_eq!(plain.fields.firm_reason, None);
}

#[tokio::test]
async fn make_firm_needs_a_reason() {
    let memory = Memory::default();
    let commitment = create_commitment(
        &caller(OWNER),
        open_day(),
        new_commitment(Some("subject to signed order")),
        memory.ports(),
    )
    .await
    .unwrap()
    .record;
    let empty = firm_by(&memory, OWNER, commitment.id, "  ", commitment.version).await;
    assert_eq!(fields(empty), field("reason", "empty"));
    let long = firm_by(
        &memory,
        OWNER,
        commitment.id,
        &"a".repeat(501),
        commitment.version,
    )
    .await;
    assert_eq!(fields(long), field("reason", "too-long"));
    let other = firm_by(&memory, OTHER, commitment.id, "Signed", commitment.version).await;
    assert!(matches!(other, Err(WorkError::Forbidden)));
    assert_eq!(memory.audit(), [AuditAction::CommitmentCreate]);
}

#[tokio::test]
async fn a_promisor_of_another_organization_is_unknown() {
    let memory = Memory::default();
    for promisor in [
        Party::Person(foreign_person()),
        Party::Person(PersonId::from_uuid(Uuid::now_v7())),
        Party::Institution(InstitutionId::from_uuid(Uuid::now_v7())),
    ] {
        let input = NewCommitment {
            promisor,
            ..new_commitment(None)
        };
        let result = create_commitment(&caller(OWNER), open_day(), input, memory.ports()).await;
        assert_eq!(fields(result), field("promisor", "unknown-record"));
    }
    assert!(memory.audit().is_empty());
}

#[tokio::test]
async fn a_closed_workstream_cannot_be_set() {
    let memory = Memory::default();
    let closed = action_by(&memory, OWNER, new_action(OWNER, Some(kitchen()))).await;
    assert_eq!(fields(closed), field("workstream", "closed"));
    let unknown = action_by(
        &memory,
        OWNER,
        new_action(OWNER, Some(WorkstreamId::from_uuid(Uuid::now_v7()))),
    )
    .await;
    assert_eq!(fields(unknown), field("workstream", "unknown-record"));
    let commitment = create_commitment(
        &caller(OWNER),
        open_day(),
        NewCommitment {
            workstream: Some(kitchen()),
            ..new_commitment(None)
        },
        memory.ports(),
    )
    .await;
    assert_eq!(fields(commitment), field("workstream", "closed"));

    let action = action_by(&memory, OWNER, new_action(OWNER, Some(ground())))
        .await
        .unwrap();
    let move_to_kitchen = ActionChange {
        workstream: Some(Some(kitchen())),
        ..no_change(action.version)
    };
    let result = change_action_by(&memory, OWNER, action.id, move_to_kitchen).await;
    assert_eq!(fields(result), field("workstream", "closed"));

    // A record keeps its workstream after the workstream closes, and other changes still work.
    memory.workstreams.lock().unwrap()[0].status = WorkstreamStatus::Closed;
    let change = ActionChange {
        title: Some("Generator bestellt".to_owned()),
        workstream: Some(Some(ground())),
        ..no_change(action.version)
    };
    let changed = change_action_by(&memory, OWNER, action.id, change)
        .await
        .unwrap();
    assert_eq!(changed.fields.workstream_id, Some(ground()));
}

#[tokio::test]
async fn a_stale_version_conflicts() {
    let memory = Memory::default();
    let action = action_by(&memory, OWNER, new_action(OWNER, None))
        .await
        .unwrap();
    let change = |version| ActionChange {
        due_date: Some(Some(jiff::civil::date(2030, 6, 1))),
        ..no_change(version)
    };
    change_action_by(&memory, OWNER, action.id, change(action.version))
        .await
        .unwrap();
    let stale = change_action_by(&memory, OWNER, action.id, change(action.version)).await;
    assert!(matches!(stale, Err(WorkError::VersionConflict)));
}

#[tokio::test]
async fn a_list_pages_and_filters_by_owner() {
    let memory = Memory::default();
    for owner in [OWNER, LEAD, OWNER] {
        action_by(&memory, OWNER, new_action(owner, None))
            .await
            .unwrap();
    }
    let query = WorkQuery {
        owner: Some(user(OWNER)),
        status: None,
        workstream: None,
        after: None,
        limit: PageLimit::new(1).unwrap(),
    };
    let first = list_actions(&caller(VIEWER), open_day(), query, memory.ports())
        .await
        .unwrap();
    assert_eq!(first.items.len(), 1);
    assert_eq!(first.next, Some(NumberCursor(1)));
    let second = list_actions(
        &caller(VIEWER),
        open_day(),
        WorkQuery {
            after: first.next,
            ..query
        },
        memory.ports(),
    )
    .await
    .unwrap();
    assert_eq!(second.items[0].record.local_number, 3);
    assert_eq!(second.next, None);
}

fn new_milestone(owner: u128, workstream: Option<WorkstreamId>) -> NewMilestone {
    NewMilestone {
        id: None,
        title: "Bewilligung liegt vor".to_owned(),
        description: None,
        date: jiff::civil::date(2030, 5, 1),
        owner: user(owner),
        workstream,
    }
}

fn milestone_change(expected_version: RecordVersion) -> MilestoneChange {
    MilestoneChange {
        title: None,
        description: None,
        date: None,
        owner: None,
        workstream: None,
        status: None,
        expected_version,
    }
}

async fn milestone_by(
    memory: &Memory,
    by: u128,
    input: NewMilestone,
) -> Result<MilestoneView, WorkError> {
    create_milestone(&caller(by), open_day(), input, memory.ports())
        .await
        .map(|shown| shown.record)
}

async fn change_milestone_by(
    memory: &Memory,
    by: u128,
    id: MilestoneId,
    change: MilestoneChange,
) -> Result<MilestoneView, WorkError> {
    change_milestone(&caller(by), open_day(), id, change, memory.ports())
        .await
        .map(|shown| shown.record)
}

async fn milestone_status(
    memory: &Memory,
    id: MilestoneId,
    status: MilestoneStatus,
    expected_version: RecordVersion,
) -> Result<MilestoneView, WorkError> {
    let change = MilestoneChange {
        status: Some(status),
        ..milestone_change(expected_version)
    };
    change_milestone_by(memory, OWNER, id, change).await
}

#[tokio::test]
async fn contributor_creates_a_milestone() {
    let memory = Memory::default();
    let milestone = milestone_by(&memory, OTHER, new_milestone(OWNER, Some(ground())))
        .await
        .unwrap();
    assert_eq!(milestone.fields.owner, user(OWNER));
    assert_eq!(milestone.fields.status, MilestoneStatus::Open);
    assert_eq!(milestone.fields.date, jiff::civil::date(2030, 5, 1));
    assert_eq!(milestone.local_id(), "MIL-001");
    assert_eq!(memory.audit(), [AuditAction::MilestoneCreate]);

    // The owner is a contributor or a manager, the workstream an active one of the event.
    let result = milestone_by(&memory, OTHER, new_milestone(VIEWER, Some(kitchen()))).await;
    assert_eq!(
        fields(result),
        [
            ("owner".to_owned(), "unknown-member"),
            ("workstream".to_owned(), "closed"),
        ]
    );
    let long = NewMilestone {
        title: "a".repeat(201),
        ..new_milestone(OWNER, None)
    };
    let result = milestone_by(&memory, OTHER, long).await;
    assert_eq!(fields(result), field("title", "too-long"));
}

#[tokio::test]
async fn viewer_cannot_create_a_milestone() {
    let memory = Memory::default();
    let viewer = milestone_by(&memory, VIEWER, new_milestone(OWNER, None)).await;
    assert!(matches!(viewer, Err(WorkError::Forbidden)), "{viewer:?}");
    let stranger = milestone_by(&memory, STRANGER, new_milestone(OWNER, None)).await;
    assert!(matches!(stranger, Err(WorkError::NotFound)), "{stranger:?}");
    assert!(memory.audit().is_empty());
}

#[tokio::test]
async fn owner_reaches_and_reopens_a_milestone() {
    let memory = Memory::default();
    let milestone = milestone_by(&memory, OWNER, new_milestone(OWNER, Some(ground())))
        .await
        .unwrap();
    let reached = milestone_status(
        &memory,
        milestone.id,
        MilestoneStatus::Reached,
        milestone.version,
    )
    .await
    .unwrap();
    assert_eq!(reached.fields.status, MilestoneStatus::Reached);
    let open = milestone_status(
        &memory,
        milestone.id,
        MilestoneStatus::Open,
        reached.version,
    )
    .await
    .unwrap();
    assert_eq!(open.fields.status, MilestoneStatus::Open);
    assert_eq!(
        memory.audit(),
        [
            AuditAction::MilestoneCreate,
            AuditAction::MilestoneChange,
            AuditAction::MilestoneChange
        ]
    );

    // The lead and a manager change it too; another contributor and a viewer do not.
    let mut version = open.version;
    for by in [LEAD, MANAGER] {
        let change = MilestoneChange {
            date: Some(jiff::civil::date(2030, 5, 2)),
            ..milestone_change(version)
        };
        version = change_milestone_by(&memory, by, milestone.id, change)
            .await
            .unwrap()
            .version;
    }
    for by in [OTHER, VIEWER] {
        let change = MilestoneChange {
            title: Some("X".to_owned()),
            ..milestone_change(version)
        };
        let result = change_milestone_by(&memory, by, milestone.id, change).await;
        assert!(
            matches!(result, Err(WorkError::Forbidden)),
            "{by}: {result:?}"
        );
    }

    // The view tells the owner the next statuses.
    let shown = get_milestone(&caller(OWNER), open_day(), milestone.id, memory.ports())
        .await
        .unwrap();
    assert_eq!(
        shown.next_statuses(),
        [MilestoneStatus::Reached, MilestoneStatus::Canceled]
    );
    let shown = get_milestone(&caller(VIEWER), open_day(), milestone.id, memory.ports())
        .await
        .unwrap();
    assert!(shown.next_statuses().is_empty());
}

#[tokio::test]
async fn canceled_milestone_is_final() {
    let memory = Memory::default();
    let milestone = milestone_by(&memory, OWNER, new_milestone(OWNER, None))
        .await
        .unwrap();
    let canceled = milestone_status(
        &memory,
        milestone.id,
        MilestoneStatus::Canceled,
        milestone.version,
    )
    .await
    .unwrap();
    for status in MilestoneStatus::ALL {
        let result = milestone_status(&memory, milestone.id, status, canceled.version).await;
        assert!(
            matches!(result, Err(WorkError::InvalidTransition)),
            "{status:?}: {result:?}"
        );
    }
    let page = list_milestones(
        &caller(VIEWER),
        open_day(),
        WorkQuery {
            owner: None,
            status: Some(MilestoneStatus::Canceled),
            workstream: None,
            after: None,
            limit: crate::paging::PageLimit::LARGEST,
        },
        memory.ports(),
    )
    .await
    .unwrap();
    assert_eq!(page.items.len(), 1);
}

#[tokio::test]
async fn action_start_after_due_is_refused() {
    let memory = Memory::default();
    let (may, june) = (jiff::civil::date(2030, 5, 1), jiff::civil::date(2030, 6, 1));
    let late_start = NewAction {
        start_date: Some(june),
        due_date: Some(may),
        ..new_action(OWNER, None)
    };
    let result = action_by(&memory, OWNER, late_start).await;
    assert_eq!(fields(result), field("start_date", "start-after-due"));

    // The same day is in order. A change checks the dates that the action has after the change.
    let action = action_by(
        &memory,
        OWNER,
        NewAction {
            start_date: Some(may),
            due_date: Some(may),
            ..new_action(OWNER, None)
        },
    )
    .await
    .unwrap();
    assert_eq!(action.fields.start_date, Some(may));
    let earlier_due = ActionChange {
        due_date: Some(Some(jiff::civil::date(2030, 4, 1))),
        ..no_change(action.version)
    };
    let result = change_action_by(&memory, OWNER, action.id, earlier_due).await;
    assert_eq!(fields(result), field("start_date", "start-after-due"));
    let moved = ActionChange {
        start_date: Some(Some(june)),
        due_date: Some(None),
        ..no_change(action.version)
    };
    let moved = change_action_by(&memory, OWNER, action.id, moved)
        .await
        .unwrap();
    assert_eq!(
        (moved.fields.start_date, moved.fields.due_date),
        (Some(june), None)
    );
}

/// A milestone of another event: the store of the event does not find it.
fn foreign_milestone() -> MilestoneId {
    MilestoneId::from_uuid(Uuid::from_u128(300))
}

#[tokio::test]
async fn action_cannot_name_a_milestone_of_another_event() {
    let memory = Memory::default();
    let foreign = NewAction {
        milestone: Some(foreign_milestone()),
        ..new_action(OWNER, None)
    };
    let result = action_by(&memory, OWNER, foreign).await;
    assert_eq!(fields(result), field("milestone", "unknown-record"));

    let milestone = milestone_by(&memory, OWNER, new_milestone(OWNER, None))
        .await
        .unwrap();
    let action = action_by(
        &memory,
        OWNER,
        NewAction {
            milestone: Some(milestone.id),
            ..new_action(OWNER, None)
        },
    )
    .await
    .unwrap();
    assert_eq!(action.fields.milestone_id, Some(milestone.id));
    let change = ActionChange {
        milestone: Some(Some(foreign_milestone())),
        ..no_change(action.version)
    };
    let result = change_action_by(&memory, OWNER, action.id, change).await;
    assert_eq!(fields(result), field("milestone", "unknown-record"));
    let cleared = ActionChange {
        milestone: Some(None),
        ..no_change(action.version)
    };
    let cleared = change_action_by(&memory, OWNER, action.id, cleared)
        .await
        .unwrap();
    assert_eq!(cleared.fields.milestone_id, None);
}

#[tokio::test]
async fn action_cannot_name_a_canceled_milestone() {
    let memory = Memory::default();
    let milestone = milestone_by(&memory, OWNER, new_milestone(OWNER, None))
        .await
        .unwrap();
    let action = action_by(
        &memory,
        OWNER,
        NewAction {
            milestone: Some(milestone.id),
            ..new_action(OWNER, None)
        },
    )
    .await
    .unwrap();
    milestone_status(
        &memory,
        milestone.id,
        MilestoneStatus::Canceled,
        milestone.version,
    )
    .await
    .unwrap();
    let result = action_by(
        &memory,
        OWNER,
        NewAction {
            milestone: Some(milestone.id),
            ..new_action(OWNER, None)
        },
    )
    .await;
    assert_eq!(fields(result), field("milestone", "unknown-record"));
    // The existing link stays, and a change of another field keeps it.
    let change = ActionChange {
        title: Some("Zelt bestellen".to_owned()),
        ..no_change(action.version)
    };
    let changed = change_action_by(&memory, OWNER, action.id, change)
        .await
        .unwrap();
    assert_eq!(changed.fields.milestone_id, Some(milestone.id));
}
