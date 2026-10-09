//! Draft proposals and their acceptance (ADR 0051).

use tada_app::documents::{DocumentStore, DraftStatus, VersionContent};
use tada_app::domain::ids::DocumentId;
use tada_app::domain::sources::SourceText;
use tada_app::proposals::ProposeError;
use tada_app::review::LocalRecord;
use tada_app::sources::SourceStore;

use super::*;
use crate::testing::sqlstate;

/// A draft proposal of the event, for a new document or an existing one.
pub(super) fn draft(event: EventId, document: Value, markdown: &str) -> Value {
    json!({
        "kind": "create-document-draft",
        "event_id": event.as_uuid(),
        "document": document,
        "markdown": markdown,
    })
}

pub(super) fn new_document(id: Uuid) -> Value {
    json!({"new": {"id": id, "name": "Konzept Open Day"}})
}

/// Applies a date window to the event and returns the ID of its fact and the changeset with the source text.
pub(super) async fn accepted_fact(test: &TestDatabase, open_day: &OpenDay) -> (Uuid, Changeset) {
    let id = Uuid::now_v7();
    let changeset = propose(
        test,
        &open_day.contributor,
        Some(open_day.event),
        vec![proposal(
            id,
            date_window(open_day.event, 5, None),
            &[],
            "im Mai 2030",
        )],
    )
    .await;
    apply(test, &open_day.manager, &changeset, select(&[id]))
        .await
        .unwrap();
    let fact = sqlx::query_scalar("SELECT id FROM fact WHERE event_id = $1")
        .bind(open_day.event.as_uuid())
        .fetch_one(&test.database.pool)
        .await
        .unwrap();
    (fact, changeset)
}

/// A draft that cites fact version `version` and the words "Open Day" of the source version `source`.
fn markdown(fact: Uuid, version: i64, source: Uuid) -> String {
    format!(
        "# Konzept\n\nDas Open Day ist am [](tada:fact/{fact}?v={version}).\n\
         Es ist [ein Open Day](tada:source/{source}#4-12).\nWir erwarten 20000 Gäste.\n"
    )
}

async fn propose_draft(
    test: &TestDatabase,
    caller: &MemberCaller,
    event: EventId,
    proposals: Vec<Value>,
) -> Result<Created, ProposeError> {
    let input: NewChangeset = serde_json::from_value(json!({
        "event_id": event.as_uuid(),
        "source_text": SOURCE,
        "proposals": proposals,
    }))
    .unwrap();
    create_changeset(caller, input, propose_stores(test), &FixedClock).await
}

fn rejected_link(result: Result<Created, ProposeError>) {
    let Err(ProposeError::Invalid(errors)) = result else {
        panic!("not invalid: {result:?}");
    };
    let errors: Vec<_> = errors
        .into_iter()
        .map(|error| (error.field.into_owned(), error.code))
        .collect();
    assert_eq!(
        errors,
        [(
            "proposals/0/operation/markdown".to_owned(),
            "link-not-found"
        )]
    );
}

#[tokio::test]
async fn a_draft_proposal_stores_its_manifest_and_lint_warnings_and_they_never_change() {
    let test = TestDatabase::start().await;
    let open_day = open_day(&test).await;
    let (fact, facts) = accepted_fact(&test, &open_day).await;
    let id = Uuid::now_v7();
    let source = facts.source_version_id.as_uuid();
    let changeset = propose(
        &test,
        &open_day.contributor,
        Some(open_day.event),
        vec![proposal(
            id,
            draft(
                open_day.event,
                new_document(Uuid::now_v7()),
                &markdown(fact, 1, source),
            ),
            &[],
            "Das Open Day",
        )],
    )
    .await;

    let stored = ProposalStore::get(&test.database, open_day.contributor.scope(), changeset.id)
        .await
        .unwrap()
        .unwrap()
        .0;
    assert_eq!(stored.drafts, changeset.drafts);
    let [draft] = stored.drafts.as_slice() else {
        panic!("not one draft");
    };
    assert_eq!(draft.manifest.facts[0].fact_id.as_uuid(), fact);
    assert_eq!(draft.manifest.sources[0].passage.quote, "Open Day");
    assert_eq!(draft.lint_warnings.len(), 1, "the visitors in line 5");
    let (manifest, lint): (Value, Value) =
        sqlx::query_as("SELECT manifest, lint_warnings FROM proposal WHERE id = $1")
            .bind(id)
            .fetch_one(&test.database.pool)
            .await
            .unwrap();
    assert_eq!(manifest["facts"][0]["fact_id"], json!(fact));
    assert_eq!(lint[0]["kind"], "number");

    for change in [
        "UPDATE proposal SET manifest = '{\"facts\": [], \"sources\": []}' WHERE id = $1",
        "UPDATE proposal SET lint_warnings = '[]' WHERE id = $1",
    ] {
        let error = sqlx::query(change)
            .bind(id)
            .execute(&test.database.pool)
            .await
            .unwrap_err();
        assert_eq!(sqlstate(&error), "23001", "{change}");
    }
    // Only a draft proposal has a manifest.
    let error = sqlx::query(
        "INSERT INTO proposal (id, organization_id, changeset_id, event_id, operation, operation_version,
                               target_kind, target_id, reason, created_at, manifest, lint_warnings)
         SELECT $1, organization_id, changeset_id, event_id, operation, operation_version,
                'event', $1, reason, created_at, manifest, lint_warnings
         FROM proposal WHERE id = $2",
    )
    .bind(Uuid::now_v7())
    .bind(id)
    .execute(&test.database.pool)
    .await
    .unwrap_err();
    assert_eq!(sqlstate(&error), "23514");
}

/// Citation isolation, part of acceptance (1): a draft cannot cite a fact of another organization.
#[tokio::test]
async fn a_draft_cannot_cite_a_fact_of_another_organization() {
    let test = TestDatabase::start().await;
    let open_day = open_day(&test).await;
    let (own_fact, own) = accepted_fact(&test, &open_day).await;
    let other = test.create_organization("musterhausen").await;
    let other_event = test.create_event(other, "FLY31").await;
    let owner = test
        .create_user(
            &DisplayName::parse("Otto").unwrap(),
            &Email::parse("otto@example.org").unwrap(),
        )
        .await;
    test.add_membership(other, owner, OrganizationRole::Owner)
        .await;
    let otto = MemberCaller::new(owner, other, OrganizationRole::Owner);
    let id = Uuid::now_v7();
    let theirs = propose(
        &test,
        &otto,
        Some(other_event),
        vec![proposal(
            id,
            date_window(other_event, 6, None),
            &[],
            "im Mai 2030",
        )],
    )
    .await;
    apply(&test, &otto, &theirs, select(&[id])).await.unwrap();
    let foreign: Uuid = sqlx::query_scalar("SELECT id FROM fact WHERE event_id = $1")
        .bind(other_event.as_uuid())
        .fetch_one(&test.database.pool)
        .await
        .unwrap();

    let source = own.source_version_id.as_uuid();
    let result = propose_draft(
        &test,
        &open_day.contributor,
        open_day.event,
        vec![proposal(
            Uuid::now_v7(),
            draft(
                open_day.event,
                new_document(Uuid::now_v7()),
                &markdown(foreign, 1, source),
            ),
            &[],
            "Das Open Day",
        )],
    )
    .await;
    rejected_link(result);
    // Nor a source version of another organization.
    let result = propose_draft(
        &test,
        &open_day.contributor,
        open_day.event,
        vec![proposal(
            Uuid::now_v7(),
            draft(
                open_day.event,
                new_document(Uuid::now_v7()),
                &markdown(own_fact, 1, theirs.source_version_id.as_uuid()),
            ),
            &[],
            "Das Open Day",
        )],
    )
    .await;
    rejected_link(result);
}

#[tokio::test]
async fn a_draft_cannot_cite_an_open_proposal() {
    let test = TestDatabase::start().await;
    let open_day = open_day(&test).await;
    let (fact, facts) = accepted_fact(&test, &open_day).await;
    let open = Uuid::now_v7();
    // An open proposal for version 2 of the fact.
    propose(
        &test,
        &open_day.contributor,
        Some(open_day.event),
        vec![proposal(
            open,
            date_window(open_day.event, 6, Some(1)),
            &[],
            "im Mai 2030",
        )],
    )
    .await;
    let source = facts.source_version_id.as_uuid();
    for (target, version) in [(fact, 2), (open, 1)] {
        let result = propose_draft(
            &test,
            &open_day.contributor,
            open_day.event,
            vec![proposal(
                Uuid::now_v7(),
                draft(
                    open_day.event,
                    new_document(Uuid::now_v7()),
                    &markdown(target, version, source),
                ),
                &[],
                "Das Open Day",
            )],
        )
        .await;
        rejected_link(result);
    }
}

/// "Every asserted fact traces to an accepted field or an exact source version" (drafts).
#[tokio::test]
async fn an_accepted_draft_is_a_draft_version_with_its_manifest() {
    let test = TestDatabase::start().await;
    let open_day = open_day(&test).await;
    let (fact, facts) = accepted_fact(&test, &open_day).await;
    let source = facts.source_version_id.as_uuid();
    let (id, document) = (Uuid::now_v7(), Uuid::now_v7());
    let text = markdown(fact, 1, source);
    let changeset = propose(
        &test,
        &open_day.contributor,
        Some(open_day.event),
        vec![proposal(
            id,
            draft(open_day.event, new_document(document), &text),
            &[],
            "Das Open Day",
        )],
    )
    .await;

    let applied = apply(&test, &open_day.manager, &changeset, select(&[id]))
        .await
        .unwrap();
    let [local] = applied.local_ids.as_slice() else {
        panic!("not one local ID");
    };
    assert_eq!(
        local.record,
        LocalRecord::Document(DocumentId::from_uuid(document))
    );
    assert_eq!(local.local_number, 1);

    let scope = open_day.manager.scope();
    let view = DocumentStore::get(&test.database, scope, DocumentId::from_uuid(document))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(view.readable_id(), "DOC-001");
    assert_eq!(view.name, "Konzept Open Day");
    assert_eq!(view.owner, open_day.manager.user_id());
    assert_eq!(
        view.newest_version.content,
        VersionContent::Draft {
            status: DraftStatus::Draft
        }
    );
    // A draft row does not break the document list of the event.
    let listed = test
        .database
        .list(scope, open_day.event, None, None, 10)
        .await
        .unwrap();
    assert_eq!(listed, std::slice::from_ref(&view));
    assert_eq!(
        test.database
            .version(scope, view.newest_version.id)
            .await
            .unwrap(),
        None,
        "a draft has no file to download"
    );
    let stored: (String, String, String, Uuid) = sqlx::query_as(
        "SELECT kind, status, markdown, uploaded_by FROM document_version WHERE id = $1",
    )
    .bind(view.newest_version.id.as_uuid())
    .fetch_one(&test.database.pool)
    .await
    .unwrap();
    assert_eq!(
        stored,
        (
            "draft".to_owned(),
            "draft".to_owned(),
            text,
            open_day.manager.user_id().as_uuid()
        )
    );
    // Each manifest fact is an exact stored fact version.
    let traced: (i64, i64) = sqlx::query_as(
        "SELECT count(*), count(v.id) FROM document_manifest_fact m
         LEFT JOIN fact_version v ON v.fact_id = m.fact_id AND v.number = m.fact_version_number
         WHERE m.document_version_id = $1",
    )
    .bind(view.newest_version.id.as_uuid())
    .fetch_one(&test.database.pool)
    .await
    .unwrap();
    assert_eq!(traced, (1, 1));
    let passages: Vec<(Uuid, i32, i32)> = sqlx::query_as(
        "SELECT source_version_id, start_offset, end_offset FROM document_manifest_source
         WHERE document_version_id = $1",
    )
    .bind(view.newest_version.id.as_uuid())
    .fetch_all(&test.database.pool)
    .await
    .unwrap();
    assert_eq!(passages, [(source, 4, 12)]);
}

#[tokio::test]
async fn a_draft_of_an_existing_document_expects_its_version() {
    let test = TestDatabase::start().await;
    let open_day = open_day(&test).await;
    let (id, document) = (Uuid::now_v7(), Uuid::now_v7());
    let first = propose(
        &test,
        &open_day.contributor,
        Some(open_day.event),
        vec![proposal(
            id,
            draft(open_day.event, new_document(document), "Erste Fassung.\n"),
            &[],
            "Das Open Day",
        )],
    )
    .await;
    apply(&test, &open_day.manager, &first, select(&[id]))
        .await
        .unwrap();
    let existing = json!({"existing": {"document_id": document, "expected_version": 1}});
    let (second, third) = (Uuid::now_v7(), Uuid::now_v7());
    let changesets = [
        propose(
            &test,
            &open_day.contributor,
            Some(open_day.event),
            vec![proposal(
                second,
                draft(open_day.event, existing.clone(), "Zweite Fassung.\n"),
                &[],
                "Das Open Day",
            )],
        )
        .await,
        propose(
            &test,
            &open_day.contributor,
            Some(open_day.event),
            vec![proposal(
                third,
                draft(open_day.event, existing, "Andere Fassung.\n"),
                &[],
                "Das Open Day",
            )],
        )
        .await,
    ];

    apply(&test, &open_day.manager, &changesets[0], select(&[second]))
        .await
        .unwrap();
    let result = apply(&test, &open_day.manager, &changesets[1], select(&[third])).await;
    assert!(matches!(result, Err(ApplyError::Conflict(_))), "{result:?}");
    let versions: Vec<(i32, String)> = sqlx::query_as(
        "SELECT number, markdown FROM document_version WHERE document_id = $1 ORDER BY number",
    )
    .bind(document)
    .fetch_all(&test.database.pool)
    .await
    .unwrap();
    assert_eq!(
        versions,
        [
            (1, "Erste Fassung.\n".to_owned()),
            (2, "Zweite Fassung.\n".to_owned())
        ]
    );
    let version: i64 = test.scalar("SELECT version FROM document").await;
    assert_eq!(version, 2);
}

#[tokio::test]
async fn the_markdown_and_the_manifest_of_a_draft_version_never_change() {
    let test = TestDatabase::start().await;
    let open_day = open_day(&test).await;
    let (fact, facts) = accepted_fact(&test, &open_day).await;
    let id = Uuid::now_v7();
    let changeset = propose(
        &test,
        &open_day.contributor,
        Some(open_day.event),
        vec![proposal(
            id,
            draft(
                open_day.event,
                new_document(Uuid::now_v7()),
                &markdown(fact, 1, facts.source_version_id.as_uuid()),
            ),
            &[],
            "Das Open Day",
        )],
    )
    .await;
    apply(&test, &open_day.manager, &changeset, select(&[id]))
        .await
        .unwrap();

    for change in [
        "UPDATE document_version SET markdown = 'Anders.' WHERE kind = 'draft'",
        "UPDATE document_manifest_fact SET fact_version_number = 1",
        "DELETE FROM document_manifest_fact",
        "UPDATE document_manifest_source SET end_offset = 13",
        "DELETE FROM document_manifest_source",
        "TRUNCATE document_manifest_source",
    ] {
        let error = sqlx::query(change)
            .execute(&test.database.pool)
            .await
            .unwrap_err();
        assert_eq!(sqlstate(&error), "23001", "{change}");
    }
    // The status of a draft is not content: it can change.
    sqlx::query(
        "UPDATE document_version SET status = 'approved', approved_by = uploaded_by, approved_at = now()
         WHERE kind = 'draft'",
    )
        .execute(&test.database.pool)
        .await
        .unwrap();
}

/// The reach of `access::source_reach`: a member cites the sources that the facts of a readable event cite,
/// also the intake text of the organization changeset that created the event, but not a source of an event
/// without a role.
#[tokio::test]
async fn a_contributor_cites_the_intake_text_that_the_facts_of_the_event_cite() {
    let test = TestDatabase::start().await;
    let open_day = open_day(&test).await;
    let (changeset, event, ids) = new_event(&test, &open_day, "OPEN31").await;
    apply(&test, &open_day.owner, &changeset, select(&ids))
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO event_membership (organization_id, event_id, user_id, event_role, version, created_at)
         VALUES ($1, $2, $3, 'event-contributor', 1, now())",
    )
    .bind(open_day.organization.as_uuid())
    .bind(event.as_uuid())
    .bind(open_day.contributor.user_id().as_uuid())
    .execute(&test.database.pool)
    .await
    .unwrap();
    let intake = changeset.source_version_id.as_uuid();
    let has_event: bool = test
        .scalar(&format!(
            "SELECT i.event_id IS NOT NULL FROM source_version v
             JOIN source_item i ON i.id = v.source_item_id WHERE v.id = '{intake}'"
        ))
        .await;
    assert!(!has_event, "the intake text belongs to the organization");

    let cite = |source: Uuid| {
        vec![proposal(
            Uuid::now_v7(),
            draft(
                event,
                new_document(Uuid::now_v7()),
                &format!("Es ist [ein Open Day](tada:source/{source}#4-12).\n"),
            ),
            &[],
            "Das Open Day",
        )]
    };
    propose_draft(&test, &open_day.contributor, event, cite(intake))
        .await
        .unwrap();

    // A member text of an event without a role of the contributor, which no fact or proposal of her events cites.
    let elsewhere = test.create_event(open_day.organization, "FLY31").await;
    let text = SourceText::normalize(SOURCE);
    let hidden = test
        .database
        .add_member_text(
            open_day.owner.scope(),
            elsewhere,
            &text,
            &open_day.owner.actor(),
            FixedClock.now(),
        )
        .await
        .unwrap();
    rejected_link(
        propose_draft(
            &test,
            &open_day.contributor,
            event,
            cite(hidden.id.as_uuid()),
        )
        .await,
    );
}

/// A draft cites the facts of its own event only, also for an owner who can read every event.
#[tokio::test]
async fn a_draft_cannot_cite_a_fact_of_another_event() {
    let test = TestDatabase::start().await;
    let open_day = open_day(&test).await;
    let (_, facts) = accepted_fact(&test, &open_day).await;
    let elsewhere = test.create_event(open_day.organization, "FLY31").await;
    let id = Uuid::now_v7();
    let theirs = propose(
        &test,
        &open_day.owner,
        Some(elsewhere),
        vec![proposal(
            id,
            date_window(elsewhere, 6, None),
            &[],
            "im Mai 2030",
        )],
    )
    .await;
    apply(&test, &open_day.owner, &theirs, select(&[id]))
        .await
        .unwrap();
    let other_fact: Uuid = sqlx::query_scalar("SELECT id FROM fact WHERE event_id = $1")
        .bind(elsewhere.as_uuid())
        .fetch_one(&test.database.pool)
        .await
        .unwrap();

    let source = facts.source_version_id.as_uuid();
    for caller in [&open_day.contributor, &open_day.owner] {
        let result = propose_draft(
            &test,
            caller,
            open_day.event,
            vec![proposal(
                Uuid::now_v7(),
                draft(
                    open_day.event,
                    new_document(Uuid::now_v7()),
                    &markdown(other_fact, 1, source),
                ),
                &[],
                "Das Open Day",
            )],
        )
        .await;
        rejected_link(result);
    }
}
