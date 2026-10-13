//! The apply of proposals that create or change work records and parties (ADR 0068, ADR 0069).

use tada_app::access::SourceReach;
use tada_app::caller::TestScope;
use tada_app::domain::ids::{CommitmentId, LocalIdKind, MilestoneId, PersonId};
use tada_app::records::{EvidenceStore, RecordRef};
use tada_app::review::{LocalRecord, NewLocalId, RecordEditInput};

use super::*;

fn person(id: Uuid) -> Value {
    json!({"kind": "create-person", "id": id, "name": "Moritz Muster", "phone": "+41 79 000 00 00"})
}

fn institution(id: Uuid) -> Value {
    json!({"kind": "create-institution", "id": id, "name": "Zeltbau AG", "institution_kind": "company"})
}

fn commitment(event: EventId, id: Uuid, promisor: Value, owner: UserId) -> Value {
    json!({
        "kind": "create-commitment", "id": id, "event_id": event.as_uuid(),
        "text": "Liefert das Zelt", "promisor": promisor, "owner": owner.as_uuid(),
        "due_date": "2030-05-10", "condition": "wenn der Auftrag unterschrieben ist",
    })
}

fn action(event: EventId, id: Uuid, owner: UserId, workstream: Option<Uuid>) -> Value {
    json!({
        "kind": "create-action", "id": id, "event_id": event.as_uuid(),
        "title": "Bewilligung klären", "owner": owner.as_uuid(), "workstream": workstream,
    })
}

/// The record version, the proposal and the quote of each passage of `record_evidence` for the record `id`.
async fn record_evidence(test: &TestDatabase, column: &str, id: Uuid) -> Vec<(i64, Uuid, String)> {
    sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT record_version, proposal_id, quote FROM record_evidence WHERE {column} = $1
         ORDER BY record_version, id"
    )))
    .bind(id)
    .fetch_all(&test.database.pool)
    .await
    .unwrap()
}

async fn audit_actions(test: &TestDatabase, subject: Uuid) -> Vec<String> {
    sqlx::query_scalar("SELECT action FROM audit_event WHERE record_id = $1 ORDER BY id")
        .bind(subject)
        .fetch_all(&test.database.pool)
        .await
        .unwrap()
}

/// A changeset of the contributor with a new person and a conditional commitment of that person.
async fn supplier(test: &TestDatabase, open_day: &OpenDay) -> (Changeset, [Uuid; 4]) {
    let [person_id, commitment_id, first, second] = [
        Uuid::now_v7(),
        Uuid::now_v7(),
        Uuid::now_v7(),
        Uuid::now_v7(),
    ];
    let anna = open_day.contributor.user_id();
    let changeset = propose(
        test,
        &open_day.contributor,
        Some(open_day.event),
        vec![
            proposal(first, person(person_id), &[], "Das Open Day"),
            proposal(
                second,
                commitment(
                    open_day.event,
                    commitment_id,
                    json!({"person": person_id}),
                    anna,
                ),
                &[first],
                "im Mai 2030",
            ),
        ],
    )
    .await;
    (changeset, [person_id, commitment_id, first, second])
}

#[tokio::test]
async fn applying_a_commitment_writes_its_evidence_with_version_1() {
    let test = TestDatabase::start().await;
    let open_day = open_day(&test).await;
    let (changeset, [person_id, commitment_id, first, second]) = supplier(&test, &open_day).await;

    // Selecting the commitment also applies the person that it depends on.
    let applied = apply(&test, &open_day.manager, &changeset, select(&[second]))
        .await
        .unwrap();
    assert_eq!(
        applied.proposals,
        [
            (ProposalId::from_uuid(first), ProposalStatus::Accepted),
            (ProposalId::from_uuid(second), ProposalStatus::Accepted),
        ]
    );
    let (status, version, person, condition): (String, i64, Uuid, Option<String>) = sqlx::query_as(
        "SELECT status, version, person_id, condition FROM commitment WHERE id = $1",
    )
    .bind(commitment_id)
    .fetch_one(&test.database.pool)
    .await
    .unwrap();
    assert_eq!(
        (status.as_str(), version, person, condition.as_deref()),
        (
            "conditional",
            1,
            person_id,
            Some("wenn der Auftrag unterschrieben ist")
        )
    );
    assert_eq!(
        record_evidence(&test, "commitment_id", commitment_id).await,
        [(1, second, "im Mai 2030".to_owned())]
    );
    assert_eq!(
        record_evidence(&test, "person_id", person_id).await,
        [(1, first, "Das Open Day".to_owned())]
    );
    assert_eq!(
        audit_actions(&test, commitment_id).await,
        ["commitment.create"]
    );
    assert_eq!(audit_actions(&test, person_id).await, ["person.create"]);
    assert_eq!(audit_actions(&test, second).await, ["proposal.accept"]);

    // The commitment and the person show the evidence with the capture time of its source version.
    let commitment = RecordRef::Commitment(CommitmentId::from_uuid(commitment_id));
    let person = RecordRef::Person(PersonId::from_uuid(person_id));
    let evidence = test
        .database
        .evidence_of(
            open_day.manager.scope(),
            &[commitment, person],
            &SourceReach::Events(vec![open_day.event]),
        )
        .await
        .unwrap();
    let records: Vec<_> = evidence.iter().map(|(record, _)| *record).collect();
    assert_eq!(records, [person, commitment]);
    for (_, passage) in &evidence {
        assert_eq!(
            passage.source_version_id,
            changeset.source_version_id.unwrap()
        );
        assert_eq!(
            passage.record_version,
            tada_app::domain::RecordVersion::FIRST
        );
    }
    // A person belongs to the organization, but a member of another event does not read the source of its evidence.
    let elsewhere = test
        .database
        .evidence_of(
            open_day.manager.scope(),
            &[person],
            &SourceReach::Events(Vec::new()),
        )
        .await
        .unwrap();
    assert!(elsewhere.is_empty());
}

fn commitment_status(event: EventId, id: Uuid, status: &str, expected_version: i64) -> Value {
    json!({
        "kind": "change-commitment-status", "event_id": event.as_uuid(), "commitment_id": id,
        "status": status, "expected_version": expected_version,
    })
}

#[tokio::test]
async fn applying_a_firm_change_writes_evidence_with_the_new_version() {
    let test = TestDatabase::start().await;
    let open_day = open_day(&test).await;
    let (changeset, [_, commitment_id, _, second]) = supplier(&test, &open_day).await;
    apply(&test, &open_day.manager, &changeset, select(&[second]))
        .await
        .unwrap();

    let id = Uuid::now_v7();
    let mut firm = proposal(
        id,
        commitment_status(open_day.event, commitment_id, "firm", 1),
        &[],
        "20000 Besuchern",
    );
    firm["reason"] = json!("Der Auftrag ist unterschrieben.");
    let change = propose(
        &test,
        &open_day.contributor,
        Some(open_day.event),
        vec![firm],
    )
    .await;
    apply(&test, &open_day.manager, &change, select(&[id]))
        .await
        .unwrap();

    let (status, version, condition, reason): (String, i64, Option<String>, Option<String>) =
        sqlx::query_as(
            "SELECT status, version, condition, firm_reason FROM commitment WHERE id = $1",
        )
        .bind(commitment_id)
        .fetch_one(&test.database.pool)
        .await
        .unwrap();
    assert_eq!(
        (status.as_str(), version, reason.as_deref()),
        ("firm", 2, Some("Der Auftrag ist unterschrieben."))
    );
    // The condition stays as history (ADR 0068).
    assert!(condition.is_some());
    assert_eq!(
        record_evidence(&test, "commitment_id", commitment_id).await,
        [
            (1, second, "im Mai 2030".to_owned()),
            (2, id, "20000 Besuchern".to_owned()),
        ]
    );
    assert_eq!(
        audit_actions(&test, commitment_id).await,
        ["commitment.create", "commitment.firm"]
    );

    // A second proposal that expects the old version conflicts and changes nothing.
    let stale = Uuid::now_v7();
    let late = propose(
        &test,
        &open_day.contributor,
        Some(open_day.event),
        vec![proposal(
            stale,
            commitment_status(open_day.event, commitment_id, "fulfilled", 2),
            &[],
            "im Mai 2030",
        )],
    )
    .await;
    let withdrawn = Uuid::now_v7();
    let other = propose(
        &test,
        &open_day.contributor,
        Some(open_day.event),
        vec![proposal(
            withdrawn,
            commitment_status(open_day.event, commitment_id, "withdrawn", 2),
            &[],
            "im Mai 2030",
        )],
    )
    .await;
    apply(&test, &open_day.manager, &late, select(&[stale]))
        .await
        .unwrap();
    let result = apply(&test, &open_day.manager, &other, select(&[withdrawn])).await;
    assert!(
        matches!(result, Err(ApplyError::Conflict(ref ids)) if ids == &[ProposalId::from_uuid(withdrawn)]),
        "{result:?}"
    );
    let status: String = sqlx::query_scalar("SELECT status FROM commitment WHERE id = $1")
        .bind(commitment_id)
        .fetch_one(&test.database.pool)
        .await
        .unwrap();
    assert_eq!(status, "fulfilled");
}

#[tokio::test]
async fn applying_gives_readable_ids_act_com_per_ins() {
    let test = TestDatabase::start().await;
    let open_day = open_day(&test).await;
    let anna = open_day.contributor.user_id();
    let intake = || {
        let ids: [Uuid; 8] = std::array::from_fn(|_| Uuid::now_v7());
        let [
            person_id,
            institution_id,
            action_id,
            commitment_id,
            p1,
            p2,
            p3,
            p4,
        ] = ids;
        let proposals = vec![
            proposal(p1, person(person_id), &[], "Das Open Day"),
            proposal(p2, institution(institution_id), &[], "Das Open Day"),
            proposal(
                p3,
                action(open_day.event, action_id, anna, None),
                &[],
                "im Mai 2030",
            ),
            proposal(
                p4,
                commitment(
                    open_day.event,
                    commitment_id,
                    json!({"institution": institution_id}),
                    anna,
                ),
                &[p2],
                "im Mai 2030",
            ),
        ];
        (proposals, ids)
    };
    let mut readable = Vec::new();
    for _ in 0..2 {
        let (proposals, ids) = intake();
        let changeset = propose(
            &test,
            &open_day.contributor,
            Some(open_day.event),
            proposals,
        )
        .await;
        let applied = apply(&test, &open_day.manager, &changeset, select(&ids[4..]))
            .await
            .unwrap();
        let mut local: Vec<(String, Uuid)> = applied
            .local_ids
            .iter()
            .map(|local| match local.record {
                LocalRecord::Person(id) => (
                    LocalIdKind::Person.readable_id(local.local_number),
                    id.as_uuid(),
                ),
                LocalRecord::Institution(id) => (
                    LocalIdKind::Institution.readable_id(local.local_number),
                    id.as_uuid(),
                ),
                LocalRecord::Action(id) => (
                    LocalIdKind::Action.readable_id(local.local_number),
                    id.as_uuid(),
                ),
                LocalRecord::Commitment(id) => (
                    LocalIdKind::Commitment.readable_id(local.local_number),
                    id.as_uuid(),
                ),
                other => panic!("unexpected record {other:?}"),
            })
            .collect();
        local.sort();
        let mut expected: Vec<(&str, Uuid)> = vec![
            ("ACT", ids[2]),
            ("COM", ids[3]),
            ("INS", ids[1]),
            ("PER", ids[0]),
        ];
        expected.sort();
        assert_eq!(
            local.iter().map(|(_, id)| *id).collect::<Vec<_>>(),
            expected.iter().map(|(_, id)| *id).collect::<Vec<_>>()
        );
        readable.extend(local.into_iter().map(|(readable, _)| readable));
    }
    assert_eq!(
        readable,
        [
            "ACT-001", "COM-001", "INS-001", "PER-001", "ACT-002", "COM-002", "INS-002", "PER-002"
        ]
    );
    let (status, version): (String, i64) =
        sqlx::query_as("SELECT status, version FROM action ORDER BY local_number LIMIT 1")
            .fetch_one(&test.database.pool)
            .await
            .unwrap();
    assert_eq!((status.as_str(), version), ("open", 1));
    // A commitment with a condition starts conditional.
    let conditional: i64 = test
        .scalar("SELECT count(*) FROM commitment WHERE status = 'conditional'")
        .await;
    assert_eq!(conditional, 2);
}

fn milestone(event: EventId, id: Uuid, owner: UserId) -> Value {
    json!({
        "kind": "create-milestone", "id": id, "event_id": event.as_uuid(),
        "title": "Bewilligung liegt vor", "date": "2030-05-01", "owner": owner.as_uuid(),
    })
}

#[tokio::test]
async fn accepted_milestone_gets_a_readable_id_and_evidence() {
    let test = TestDatabase::start().await;
    let open_day = open_day(&test).await;
    let anna = open_day.contributor.user_id();
    let [milestone_id, first, second] = [Uuid::now_v7(), Uuid::now_v7(), Uuid::now_v7()];
    let changeset = propose(
        &test,
        &open_day.contributor,
        Some(open_day.event),
        vec![proposal(
            first,
            milestone(open_day.event, milestone_id, anna),
            &[],
            "im Mai 2030",
        )],
    )
    .await;
    let applied = apply(&test, &open_day.manager, &changeset, select(&[first]))
        .await
        .unwrap();
    assert_eq!(
        applied.local_ids,
        [NewLocalId {
            record: LocalRecord::Milestone(MilestoneId::from_uuid(milestone_id)),
            local_number: 1,
        }]
    );
    let (status, date, version): (String, String, i64) =
        sqlx::query_as("SELECT status, date::text, version FROM milestone WHERE id = $1")
            .bind(milestone_id)
            .fetch_one(&test.database.pool)
            .await
            .unwrap();
    assert_eq!(
        (status.as_str(), date.as_str(), version),
        ("open", "2030-05-01", 1)
    );
    assert_eq!(
        record_evidence(&test, "milestone_id", milestone_id).await,
        [(1, first, "im Mai 2030".to_owned())]
    );
    assert_eq!(
        audit_actions(&test, milestone_id).await,
        ["milestone.create"]
    );

    // A status change goes to the owner, writes evidence with the new version and an audit event.
    let changeset = propose(
        &test,
        &open_day.contributor,
        Some(open_day.event),
        vec![proposal(
            second,
            json!({
                "kind": "change-milestone-status", "event_id": open_day.event.as_uuid(),
                "milestone_id": milestone_id, "status": "reached", "expected_version": 1,
            }),
            &[],
            "Das Open Day",
        )],
    )
    .await;
    apply(&test, &open_day.contributor, &changeset, select(&[second]))
        .await
        .unwrap();
    let status: String = test
        .scalar(&format!(
            "SELECT status FROM milestone WHERE id = '{milestone_id}'"
        ))
        .await;
    assert_eq!(status, "reached");
    assert_eq!(
        record_evidence(&test, "milestone_id", milestone_id).await,
        [
            (1, first, "im Mai 2030".to_owned()),
            (2, second, "Das Open Day".to_owned()),
        ]
    );
    assert_eq!(
        audit_actions(&test, milestone_id).await,
        ["milestone.create", "milestone.change"]
    );
    let evidence = test
        .database
        .evidence_of(
            open_day.manager.scope(),
            &[RecordRef::Milestone(MilestoneId::from_uuid(milestone_id))],
            &SourceReach::Organization,
        )
        .await
        .unwrap();
    assert_eq!(evidence.len(), 2);
}

fn edit(id: Uuid, fields: Value) -> Edit {
    Edit {
        proposal_id: ProposalId::from_uuid(id),
        state: None,
        fields: Some(serde_json::from_value::<RecordEditInput>(fields).unwrap()),
    }
}

fn invalid(result: Result<Applied, ApplyError>) -> Vec<(String, &'static str)> {
    let Err(ApplyError::Invalid(errors)) = result else {
        panic!("not invalid: {result:?}");
    };
    errors
        .into_iter()
        .map(|error| (error.field.into_owned(), error.code))
        .collect()
}

#[tokio::test]
async fn an_edit_cannot_remove_the_condition_of_a_commitment() {
    let test = TestDatabase::start().await;
    let open_day = open_day(&test).await;
    let (changeset, [person_id, commitment_id, first, second]) = supplier(&test, &open_day).await;
    let input = |fields: Value| ApplyInput {
        selected: vec![ProposalId::from_uuid(second)],
        links: Vec::new(),
        edits: vec![edit(second, fields)],
    };

    let result = apply(
        &test,
        &open_day.manager,
        &changeset,
        input(json!({"condition": null})),
    )
    .await;
    assert_eq!(
        invalid(result),
        [("edits/0/fields/condition".to_owned(), "condition-fixed")]
    );
    let result = apply(
        &test,
        &open_day.manager,
        &changeset,
        input(json!({"title": "Zelt", "text": ""})),
    )
    .await;
    assert_eq!(
        invalid(result),
        [
            ("edits/0/fields/title".to_owned(), "not-editable"),
            ("edits/0/fields/text".to_owned(), "empty"),
        ]
    );
    assert_eq!(count(&test, "commitment").await, 0);
    assert_eq!(count(&test, "person").await, 0);

    // A changed text and a changed condition apply with the edit as more evidence.
    let applied = apply(
        &test,
        &open_day.manager,
        &changeset,
        input(json!({"text": "Liefert zwei Zelte", "condition": "wenn der Vertrag unterschrieben ist", "due_date": null})),
    )
    .await
    .unwrap();
    assert_eq!(
        applied.proposals,
        [
            (ProposalId::from_uuid(first), ProposalStatus::Accepted),
            (
                ProposalId::from_uuid(second),
                ProposalStatus::AcceptedWithEdit
            ),
        ]
    );
    let (text, condition, status, due): (String, Option<String>, String, Option<jiff_sqlx::Date>) =
        sqlx::query_as("SELECT text, condition, status, due_date FROM commitment WHERE id = $1")
            .bind(commitment_id)
            .fetch_one(&test.database.pool)
            .await
            .unwrap();
    assert_eq!(
        (
            text.as_str(),
            condition.as_deref(),
            status.as_str(),
            due.is_none()
        ),
        (
            "Liefert zwei Zelte",
            Some("wenn der Vertrag unterschrieben ist"),
            "conditional",
            true
        )
    );
    let evidence = record_evidence(&test, "commitment_id", commitment_id).await;
    assert_eq!(evidence.len(), 2);
    assert_eq!(evidence[0], (1, second, "im Mai 2030".to_owned()));
    assert!(evidence[1].2.contains("Liefert zwei Zelte"), "{evidence:?}");
    let (kind, author): (String, Value) = sqlx::query_as(
        "SELECT v.kind, v.author_actor FROM review_result r
         JOIN source_version v ON v.id = r.edit_source_version_id
         WHERE r.proposal_id = $1",
    )
    .bind(second)
    .fetch_one(&test.database.pool)
    .await
    .unwrap();
    assert_eq!(kind, "review");
    assert_eq!(actor::from_json(&author).unwrap(), open_day.manager.actor());
    assert_eq!(
        record_evidence(&test, "person_id", person_id).await.len(),
        1
    );
}

#[tokio::test]
async fn an_edit_that_adds_a_condition_makes_a_commitment_conditional() {
    let test = TestDatabase::start().await;
    let open_day = open_day(&test).await;
    let (person_id, commitment_id, first, second) = (
        Uuid::now_v7(),
        Uuid::now_v7(),
        Uuid::now_v7(),
        Uuid::now_v7(),
    );
    let mut firm = commitment(
        open_day.event,
        commitment_id,
        json!({"person": person_id}),
        open_day.contributor.user_id(),
    );
    firm.as_object_mut().unwrap().remove("condition");
    let changeset = propose(
        &test,
        &open_day.contributor,
        Some(open_day.event),
        vec![
            proposal(first, person(person_id), &[], "Das Open Day"),
            proposal(second, firm, &[first], "im Mai 2030"),
        ],
    )
    .await;
    let input = ApplyInput {
        selected: vec![ProposalId::from_uuid(second)],
        links: Vec::new(),
        edits: vec![
            edit(second, json!({"condition": "wenn es nicht regnet"})),
            edit(
                first,
                json!({"name": "Moritz Beispiel", "email": "moritz@example.org"}),
            ),
        ],
    };
    apply(&test, &open_day.manager, &changeset, input)
        .await
        .unwrap();
    let status: String = sqlx::query_scalar("SELECT status FROM commitment WHERE id = $1")
        .bind(commitment_id)
        .fetch_one(&test.database.pool)
        .await
        .unwrap();
    assert_eq!(status, "conditional");
    let (name, email): (String, Option<String>) =
        sqlx::query_as("SELECT name, email FROM person WHERE id = $1")
            .bind(person_id)
            .fetch_one(&test.database.pool)
            .await
            .unwrap();
    assert_eq!(
        (name.as_str(), email.as_deref()),
        ("Moritz Beispiel", Some("moritz@example.org"))
    );
    // The review text of each edit belongs to the event of the changeset, also the one of the person,
    // so the members of the event can read the evidence.
    let events: Vec<Option<Uuid>> = sqlx::query_scalar(
        "SELECT i.event_id FROM source_version v JOIN source_item i ON i.id = v.source_item_id
         WHERE v.kind = 'review'",
    )
    .fetch_all(&test.database.pool)
    .await
    .unwrap();
    assert_eq!(events, [Some(open_day.event.as_uuid()); 2]);
}

#[tokio::test]
async fn an_edited_owner_must_be_a_contributor() {
    let test = TestDatabase::start().await;
    let open_day = open_day(&test).await;
    let viewer = user(&test, open_day.organization, "Vera").await;
    add_event_role(&test, &open_day, viewer, EventRole::EventViewer).await;
    let outsider = user(&test, open_day.organization, "Otto").await;
    let (action_id, id) = (Uuid::now_v7(), Uuid::now_v7());
    let changeset = propose(
        &test,
        &open_day.contributor,
        Some(open_day.event),
        vec![proposal(
            id,
            action(
                open_day.event,
                action_id,
                open_day.contributor.user_id(),
                None,
            ),
            &[],
            "im Mai 2030",
        )],
    )
    .await;
    let input = |owner: UserId| ApplyInput {
        selected: vec![ProposalId::from_uuid(id)],
        links: Vec::new(),
        edits: vec![edit(
            id,
            json!({"owner": owner.as_uuid(), "due_date": "2030-04-30"}),
        )],
    };
    for owner in [viewer, outsider] {
        let result = apply(&test, &open_day.manager, &changeset, input(owner)).await;
        assert_eq!(
            invalid(result),
            [("edits/0/fields/owner".to_owned(), "unknown-member")]
        );
    }
    assert_eq!(count(&test, "action").await, 0);

    let mia = open_day.manager.user_id();
    apply(&test, &open_day.manager, &changeset, input(mia))
        .await
        .unwrap();
    let (owner, due): (Uuid, Option<jiff_sqlx::Date>) =
        sqlx::query_as("SELECT owner_user_id, due_date FROM action WHERE id = $1")
            .bind(action_id)
            .fetch_one(&test.database.pool)
            .await
            .unwrap();
    assert_eq!(owner, mia.as_uuid());
    assert_eq!(
        due.map(|date| date.to_jiff().to_string()).as_deref(),
        Some("2030-04-30")
    );
    assert_eq!(audit_actions(&test, action_id).await, ["action.create"]);
}

async fn insert_workstream(test: &TestDatabase, open_day: &OpenDay, status: &str) -> Uuid {
    let id = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO workstream (id, organization_id, event_id, name, lead_user_id, status, version, created_at, updated_at)
         VALUES ($1, $2, $3, $4, $5, $6, 1, now(), now())",
    )
    .bind(id)
    .bind(open_day.organization.as_uuid())
    .bind(open_day.event.as_uuid())
    .bind(format!("Bodenbetrieb {id}"))
    .bind(open_day.manager.user_id().as_uuid())
    .bind(status)
    .execute(&test.database.pool)
    .await
    .unwrap();
    id
}

#[tokio::test]
async fn a_workstream_that_closes_after_the_proposal_makes_the_apply_conflict() {
    let test = TestDatabase::start().await;
    let open_day = open_day(&test).await;
    let workstream = insert_workstream(&test, &open_day, "active").await;
    let other = insert_workstream(&test, &open_day, "closed").await;
    let id = Uuid::now_v7();
    let anna = open_day.contributor.user_id();
    let changeset = propose(
        &test,
        &open_day.contributor,
        Some(open_day.event),
        vec![proposal(
            id,
            action(open_day.event, Uuid::now_v7(), anna, Some(workstream)),
            &[],
            "im Mai 2030",
        )],
    )
    .await;
    // An edit cannot set a closed workstream either.
    let result = apply(
        &test,
        &open_day.manager,
        &changeset,
        ApplyInput {
            selected: vec![ProposalId::from_uuid(id)],
            links: Vec::new(),
            edits: vec![edit(id, json!({"workstream": other}))],
        },
    )
    .await;
    assert_eq!(
        invalid(result),
        [("edits/0/fields/workstream".to_owned(), "closed")]
    );

    sqlx::query("UPDATE workstream SET status = 'closed' WHERE id = $1")
        .bind(workstream)
        .execute(&test.database.pool)
        .await
        .unwrap();
    let result = apply(&test, &open_day.manager, &changeset, select(&[id])).await;
    assert!(matches!(result, Err(ApplyError::Conflict(_))), "{result:?}");
    assert_eq!(count(&test, "action").await, 0);
    assert_eq!(
        status_of(&test, &open_day, changeset.id, id).await,
        ProposalStatus::Conflict
    );
}

#[tokio::test]
async fn an_owner_who_lost_the_event_role_makes_the_apply_conflict() {
    let test = TestDatabase::start().await;
    let open_day = open_day(&test).await;
    let anna = open_day.contributor.user_id();
    let (first, second) = (Uuid::now_v7(), Uuid::now_v7());
    let changeset = propose(
        &test,
        &open_day.contributor,
        Some(open_day.event),
        vec![
            proposal(
                first,
                action(open_day.event, Uuid::now_v7(), anna, None),
                &[],
                "im Mai 2030",
            ),
            proposal(
                second,
                action(
                    open_day.event,
                    Uuid::now_v7(),
                    open_day.manager.user_id(),
                    None,
                ),
                &[],
                "im Mai 2030",
            ),
        ],
    )
    .await;
    sqlx::query("DELETE FROM event_membership WHERE event_id = $1 AND user_id = $2")
        .bind(open_day.event.as_uuid())
        .bind(anna.as_uuid())
        .execute(&test.database.pool)
        .await
        .unwrap();
    let result = apply(
        &test,
        &open_day.manager,
        &changeset,
        select(&[first, second]),
    )
    .await;
    assert!(
        matches!(result, Err(ApplyError::Conflict(ref ids)) if ids == &[ProposalId::from_uuid(first)]),
        "{result:?}"
    );
    assert_eq!(count(&test, "action").await, 0);
    assert_eq!(
        status_of(&test, &open_day, changeset.id, first).await,
        ProposalStatus::Conflict
    );
    assert_eq!(
        status_of(&test, &open_day, changeset.id, second).await,
        ProposalStatus::Open
    );
}

#[tokio::test]
async fn a_promisor_of_another_organization_is_refused() {
    let test = TestDatabase::start().await;
    let open_day = open_day(&test).await;
    let other = test.create_organization("musterhausen").await;
    let (person_id, institution_id) = (Uuid::now_v7(), Uuid::now_v7());
    sqlx::query(
        "INSERT INTO person (id, organization_id, local_number, name, version, created_at, updated_at)
         VALUES ($1, $2, 1, 'Moritz Muster', 1, now(), now())",
    )
    .bind(person_id)
    .bind(other.as_uuid())
    .execute(&test.database.pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO institution (id, organization_id, local_number, name, kind, version, created_at, updated_at)
         VALUES ($1, $2, 1, 'Zeltbau AG', 'company', 1, now(), now())",
    )
    .bind(institution_id)
    .bind(other.as_uuid())
    .execute(&test.database.pool)
    .await
    .unwrap();
    for promisor in [
        json!({"person": person_id}),
        json!({"institution": institution_id}),
    ] {
        let input: NewChangeset = serde_json::from_value(json!({
            "event_id": open_day.event.as_uuid(),
            "source_text": SOURCE,
            "proposals": [proposal(
                Uuid::now_v7(),
                commitment(open_day.event, Uuid::now_v7(), promisor, open_day.contributor.user_id()),
                &[],
                "im Mai 2030",
            )],
        }))
        .unwrap();
        let result = create_changeset(
            &open_day.contributor,
            input,
            propose_stores(&test),
            &FixedClock,
        )
        .await;
        let Err(tada_app::proposals::ProposeError::Invalid(errors)) = result else {
            panic!("not invalid: {result:?}");
        };
        let fields: Vec<_> = errors
            .iter()
            .map(|error| (error.field.as_ref(), error.code))
            .collect();
        assert_eq!(
            fields,
            [("proposals/0/operation/promisor", "unknown-record")]
        );
    }
    assert_eq!(count(&test, "changeset").await, 0);
    assert_eq!(count(&test, "proposal").await, 0);
}

fn action_change(event: EventId, id: Uuid, change: Value) -> Value {
    let mut operation =
        json!({"event_id": event.as_uuid(), "action_id": id, "expected_version": 1});
    for (key, value) in change.as_object().unwrap() {
        operation[key] = value.clone();
    }
    operation
}

#[tokio::test]
async fn applying_changes_of_an_action_counts_its_version_and_keeps_the_evidence() {
    let test = TestDatabase::start().await;
    let open_day = open_day(&test).await;
    let (action_id, created) = (Uuid::now_v7(), Uuid::now_v7());
    let anna = open_day.contributor.user_id();
    let changeset = propose(
        &test,
        &open_day.contributor,
        Some(open_day.event),
        vec![proposal(
            created,
            action(open_day.event, action_id, anna, None),
            &[],
            "im Mai 2030",
        )],
    )
    .await;
    apply(&test, &open_day.manager, &changeset, select(&[created]))
        .await
        .unwrap();

    let (status, due) = (Uuid::now_v7(), Uuid::now_v7());
    let mut due_change = action_change(
        open_day.event,
        action_id,
        json!({"kind": "change-action-due", "due_date": "2030-05-01"}),
    );
    due_change["expected_version"] = json!(2);
    let changes = propose(
        &test,
        &open_day.contributor,
        Some(open_day.event),
        vec![
            proposal(
                status,
                action_change(
                    open_day.event,
                    action_id,
                    json!({"kind": "change-action-status", "status": "in-progress"}),
                ),
                &[],
                "Das Open Day",
            ),
            proposal(due, due_change, &[status], "20000 Besuchern"),
        ],
    )
    .await;
    apply(&test, &open_day.manager, &changes, select(&[due]))
        .await
        .unwrap();
    let (state, version, due_date): (String, i64, Option<jiff_sqlx::Date>) =
        sqlx::query_as("SELECT status, version, due_date FROM action WHERE id = $1")
            .bind(action_id)
            .fetch_one(&test.database.pool)
            .await
            .unwrap();
    assert_eq!((state.as_str(), version), ("in-progress", 3));
    assert_eq!(
        due_date.map(|date| date.to_jiff().to_string()).as_deref(),
        Some("2030-05-01")
    );
    assert_eq!(
        record_evidence(&test, "action_id", action_id).await,
        [
            (1, created, "im Mai 2030".to_owned()),
            (2, status, "Das Open Day".to_owned()),
            (3, due, "20000 Besuchern".to_owned()),
        ]
    );
    assert_eq!(
        audit_actions(&test, action_id).await,
        ["action.create", "action.change", "action.change"]
    );
}

/// Two status changes of one action in one changeset: the second step starts from the status that the first wrote.
async fn status_chain(
    test: &TestDatabase,
    open_day: &OpenDay,
    action_id: Uuid,
    first: &str,
    second: &str,
) -> (Uuid, Uuid, Result<Applied, ApplyError>) {
    let (one, two) = (Uuid::now_v7(), Uuid::now_v7());
    let status = |status: &str, version: i64| {
        let mut change = action_change(
            open_day.event,
            action_id,
            json!({"kind": "change-action-status", "status": status}),
        );
        change["expected_version"] = json!(version);
        change
    };
    let changes = propose(
        test,
        &open_day.contributor,
        Some(open_day.event),
        vec![
            proposal(one, status(first, 1), &[], "Das Open Day"),
            proposal(two, status(second, 2), &[one], "20000 Besuchern"),
        ],
    )
    .await;
    let result = apply(test, &open_day.manager, &changes, select(&[two])).await;
    (one, two, result)
}

async fn action_state(test: &TestDatabase, action_id: Uuid) -> (String, i64) {
    sqlx::query_as("SELECT status, version FROM action WHERE id = $1")
        .bind(action_id)
        .fetch_one(&test.database.pool)
        .await
        .unwrap()
}

/// The apply checks each status step against the status of the row at that time (ADR 0068), not only its version.
#[tokio::test]
async fn a_status_chain_that_breaks_a_transition_conflicts_and_applies_nothing() {
    let test = TestDatabase::start().await;
    let open_day = open_day(&test).await;
    let (action_id, created) = (Uuid::now_v7(), Uuid::now_v7());
    let anna = open_day.contributor.user_id();
    let changeset = propose(
        &test,
        &open_day.contributor,
        Some(open_day.event),
        vec![proposal(
            created,
            action(open_day.event, action_id, anna, None),
            &[],
            "im Mai 2030",
        )],
    )
    .await;
    apply(&test, &open_day.manager, &changeset, select(&[created]))
        .await
        .unwrap();

    // "open → done", then "open → in-progress": the second step starts from done, which cannot go to in-progress.
    let (_, second, result) =
        status_chain(&test, &open_day, action_id, "done", "in-progress").await;
    assert!(
        matches!(&result, Err(ApplyError::Conflict(proposals)) if proposals == &[ProposalId::from_uuid(second)]),
        "{result:?}"
    );
    assert_eq!(action_state(&test, action_id).await, ("open".to_owned(), 1));

    // "open → in-progress", then "in-progress → done": both apply.
    let (_, _, result) = status_chain(&test, &open_day, action_id, "in-progress", "done").await;
    assert!(result.is_ok(), "{result:?}");
    assert_eq!(action_state(&test, action_id).await, ("done".to_owned(), 3));
}

fn workstream_proposal(event: EventId, id: Uuid, name: &str, lead: UserId) -> Value {
    json!({
        "kind": "create-workstream", "id": id, "event_id": event.as_uuid(),
        "name": name, "lead": lead.as_uuid(),
    })
}

#[tokio::test]
async fn action_in_a_new_workstream_applies_with_it() {
    let test = TestDatabase::start().await;
    let open_day = open_day(&test).await;
    let anna = open_day.contributor.user_id();
    let [workstream, action_id, first, second] = [
        Uuid::now_v7(),
        Uuid::now_v7(),
        Uuid::now_v7(),
        Uuid::now_v7(),
    ];
    let changeset = propose(
        &test,
        &open_day.contributor,
        Some(open_day.event),
        vec![
            proposal(
                first,
                workstream_proposal(open_day.event, workstream, "Programm", anna),
                &[],
                "Das Open Day",
            ),
            proposal(
                second,
                action(open_day.event, action_id, anna, Some(workstream)),
                &[first],
                "im Mai 2030",
            ),
        ],
    )
    .await;

    // The contributor who leads the workstream cannot apply it: the managers review the new workstream.
    let result = apply(&test, &open_day.contributor, &changeset, select(&[second])).await;
    assert!(matches!(result, Err(ApplyError::Forbidden)), "{result:?}");
    assert_eq!(count(&test, "workstream").await, 0);

    // Selecting the action also applies the workstream that it depends on.
    let applied = apply(&test, &open_day.manager, &changeset, select(&[second]))
        .await
        .unwrap();
    assert_eq!(applied.local_ids.len(), 1);
    let (name, lead, status, version): (String, Uuid, String, i64) =
        sqlx::query_as("SELECT name, lead_user_id, status, version FROM workstream WHERE id = $1")
            .bind(workstream)
            .fetch_one(&test.database.pool)
            .await
            .unwrap();
    assert_eq!(
        (name.as_str(), lead, status.as_str(), version),
        ("Programm", anna.as_uuid(), "active", 1)
    );
    let in_workstream: Option<Uuid> =
        sqlx::query_scalar("SELECT workstream_id FROM action WHERE id = $1")
            .bind(action_id)
            .fetch_one(&test.database.pool)
            .await
            .unwrap();
    assert_eq!(in_workstream, Some(workstream));
    assert_eq!(
        audit_actions(&test, workstream).await,
        ["workstream.create"]
    );
}

#[tokio::test]
async fn a_lead_who_lost_the_role_makes_the_workstream_apply_conflict() {
    let test = TestDatabase::start().await;
    let open_day = open_day(&test).await;
    let anna = open_day.contributor.user_id();
    let id = Uuid::now_v7();
    let changeset = propose(
        &test,
        &open_day.contributor,
        Some(open_day.event),
        vec![proposal(
            id,
            workstream_proposal(open_day.event, Uuid::now_v7(), "Programm", anna),
            &[],
            "Das Open Day",
        )],
    )
    .await;
    sqlx::query("DELETE FROM event_membership WHERE event_id = $1 AND user_id = $2")
        .bind(open_day.event.as_uuid())
        .bind(anna.as_uuid())
        .execute(&test.database.pool)
        .await
        .unwrap();
    let result = apply(&test, &open_day.manager, &changeset, select(&[id])).await;
    assert!(
        matches!(result, Err(ApplyError::Conflict(ref ids)) if ids == &[ProposalId::from_uuid(id)]),
        "{result:?}"
    );
    assert_eq!(count(&test, "workstream").await, 0);
}

#[tokio::test]
async fn a_review_edit_cannot_change_a_workstream_proposal() {
    let test = TestDatabase::start().await;
    let open_day = open_day(&test).await;
    let id = Uuid::now_v7();
    let changeset = propose(
        &test,
        &open_day.contributor,
        Some(open_day.event),
        vec![proposal(
            id,
            workstream_proposal(
                open_day.event,
                Uuid::now_v7(),
                "Programm",
                open_day.contributor.user_id(),
            ),
            &[],
            "Das Open Day",
        )],
    )
    .await;
    let input = ApplyInput {
        selected: vec![ProposalId::from_uuid(id)],
        links: Vec::new(),
        edits: vec![edit(id, json!({"name": "Bühne"}))],
    };
    let result = apply(&test, &open_day.manager, &changeset, input).await;
    assert_eq!(
        invalid(result),
        [("edits/0/proposal_id".to_owned(), "not-editable")]
    );
    assert_eq!(count(&test, "workstream").await, 0);
}

#[tokio::test]
async fn a_workstream_name_taken_after_the_proposal_makes_the_apply_conflict() {
    let test = TestDatabase::start().await;
    let open_day = open_day(&test).await;
    let id = Uuid::now_v7();
    let changeset = propose(
        &test,
        &open_day.contributor,
        Some(open_day.event),
        vec![proposal(
            id,
            workstream_proposal(
                open_day.event,
                Uuid::now_v7(),
                "Programm",
                open_day.contributor.user_id(),
            ),
            &[],
            "Das Open Day",
        )],
    )
    .await;
    sqlx::query(
        "INSERT INTO workstream (id, organization_id, event_id, name, lead_user_id, status, version, created_at, updated_at)
         VALUES ($1, $2, $3, 'PROGRAMM', $4, 'active', 1, now(), now())",
    )
    .bind(Uuid::now_v7())
    .bind(open_day.organization.as_uuid())
    .bind(open_day.event.as_uuid())
    .bind(open_day.manager.user_id().as_uuid())
    .execute(&test.database.pool)
    .await
    .unwrap();
    let result = apply(&test, &open_day.manager, &changeset, select(&[id])).await;
    assert!(matches!(result, Err(ApplyError::Conflict(_))), "{result:?}");
    assert_eq!(count(&test, "workstream").await, 1);
}

/// A concurrent transaction creates the same name after the check of the apply: the apply waits on the unique
/// index, loses the race when the other transaction commits, and returns a conflict, not an internal error.
#[tokio::test]
async fn an_apply_that_loses_the_race_for_a_workstream_name_conflicts() {
    let test = TestDatabase::start().await;
    let open_day = open_day(&test).await;
    let id = Uuid::now_v7();
    let changeset = propose(
        &test,
        &open_day.contributor,
        Some(open_day.event),
        vec![proposal(
            id,
            workstream_proposal(
                open_day.event,
                Uuid::now_v7(),
                "Programm",
                open_day.contributor.user_id(),
            ),
            &[],
            "Das Open Day",
        )],
    )
    .await;
    let mut holder = test.database.pool.begin().await.unwrap();
    sqlx::query(
        "INSERT INTO workstream (id, organization_id, event_id, name, lead_user_id, status, version, created_at, updated_at)
         VALUES ($1, $2, $3, 'PROGRAMM', $4, 'active', 1, now(), now())",
    )
    .bind(Uuid::now_v7())
    .bind(open_day.organization.as_uuid())
    .bind(open_day.event.as_uuid())
    .bind(open_day.manager.user_id().as_uuid())
    .execute(&mut *holder)
    .await
    .unwrap();
    let release = async {
        let waiting = "SELECT count(DISTINCT pid) FROM pg_locks WHERE NOT granted";
        let wait_for_apply = async {
            while test.scalar::<i64>(waiting).await < 1 {
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
        };
        tokio::time::timeout(std::time::Duration::from_secs(30), wait_for_apply)
            .await
            .expect("the apply waits for the name");
        holder.commit().await.unwrap();
    };
    let (result, ()) = tokio::join!(
        apply(&test, &open_day.manager, &changeset, select(&[id])),
        release,
    );
    let Err(ApplyError::Conflict(conflicts)) = result else {
        panic!("not a conflict: {result:?}");
    };
    assert_eq!(conflicts, [ProposalId::from_uuid(id)]);
    assert_eq!(count(&test, "workstream").await, 1);
    assert_eq!(
        status_of(&test, &open_day, changeset.id, id).await,
        ProposalStatus::Conflict
    );
}
