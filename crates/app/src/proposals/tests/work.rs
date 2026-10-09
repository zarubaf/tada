//! Proposals of work records and parties (ADR 0068, ADR 0069).

use jiff::civil::Date;
use tada_domain::ids::{
    ActionId, CommitmentId, InstitutionId, LocalIdKind, PersonId, WorkstreamId,
};
use tada_domain::parties::{Party, PartyName};
use tada_domain::work::{
    ActionStatus, ActionTitle, CommitmentStatus, CommitmentText, ConditionText, WorkstreamName,
    WorkstreamStatus,
};

use super::*;
use crate::parties::{
    InstitutionFields, InstitutionView, PartyChanged, PartyCursor, PartyRef, PersonFields,
    PersonView,
};
use crate::work::{
    ActionFields, CommitmentFields, MyWork, NewActionRecord, NewCommitmentRecord, WorkChanged,
    WorkCreated, WorkFilter,
};
use crate::workstreams::{Changed, Created as WorkstreamCreated, WorkstreamUpdate};

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
        _: &AuditEvent,
    ) -> Result<Changed, StoreError> {
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

    async fn list(&self, _: OrgScope, _: EventId) -> Result<Vec<Workstream>, StoreError> {
        unreachable!()
    }
}

#[async_trait]
impl PartyStore for Memory {
    async fn create_person(
        &self,
        _: OrgScope,
        _: PersonId,
        _: &PersonFields,
        _: Option<UserId>,
        _: Timestamp,
        _: &AuditEvent,
    ) -> Result<PersonView, StoreError> {
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
    ) -> Result<PartyChanged<PersonView>, StoreError> {
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
            name: PartyName::parse("Moritz Muster").unwrap(),
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
        _: Option<PartyCursor>,
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
    ) -> Result<InstitutionView, StoreError> {
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
    ) -> Result<PartyChanged<InstitutionView>, StoreError> {
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
        _: Option<PartyCursor>,
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
    ) -> Result<WorkCreated<ActionView>, StoreError> {
        unreachable!()
    }

    async fn change_action(
        &self,
        _: OrgScope,
        _: EventId,
        _: ActionId,
        _: &ActionFields,
        _: RecordVersion,
        _: &AuditEvent,
    ) -> Result<WorkChanged<ActionView>, StoreError> {
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
    ) -> Result<WorkCreated<CommitmentView>, StoreError> {
        unreachable!()
    }

    async fn change_commitment(
        &self,
        _: OrgScope,
        _: EventId,
        _: CommitmentId,
        _: &CommitmentFields,
        _: RecordVersion,
        _: &AuditEvent,
    ) -> Result<WorkChanged<CommitmentView>, StoreError> {
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

    async fn my_open_work(&self, _: OrgScope, _: UserId, _: bool) -> Result<MyWork, StoreError> {
        unreachable!()
    }
}

fn action(id: Uuid, workstream: Option<WorkstreamId>) -> Value {
    json!({
        "kind": "create-action", "id": id, "event_id": open_day().as_uuid(),
        "title": "Bewilligung klären", "owner": anna().as_uuid(),
        "workstream": workstream.map(WorkstreamId::as_uuid), "due": "2030-04-30",
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
        name: WorkstreamName::parse("Bodenbetrieb").unwrap(),
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
        due, workstream, ..
    } = &stored.proposals[0].operation
    else {
        panic!("not an action");
    };
    assert_eq!(*due, Some(Date::constant(2030, 4, 30)));
    assert_eq!(*workstream, Some(active));
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
            title: ActionTitle::parse("Bewilligung klären").unwrap(),
            description: None,
            owner: anna(),
            workstream_id: None,
            due_date: None,
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
            name: PartyName::parse("Moritz Muster").unwrap(),
        },
        fields: CommitmentFields {
            text: CommitmentText::parse("Liefert das Zelt").unwrap(),
            owner: anna(),
            workstream_id: None,
            due_date: None,
            status,
            firm_reason: None,
        },
        version: RecordVersion::FIRST,
        evidence: Vec::new(),
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
