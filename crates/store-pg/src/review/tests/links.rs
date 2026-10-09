//! Duplicate candidates of proposed persons and institutions, and links to existing records (ADR 0069).

use tada_app::domain::parties::Party;
use tada_app::parties::{NewInstitution, NewPerson, create_institution, create_person};
use tada_app::review::{Link, get_changeset};

use super::*;

const PROPOSED_NAME: &str = "Testwil Generatoren AG";

fn new_institution(id: Uuid) -> Value {
    json!({"kind": "create-institution", "id": id, "name": PROPOSED_NAME, "institution_kind": "company"})
}

fn promise(event: EventId, id: Uuid, promisor: Value, owner: UserId) -> Value {
    json!({
        "kind": "create-commitment", "id": id, "event_id": event.as_uuid(),
        "text": "Liefert den Generator", "promisor": promisor, "owner": owner.as_uuid(),
    })
}

/// Creates an institution of the organization of `caller` and returns its ID.
async fn existing_institution(test: &TestDatabase, caller: &MemberCaller, name: &str) -> Uuid {
    let input = NewInstitution {
        id: None,
        name: name.to_owned(),
        kind: "company".to_owned(),
        email: None,
        phone: None,
    };
    create_institution(caller, input, &test.database, &test.database, &FixedClock)
        .await
        .unwrap()
        .record
        .id
        .as_uuid()
}

async fn existing_person(test: &TestDatabase, caller: &MemberCaller, name: &str) -> Uuid {
    let input = NewPerson {
        id: None,
        name: name.to_owned(),
        email: None,
        phone: None,
        user_id: None,
    };
    create_person(caller, input, &test.database, &test.database, &FixedClock)
        .await
        .unwrap()
        .record
        .id
        .as_uuid()
}

/// A changeset of the contributor with a new institution and a commitment of that institution.
async fn generator(test: &TestDatabase, open_day: &OpenDay) -> (Changeset, [Uuid; 4]) {
    let [institution_id, commitment_id, first, second] = [
        Uuid::now_v7(),
        Uuid::now_v7(),
        Uuid::now_v7(),
        Uuid::now_v7(),
    ];
    let changeset = propose(
        test,
        &open_day.contributor,
        Some(open_day.event),
        vec![
            proposal(first, new_institution(institution_id), &[], "Das Open Day"),
            proposal(
                second,
                promise(
                    open_day.event,
                    commitment_id,
                    json!({"institution": institution_id}),
                    open_day.contributor.user_id(),
                ),
                &[first],
                "im Mai 2030",
            ),
        ],
    )
    .await;
    (changeset, [institution_id, commitment_id, first, second])
}

fn link(proposal: Uuid, record: Uuid) -> Link {
    Link {
        proposal_id: ProposalId::from_uuid(proposal),
        record_id: record,
    }
}

fn linked(selected: &[Uuid], links: Vec<Link>) -> ApplyInput {
    ApplyInput {
        links,
        ..select(selected)
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

/// The number of rows of each table that an apply writes.
async fn written(test: &TestDatabase) -> [i64; 6] {
    [
        count(test, "review_result").await,
        count(test, "institution").await,
        count(test, "person").await,
        count(test, "commitment").await,
        count(test, "record_evidence").await,
        count(test, "audit_event").await,
    ]
}

#[tokio::test]
async fn the_detail_shows_similar_institutions_as_duplicates() {
    let test = TestDatabase::start().await;
    let open_day = open_day(&test).await;
    let similar = existing_institution(&test, &open_day.owner, "Generatoren Testwil").await;
    existing_institution(&test, &open_day.owner, "Zeltbau AG").await;
    // A person is not a duplicate of an institution, and a record of another organization is invisible.
    existing_person(&test, &open_day.owner, "Testwil Generatoren").await;
    let other = test.create_organization("musterhausen").await;
    sqlx::query(
        "INSERT INTO institution (id, organization_id, local_number, name, kind, version, created_at, updated_at)
         VALUES ($1, $2, 1, 'Generatoren Testwil', 'company', 1, now(), now())",
    )
    .bind(Uuid::now_v7())
    .bind(other.as_uuid())
    .execute(&test.database.pool)
    .await
    .unwrap();
    let (changeset, [_, _, first, second]) = generator(&test, &open_day).await;

    let review = get_changeset(&open_day.manager, changeset.id, stores(&test), &FixedClock)
        .await
        .unwrap();
    let proposed = review
        .proposals
        .iter()
        .find(|proposal| proposal.proposal.id.as_uuid() == first)
        .unwrap();
    let duplicates: Vec<_> = proposed
        .duplicates
        .as_ref()
        .unwrap()
        .iter()
        .map(|party| (party.party, party.local_id.as_str(), party.name.as_str()))
        .collect();
    assert_eq!(
        duplicates,
        [(
            Party::Institution(tada_app::domain::ids::InstitutionId::from_uuid(similar)),
            "INS-001",
            "Generatoren Testwil"
        )]
    );
    // Only a proposal that creates a person or an institution has duplicates.
    let commitment = review
        .proposals
        .iter()
        .find(|proposal| proposal.proposal.id.as_uuid() == second)
        .unwrap();
    assert_eq!(commitment.duplicates, None);
}

#[tokio::test]
async fn linking_uses_the_existing_institution_and_creates_none() {
    let test = TestDatabase::start().await;
    let open_day = open_day(&test).await;
    let existing = existing_institution(&test, &open_day.owner, "Generatoren Testwil").await;
    let (changeset, [proposed, commitment_id, first, second]) = generator(&test, &open_day).await;

    let applied = apply(
        &test,
        &open_day.manager,
        &changeset,
        linked(&[second], vec![link(first, existing)]),
    )
    .await
    .unwrap();
    assert_eq!(
        applied.proposals,
        [
            (
                ProposalId::from_uuid(first),
                ProposalStatus::AcceptedWithEdit
            ),
            (ProposalId::from_uuid(second), ProposalStatus::Accepted),
        ]
    );
    // The commitment takes the existing institution, and no institution is new.
    assert_eq!(applied.local_ids.len(), 1);
    assert_eq!(count(&test, "institution").await, 1);
    let promisor: Option<Uuid> =
        sqlx::query_scalar("SELECT institution_id FROM commitment WHERE id = $1")
            .bind(commitment_id)
            .fetch_one(&test.database.pool)
            .await
            .unwrap();
    assert_eq!(promisor, Some(existing));
    let none: i64 = sqlx::query_scalar("SELECT count(*) FROM institution WHERE id = $1")
        .bind(proposed)
        .fetch_one(&test.database.pool)
        .await
        .unwrap();
    assert_eq!(none, 0);
    let evidence: Vec<(i64, Uuid)> = sqlx::query_as(
        "SELECT record_version, proposal_id FROM record_evidence WHERE commitment_id = $1",
    )
    .bind(commitment_id)
    .fetch_all(&test.database.pool)
    .await
    .unwrap();
    assert_eq!(evidence, [(1, second)]);

    // The review text of the link names the chosen record, in the event of the changeset.
    let (text, kind, event): (String, String, Option<Uuid>) = sqlx::query_as(
        "SELECT v.text, v.kind, i.event_id FROM review_result r
         JOIN source_version v ON v.id = r.edit_source_version_id
         JOIN source_item i ON i.id = v.source_item_id
         WHERE r.proposal_id = $1 AND r.result = 'accepted-with-edit'",
    )
    .bind(first)
    .fetch_one(&test.database.pool)
    .await
    .unwrap();
    assert_eq!(
        (text.as_str(), kind.as_str(), event),
        (
            "linked to INS-001",
            "review",
            Some(open_day.event.as_uuid())
        )
    );
    let creations: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM audit_event WHERE action = 'institution.create' AND record_id = $1",
    )
    .bind(proposed)
    .fetch_one(&test.database.pool)
    .await
    .unwrap();
    assert_eq!(creations, 0);
}

#[tokio::test]
async fn a_link_to_a_record_of_another_organization_fails_and_changes_nothing() {
    let test = TestDatabase::start().await;
    let open_day = open_day(&test).await;
    let other = test.create_organization("musterhausen").await;
    let foreign = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO institution (id, organization_id, local_number, name, kind, version, created_at, updated_at)
         VALUES ($1, $2, 1, 'Generatoren Testwil', 'company', 1, now(), now())",
    )
    .bind(foreign)
    .bind(other.as_uuid())
    .execute(&test.database.pool)
    .await
    .unwrap();
    // A person of the same organization is not an institution, and an unknown ID is no record.
    let person = existing_person(&test, &open_day.owner, "Generatoren Testwil").await;
    let (changeset, [_, _, first, second]) = generator(&test, &open_day).await;
    let before = written(&test).await;

    for record in [foreign, person, Uuid::now_v7()] {
        let result = apply(
            &test,
            &open_day.manager,
            &changeset,
            linked(&[second], vec![link(first, record)]),
        )
        .await;
        assert_eq!(invalid(result), [("links/0".to_owned(), "invalid-link")]);
    }
    assert_eq!(written(&test).await, before);
    assert_eq!(
        status_of(&test, &open_day, changeset.id, first).await,
        ProposalStatus::Open
    );
}

#[tokio::test]
async fn a_link_on_a_create_action_proposal_fails() {
    let test = TestDatabase::start().await;
    let open_day = open_day(&test).await;
    let existing = existing_institution(&test, &open_day.owner, "Generatoren Testwil").await;
    let (task, task_proposal) = (Uuid::now_v7(), Uuid::now_v7());
    let changeset = propose(
        &test,
        &open_day.contributor,
        Some(open_day.event),
        vec![proposal(
            task_proposal,
            json!({
                "kind": "create-action", "id": task, "event_id": open_day.event.as_uuid(),
                "title": "Generator bestellen", "owner": open_day.contributor.user_id().as_uuid(),
            }),
            &[],
            "im Mai 2030",
        )],
    )
    .await;
    let before = written(&test).await;
    let actions = count(&test, "action").await;

    let result = apply(
        &test,
        &open_day.manager,
        &changeset,
        linked(&[task_proposal], vec![link(task_proposal, existing)]),
    )
    .await;
    assert_eq!(invalid(result), [("links/0".to_owned(), "invalid-link")]);
    assert_eq!(written(&test).await, before);
    assert_eq!(count(&test, "action").await, actions);
}

#[tokio::test]
async fn a_link_needs_its_proposal_in_the_selection_and_its_open_dependents() {
    let test = TestDatabase::start().await;
    let open_day = open_day(&test).await;
    let existing = existing_institution(&test, &open_day.owner, "Generatoren Testwil").await;
    let (changeset, [_, commitment_id, first, second]) = generator(&test, &open_day).await;
    let other = propose(
        &test,
        &open_day.contributor,
        Some(open_day.event),
        vec![proposal(
            Uuid::now_v7(),
            new_institution(Uuid::now_v7()),
            &[],
            "im Mai 2030",
        )],
    )
    .await;
    let before = written(&test).await;

    // A proposal outside the selection, of another changeset, or linked twice cannot take a link.
    let outside = linked(
        &[other.proposals[0].id.as_uuid()],
        vec![
            link(first, existing),
            link(other.proposals[0].id.as_uuid(), existing),
            link(other.proposals[0].id.as_uuid(), existing),
        ],
    );
    let result = apply(&test, &open_day.manager, &other, outside).await;
    assert_eq!(
        invalid(result),
        [
            ("links/0".to_owned(), "invalid-link"),
            ("links/2".to_owned(), "invalid-link"),
        ]
    );
    // The open commitment would keep the proposed institution, which never comes.
    let result = apply(
        &test,
        &open_day.manager,
        &changeset,
        linked(&[first], vec![link(first, existing)]),
    )
    .await;
    assert_eq!(
        invalid(result),
        [("links/0".to_owned(), "dependents-not-selected")]
    );
    assert_eq!(written(&test).await, before);

    // After the rejection of the commitment, the institution links alone.
    reject_proposals(
        &open_day.manager,
        changeset.id,
        vec![ProposalId::from_uuid(second)],
        stores(&test),
        &FixedClock,
    )
    .await
    .unwrap();
    apply(
        &test,
        &open_day.manager,
        &changeset,
        linked(&[first], vec![link(first, existing)]),
    )
    .await
    .unwrap();
    assert_eq!(count(&test, "institution").await, 1);
    assert_eq!(
        count(&test, "commitment").await,
        0,
        "{commitment_id} stays rejected"
    );
}
