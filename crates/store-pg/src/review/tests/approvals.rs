//! The approval of draft versions, „Fakten geändert“, the difference of two versions and the rendering (ADR 0051).

use tada_app::audit::{AuditAction, AuditEvent};
use tada_app::documents::{
    ApproveError, DocumentReads, DocumentStore, DraftStatus, LineKind, Resolution, VersionContent,
    approve_version, diff_versions, facts_changed, render_context,
};
use tada_app::domain::RecordVersion;
use tada_app::domain::documents::DraftMarkdown;
use tada_app::domain::ids::{ChangesetId, DocumentId, DocumentVersionId, SourceVersionId};
use tada_app::domain::proposals::Operation;
use tada_app::domain::sources::{Evidence, Passage, SourceText};
use tada_app::drafts::source_uri;
use tada_app::proposals::Inserted;
use tada_app::review::get_changeset;
use tada_app::sources::SourceStore;

use super::drafts::{accepted_fact, draft, new_document};
use super::*;
use crate::testing::sqlstate;

fn reads(test: &TestDatabase) -> DocumentReads<'_> {
    DocumentReads {
        identity: &test.database,
        documents: &test.database,
        facts: &test.database,
        sources: &test.database,
    }
}

/// Proposes the draft `markdown` for `document` and applies it. Returns the ID of the new version.
async fn add_draft(
    test: &TestDatabase,
    open_day: &OpenDay,
    document: Value,
    document_id: Uuid,
    markdown: &str,
) -> DocumentVersionId {
    let id = Uuid::now_v7();
    let changeset = propose(
        test,
        &open_day.contributor,
        Some(open_day.event),
        vec![proposal(
            id,
            draft(open_day.event, document, markdown),
            &[],
            "Das Open Day",
        )],
    )
    .await;
    apply(test, &open_day.manager, &changeset, select(&[id]))
        .await
        .unwrap();
    newest(test, open_day, document_id).await
}

async fn newest(test: &TestDatabase, open_day: &OpenDay, document: Uuid) -> DocumentVersionId {
    DocumentStore::get(
        &test.database,
        open_day.owner.scope(),
        DocumentId::from_uuid(document),
    )
    .await
    .unwrap()
    .unwrap()
    .newest_version
    .id
}

fn existing(document: Uuid, expected_version: i64) -> Value {
    json!({"existing": {"document_id": document, "expected_version": expected_version}})
}

async fn approve(
    test: &TestDatabase,
    caller: &MemberCaller,
    version: DocumentVersionId,
    expected: i64,
) -> Result<DraftStatus, ApproveError> {
    let approved = approve_version(
        caller,
        version,
        RecordVersion::new(expected).unwrap(),
        reads(test),
        &FixedClock,
    )
    .await?;
    match approved.content {
        VersionContent::Draft { status } => Ok(status),
        VersionContent::Upload(_) => panic!("an upload"),
    }
}

async fn status_of_version(test: &TestDatabase, version: DocumentVersionId) -> String {
    test.scalar(&format!(
        "SELECT status FROM document_version WHERE id = '{version}'"
    ))
    .await
}

/// Acceptance (7): nobody can overwrite an approved version. A new draft is a new version, and the approved
/// version stays approved until an event manager approves the new one.
#[tokio::test]
async fn nobody_can_overwrite_an_approved_version() {
    let test = TestDatabase::start().await;
    let open_day = open_day(&test).await;
    let viewer = user(&test, open_day.organization, "Vera").await;
    add_event_role(&test, &open_day, viewer, EventRole::EventViewer).await;
    let viewer = MemberCaller::new(viewer, open_day.organization, OrganizationRole::Member);
    let document = Uuid::now_v7();
    let first = add_draft(
        &test,
        &open_day,
        new_document(document),
        document,
        "Erste Fassung.\n",
    )
    .await;

    for caller in [&viewer, &open_day.contributor] {
        let result = approve(&test, caller, first, 1).await;
        assert!(matches!(result, Err(ApproveError::Forbidden)), "{result:?}");
    }
    assert_eq!(
        approve(&test, &open_day.manager, first, 1).await.unwrap(),
        DraftStatus::Approved
    );
    let (approved_by, action): (Uuid, String) = sqlx::query_as(
        "SELECT v.approved_by, a.action FROM document_version v
         JOIN audit_event a ON a.record_id = v.id WHERE v.id = $1",
    )
    .bind(first.as_uuid())
    .fetch_one(&test.database.pool)
    .await
    .unwrap();
    assert_eq!(approved_by, open_day.manager.user_id().as_uuid());
    assert_eq!(action, "document_version.approve");
    let again = approve(&test, &open_day.manager, first, 1).await;
    assert!(
        matches!(again, Err(ApproveError::InvalidTransition)),
        "{again:?}"
    );

    let second = add_draft(
        &test,
        &open_day,
        existing(document, 1),
        document,
        "Zweite Fassung.\n",
    )
    .await;
    assert_ne!(second, first, "a new draft is a new version");
    assert_eq!(status_of_version(&test, first).await, "approved");
    assert_eq!(status_of_version(&test, second).await, "draft");

    let stale = approve(&test, &open_day.manager, second, 1).await;
    assert!(
        matches!(stale, Err(ApproveError::VersionConflict)),
        "{stale:?}"
    );
    approve(&test, &open_day.manager, second, 2).await.unwrap();
    assert_eq!(status_of_version(&test, first).await, "superseded");
    assert_eq!(status_of_version(&test, second).await, "approved");
    let back = approve(&test, &open_day.manager, first, 2).await;
    assert!(
        matches!(back, Err(ApproveError::InvalidTransition)),
        "{back:?}"
    );

    let error = sqlx::query("UPDATE document_version SET markdown = 'Anders.' WHERE id = $1")
        .bind(first.as_uuid())
        .execute(&test.database.pool)
        .await
        .unwrap_err();
    assert_eq!(sqlstate(&error), "23001");
    let markdown: String = test
        .scalar(&format!(
            "SELECT markdown FROM document_version WHERE id = '{first}'"
        ))
        .await;
    assert_eq!(markdown, "Erste Fassung.\n");
}

/// Demonstration steps 6 and 7: a changed date shows „Fakten geändert“, and the new draft shows the differences.
#[tokio::test]
async fn a_changed_date_window_marks_the_document_and_the_difference_lists_it() {
    let test = TestDatabase::start().await;
    let open_day = open_day(&test).await;
    let (fact, facts) = accepted_fact(&test, &open_day).await;
    let source = facts.source_version_id.as_uuid();
    let document = Uuid::now_v7();
    let text = |version: i64, month: &str| {
        format!(
            "# Konzept\n\nDas Open Day ist am [](tada:fact/{fact}?v={version}).\n\
             Es ist [ein Open Day](tada:source/{source}#4-12) im {month}.\n"
        )
    };
    let first = add_draft(
        &test,
        &open_day,
        new_document(document),
        document,
        &text(1, "Mai"),
    )
    .await;
    let changed = || async {
        facts_changed(
            &open_day.contributor,
            DocumentId::from_uuid(document),
            reads(&test),
        )
        .await
        .unwrap()
    };
    assert!(!changed().await);

    let id = Uuid::now_v7();
    let june = propose(
        &test,
        &open_day.contributor,
        Some(open_day.event),
        vec![proposal(
            id,
            date_window(open_day.event, 6, Some(1)),
            &[],
            "im Mai 2030",
        )],
    )
    .await;
    apply(&test, &open_day.manager, &june, select(&[id]))
        .await
        .unwrap();
    assert!(changed().await, "the draft cites version 1 of the fact");

    let second = add_draft(
        &test,
        &open_day,
        existing(document, 1),
        document,
        &text(2, "Juni"),
    )
    .await;
    assert!(!changed().await, "the newest version cites version 2");

    let diff = diff_versions(
        &open_day.contributor,
        DocumentId::from_uuid(document),
        first,
        second,
        reads(&test),
    )
    .await
    .unwrap();
    assert_eq!(diff.facts.changed.len(), 1);
    let change = diff.facts.changed[0];
    assert_eq!(change.fact_id.as_uuid(), fact);
    assert_eq!((change.from.get(), change.to.get()), (1, 2));
    assert!(diff.facts.added.is_empty() && diff.facts.removed.is_empty());
    let changed_lines: Vec<_> = diff
        .lines
        .iter()
        .filter(|line| line.kind != LineKind::Unchanged)
        .map(|line| (line.kind, line.text.as_str()))
        .collect();
    assert_eq!(
        changed_lines,
        [
            (
                LineKind::Removed,
                format!("Das Open Day ist am [](tada:fact/{fact}?v=1).").as_str()
            ),
            (
                LineKind::Removed,
                format!("Es ist [ein Open Day](tada:source/{source}#4-12) im Mai.").as_str()
            ),
            (
                LineKind::Added,
                format!("Das Open Day ist am [](tada:fact/{fact}?v=2).").as_str()
            ),
            (
                LineKind::Added,
                format!("Es ist [ein Open Day](tada:source/{source}#4-12) im Juni.").as_str()
            ),
        ]
    );

    // A version of another document is not found.
    let other = Uuid::now_v7();
    let foreign = add_draft(&test, &open_day, new_document(other), other, "Anderes.\n").await;
    let result = diff_versions(
        &open_day.contributor,
        DocumentId::from_uuid(document),
        first,
        foreign,
        reads(&test),
    )
    .await;
    assert!(result.is_err(), "{result:?}");
}

/// A fact that was unknown and is known now also changes the facts of a document (ADR 0051).
#[tokio::test]
async fn an_unknown_that_becomes_known_changes_the_facts() {
    let test = TestDatabase::start().await;
    let open_day = open_day(&test).await;
    let id = Uuid::now_v7();
    let unknown = json!({
        "kind": "set-fact", "event_id": open_day.event.as_uuid(), "field_id": core_field("date_window"),
        "state": {"state": "unknown"},
    });
    let changeset = propose(
        &test,
        &open_day.contributor,
        Some(open_day.event),
        vec![proposal(id, unknown, &[], "im Mai 2030")],
    )
    .await;
    apply(&test, &open_day.manager, &changeset, select(&[id]))
        .await
        .unwrap();
    let fact: Uuid = test.scalar("SELECT id FROM fact").await;
    let document = Uuid::now_v7();
    let version = add_draft(
        &test,
        &open_day,
        new_document(document),
        document,
        &format!("Das Datum ist [](tada:fact/{fact}?v=1).\n"),
    )
    .await;
    let rendering = render_context(&open_day.contributor, version, reads(&test))
        .await
        .unwrap();
    let Some(Resolution::Fact(cited)) = rendering.draft.links.values().next() else {
        panic!("not a fact: {rendering:?}");
    };
    assert_eq!(cited.state, tada_app::domain::facts::FactState::Unknown);
    let changed = || async {
        facts_changed(
            &open_day.manager,
            DocumentId::from_uuid(document),
            reads(&test),
        )
        .await
        .unwrap()
    };
    assert!(!changed().await);

    let known = Uuid::now_v7();
    let may = propose(
        &test,
        &open_day.contributor,
        Some(open_day.event),
        vec![proposal(
            known,
            date_window(open_day.event, 5, Some(1)),
            &[],
            "im Mai 2030",
        )],
    )
    .await;
    apply(&test, &open_day.manager, &may, select(&[known]))
        .await
        .unwrap();
    assert!(changed().await);
}

/// A reader sees each target of a draft that the access rules give, and „entfernt“ for each other target (ADR 0051).
/// The checks of a draft proposal let it cite only the sources of its event, so this test stores a draft that
/// cites a source of another event without these checks, as a store with older data or a changed rule could have.
#[tokio::test]
async fn a_reader_without_access_to_a_cited_source_gets_hidden() {
    let test = TestDatabase::start().await;
    let open_day = open_day(&test).await;
    let (fact, facts) = accepted_fact(&test, &open_day).await;
    let source = facts.source_version_id.as_uuid();
    let elsewhere = test.create_event(open_day.organization, "FLY31").await;
    let secret = test
        .database
        .add_member_text(
            open_day.owner.scope(),
            elsewhere,
            &SourceText::normalize(SOURCE),
            &open_day.owner.actor(),
            FixedClock.now(),
        )
        .await
        .unwrap()
        .id;
    let public = format!(
        "Am [](tada:fact/{fact}?v=1).\nEs ist [ein Open Day](tada:source/{source}#4-12).\n"
    );
    let markdown = format!("{public}Geheim: [Open Day](tada:source/{secret}#4-12).\n");
    let (id, document) = (Uuid::now_v7(), Uuid::now_v7());
    let mut changeset = propose(
        &test,
        &open_day.contributor,
        Some(open_day.event),
        vec![proposal(
            id,
            draft(open_day.event, new_document(document), &public),
            &[],
            "Das Open Day",
        )],
    )
    .await;
    // The same changeset again, with a manifest that also cites the source of the other event.
    let hidden = Evidence {
        source_version_id: secret,
        passage: Passage::of_range(SOURCE, 4, 12).unwrap(),
    };
    changeset.id = ChangesetId::from_uuid(Uuid::now_v7());
    changeset.source_version_id = SourceVersionId::from_uuid(Uuid::now_v7());
    let proposal = &mut changeset.proposals[0];
    proposal.id = ProposalId::from_uuid(Uuid::now_v7());
    for evidence in &mut proposal.evidence {
        evidence.source_version_id = changeset.source_version_id;
    }
    let Operation::CreateDocumentDraft {
        markdown: stored, ..
    } = &mut proposal.operation
    else {
        panic!("not a draft");
    };
    *stored = DraftMarkdown::parse(&markdown).unwrap();
    changeset.drafts[0].proposal_id = proposal.id;
    changeset.drafts[0].manifest.sources.push(hidden.clone());
    let audit = AuditEvent::new(
        open_day.contributor.actor(),
        AuditAction::ChangesetCreate,
        Some(changeset.id.as_uuid()),
        Some(open_day.contributor.scope()),
    );
    let inserted = ProposalStore::insert(
        &test.database,
        open_day.contributor.scope(),
        &changeset,
        &SourceText::normalize(SOURCE),
        &audit,
    )
    .await
    .unwrap();
    assert_eq!(inserted, Inserted::Inserted);

    // The owner reads every event, but a draft of OPEN30 shows only what OPEN30 can cite.
    let review = get_changeset(&open_day.owner, changeset.id, stores(&test), &FixedClock)
        .await
        .unwrap();
    let rendering = review.proposals[0].draft.as_ref().unwrap();
    assert_eq!(rendering.markdown.as_str(), markdown);
    assert_eq!(rendering.lint_warnings, changeset.drafts[0].lint_warnings);
    let links = &rendering.links;
    assert_eq!(links.len(), 3, "{links:?}");
    assert_eq!(links[&source_uri(&hidden)], Resolution::Hidden);
    let Resolution::Source(cited) = &links[&format!("tada:source/{source}#4-12")] else {
        panic!("not a source: {links:?}");
    };
    assert_eq!(cited.passage.quote, "Open Day");
    let Resolution::Fact(cited) = &links[&format!("tada:fact/{fact}?v=1")] else {
        panic!("not a fact: {links:?}");
    };
    assert_eq!(cited.number, RecordVersion::FIRST);
    assert!(matches!(
        cited.state,
        tada_app::domain::facts::FactState::Accepted(_)
    ));

    // The accepted draft version keeps the manifest, and its rendering applies the same rule.
    let proposal_id = changeset.proposals[0].id.as_uuid();
    apply(&test, &open_day.manager, &changeset, select(&[proposal_id]))
        .await
        .unwrap();
    let version = newest(&test, &open_day, document).await;
    let rendering = render_context(&open_day.contributor, version, reads(&test))
        .await
        .unwrap();
    assert_eq!(rendering.draft.links, *links);
    assert_eq!(rendering.draft.markdown.as_str(), markdown);
}
