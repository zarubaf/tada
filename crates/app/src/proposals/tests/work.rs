//! Proposals of work records and parties (ADR 0068, ADR 0069).

use jiff::civil::Date;
use tada_domain::ids::{
    ActionId, CommitmentId, DecisionId, InstitutionId, LocalIdKind, MilestoneId, PersonId, RiskId,
    WorkstreamId,
};
use tada_domain::parties::{Party, PartyName};
use tada_domain::records::{
    DecisionParties, DecisionStatus, DecisionWording, Level, MilestoneStatus, RecordReason,
    RiskRating, RiskStatus, RiskTitle,
};
use tada_domain::work::{
    ActionStatus, ActionTitle, CommitmentStatus, CommitmentText, ConditionText, WorkstreamName,
    WorkstreamStatus,
};

use super::*;
use crate::access::SourceReach;
use crate::parties::{InstitutionFields, InstitutionView, PartyRef, PersonFields, PersonView};
use crate::records::{Changed, Created, NumberCursor};
use crate::records::{EvidenceStore, RecordEvidenceView, RecordRef};
use crate::work::{
    ActionFields, CommitmentFields, DecisionFields, DecisionState, DecisionStore, MilestoneFields,
    MilestoneStore, MyWork, NewActionRecord, NewCommitmentRecord, NewDecisionRecord,
    NewMilestoneRecord, NewRiskRecord, RiskCursor, RiskFields, RiskStore, Superseded, WorkFilter,
    WorkOwners,
};
use crate::workstreams::{
    Changed as WorkstreamChanged, Created as WorkstreamCreated, WorkstreamUpdate,
};

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
        scope: OrgScope,
        event: EventId,
        id: WorkstreamId,
    ) -> Result<Option<Workstream>, StoreError> {
        let found = scope.organization_id() == testwil();
        Ok(self
            .workstreams
            .lock()
            .unwrap()
            .iter()
            .find(|workstream| found && workstream.event_id == event && workstream.id == id)
            .cloned())
    }

    async fn list(&self, scope: OrgScope, event: EventId) -> Result<Vec<Workstream>, StoreError> {
        let found = scope.organization_id() == testwil();
        Ok(self
            .workstreams
            .lock()
            .unwrap()
            .iter()
            .filter(|workstream| found && workstream.event_id == event)
            .cloned()
            .collect())
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

    async fn person(
        &self,
        scope: OrgScope,
        id: PersonId,
    ) -> Result<Option<PersonView>, StoreError> {
        let found = scope.organization_id() == testwil()
            && self.parties.lock().unwrap().contains(&id.as_uuid());
        Ok(found.then(|| PersonView {
            id,
            local_number: 1,
            name: tada_domain::redaction::Redactable::Present(
                PartyName::parse("Moritz Muster").unwrap(),
            ),
            email: None,
            phone: None,
            user_id: None,
            version: RecordVersion::FIRST,
        }))
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

#[async_trait]
impl WorkStore for Memory {
    async fn create_action(
        &self,
        _: OrgScope,
        _: &NewActionRecord,
        _: Timestamp,
        _: &AuditEvent,
    ) -> Result<Created<ActionView>, StoreError> {
        unreachable!()
    }

    async fn change_action(
        &self,
        _: OrgScope,
        _: EventId,
        _: ActionId,
        _: &ActionFields,
        _: RecordVersion,
        _: Timestamp,
        _: &AuditEvent,
    ) -> Result<Changed<ActionView>, StoreError> {
        unreachable!()
    }

    async fn action(
        &self,
        scope: OrgScope,
        event: EventId,
        id: ActionId,
    ) -> Result<Option<ActionView>, StoreError> {
        let found = scope.organization_id() == testwil();
        Ok(self
            .actions
            .lock()
            .unwrap()
            .iter()
            .find(|action| found && action.event_id == event && action.id == id)
            .cloned())
    }

    async fn actions(
        &self,
        _: OrgScope,
        _: EventId,
        _: &WorkFilter<ActionStatus>,
    ) -> Result<Vec<ActionView>, StoreError> {
        unreachable!()
    }

    async fn create_commitment(
        &self,
        _: OrgScope,
        _: &NewCommitmentRecord,
        _: Timestamp,
        _: &AuditEvent,
    ) -> Result<Created<CommitmentView>, StoreError> {
        unreachable!()
    }

    async fn change_commitment(
        &self,
        _: OrgScope,
        _: EventId,
        _: CommitmentId,
        _: &CommitmentFields,
        _: RecordVersion,
        _: Timestamp,
        _: &AuditEvent,
    ) -> Result<Changed<CommitmentView>, StoreError> {
        unreachable!()
    }

    async fn commitment(
        &self,
        scope: OrgScope,
        event: EventId,
        id: CommitmentId,
    ) -> Result<Option<CommitmentView>, StoreError> {
        let found = scope.organization_id() == testwil();
        Ok(self
            .commitments
            .lock()
            .unwrap()
            .iter()
            .find(|commitment| found && commitment.event_id == event && commitment.id == id)
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

#[async_trait]
impl MilestoneStore for Memory {
    async fn create_milestone(
        &self,
        _: OrgScope,
        _: &NewMilestoneRecord,
        _: Timestamp,
        _: &AuditEvent,
    ) -> Result<Created<MilestoneView>, StoreError> {
        unreachable!()
    }

    async fn change_milestone(
        &self,
        _: OrgScope,
        _: EventId,
        _: MilestoneId,
        _: &MilestoneFields,
        _: RecordVersion,
        _: Timestamp,
        _: &AuditEvent,
    ) -> Result<Changed<MilestoneView>, StoreError> {
        unreachable!()
    }

    async fn milestone(
        &self,
        scope: OrgScope,
        event: EventId,
        id: MilestoneId,
        _: Timestamp,
    ) -> Result<Option<MilestoneView>, StoreError> {
        let found = scope.organization_id() == testwil();
        Ok(self
            .milestones
            .lock()
            .unwrap()
            .iter()
            .find(|milestone| found && milestone.event_id == event && milestone.id == id)
            .cloned())
    }

    async fn milestones(
        &self,
        _: OrgScope,
        _: EventId,
        _: &WorkFilter<MilestoneStatus>,
        _: Timestamp,
    ) -> Result<Vec<MilestoneView>, StoreError> {
        unreachable!()
    }
}

/// The proposal checks only read decisions.
#[async_trait]
impl DecisionStore for Memory {
    async fn create_decision(
        &self,
        _: OrgScope,
        _: &NewDecisionRecord,
        _: Timestamp,
        _: &AuditEvent,
    ) -> Result<Created<DecisionView>, StoreError> {
        unreachable!()
    }

    async fn change_decision(
        &self,
        _: OrgScope,
        _: EventId,
        _: DecisionId,
        _: &DecisionFields,
        _: RecordVersion,
        _: Timestamp,
        _: &AuditEvent,
    ) -> Result<Changed<DecisionView>, StoreError> {
        unreachable!()
    }

    async fn supersede_decision(
        &self,
        _: OrgScope,
        _: EventId,
        _: DecisionId,
        _: &NewDecisionRecord,
        _: RecordVersion,
        _: Timestamp,
        _: &[AuditEvent],
    ) -> Result<Created<Changed<Superseded>>, StoreError> {
        unreachable!()
    }

    async fn revoke_decision(
        &self,
        _: OrgScope,
        _: EventId,
        _: DecisionId,
        _: &RecordReason,
        _: RecordVersion,
        _: Timestamp,
        _: &AuditEvent,
    ) -> Result<Changed<DecisionView>, StoreError> {
        unreachable!()
    }

    async fn decision(
        &self,
        scope: OrgScope,
        event: EventId,
        id: DecisionId,
    ) -> Result<Option<DecisionView>, StoreError> {
        let found = scope.organization_id() == testwil();
        Ok(self
            .decisions
            .lock()
            .unwrap()
            .iter()
            .find(|decision| found && decision.event_id == event && decision.id == id)
            .cloned())
    }

    async fn decisions(
        &self,
        _: OrgScope,
        _: EventId,
        _: &WorkFilter<DecisionStatus>,
    ) -> Result<Vec<DecisionView>, StoreError> {
        unreachable!()
    }
}

/// The proposal checks only read risks.
#[async_trait]
impl RiskStore for Memory {
    async fn create_risk(
        &self,
        _: OrgScope,
        _: &NewRiskRecord,
        _: Timestamp,
        _: &AuditEvent,
    ) -> Result<Created<RiskView>, StoreError> {
        unreachable!()
    }

    async fn change_risk(
        &self,
        _: OrgScope,
        _: EventId,
        _: RiskId,
        _: &RiskFields,
        _: RecordVersion,
        _: Timestamp,
        _: &AuditEvent,
    ) -> Result<Changed<RiskView>, StoreError> {
        unreachable!()
    }

    async fn risk(
        &self,
        scope: OrgScope,
        event: EventId,
        id: RiskId,
    ) -> Result<Option<RiskView>, StoreError> {
        let found = scope.organization_id() == testwil();
        Ok(self
            .risks
            .lock()
            .unwrap()
            .iter()
            .find(|risk| found && risk.event_id == event && risk.id == id)
            .cloned())
    }

    async fn risks(
        &self,
        _: OrgScope,
        _: EventId,
        _: &WorkFilter<RiskStatus, RiskCursor>,
    ) -> Result<Vec<RiskView>, StoreError> {
        unreachable!()
    }
}

fn action(id: Uuid, workstream: Option<WorkstreamId>) -> Value {
    json!({
        "kind": "create-action", "id": id, "event_id": open_day().as_uuid(),
        "title": "Bewilligung klären", "owner": anna().as_uuid(),
        "workstream": workstream.map(WorkstreamId::as_uuid), "due_date": "2030-04-30",
    })
}

fn commitment(promisor: Value) -> Value {
    json!({
        "kind": "create-commitment", "id": Uuid::now_v7(), "event_id": open_day().as_uuid(),
        "text": "Liefert das Zelt", "promisor": promisor, "owner": anna().as_uuid(),
        "condition": "wenn der Auftrag unterschrieben ist",
    })
}

fn person(id: Uuid) -> Value {
    json!({"kind": "create-person", "id": id, "name": "Moritz Muster", "email": "moritz@example.org"})
}

fn workstream(memory: &Memory, event: EventId, status: WorkstreamStatus) -> WorkstreamId {
    let id = WorkstreamId::from_uuid(Uuid::now_v7());
    memory.workstreams.lock().unwrap().push(Workstream {
        id,
        event_id: event,
        name: tada_domain::redaction::Redactable::Present(
            WorkstreamName::parse("Bodenbetrieb").unwrap(),
        ),
        lead: anna(),
        status,
        version: RecordVersion::FIRST,
    });
    id
}

#[tokio::test]
async fn a_commitment_can_name_a_promisor_from_its_changeset_only_as_a_dependency() {
    let memory = Memory::default();
    let anna = contributor(&memory);
    let (new_person, proposals) = (Uuid::now_v7(), [Uuid::now_v7(), Uuid::now_v7()]);
    let intake = |depends_on: &[Uuid], promisor: Value| {
        changeset(
            Some(open_day()),
            vec![
                proposal(proposals[0], person(new_person), &[], "Das Open Day"),
                proposal(
                    proposals[1],
                    commitment(promisor),
                    depends_on,
                    "Das Open Day",
                ),
            ],
        )
    };

    let result = create_changeset(
        &anna,
        intake(&[], json!({"person": new_person})),
        stores(&memory),
        &FixedClock,
    )
    .await;
    assert_eq!(
        invalid_fields(result),
        [("proposals/1/depends_on".to_owned(), "dependency-missing")]
    );

    // The new record is a person, so it cannot be the institution of the commitment.
    let result = create_changeset(
        &anna,
        intake(&[proposals[0]], json!({"institution": new_person})),
        stores(&memory),
        &FixedClock,
    )
    .await;
    assert_eq!(
        invalid_fields(result),
        [(
            "proposals/1/operation/promisor".to_owned(),
            "unknown-record"
        )]
    );

    // A person of another organization or an unknown ID is not a promisor.
    let unknown = changeset(
        Some(open_day()),
        vec![proposal(
            Uuid::now_v7(),
            commitment(json!({"person": Uuid::now_v7()})),
            &[],
            "Das Open Day",
        )],
    );
    let result = create_changeset(&anna, unknown, stores(&memory), &FixedClock).await;
    assert_eq!(
        invalid_fields(result),
        [(
            "proposals/0/operation/promisor".to_owned(),
            "unknown-record"
        )]
    );

    let created = create_changeset(
        &anna,
        intake(&[proposals[0]], json!({"person": new_person})),
        stores(&memory),
        &FixedClock,
    )
    .await;
    let stored = changeset_of(created.unwrap());
    let operation = |id: Uuid| {
        &stored
            .proposals
            .iter()
            .find(|proposal| proposal.id.as_uuid() == id)
            .unwrap()
            .operation
    };
    assert_eq!(operation(proposals[0]).event_id(), None);
    let Operation::CreateCommitment {
        promisor,
        condition,
        ..
    } = operation(proposals[1])
    else {
        panic!("not a commitment");
    };
    assert_eq!(*promisor, Party::Person(PersonId::from_uuid(new_person)));
    assert!(condition.is_some());

    // An existing person of the organization needs no dependency.
    let known = Uuid::now_v7();
    memory.parties.lock().unwrap().push(known);
    let existing = changeset(
        Some(open_day()),
        vec![proposal(
            Uuid::now_v7(),
            commitment(json!({"person": known})),
            &[],
            "Das Open Day",
        )],
    );
    let result = create_changeset(&anna, existing, stores(&memory), &FixedClock).await;
    assert!(result.is_ok(), "{result:?}");
}

#[tokio::test]
async fn a_proposal_cannot_set_a_closed_workstream() {
    let memory = Memory::default();
    let anna = contributor(&memory);
    let active = workstream(&memory, open_day(), WorkstreamStatus::Active);
    let closed = workstream(&memory, open_day(), WorkstreamStatus::Closed);
    let elsewhere = workstream(&memory, other_event(), WorkstreamStatus::Active);
    let intake = |workstream: WorkstreamId| {
        changeset(
            Some(open_day()),
            vec![proposal(
                Uuid::now_v7(),
                action(Uuid::now_v7(), Some(workstream)),
                &[],
                "im Mai oder Juni 2030",
            )],
        )
    };
    for (workstream, code) in [(closed, "closed"), (elsewhere, "unknown-record")] {
        let result =
            create_changeset(&anna, intake(workstream), stores(&memory), &FixedClock).await;
        assert_eq!(
            invalid_fields(result),
            [("proposals/0/operation/workstream".to_owned(), code)]
        );
    }
    let result = create_changeset(&anna, intake(active), stores(&memory), &FixedClock).await;
    let stored = changeset_of(result.unwrap());
    let Operation::CreateAction {
        due_date,
        workstream,
        ..
    } = &stored.proposals[0].operation
    else {
        panic!("not an action");
    };
    assert_eq!(*due_date, Some(Date::constant(2030, 4, 30)));
    assert_eq!(*workstream, Some(active));
}

fn new_workstream(id: Uuid, name: &str, lead: UserId) -> Value {
    json!({
        "kind": "create-workstream", "id": id, "event_id": open_day().as_uuid(),
        "name": name, "lead": lead.as_uuid(),
    })
}

#[tokio::test]
async fn a_workstream_proposal_checks_its_lead_and_name() {
    let memory = Memory::default();
    let proposer = contributor(&memory);
    workstream(&memory, open_day(), WorkstreamStatus::Active);
    let intake = |name: &str, lead: UserId| {
        changeset(
            Some(open_day()),
            vec![proposal(
                Uuid::now_v7(),
                new_workstream(Uuid::now_v7(), name, lead),
                &[],
                "Das Open Day",
            )],
        )
    };
    for (name, lead, field, code) in [
        ("Programm", bruno(), "lead", "unknown-member"),
        (
            "Programm",
            UserId::from_uuid(Uuid::from_u128(99)),
            "lead",
            "unknown-member",
        ),
        ("bodenbetrieb", anna(), "name", "taken"),
        (&"a".repeat(201), anna(), "name", "too-long"),
        ("", anna(), "name", "empty"),
    ] {
        let result =
            create_changeset(&proposer, intake(name, lead), stores(&memory), &FixedClock).await;
        assert_eq!(
            invalid_fields(result),
            [(format!("proposals/0/operation/{field}"), code)]
        );
    }
    let result = create_changeset(
        &proposer,
        intake("Programm", anna()),
        stores(&memory),
        &FixedClock,
    )
    .await;
    let stored = changeset_of(result.unwrap());
    assert!(matches!(
        stored.proposals[0].operation,
        Operation::CreateWorkstream { .. }
    ));
}

#[tokio::test]
async fn a_closed_workstream_keeps_its_name() {
    let memory = Memory::default();
    let proposer = contributor(&memory);
    workstream(&memory, open_day(), WorkstreamStatus::Closed);
    let intake = changeset(
        Some(open_day()),
        vec![proposal(
            Uuid::now_v7(),
            new_workstream(Uuid::now_v7(), "Bodenbetrieb", anna()),
            &[],
            "Das Open Day",
        )],
    );
    let result = create_changeset(&proposer, intake, stores(&memory), &FixedClock).await;
    assert_eq!(
        invalid_fields(result),
        [("proposals/0/operation/name".to_owned(), "taken")]
    );
}

#[tokio::test]
async fn two_workstreams_of_a_changeset_cannot_differ_only_in_case() {
    let memory = Memory::default();
    let proposer = contributor(&memory);
    let intake = changeset(
        Some(open_day()),
        vec![
            proposal(
                Uuid::now_v7(),
                new_workstream(Uuid::now_v7(), "Programm", anna()),
                &[],
                "Das Open Day",
            ),
            proposal(
                Uuid::now_v7(),
                new_workstream(Uuid::now_v7(), "PROGRAMM", anna()),
                &[],
                "Das Open Day",
            ),
        ],
    );
    let result = create_changeset(&proposer, intake, stores(&memory), &FixedClock).await;
    assert_eq!(
        invalid_fields(result),
        [("proposals/1/operation/name".to_owned(), "taken")]
    );
}

#[tokio::test]
async fn an_action_can_name_a_workstream_of_its_changeset_as_a_dependency() {
    let memory = Memory::default();
    let proposer = contributor(&memory);
    let (new, proposals) = (Uuid::now_v7(), [Uuid::now_v7(), Uuid::now_v7()]);
    let intake = |depends_on: &[Uuid]| {
        changeset(
            Some(open_day()),
            vec![
                proposal(
                    proposals[0],
                    new_workstream(new, "Programm", anna()),
                    &[],
                    "Das Open Day",
                ),
                proposal(
                    proposals[1],
                    action(Uuid::now_v7(), Some(WorkstreamId::from_uuid(new))),
                    depends_on,
                    "Das Open Day",
                ),
            ],
        )
    };
    let result = create_changeset(&proposer, intake(&[]), stores(&memory), &FixedClock).await;
    assert_eq!(
        invalid_fields(result),
        [("proposals/1/depends_on".to_owned(), "dependency-missing")]
    );
    let result = create_changeset(
        &proposer,
        intake(&[proposals[0]]),
        stores(&memory),
        &FixedClock,
    )
    .await;
    assert!(result.is_ok(), "{result:?}");
}

#[tokio::test]
async fn an_action_needs_an_owner_who_contributes_to_the_event() {
    let memory = Memory::default();
    let anna = contributor(&memory);
    for owner in [bruno(), UserId::from_uuid(Uuid::from_u128(99))] {
        let mut operation = action(Uuid::now_v7(), None);
        operation["owner"] = json!(owner.as_uuid());
        let intake = changeset(
            Some(open_day()),
            vec![proposal(
                Uuid::now_v7(),
                operation,
                &[],
                "im Mai oder Juni 2030",
            )],
        );
        let result = create_changeset(&anna, intake, stores(&memory), &FixedClock).await;
        assert_eq!(
            invalid_fields(result),
            [("proposals/0/operation/owner".to_owned(), "unknown-member")]
        );
    }
}

fn existing_action(memory: &Memory, status: ActionStatus) -> ActionId {
    let id = ActionId::from_uuid(Uuid::now_v7());
    memory.actions.lock().unwrap().push(ActionView {
        id,
        local_number: 1,
        event_id: open_day(),
        fields: ActionFields {
            title: tada_domain::redaction::Redactable::Present(
                ActionTitle::parse("Bewilligung klären").unwrap(),
            ),
            description: None,
            owner: anna(),
            workstream_id: None,
            start_date: None,
            due_date: None,
            milestone_id: None,
            status,
        },
        version: RecordVersion::new(2).unwrap(),
    });
    id
}

fn existing_commitment(memory: &Memory, status: CommitmentStatus) -> CommitmentId {
    let id = CommitmentId::from_uuid(Uuid::now_v7());
    let person = PersonId::from_uuid(Uuid::now_v7());
    memory.commitments.lock().unwrap().push(CommitmentView {
        id,
        local_number: 1,
        event_id: open_day(),
        condition: Some(ConditionText::parse("wenn unterschrieben").unwrap()),
        promisor: PartyRef {
            party: Party::Person(person),
            local_id: LocalIdKind::Person.readable_id(1),
            name: tada_domain::redaction::Redactable::Present(
                PartyName::parse("Moritz Muster").unwrap(),
            ),
        },
        fields: CommitmentFields {
            text: tada_domain::redaction::Redactable::Present(
                CommitmentText::parse("Liefert das Zelt").unwrap(),
            ),
            owner: anna(),
            workstream_id: None,
            due_date: None,
            status,
            firm_reason: None,
        },
        version: RecordVersion::FIRST,
    });
    id
}

fn one(operation: Value, reason: &str) -> NewChangeset {
    let mut proposal = proposal(Uuid::now_v7(), operation, &[], "im Mai oder Juni 2030");
    proposal["reason"] = json!(reason);
    changeset(Some(open_day()), vec![proposal])
}

#[tokio::test]
async fn a_change_of_status_needs_an_allowed_transition() {
    let memory = Memory::default();
    let anna = contributor(&memory);
    let done = existing_action(&memory, ActionStatus::Done);
    let action_status = |id: ActionId, status: &str| {
        json!({
            "kind": "change-action-status", "event_id": open_day().as_uuid(),
            "action_id": id.as_uuid(), "status": status, "expected_version": 2,
        })
    };
    let result = create_changeset(
        &anna,
        one(action_status(done, "in-progress"), "Es geht weiter."),
        stores(&memory),
        &FixedClock,
    )
    .await;
    assert_eq!(
        invalid_fields(result),
        [(
            "proposals/0/operation/status".to_owned(),
            "invalid-transition"
        )]
    );
    let unknown = ActionId::from_uuid(Uuid::now_v7());
    let result = create_changeset(
        &anna,
        one(action_status(unknown, "done"), "Erledigt."),
        stores(&memory),
        &FixedClock,
    )
    .await;
    assert_eq!(
        invalid_fields(result),
        [(
            "proposals/0/operation/action_id".to_owned(),
            "unknown-record"
        )]
    );
    let reopened = create_changeset(
        &anna,
        one(action_status(done, "open"), "Es geht weiter."),
        stores(&memory),
        &FixedClock,
    )
    .await;
    assert!(reopened.is_ok(), "{reopened:?}");

    let conditional = existing_commitment(&memory, CommitmentStatus::Conditional);
    let firm = existing_commitment(&memory, CommitmentStatus::Firm);
    let commitment_status = |id: CommitmentId, status: &str| {
        json!({
            "kind": "change-commitment-status", "event_id": open_day().as_uuid(),
            "commitment_id": id.as_uuid(), "status": status, "expected_version": 1,
        })
    };
    let result = create_changeset(
        &anna,
        one(commitment_status(firm, "conditional"), "Doch nicht."),
        stores(&memory),
        &FixedClock,
    )
    .await;
    assert_eq!(
        invalid_fields(result),
        [(
            "proposals/0/operation/status".to_owned(),
            "invalid-transition"
        )]
    );
    // The reason of a change to firm becomes the reason of the commitment, which has at most 500 characters.
    let result = create_changeset(
        &anna,
        one(commitment_status(conditional, "firm"), &"a".repeat(501)),
        stores(&memory),
        &FixedClock,
    )
    .await;
    assert_eq!(
        invalid_fields(result),
        [("proposals/0/reason".to_owned(), "too-long")]
    );
    let result = create_changeset(
        &anna,
        one(
            commitment_status(conditional, "firm"),
            "Der Auftrag ist unterschrieben.",
        ),
        stores(&memory),
        &FixedClock,
    )
    .await;
    assert!(result.is_ok(), "{result:?}");
}

#[tokio::test]
async fn a_person_belongs_to_no_event_and_fits_each_changeset() {
    let memory = Memory::default();
    let anna = contributor(&memory);
    let intake = changeset(
        Some(open_day()),
        vec![proposal(
            Uuid::now_v7(),
            person(Uuid::now_v7()),
            &[],
            "Das Open Day",
        )],
    );
    assert!(
        create_changeset(&anna, intake, stores(&memory), &FixedClock)
            .await
            .is_ok()
    );
    let owner = caller(OrganizationRole::Owner);
    let organization = changeset(
        None,
        vec![proposal(
            Uuid::now_v7(),
            json!({"kind": "create-institution", "id": Uuid::now_v7(), "name": "Zeltbau AG",
                   "institution_kind": "company"}),
            &[],
            "Das Open Day",
        )],
    );
    let result = create_changeset(&owner, organization, stores(&memory), &FixedClock).await;
    assert!(result.is_ok(), "{result:?}");
}

/// A decision of `event` in `state`, decided by a person of Testwil.
fn existing_decision(memory: &Memory, event: EventId, state: DecisionState) -> DecisionId {
    let id = DecisionId::from_uuid(Uuid::now_v7());
    let person = PersonId::from_uuid(Uuid::now_v7());
    memory.decisions.lock().unwrap().push(DecisionView {
        id,
        local_number: 1,
        event_id: event,
        wording: tada_domain::redaction::Redactable::Present(
            DecisionWording::parse("Das Fest findet bei jedem Wetter statt.").unwrap(),
        ),
        decided_on: jiff::civil::date(2030, 3, 1),
        parties: vec![PartyRef {
            party: Party::Person(person),
            local_id: LocalIdKind::Person.readable_id(1),
            name: tada_domain::redaction::Redactable::Present(
                PartyName::parse("Moritz Muster").unwrap(),
            ),
        }],
        fields: DecisionFields {
            owner: anna(),
            workstream_id: None,
        },
        state,
        version: RecordVersion::FIRST,
    });
    id
}

fn supersede(old: DecisionId, successor: Uuid) -> Value {
    json!({
        "kind": "supersede-decision", "event_id": open_day().as_uuid(),
        "decision_id": old.as_uuid(), "successor": successor, "expected_version": 1,
    })
}

fn new_decision(id: Uuid, decided_by: Value) -> Value {
    json!({
        "kind": "create-decision", "id": id, "event_id": open_day().as_uuid(),
        "wording": "Das Fest findet nur bei trockenem Wetter statt.\nSonst fällt es aus.",
        "decided_on": "2030-03-01", "decided_by": decided_by, "owner": anna().as_uuid(),
    })
}

/// A supersede names a successor in force of the same event, or a new decision of its changeset (ADR 0081).
/// A decision of another event, the decision itself, an unknown one and an ended one give `unknown-record`.
#[tokio::test]
async fn supersede_refuses_foreign_self_and_ended_decisions() {
    let memory = Memory::default();
    let anna = contributor(&memory);
    let old = existing_decision(&memory, open_day(), DecisionState::InForce);
    let live = existing_decision(&memory, open_day(), DecisionState::InForce);
    let foreign = existing_decision(&memory, other_event(), DecisionState::InForce);
    let superseded = existing_decision(&memory, open_day(), DecisionState::Superseded { by: live });
    let revoked = existing_decision(
        &memory,
        open_day(),
        DecisionState::Revoked {
            reason: tada_domain::redaction::Redactable::Present(
                RecordReason::parse("Die Gemeinde verbietet es.").unwrap(),
            ),
        },
    );
    for successor in [
        foreign.as_uuid(),
        old.as_uuid(),
        superseded.as_uuid(),
        revoked.as_uuid(),
        Uuid::now_v7(),
    ] {
        let result = create_changeset(
            &anna,
            one(supersede(old, successor), "Der neue Entscheid gilt."),
            stores(&memory),
            &FixedClock,
        )
        .await;
        assert_eq!(
            invalid_fields(result),
            [(
                "proposals/0/operation/successor".to_owned(),
                "unknown-record"
            )],
            "{successor}"
        );
    }
    // An ended decision takes no successor either.
    let result = create_changeset(
        &anna,
        one(
            supersede(revoked, live.as_uuid()),
            "Der neue Entscheid gilt.",
        ),
        stores(&memory),
        &FixedClock,
    )
    .await;
    assert_eq!(
        invalid_fields(result),
        [(
            "proposals/0/operation/decision_id".to_owned(),
            "invalid-transition"
        )]
    );
    let accepted = create_changeset(
        &anna,
        one(supersede(old, live.as_uuid()), "Der neue Entscheid gilt."),
        stores(&memory),
        &FixedClock,
    )
    .await;
    assert!(accepted.is_ok(), "{accepted:?}");

    // A new successor of the changeset needs the dependency on its proposal.
    let person = Uuid::now_v7();
    memory.parties.lock().unwrap().push(person);
    let (successor, proposals) = (Uuid::now_v7(), [Uuid::now_v7(), Uuid::now_v7()]);
    let intake = |depends_on: &[Uuid]| {
        changeset(
            Some(open_day()),
            vec![
                proposal(
                    proposals[0],
                    new_decision(successor, json!([{"person": person}])),
                    &[],
                    "Das Open Day",
                ),
                proposal(
                    proposals[1],
                    supersede(old, successor),
                    depends_on,
                    "Das Open Day",
                ),
            ],
        )
    };
    let result = create_changeset(&anna, intake(&[]), stores(&memory), &FixedClock).await;
    assert_eq!(
        invalid_fields(result),
        [("proposals/1/depends_on".to_owned(), "dependency-missing")]
    );
    let result =
        create_changeset(&anna, intake(&[proposals[0]]), stores(&memory), &FixedClock).await;
    assert!(result.is_ok(), "{result:?}");
}

/// The parties of a new decision are persons or institutions of the organization, or new ones of the changeset.
#[tokio::test]
async fn a_decision_names_known_parties_and_keeps_its_line_breaks() {
    let memory = Memory::default();
    let anna = contributor(&memory);
    let known = Uuid::now_v7();
    memory.parties.lock().unwrap().push(known);
    let unknown = Uuid::now_v7();
    let result = create_changeset(
        &anna,
        one(
            new_decision(
                Uuid::now_v7(),
                json!([{"person": known}, {"institution": unknown}]),
            ),
            "Der Vorstand hat entschieden.",
        ),
        stores(&memory),
        &FixedClock,
    )
    .await;
    assert_eq!(
        invalid_fields(result),
        [(
            "proposals/0/operation/decided_by/1".to_owned(),
            "unknown-record"
        )]
    );
    // The path of an error counts the position in the given list, also after the duplicates are dropped.
    let result = create_changeset(
        &anna,
        one(
            new_decision(
                Uuid::now_v7(),
                json!([{"person": known}, {"person": known}, {"institution": unknown}]),
            ),
            "Der Vorstand hat entschieden.",
        ),
        stores(&memory),
        &FixedClock,
    )
    .await;
    assert_eq!(
        invalid_fields(result),
        [(
            "proposals/0/operation/decided_by/2".to_owned(),
            "unknown-record"
        )]
    );
    let result = create_changeset(
        &anna,
        one(
            new_decision(Uuid::now_v7(), json!([])),
            "Der Vorstand hat entschieden.",
        ),
        stores(&memory),
        &FixedClock,
    )
    .await;
    assert_eq!(
        invalid_fields(result),
        [("proposals/0/operation/decided_by".to_owned(), "required")]
    );
    let created = create_changeset(
        &anna,
        one(
            new_decision(Uuid::now_v7(), json!([{"person": known}])),
            "Der Vorstand hat entschieden.",
        ),
        stores(&memory),
        &FixedClock,
    )
    .await
    .unwrap();
    let Operation::CreateDecision {
        wording,
        decided_by,
        ..
    } = &changeset_of(created).proposals[0].operation
    else {
        panic!("not a decision");
    };
    assert_eq!(
        wording.as_str(),
        "Das Fest findet nur bei trockenem Wetter statt.\nSonst fällt es aus."
    );
    assert_eq!(
        decided_by,
        &DecisionParties::new(vec![Party::Person(PersonId::from_uuid(known))]).unwrap()
    );
}

fn existing_risk(memory: &Memory, status: RiskStatus) -> RiskId {
    let id = RiskId::from_uuid(Uuid::now_v7());
    memory.risks.lock().unwrap().push(RiskView {
        id,
        local_number: 1,
        event_id: open_day(),
        fields: RiskFields {
            title: tada_domain::redaction::Redactable::Present(
                RiskTitle::parse("Starkregen am Festtag").unwrap(),
            ),
            description: None,
            rating: RiskRating {
                likelihood: Level::Medium,
                impact: Level::High,
            },
            mitigation: None,
            owner: anna(),
            workstream_id: None,
            status,
        },
        version: RecordVersion::new(2).unwrap(),
    });
    id
}

fn new_risk(id: Uuid) -> Value {
    json!({
        "kind": "create-risk", "id": id, "event_id": open_day().as_uuid(),
        "title": "Starkregen am Festtag", "description": "Das Zelt steht\nauf der Wiese.",
        "likelihood": "medium", "impact": "high", "mitigation": "Zweiter Eingang",
        "owner": anna().as_uuid(),
    })
}

#[tokio::test]
async fn a_risk_proposal_checks_its_texts_owner_and_transitions() {
    let memory = Memory::default();
    let anna = contributor(&memory);
    let created = create_changeset(
        &anna,
        one(new_risk(Uuid::now_v7()), "Der Wetterdienst warnt."),
        stores(&memory),
        &FixedClock,
    )
    .await;
    assert!(created.is_ok(), "{created:?}");

    // The limits, an owner who does not contribute, and a level that does not exist.
    let mut bad = new_risk(Uuid::now_v7());
    bad["title"] = json!("a".repeat(201));
    bad["mitigation"] = json!("c".repeat(4001));
    let result = create_changeset(&anna, one(bad, "Fehler."), stores(&memory), &FixedClock).await;
    let mut codes = invalid_fields(result);
    codes.sort();
    assert_eq!(
        codes,
        [
            ("proposals/0/operation/mitigation".to_owned(), "too-long"),
            ("proposals/0/operation/title".to_owned(), "too-long"),
        ]
    );
    let mut stranger = new_risk(Uuid::now_v7());
    stranger["owner"] = json!(Uuid::now_v7());
    let result = create_changeset(
        &anna,
        one(stranger, "Fehler."),
        stores(&memory),
        &FixedClock,
    )
    .await;
    assert_eq!(
        invalid_fields(result),
        [("proposals/0/operation/owner".to_owned(), "unknown-member")]
    );
    let mut level = new_risk(Uuid::now_v7());
    level["impact"] = json!("extreme");
    let rejected = serde_json::from_value::<crate::proposals::OperationInput>(level);
    assert!(rejected.is_err());

    let open = existing_risk(&memory, RiskStatus::Open);
    let closed = existing_risk(&memory, RiskStatus::Closed);
    let status = |id: RiskId, status: &str| {
        json!({
            "kind": "change-risk-status", "event_id": open_day().as_uuid(),
            "risk_id": id.as_uuid(), "status": status, "expected_version": 2,
        })
    };
    // closed does not change to mitigated; a risk that is unknown is refused.
    let result = create_changeset(
        &anna,
        one(status(closed, "mitigated"), "Doch nicht."),
        stores(&memory),
        &FixedClock,
    )
    .await;
    assert_eq!(
        invalid_fields(result),
        [(
            "proposals/0/operation/status".to_owned(),
            "invalid-transition"
        )]
    );
    let result = create_changeset(
        &anna,
        one(status(RiskId::from_uuid(Uuid::now_v7()), "closed"), "Weg."),
        stores(&memory),
        &FixedClock,
    )
    .await;
    assert_eq!(
        invalid_fields(result),
        [("proposals/0/operation/risk_id".to_owned(), "unknown-record")]
    );
    for ok in [status(closed, "open"), status(open, "mitigated")] {
        let result = create_changeset(
            &anna,
            one(ok, "Es hat sich geändert."),
            stores(&memory),
            &FixedClock,
        )
        .await;
        assert!(result.is_ok(), "{result:?}");
    }
    // A rating change names a known risk and has no transition.
    let rating = |id: RiskId| {
        json!({
            "kind": "change-risk-rating", "event_id": open_day().as_uuid(),
            "risk_id": id.as_uuid(), "likelihood": "high", "impact": "high", "expected_version": 2,
        })
    };
    let result = create_changeset(
        &anna,
        one(
            rating(RiskId::from_uuid(Uuid::now_v7())),
            "Es wird schlimmer.",
        ),
        stores(&memory),
        &FixedClock,
    )
    .await;
    assert_eq!(
        invalid_fields(result),
        [("proposals/0/operation/risk_id".to_owned(), "unknown-record")]
    );
    let result = create_changeset(
        &anna,
        one(rating(closed), "Es wird schlimmer."),
        stores(&memory),
        &FixedClock,
    )
    .await;
    assert!(result.is_ok(), "{result:?}");
}
