use jiff::civil::date;
use tada_app::audit::AuditAction;
use tada_app::caller::TestScope;
use tada_app::caller::{MemberCaller, OrganizationRole};
use tada_app::domain::identity::{DisplayName, Email};
use tada_app::domain::parties::InstitutionKind;
use tada_app::parties::{InstitutionFields, PartyStore};

use tada_app::access::{EventReach, SourceReach};
use tada_app::domain::ids::MilestoneId;
use tada_app::domain::ids::{ProposalId, SourceVersionId};
use tada_app::domain::records::{MilestoneStatus, MilestoneTitle};
use tada_app::records::{EvidenceStore, RecordEvidenceView, RecordRef};
use tada_app::work::{
    MilestoneFields, MilestoneStore, MilestoneView, MilestoneWarning, NewMilestoneRecord,
};

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
/// The app clock at a change. The store keeps it as the change time; the database clock plays no part.
const CHANGED_AT: &str = "2030-05-19T09:30:00Z";

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
            title: Redactable::Present(ActionTitle::parse(title).unwrap()),
            description: None,
            owner: self.owner,
            workstream_id: None,
            start_date: None,
            due_date: due,
            milestone_id: None,
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
            Created::Created(view) => view,
            Created::IdTaken => panic!("id taken"),
        }
    }

    async fn commitment(&self, condition: Option<&str>) -> CommitmentView {
        let condition = condition.map(|text| ConditionText::parse(text).unwrap());
        let record = NewCommitmentRecord {
            id: CommitmentId::from_uuid(Uuid::now_v7()),
            event_id: self.event,
            promisor: Party::Institution(self.supplier),
            fields: CommitmentFields {
                text: Redactable::Present(
                    CommitmentText::parse("Generator delivery Friday 15:00").unwrap(),
                ),
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
            Created::Created(view) => view,
            Created::IdTaken => panic!("id taken"),
        }
    }

    async fn change_action(
        &self,
        scope: OrgScope,
        event: EventId,
        id: ActionId,
        fields: &ActionFields,
        expected: RecordVersion,
    ) -> Changed<ActionView> {
        self.test
            .database
            .change_action(
                scope,
                event,
                id,
                fields,
                expected,
                CHANGED_AT.parse().unwrap(),
                &self.audit(AuditAction::ActionChange, id.as_uuid()),
            )
            .await
            .unwrap()
    }

    async fn milestone(&self, date: jiff::civil::Date) -> MilestoneView {
        let record = NewMilestoneRecord {
            id: MilestoneId::from_uuid(Uuid::now_v7()),
            event_id: self.event,
            fields: MilestoneFields {
                title: Redactable::Present(MilestoneTitle::parse("Bewilligung liegt vor").unwrap()),
                description: None,
                date,
                owner: self.owner,
                workstream_id: None,
                status: MilestoneStatus::Open,
            },
        };
        let audit = self.audit(AuditAction::MilestoneCreate, record.id.as_uuid());
        match self
            .test
            .database
            .create_milestone(self.scope, &record, AT.parse().unwrap(), &audit)
            .await
            .unwrap()
        {
            Created::Created(view) => view,
            Created::IdTaken => panic!("id taken"),
        }
    }

    /// The warnings of the milestone `id` at `now`.
    async fn warnings(&self, id: MilestoneId, now: &str) -> Vec<MilestoneWarning> {
        self.test
            .database
            .milestone(self.scope, self.event, id, now.parse().unwrap())
            .await
            .unwrap()
            .unwrap()
            .warnings
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
        description: Some(RecordDescription::parse("Mit Diesel").unwrap()),
        due_date: None,
        status: ActionStatus::Done,
        ..first.fields.clone()
    };
    let Changed::Changed(changed) = f
        .change_action(f.scope, f.event, first.id, &fields, RecordVersion::FIRST)
        .await
    else {
        panic!("not changed");
    };
    assert_eq!(changed.fields, fields);
    assert_eq!(changed.version.get(), 2);
    let updated_at: jiff_sqlx::Timestamp =
        sqlx::query_scalar("SELECT updated_at FROM action WHERE id = $1")
            .bind(first.id.as_uuid())
            .fetch_one(&f.test.database.pool)
            .await
            .unwrap();
    assert_eq!(updated_at.to_jiff(), CHANGED_AT.parse().unwrap());
    assert_eq!(
        f.change_action(f.scope, f.event, first.id, &fields, RecordVersion::FIRST)
            .await,
        Changed::VersionConflict
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
    let Created::Created(other) = db
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
        Changed::NotFound
    );
    assert!(
        db.my_open_work(stranger, f.owner, &EventReach::Organization)
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
        .filter(|changed| matches!(changed, Changed::Changed(_)))
        .count();
    assert_eq!(winners, 1, "{a:?} {b:?}");
    assert!([&a, &b].contains(&&Changed::VersionConflict));
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

    // The schema refuses a conditional commitment with a firm reason.
    let refused = sqlx::query("UPDATE commitment SET firm_reason = 'x' WHERE id = $1")
        .bind(commitment.id.as_uuid())
        .execute(&db.pool)
        .await;
    assert!(refused.is_err());

    let fields = CommitmentFields {
        status: CommitmentStatus::Firm,
        firm_reason: Some(RecordReason::parse("The order is signed.").unwrap()),
        ..commitment.fields.clone()
    };
    let Changed::Changed(firm) = db
        .change_commitment(
            f.scope,
            f.event,
            commitment.id,
            &fields,
            RecordVersion::FIRST,
            CHANGED_AT.parse().unwrap(),
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

/// The apply writes the evidence of accepted proposals; the read shows it with the capture time,
/// as far as the caller can read the source.
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

    let record = RecordRef::Commitment(commitment.id);
    let read = db
        .evidence_of(f.scope, &[record], &SourceReach::Events(vec![f.event]))
        .await
        .unwrap();
    assert_eq!(
        read,
        [(
            record,
            RecordEvidenceView {
                record_version: RecordVersion::FIRST,
                proposal_id: ProposalId::from_uuid(proposal),
                source_version_id: SourceVersionId::from_uuid(version),
                captured_at: "2030-05-02T08:00:00Z".parse().unwrap(),
                start_offset: 0,
                end_offset: 23,
                quote: tada_app::domain::redaction::Redactable::Present(
                    "Lieferung Freitag 15:00".to_owned()
                ),
                page: None,
            }
        )]
    );
    // A caller who cannot read the source of the event sees no passage of it.
    let hidden = db
        .evidence_of(f.scope, &[record], &SourceReach::Events(Vec::new()))
        .await
        .unwrap();
    assert!(hidden.is_empty());
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
        .my_open_work(f.scope, f.owner, &EventReach::Events(vec![f.event]))
        .await
        .unwrap();
    let ids: Vec<_> = work.actions.iter().map(|a| a.record.id).collect();
    assert_eq!(ids, [early.id, late.id, undated.id]);
    let commitments: Vec<_> = work.commitments.iter().map(|c| &c.record).collect();
    assert_eq!(commitments, [&commitment]);
}

/// A closed event is read-only, so its open records leave „Meine Arbeit“ and come back when it reopens.
#[tokio::test]
async fn my_open_work_leaves_out_the_records_of_a_closed_event() {
    let f = Fixture::start().await;
    f.action("Generator bestellen", None).await;
    f.commitment(None).await;
    let db = &f.test.database;
    let set_status = |status: &str| {
        format!(
            "UPDATE event SET status = '{status}', closed_at = CASE WHEN '{status}' = 'closed' THEN now() END
             WHERE id = '{}'",
            f.event.as_uuid()
        )
    };

    f.test.execute(&[&set_status("closed")]).await;
    let closed = db
        .my_open_work(f.scope, f.owner, &EventReach::Organization)
        .await
        .unwrap();
    assert!(closed.actions.is_empty());
    assert!(closed.commitments.is_empty());

    f.test.execute(&[&set_status("active")]).await;
    let reopened = db
        .my_open_work(f.scope, f.owner, &EventReach::Organization)
        .await
        .unwrap();
    assert_eq!(reopened.actions.len(), 1);
    assert_eq!(reopened.commitments.len(), 1);
}

/// The store reads the events that the app gives (`access::readable_events`); it decides no access itself.
#[tokio::test]
async fn my_open_work_reads_only_the_given_events() {
    let f = Fixture::start().await;
    f.action("Im Anlass", None).await;
    let db = &f.test.database;

    let none = db
        .my_open_work(f.scope, f.owner, &EventReach::Events(Vec::new()))
        .await
        .unwrap();
    assert!(none.actions.is_empty());
    let everywhere = db
        .my_open_work(f.scope, f.owner, &EventReach::Organization)
        .await
        .unwrap();
    assert_eq!(everywhere.actions.len(), 1);
    assert_eq!(everywhere.actions[0].event_key.as_str(), "TEST30");
}

#[tokio::test]
async fn owners_reads_only_the_records_of_the_given_event() {
    let f = Fixture::start().await;
    let action = f.action("Generator bestellen", None).await;
    let commitment = f.commitment(None).await;
    let milestone = f.milestone(date(2030, 5, 1)).await;
    let db = &f.test.database;

    let records = [
        RecordRef::Action(action.id),
        RecordRef::Commitment(commitment.id),
        RecordRef::Milestone(milestone.id),
    ];

    let owners = db.owners(f.scope, f.event, &records).await.unwrap();
    assert_eq!(
        owners,
        WorkOwners::from([
            (RecordRef::Action(action.id), f.owner),
            (RecordRef::Commitment(commitment.id), f.owner),
            (RecordRef::Milestone(milestone.id), f.owner),
        ])
    );

    let testwil = f.scope.organization_id();
    let other_event = f.test.create_event(testwil, "TEST31").await;
    let elsewhere = db.owners(f.scope, other_event, &records).await.unwrap();
    assert_eq!(elsewhere, WorkOwners::default());
}

/// "Today" is the date in the time zone of the event (Europe/Zurich), not in UTC.
#[tokio::test]
async fn milestone_in_the_past_and_open_is_overdue() {
    let f = Fixture::start().await;
    let milestone = f.milestone(date(2030, 5, 17)).await;
    assert_eq!(milestone.local_id(), "MIL-001");
    // 23:30 in Zurich on its date: not yet overdue. 00:30 on the next day: overdue.
    assert!(
        f.warnings(milestone.id, "2030-05-17T21:30:00Z")
            .await
            .is_empty()
    );
    assert_eq!(
        f.warnings(milestone.id, "2030-05-17T22:30:00Z").await,
        [MilestoneWarning::Overdue]
    );
    let page = f
        .test
        .database
        .milestones(
            f.scope,
            f.event,
            &WorkFilter {
                owner: None,
                status: Some(MilestoneStatus::Open),
                workstream: None,
                after: None,
                limit: 10,
            },
            "2030-05-17T22:30:00Z".parse().unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(page.len(), 1);
    assert_eq!(page[0].warnings, [MilestoneWarning::Overdue]);

    // A reached milestone is not overdue.
    let fields = MilestoneFields {
        status: MilestoneStatus::Reached,
        ..milestone.fields.clone()
    };
    let changed = f
        .test
        .database
        .change_milestone(
            f.scope,
            f.event,
            milestone.id,
            &fields,
            milestone.version,
            "2030-05-20T08:00:00Z".parse().unwrap(),
            &f.audit(AuditAction::MilestoneChange, milestone.id.as_uuid()),
        )
        .await
        .unwrap();
    let Changed::Changed(reached) = changed else {
        panic!("not changed: {changed:?}");
    };
    assert!(reached.warnings.is_empty());
    assert_eq!(
        f.audit_actions().await,
        ["institution.create", "milestone.create", "milestone.change"]
    );
}

#[tokio::test]
async fn milestone_warns_about_late_open_actions() {
    let f = Fixture::start().await;
    let milestone = f.milestone(date(2030, 5, 1)).await;
    // Each action of the milestone with its status and due date.
    let mut late = Vec::new();
    for (status, due) in [
        (ActionStatus::Open, Some(date(2030, 5, 10))),
        (ActionStatus::Done, Some(date(2030, 5, 10))),
        (ActionStatus::Open, Some(date(2030, 5, 1))),
        (ActionStatus::Blocked, Some(date(2030, 6, 1))),
        (ActionStatus::InProgress, None),
    ] {
        let action = f.action("Zelt bestellen", due).await;
        let fields = ActionFields {
            status,
            milestone_id: Some(milestone.id),
            ..action.fields.clone()
        };
        let Changed::Changed(changed) = f
            .change_action(f.scope, f.event, action.id, &fields, action.version)
            .await
        else {
            panic!("not changed");
        };
        if status != ActionStatus::Done && due > Some(date(2030, 5, 1)) {
            late.push(MilestoneWarning::LateAction(changed.id));
        }
    }
    // A late action without the milestone does not count.
    f.action("Bühne bauen", Some(date(2030, 7, 1))).await;
    assert_eq!(late.len(), 2);
    // Before the date of the milestone, so it is not overdue.
    assert_eq!(f.warnings(milestone.id, "2030-04-01T08:00:00Z").await, late);
}

mod decisions {
    use tada_app::domain::ids::{DecisionId, PersonId};
    use tada_app::domain::records::{
        DecisionParties, DecisionStatus, DecisionWording, RecordReason,
    };
    use tada_app::parties::PersonFields;
    use tada_app::work::{
        DecisionFields, DecisionState, DecisionStore, DecisionView, NewDecisionRecord, Superseded,
    };

    use super::*;

    const WORDING: &str = "Das Fest findet bei jedem Wetter statt.\nBei Sturm entscheidet das OK.";

    impl Fixture {
        async fn person(&self, name: &str) -> PersonId {
            let id = PersonId::from_uuid(Uuid::now_v7());
            let fields = PersonFields {
                name: PartyName::parse(name).unwrap(),
                email: None,
                phone: None,
                user_id: None,
            };
            self.test
                .database
                .create_person(
                    self.scope,
                    id,
                    &fields,
                    AT.parse().unwrap(),
                    &self.audit(AuditAction::PersonCreate, id.as_uuid()),
                )
                .await
                .unwrap();
            id
        }

        fn new_decision(&self, parties: Vec<Party>) -> NewDecisionRecord {
            NewDecisionRecord {
                id: DecisionId::from_uuid(Uuid::now_v7()),
                event_id: self.event,
                wording: DecisionWording::parse(WORDING).unwrap(),
                decided_on: date(2030, 3, 1),
                parties: DecisionParties::new(parties).unwrap(),
                fields: DecisionFields {
                    owner: self.owner,
                    workstream_id: None,
                },
            }
        }

        async fn decision(&self, parties: Vec<Party>) -> DecisionView {
            let record = self.new_decision(parties);
            let audit = self.audit(AuditAction::DecisionCreate, record.id.as_uuid());
            match self
                .test
                .database
                .create_decision(self.scope, &record, AT.parse().unwrap(), &audit)
                .await
                .unwrap()
            {
                Created::Created(view) => view,
                Created::IdTaken => panic!("id taken"),
            }
        }

        async fn supersede(
            &self,
            old: &DecisionView,
            successor: &NewDecisionRecord,
        ) -> Created<Changed<Superseded>> {
            let audit = [
                self.audit(AuditAction::DecisionCreate, successor.id.as_uuid()),
                self.audit(AuditAction::DecisionSupersede, old.id.as_uuid()),
            ];
            self.test
                .database
                .supersede_decision(
                    self.scope,
                    self.event,
                    old.id,
                    successor,
                    old.version,
                    CHANGED_AT.parse().unwrap(),
                    &audit,
                )
                .await
                .unwrap()
        }

        async fn decision_count(&self) -> i64 {
            sqlx::query_scalar("SELECT count(*) FROM decision")
                .fetch_one(&self.test.database.pool)
                .await
                .unwrap()
        }
    }

    #[tokio::test]
    async fn records_supersedes_and_revokes_decisions_with_parties_and_audit_events() {
        let f = Fixture::start().await;
        let person = f.person("Beat Muster").await;
        let parties = vec![Party::Institution(f.supplier), Party::Person(person)];
        let first = f.decision(parties.clone()).await;
        assert_eq!(first.local_id(), "DEC-001");
        assert_eq!(
            first.wording.as_present().map(DecisionWording::as_str),
            Some(WORDING)
        );
        assert_eq!(first.decided_on, date(2030, 3, 1));
        assert_eq!(first.state, DecisionState::InForce);
        // The parties keep their order, with their readable IDs and names.
        let named: Vec<(Party, &str, Option<&str>)> = first
            .parties
            .iter()
            .map(|party| {
                (
                    party.party,
                    party.local_id.as_str(),
                    party.name.as_present().map(PartyName::as_str),
                )
            })
            .collect();
        assert_eq!(
            named,
            [
                (
                    Party::Institution(f.supplier),
                    "INS-001",
                    Some("Testwil Generatoren AG")
                ),
                (Party::Person(person), "PER-001", Some("Beat Muster")),
            ]
        );

        // The owner changes; the rest stays.
        let fields = DecisionFields {
            owner: f.owner,
            workstream_id: None,
        };
        let changed = f
            .test
            .database
            .change_decision(
                f.scope,
                f.event,
                first.id,
                &fields,
                first.version,
                CHANGED_AT.parse().unwrap(),
                &f.audit(AuditAction::DecisionChange, first.id.as_uuid()),
            )
            .await
            .unwrap();
        let Changed::Changed(first) = changed else {
            panic!("not changed: {changed:?}");
        };
        assert_eq!(first.version.get(), 2);

        // A supersede creates the successor and ends the old decision in one step.
        let successor = f.new_decision(vec![Party::Person(person)]);
        let result = f.supersede(&first, &successor).await;
        let Created::Created(Changed::Changed(Superseded { old, successor })) = result else {
            panic!("not superseded: {result:?}");
        };
        assert_eq!(old.state, DecisionState::Superseded { by: successor.id });
        assert_eq!(old.version.get(), 3);
        assert_eq!(successor.local_id(), "DEC-002");
        assert_eq!(successor.state, DecisionState::InForce);

        // A revoke keeps its reason.
        let reason = RecordReason::parse("Die Gemeinde verbietet es.").unwrap();
        let revoked = f
            .test
            .database
            .revoke_decision(
                f.scope,
                f.event,
                successor.id,
                &reason,
                successor.version,
                CHANGED_AT.parse().unwrap(),
                &f.audit(AuditAction::DecisionRevoke, successor.id.as_uuid()),
            )
            .await
            .unwrap();
        let Changed::Changed(revoked) = revoked else {
            panic!("not revoked: {revoked:?}");
        };
        assert_eq!(
            revoked.state,
            DecisionState::Revoked {
                reason: Redactable::Present(reason.clone())
            }
        );
        // An ended decision does not end again.
        let again = f
            .test
            .database
            .revoke_decision(
                f.scope,
                f.event,
                revoked.id,
                &reason,
                revoked.version,
                CHANGED_AT.parse().unwrap(),
                &f.audit(AuditAction::DecisionRevoke, revoked.id.as_uuid()),
            )
            .await
            .unwrap();
        assert_eq!(again, Changed::VersionConflict);
        let in_force = WorkFilter {
            owner: None,
            status: Some(DecisionStatus::InForce),
            workstream: None,
            after: None,
            limit: 10,
        };
        assert!(
            f.test
                .database
                .decisions(f.scope, f.event, &in_force)
                .await
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            f.audit_actions().await,
            [
                "institution.create",
                "person.create",
                "decision.create",
                "decision.change",
                "decision.create",
                "decision.supersede",
                "decision.revoke",
            ]
        );
    }

    /// Two supersedes of one decision with the same version: one wins, and the successor of the other does not stay.
    #[tokio::test]
    async fn two_supersedes_of_one_decision_leave_one_winner() {
        let f = Fixture::start().await;
        let old = f.decision(vec![Party::Institution(f.supplier)]).await;
        let (alpha, beta) = (
            f.new_decision(vec![Party::Institution(f.supplier)]),
            f.new_decision(vec![Party::Institution(f.supplier)]),
        );
        let (a, b) = tokio::join!(f.supersede(&old, &alpha), f.supersede(&old, &beta));
        let won = |result: &Created<Changed<Superseded>>| {
            matches!(result, Created::Created(Changed::Changed(_)))
        };
        assert_eq!(
            [&a, &b].iter().filter(|result| won(result)).count(),
            1,
            "{a:?} {b:?}"
        );
        assert!([&a, &b].contains(&&Created::Created(Changed::VersionConflict)));
        assert_eq!(f.decision_count().await, 2);
        let winner = if won(&a) { alpha.id } else { beta.id };
        let stored = f
            .test
            .database
            .decision(f.scope, f.event, old.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(stored.state, DecisionState::Superseded { by: winner });
    }

    /// A supersede with a taken ID or of another event changes nothing.
    #[tokio::test]
    async fn a_refused_supersede_changes_nothing() {
        let f = Fixture::start().await;
        let old = f.decision(vec![Party::Institution(f.supplier)]).await;
        let itself = NewDecisionRecord {
            id: old.id,
            ..f.new_decision(vec![Party::Institution(f.supplier)])
        };
        assert_eq!(f.supersede(&old, &itself).await, Created::IdTaken);
        let other = f
            .test
            .create_event(f.scope.organization_id(), "OTHER30")
            .await;
        let foreign = f
            .test
            .database
            .supersede_decision(
                f.scope,
                other,
                old.id,
                &NewDecisionRecord {
                    event_id: other,
                    ..f.new_decision(vec![Party::Institution(f.supplier)])
                },
                old.version,
                CHANGED_AT.parse().unwrap(),
                &[],
            )
            .await
            .unwrap();
        assert_eq!(foreign, Created::Created(Changed::NotFound));
        assert_eq!(f.decision_count().await, 1);
        let stored = f
            .test
            .database
            .decision(f.scope, f.event, old.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(stored, old);
        assert_eq!(
            f.audit_actions().await,
            ["institution.create", "decision.create"]
        );
    }
}

mod risks;
