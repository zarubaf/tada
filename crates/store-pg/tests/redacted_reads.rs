//! Each read of a redactable record gives the tombstone of a legal redaction (ADR 0072): the record keeps its
//! structure, its content reads as `Redacted`, and a change of the content is an invalid transition.
//! The rows come from the fixtures of 0015 and 0025; each test makes the tombstone as the redaction does.

#![allow(clippy::unwrap_used)]

use jiff::Timestamp;
use tada_app::access::SourceReach;
use tada_app::caller::TestScope;
use tada_app::caller::{MemberCaller, OrgScope, OrganizationRole};
use tada_app::clock::Clock;
use tada_app::domain::RecordVersion;
use tada_app::domain::facts::FactState;
use tada_app::domain::ids::{
    ActionId, ChangesetId, CommitmentId, EventId, FactId, InstitutionId, OrganizationId, PersonId,
    UserId, WorkstreamId,
};
use tada_app::domain::redaction::Redactable;
use tada_app::facts::FactStore;
use tada_app::parties::{self, PartyError, PartyStore, PersonChange};
use tada_app::proposals::ProposalStore;
use tada_app::records::{EvidenceStore, RecordRef};
use tada_app::work::{self, ActionChange, WorkError, WorkFilter, WorkPorts, WorkStore};
use tada_app::workstreams::WorkstreamStore;
use tada_store_pg::testing::TestDatabase;
use uuid::Uuid;

const ORGANIZATION: &str = "0190a000-0000-7000-8000-000000000001";
const OWNER: &str = "0190a000-0000-7000-8000-000000000002";
const EVENT: &str = "0190a000-0000-7000-8000-000000000003";
const QUESTION: &str = "0190a000-0000-7000-8000-000000000101";
const FIRST_TEXT: &str = "0190a000-0000-7000-8000-000000000012";
const FIRST_CHANGESET: &str = "0190a000-0000-7000-8000-000000000031";
const FIELD_PROPOSAL: &str = "0190a000-0000-7000-8000-000000000041";
const MOTTO_PROPOSAL: &str = "0190a000-0000-7000-8000-000000000042";
const MOTTO: &str = "0190a000-0000-7000-8000-000000000051";
const MOTTO_2: &str = "0190a000-0000-7000-8000-000000000053";
const PERSON: &str = "0190a000-0000-7000-8000-000000000111";
const INSTITUTION: &str = "0190a000-0000-7000-8000-000000000112";
const WORKSTREAM: &str = "0190a000-0000-7000-8000-000000000121";
const ACTION: &str = "0190a000-0000-7000-8000-000000000122";
const COMMITMENT: &str = "0190a000-0000-7000-8000-000000000123";

#[derive(Debug)]
struct FixedClock;

impl Clock for FixedClock {
    fn now(&self) -> Timestamp {
        Timestamp::UNIX_EPOCH
    }
}

fn uuid(text: &str) -> Uuid {
    Uuid::parse_str(text).unwrap()
}

/// The owner of the organization of the fixtures. The fixture makes the owner an event manager too.
fn owner() -> MemberCaller {
    MemberCaller::new(
        UserId::from_uuid(uuid(OWNER)),
        OrganizationId::from_uuid(uuid(ORGANIZATION)),
        OrganizationRole::Owner,
    )
}

fn scope() -> OrgScope {
    owner().scope()
}

fn event() -> EventId {
    EventId::from_uuid(uuid(EVENT))
}

/// The test database with the invented rows of the fixtures of 0015 and 0025.
async fn database() -> TestDatabase {
    let test = TestDatabase::start().await;
    sqlx::raw_sql(include_str!("fixtures/0015.sql"))
        .execute(test.pool())
        .await
        .unwrap();
    sqlx::raw_sql(include_str!("fixtures/0025.sql"))
        .execute(test.pool())
        .await
        .unwrap();
    test
}

fn work_ports(test: &TestDatabase) -> WorkPorts<'_> {
    WorkPorts {
        identity: &test.database,
        work: &test.database,
        milestones: &test.database,
        decisions: &test.database,
        risks: &test.database,
        workstreams: &test.database,
        parties: &test.database,
        clock: &FixedClock,
    }
}

#[tokio::test]
async fn a_redacted_person_and_institution_read_as_redacted() {
    let test = database().await;
    test.redact(&[
        &format!(
            "UPDATE person SET name = NULL, email = NULL, phone = NULL, user_id = NULL, redacted_at = now()
             WHERE id = '{PERSON}'"
        ),
        &format!(
            "UPDATE institution SET name = NULL, email = NULL, phone = NULL, redacted_at = now()
             WHERE id = '{INSTITUTION}'"
        ),
    ])
    .await;
    let db = &test.database;

    let person = db
        .person(scope(), PersonId::from_uuid(uuid(PERSON)))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(person.name, Redactable::Redacted);
    assert_eq!(
        (&person.email, &person.phone, person.user_id),
        (&None, &None, None)
    );
    assert_eq!(person.local_id(), "PER-001");
    let persons = db.persons(scope(), None, None, 10).await.unwrap();
    assert_eq!(persons.len(), 1);
    assert_eq!(persons[0].name, Redactable::Redacted);
    // A search by name finds no redacted person.
    assert!(
        db.persons(scope(), Some("max"), None, 10)
            .await
            .unwrap()
            .is_empty()
    );
    let institution = db
        .institution(scope(), InstitutionId::from_uuid(uuid(INSTITUTION)))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(institution.name, Redactable::Redacted);
    assert_eq!(
        db.institutions(scope(), None, None, 10).await.unwrap()[0].name,
        Redactable::Redacted
    );
    assert!(
        db.named_like(scope(), "max muster")
            .await
            .unwrap()
            .is_empty()
    );

    // The promisor of the commitment reads as redacted too.
    let commitment = db
        .commitment(scope(), event(), CommitmentId::from_uuid(uuid(COMMITMENT)))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(commitment.promisor.name, Redactable::Redacted);
    assert_eq!(commitment.promisor.local_id, "PER-001");

    let change = PersonChange {
        name: None,
        email: Some(Some("max@example.org".to_owned())),
        phone: None,
        user_id: None,
        expected_version: RecordVersion::FIRST,
    };
    let result = parties::change_person(
        &owner(),
        PersonId::from_uuid(uuid(PERSON)),
        change,
        db,
        db,
        &FixedClock,
    )
    .await;
    assert!(
        matches!(result, Err(PartyError::InvalidTransition)),
        "{result:?}"
    );
}

#[tokio::test]
async fn redacted_work_records_and_questions_read_as_redacted() {
    let test = database().await;
    test.redact(&[
        &format!("UPDATE workstream SET name = NULL, redacted_at = now() WHERE id = '{WORKSTREAM}'"),
        &format!(
            "UPDATE action SET title = NULL, description = NULL, redacted_at = now() WHERE id = '{ACTION}'"
        ),
        &format!(
            "UPDATE commitment SET text = NULL, condition = NULL, firm_reason = NULL, redacted_at = now()
             WHERE id = '{COMMITMENT}'"
        ),
        &format!("UPDATE open_question SET text = NULL, redacted_at = now() WHERE id = '{QUESTION}'"),
    ])
    .await;
    let db = &test.database;

    let workstream = WorkstreamStore::get(
        db,
        scope(),
        event(),
        WorkstreamId::from_uuid(uuid(WORKSTREAM)),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(workstream.name, Redactable::Redacted);
    assert_eq!(
        db.list(scope(), event()).await.unwrap()[0].name,
        Redactable::Redacted
    );

    let action = db
        .action(scope(), event(), ActionId::from_uuid(uuid(ACTION)))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(action.fields.title, Redactable::Redacted);
    assert_eq!(action.fields.description, None);
    let filter = WorkFilter {
        owner: None,
        status: None,
        workstream: None,
        after: None,
        limit: 10,
    };
    assert_eq!(
        db.actions(scope(), event(), &filter).await.unwrap().len(),
        1
    );

    let commitment = db
        .commitment(scope(), event(), CommitmentId::from_uuid(uuid(COMMITMENT)))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(commitment.fields.text, Redactable::Redacted);
    assert_eq!(commitment.condition, None);
    let filter = WorkFilter {
        owner: None,
        status: None,
        workstream: None,
        after: None,
        limit: 10,
    };
    assert_eq!(
        db.commitments(scope(), event(), &filter)
            .await
            .unwrap()
            .len(),
        1
    );

    let profile = db.profile(scope(), event()).await.unwrap();
    assert_eq!(profile.open_questions[0].text, Redactable::Redacted);

    let change = ActionChange {
        title: None,
        description: None,
        owner: None,
        workstream: None,
        start_date: None,
        due_date: None,
        milestone: None,
        status: Some(tada_app::domain::work::ActionStatus::Done),
        expected_version: RecordVersion::FIRST,
    };
    let result = work::change_action(
        &owner(),
        event(),
        ActionId::from_uuid(uuid(ACTION)),
        change,
        work_ports(&test),
    )
    .await;
    assert!(
        matches!(result, Err(WorkError::InvalidTransition)),
        "{result:?}"
    );
}

/// The redaction of the first member text: its text, the quotes that copy it, the motto proposal of its changeset
/// and the current version of the motto.
async fn redact_the_first_text(test: &TestDatabase) {
    test.redact(&[
        &format!(
            "UPDATE source_version SET text = NULL, url = NULL, sha256 = NULL, redacted_at = now(),
                 author_actor = jsonb_build_object('kind', author_actor -> 'kind', 'id', author_actor -> 'id')
             WHERE id = '{FIRST_TEXT}'"
        ),
        &format!("UPDATE evidence_link SET quote = NULL, redacted_at = now() WHERE source_version_id = '{FIRST_TEXT}'"),
        &format!(
            "UPDATE proposal_evidence SET quote = NULL, redacted_at = now() WHERE source_version_id = '{FIRST_TEXT}'"
        ),
        &format!(
            "UPDATE record_evidence SET quote = NULL, redacted_at = now() WHERE source_version_id = '{FIRST_TEXT}'"
        ),
        &format!(
            "UPDATE proposal SET operation = jsonb_build_object('kind', operation -> 'kind'), reason = NULL,
                 redacted_at = now()
             WHERE id = '{MOTTO_PROPOSAL}'"
        ),
        &format!("UPDATE fact_version SET value = NULL, redacted_at = now() WHERE id = '{MOTTO_2}'"),
        // The evidence of the current motto cites the second text; its copy is redacted on its own.
        &format!("UPDATE evidence_link SET quote = NULL, redacted_at = now() WHERE fact_version_id = '{MOTTO_2}'"),
    ])
    .await;
}

#[tokio::test]
async fn redacted_quotes_facts_and_proposals_read_as_redacted() {
    let test = database().await;
    redact_the_first_text(&test).await;
    let db = &test.database;

    // Fact values and the quotes of their evidence.
    let profile = db.profile(scope(), event()).await.unwrap();
    let motto = &profile.fields[0];
    assert_eq!(motto.state, FactState::Accepted(Redactable::Redacted));
    assert_eq!(
        motto.evidence[0].evidence.passage.quote,
        Redactable::Redacted
    );
    let versions = db
        .fact_versions(
            scope(),
            event(),
            &[
                (FactId::from_uuid(uuid(MOTTO)), RecordVersion::FIRST),
                (
                    FactId::from_uuid(uuid(MOTTO)),
                    RecordVersion::new(2).unwrap(),
                ),
            ],
        )
        .await
        .unwrap();
    assert_eq!(versions.len(), 2);
    assert!(
        versions
            .iter()
            .any(|version| version.state == FactState::Accepted(Redactable::Redacted))
    );
    // The motto proposal is emptied, so the profile lists no open proposal of it.
    assert!(profile.proposals.is_empty());

    // The changeset: the emptied proposal keeps its kind, the other one its operation, without quotes.
    let (changeset, source) =
        ProposalStore::get(db, scope(), ChangesetId::from_uuid(uuid(FIRST_CHANGESET)))
            .await
            .unwrap()
            .unwrap();
    assert_eq!(source, Some(Redactable::Redacted));
    assert_eq!(changeset.redacted.len(), 1);
    assert_eq!(changeset.redacted[0].id.as_uuid(), uuid(MOTTO_PROPOSAL));
    assert_eq!(changeset.redacted[0].operation_kind, "set_fact");
    assert_eq!(changeset.proposals.len(), 1);
    assert_eq!(changeset.proposals[0].id.as_uuid(), uuid(FIELD_PROPOSAL));
    assert_eq!(
        changeset.proposals[0].evidence[0].passage.quote,
        Redactable::Redacted
    );

    // The evidence of the commitment.
    let evidence = db
        .evidence_of(
            scope(),
            &[RecordRef::Commitment(CommitmentId::from_uuid(uuid(
                COMMITMENT,
            )))],
            &SourceReach::Organization,
        )
        .await
        .unwrap();
    assert_eq!(evidence.len(), 1);
    assert_eq!(evidence[0].1.quote, Redactable::Redacted);
}
