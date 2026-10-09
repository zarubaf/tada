use jiff::civil::date;
use tada_app::audit::AuditAction;
use tada_app::caller::{MemberCaller, OrganizationRole};
use tada_app::domain::identity::{DisplayName, Email};
use tada_app::domain::parties::InstitutionKind;
use tada_app::parties::{InstitutionFields, PartyStore};

use super::*;
use crate::testing::TestDatabase;

struct Fixture {
    test: TestDatabase,
    scope: OrgScope,
    caller: MemberCaller,
    event: EventId,
    owner: UserId,
    supplier: InstitutionId,
}

const AT: &str = "2030-05-18T08:00:00Z";

impl Fixture {
    async fn start() -> Self {
        let test = TestDatabase::start().await;
        let testwil = test.create_organization("testwil").await;
        let event = test.create_event(testwil, "TEST30").await;
        let owner = test
            .create_user(
                &DisplayName::parse("Anna Muster").unwrap(),
                &Email::parse("anna@example.org").unwrap(),
            )
            .await;
        test.add_membership(testwil, owner, OrganizationRole::Owner)
            .await;
        sqlx::query(
            "INSERT INTO event_membership
                 (organization_id, event_id, user_id, event_role, created_at)
             VALUES ($1, $2, $3, 'event-manager', now())",
        )
        .bind(testwil.as_uuid())
        .bind(event.as_uuid())
        .bind(owner.as_uuid())
        .execute(&test.database.pool)
        .await
        .unwrap();
        let caller = MemberCaller::new(owner, testwil, OrganizationRole::Owner);
        let scope = caller.scope();
        let supplier = InstitutionId::from_uuid(Uuid::now_v7());
        let fields = InstitutionFields {
            name: PartyName::parse("Testwil Generatoren AG").unwrap(),
            kind: InstitutionKind::Company,
            email: None,
            phone: None,
        };
        let audit = AuditEvent::new(
            caller.actor(),
            AuditAction::InstitutionCreate,
            Some(supplier.as_uuid()),
            Some(scope),
        );
        test.database
            .create_institution(scope, supplier, &fields, AT.parse().unwrap(), &audit)
            .await
            .unwrap();
        Self {
            test,
            scope,
            caller,
            event,
            owner,
            supplier,
        }
    }

    fn audit(&self, action: AuditAction, id: Uuid) -> AuditEvent {
        AuditEvent::new(self.caller.actor(), action, Some(id), Some(self.scope))
    }

    fn action_fields(&self, title: &str, due: Option<jiff::civil::Date>) -> ActionFields {
        ActionFields {
            title: ActionTitle::parse(title).unwrap(),
            description: None,
            owner: self.owner,
            workstream_id: None,
            due_date: due,
            status: ActionStatus::Open,
        }
    }

    async fn action(&self, title: &str, due: Option<jiff::civil::Date>) -> ActionView {
        let record = NewActionRecord {
            id: ActionId::from_uuid(Uuid::now_v7()),
            event_id: self.event,
            fields: self.action_fields(title, due),
        };
        let audit = self.audit(AuditAction::ActionCreate, record.id.as_uuid());
        match self
            .test
            .database
            .create_action(self.scope, &record, AT.parse().unwrap(), &audit)
            .await
            .unwrap()
        {
            WorkCreated::Created(view) => view,
            WorkCreated::IdTaken => panic!("id taken"),
        }
    }

    async fn commitment(&self, condition: Option<&str>) -> CommitmentView {
        let condition = condition.map(|text| ConditionText::parse(text).unwrap());
        let record = NewCommitmentRecord {
            id: CommitmentId::from_uuid(Uuid::now_v7()),
            event_id: self.event,
            promisor: Party::Institution(self.supplier),
            fields: CommitmentFields {
                text: CommitmentText::parse("Generator delivery Friday 15:00").unwrap(),
                owner: self.owner,
                workstream_id: None,
                due_date: None,
                status: CommitmentStatus::initial(condition.as_ref()),
                firm_reason: None,
            },
            condition,
        };
        let audit = self.audit(AuditAction::CommitmentCreate, record.id.as_uuid());
        match self
            .test
            .database
            .create_commitment(self.scope, &record, AT.parse().unwrap(), &audit)
            .await
            .unwrap()
        {
            WorkCreated::Created(view) => view,
            WorkCreated::IdTaken => panic!("id taken"),
        }
    }

    async fn change_action(
        &self,
        scope: OrgScope,
        event: EventId,
        id: ActionId,
        fields: &ActionFields,
        expected: RecordVersion,
    ) -> WorkChanged<ActionView> {
        self.test
            .database
            .change_action(
                scope,
                event,
                id,
                fields,
                expected,
                &self.audit(AuditAction::ActionChange, id.as_uuid()),
            )
            .await
            .unwrap()
    }

    async fn audit_actions(&self) -> Vec<String> {
        sqlx::query_scalar("SELECT action FROM audit_event ORDER BY occurred_at, id")
            .fetch_all(&self.test.database.pool)
            .await
            .unwrap()
    }
}

#[tokio::test]
async fn creates_and_changes_an_action_with_numbers_and_audit_events() {
    let f = Fixture::start().await;
    let first = f
        .action("Generator bestellen", Some(date(2030, 6, 1)))
        .await;
    let second = f.action("Zaun stellen", None).await;
    assert_eq!(
        (first.local_id(), second.local_id()),
        ("ACT-001".into(), "ACT-002".into())
    );
    assert_eq!(first.fields.due_date, Some(date(2030, 6, 1)));
    assert_eq!(
        f.test
            .database
            .action(f.scope, f.event, first.id)
            .await
            .unwrap(),
        Some(first.clone())
    );

    let fields = ActionFields {
        description: Some(ActionDescription::parse("Mit Diesel").unwrap()),
        due_date: None,
        status: ActionStatus::Done,
        ..first.fields.clone()
    };
    let WorkChanged::Changed(changed) = f
        .change_action(f.scope, f.event, first.id, &fields, RecordVersion::FIRST)
        .await
    else {
        panic!("not changed");
    };
    assert_eq!(changed.fields, fields);
    assert_eq!(changed.version.get(), 2);
    assert_eq!(
        f.change_action(f.scope, f.event, first.id, &fields, RecordVersion::FIRST)
            .await,
        WorkChanged::VersionConflict
    );

    let filter = WorkFilter {
        owner: Some(f.owner),
        status: Some(ActionStatus::Open),
        workstream: None,
        after: None,
        limit: 10,
    };
    let open = f
        .test
        .database
        .actions(f.scope, f.event, &filter)
        .await
        .unwrap();
    assert_eq!(open, [second]);
    assert_eq!(
        f.audit_actions().await,
        [
            "institution.create",
            "action.create",
            "action.create",
            "action.change"
        ]
    );
}

/// Another event and another organization neither see nor change the record (ADR 0006).
#[tokio::test]
async fn keeps_work_records_inside_their_event_and_organization() {
    let f = Fixture::start().await;
    let db = &f.test.database;
    let action = f.action("Generator bestellen", None).await;
    let commitment = f.commitment(None).await;
    let other_event = f
        .test
        .create_event(f.scope.organization_id(), "TEST31")
        .await;
    assert_eq!(
        db.action(f.scope, other_event, action.id).await.unwrap(),
        None
    );
    assert_eq!(
        db.commitment(f.scope, other_event, commitment.id)
            .await
            .unwrap(),
        None
    );
    // The counter of readable IDs counts in each event.
    let other = NewActionRecord {
        id: ActionId::from_uuid(Uuid::now_v7()),
        event_id: other_event,
        fields: f.action_fields("Bar", None),
    };
    let WorkCreated::Created(other) = db
        .create_action(
            f.scope,
            &other,
            AT.parse().unwrap(),
            &f.audit(AuditAction::ActionCreate, other.id.as_uuid()),
        )
        .await
        .unwrap()
    else {
        panic!("not created");
    };
    assert_eq!(other.local_id(), "ACT-001");

    let musterhausen = f.test.create_organization("musterhausen").await;
    let stranger = MemberCaller::new(f.owner, musterhausen, OrganizationRole::Owner).scope();
    assert_eq!(db.action(stranger, f.event, action.id).await.unwrap(), None);
    assert_eq!(
        db.commitment(stranger, f.event, commitment.id)
            .await
            .unwrap(),
        None
    );
    assert_eq!(
        f.change_action(
            stranger,
            f.event,
            action.id,
            &action.fields,
            RecordVersion::FIRST
        )
        .await,
        WorkChanged::NotFound
    );
    assert!(
        db.my_open_work(stranger, f.owner, false)
            .await
            .unwrap()
            .actions
            .is_empty()
    );
}

/// Two changes with the same expected version: exactly one wins.
#[tokio::test]
async fn two_changes_with_one_version_leave_one_winner() {
    let f = Fixture::start().await;
    let action = f.action("Generator bestellen", None).await;
    let alpha = f.action_fields("Alpha", None);
    let beta = f.action_fields("Beta", None);
    let (a, b) = tokio::join!(
        f.change_action(f.scope, f.event, action.id, &alpha, RecordVersion::FIRST),
        f.change_action(f.scope, f.event, action.id, &beta, RecordVersion::FIRST)
    );
    let winners = [&a, &b]
        .iter()
        .filter(|changed| matches!(changed, WorkChanged::Changed(_)))
        .count();
    assert_eq!(winners, 1, "{a:?} {b:?}");
    assert!([&a, &b].contains(&&WorkChanged::VersionConflict));
}

#[tokio::test]
async fn a_commitment_keeps_its_condition_and_its_firm_reason() {
    let f = Fixture::start().await;
    let db = &f.test.database;
    let commitment = f.commitment(Some("subject to signed order")).await;
    assert_eq!(commitment.local_id(), "COM-001");
    assert_eq!(commitment.fields.status, CommitmentStatus::Conditional);
    assert_eq!(commitment.promisor.local_id, "INS-001");
    assert_eq!(commitment.promisor.party, Party::Institution(f.supplier));
    assert!(commitment.evidence.is_empty());

    // The schema refuses a conditional commitment with a firm reason.
    let refused = sqlx::query("UPDATE commitment SET firm_reason = 'x' WHERE id = $1")
        .bind(commitment.id.as_uuid())
        .execute(&db.pool)
        .await;
    assert!(refused.is_err());

    let fields = CommitmentFields {
        status: CommitmentStatus::Firm,
        firm_reason: Some(FirmReason::parse("The order is signed.").unwrap()),
        ..commitment.fields.clone()
    };
    let WorkChanged::Changed(firm) = db
        .change_commitment(
            f.scope,
            f.event,
            commitment.id,
            &fields,
            RecordVersion::FIRST,
            &f.audit(AuditAction::CommitmentFirm, commitment.id.as_uuid()),
        )
        .await
        .unwrap()
    else {
        panic!("not changed");
    };
    assert_eq!(firm.fields, fields);
    assert_eq!(firm.condition, commitment.condition);
    assert_eq!(
        f.audit_actions().await,
        ["institution.create", "commitment.create", "commitment.firm"]
    );
}

/// Task 8 writes the evidence of accepted proposals; the read shows it with the capture time.
#[tokio::test]
async fn a_commitment_shows_its_evidence() {
    let f = Fixture::start().await;
    let db = &f.test.database;
    let commitment = f.commitment(None).await;
    let organization = f.scope.organization_id().as_uuid();
    let (item, version, changeset, proposal) = (
        Uuid::now_v7(),
        Uuid::now_v7(),
        Uuid::now_v7(),
        Uuid::now_v7(),
    );
    let author = serde_json::json!({
        "kind": "member", "id": f.owner.as_uuid(), "principal": null,
        "channel": "web", "request_id": null,
    });
    let text = "Lieferung Freitag 15:00, Bestellung unterschrieben.";
    sqlx::query(
        "INSERT INTO source_item (id, organization_id, event_id, kind, created_at)
         VALUES ($1, $2, $3, 'member-text', '2030-05-02T08:00:00Z')",
    )
    .bind(item)
    .bind(organization)
    .bind(f.event.as_uuid())
    .execute(&db.pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO source_version
             (id, organization_id, source_item_id, kind, channel, author_actor, text, sha256, captured_at)
         VALUES ($1, $2, $3, 'member-text', 'web', $4, $5, sha256(convert_to($5, 'UTF8')),
                 '2030-05-02T08:00:00Z')",
    )
    .bind(version)
    .bind(organization)
    .bind(item)
    .bind(&author)
    .bind(text)
    .execute(&db.pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO changeset (id, organization_id, event_id, author, source_version_id, created_at)
         VALUES ($1, $2, $3, $4, $5, '2030-05-02T08:00:00Z')",
    )
    .bind(changeset)
    .bind(organization)
    .bind(f.event.as_uuid())
    .bind(&author)
    .bind(version)
    .execute(&db.pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO proposal
             (id, organization_id, changeset_id, event_id, operation, operation_version,
              target_kind, target_id, reason, created_at)
         VALUES ($1, $2, $3, $4, '{\"kind\": \"create_commitment\"}', 1, 'commitment', $5,
                 'Die Quelle nennt die Lieferung.', '2030-05-02T08:00:00Z')",
    )
    .bind(proposal)
    .bind(organization)
    .bind(changeset)
    .bind(f.event.as_uuid())
    .bind(commitment.id.as_uuid())
    .execute(&db.pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO record_evidence
             (id, organization_id, commitment_id, record_version, proposal_id, source_version_id,
              start_offset, end_offset, quote, page)
         VALUES ($1, $2, $3, 1, $4, $5, 0, 23, 'Lieferung Freitag 15:00', NULL)",
    )
    .bind(Uuid::now_v7())
    .bind(organization)
    .bind(commitment.id.as_uuid())
    .bind(proposal)
    .bind(version)
    .execute(&db.pool)
    .await
    .unwrap();

    let read = db
        .commitment(f.scope, f.event, commitment.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        read.evidence,
        [RecordEvidenceView {
            record_version: RecordVersion::FIRST,
            proposal_id: ProposalId::from_uuid(proposal),
            source_version_id: SourceVersionId::from_uuid(version),
            captured_at: "2030-05-02T08:00:00Z".parse().unwrap(),
            start_offset: 0,
            end_offset: 23,
            quote: "Lieferung Freitag 15:00".to_owned(),
            page: None,
        }]
    );
}

#[tokio::test]
async fn my_open_work_orders_by_due_date_and_leaves_out_closed_records() {
    let f = Fixture::start().await;
    let late = f.action("Spät", Some(date(2030, 6, 9))).await;
    let undated = f.action("Ohne Datum", None).await;
    let early = f.action("Früh", Some(date(2030, 6, 1))).await;
    let done = f.action("Erledigt", None).await;
    let fields = ActionFields {
        status: ActionStatus::Done,
        ..done.fields.clone()
    };
    f.change_action(f.scope, f.event, done.id, &fields, RecordVersion::FIRST)
        .await;
    let commitment = f.commitment(Some("subject to signed order")).await;

    let work = f
        .test
        .database
        .my_open_work(f.scope, f.owner, false)
        .await
        .unwrap();
    let ids: Vec<_> = work.actions.iter().map(|a| a.record.id).collect();
    assert_eq!(ids, [early.id, late.id, undated.id]);
    let commitments: Vec<_> = work.commitments.iter().map(|c| &c.record).collect();
    assert_eq!(commitments, [&commitment]);
}

#[tokio::test]
async fn my_open_work_reads_all_events_for_a_member_who_acts_as_manager_everywhere() {
    let f = Fixture::start().await;
    f.action("Ohne Rolle", None).await;
    sqlx::query("DELETE FROM event_membership WHERE user_id = $1")
        .bind(f.owner.as_uuid())
        .execute(&f.test.database.pool)
        .await
        .unwrap();
    let db = &f.test.database;

    let by_role = db.my_open_work(f.scope, f.owner, false).await.unwrap();
    assert!(by_role.actions.is_empty());
    let everywhere = db.my_open_work(f.scope, f.owner, true).await.unwrap();
    assert_eq!(everywhere.actions.len(), 1);
    assert_eq!(everywhere.actions[0].event_key.as_str(), "TEST30");
}
