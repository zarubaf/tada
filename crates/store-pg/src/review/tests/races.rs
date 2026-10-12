//! A legal redaction that commits after a writer read a text, and before the writer stores its copy (ADR 0072).
//! Each store wrapper runs the redaction on its own connection between the read of the command and its write,
//! so the writer holds the stale text when it writes.

use async_trait::async_trait;
use tada_app::audit::AuditEvent;
use tada_app::caller::OrgScope;
use tada_app::caller::TestScope;
use tada_app::domain::ids::SourceVersionId;
use tada_app::domain::redaction::{
    Redactable, RedactionReason, RedactionTarget, RedactionTargetKind,
};
use tada_app::domain::sources::SourceText;
use tada_app::proposals::{Inserted, ProposeError};
use tada_app::redaction::RedactionStore;
use tada_app::review::{
    ApplyOutcome, ApplyPlan, OpenChangeset, Recorded, ReviewBatch, ReviewRecord,
};
use tada_app::store::StoreError;

use super::*;
use crate::Database;

fn person(id: Uuid) -> Value {
    json!({"kind": "create-person", "id": id, "name": "Moritz Muster", "phone": "+41 79 000 00 00"})
}

/// Redacts `target` as the owner, on another connection of the pool, and commits.
async fn redact_now(database: &Database, owner: &MemberCaller, target: RedactionTarget) {
    database
        .redact(
            owner.scope(),
            &[target],
            RedactionReason::Request,
            &owner.actor(),
        )
        .await
        .unwrap()
        .unwrap();
}

fn source_item(item: Uuid) -> RedactionTarget {
    RedactionTarget {
        kind: RedactionTargetKind::SourceItem,
        id: item,
    }
}

/// The source item of a source version.
async fn item_of(test: &TestDatabase, version: SourceVersionId) -> Uuid {
    test.scalar(&format!(
        "SELECT source_item_id FROM source_version WHERE id = '{version}'"
    ))
    .await
}

/// A proposal store that redacts `target` just before each insert.
#[derive(Debug)]
struct RedactBeforeInsert<'a> {
    database: &'a Database,
    owner: MemberCaller,
    target: RedactionTarget,
}

#[async_trait]
impl ProposalStore for RedactBeforeInsert<'_> {
    async fn taken_ids(&self, ids: &[Uuid]) -> Result<Vec<Uuid>, StoreError> {
        self.database.taken_ids(ids).await
    }

    async fn insert(
        &self,
        scope: OrgScope,
        changeset: &Changeset,
        source: Option<&SourceText>,
        audit: &AuditEvent,
    ) -> Result<Inserted, StoreError> {
        redact_now(self.database, &self.owner, self.target).await;
        self.database.insert(scope, changeset, source, audit).await
    }

    async fn get(
        &self,
        scope: OrgScope,
        id: ChangesetId,
    ) -> Result<Option<(Changeset, Option<Redactable<SourceText>>)>, StoreError> {
        ProposalStore::get(self.database, scope, id).await
    }
}

/// A review store that redacts `target` just before each apply.
#[derive(Debug)]
struct RedactBeforeApply<'a> {
    database: &'a Database,
    owner: MemberCaller,
    target: RedactionTarget,
}

#[async_trait]
impl ReviewStore for RedactBeforeApply<'_> {
    async fn results(
        &self,
        scope: OrgScope,
        changeset: ChangesetId,
    ) -> Result<Vec<ReviewRecord>, StoreError> {
        self.database.results(scope, changeset).await
    }

    async fn apply(&self, scope: OrgScope, plan: &ApplyPlan) -> Result<ApplyOutcome, StoreError> {
        redact_now(self.database, &self.owner, self.target).await;
        self.database.apply(scope, plan).await
    }

    async fn record(&self, scope: OrgScope, batch: &ReviewBatch) -> Result<Recorded, StoreError> {
        self.database.record(scope, batch).await
    }

    async fn open_changesets(
        &self,
        scope: OrgScope,
        event: Option<EventId>,
    ) -> Result<Vec<OpenChangeset>, StoreError> {
        self.database.open_changesets(scope, event).await
    }
}

#[tokio::test]
async fn a_submission_that_read_a_text_before_its_redaction_is_refused() {
    let test = TestDatabase::start().await;
    let open_day = open_day(&test).await;
    let first = propose(
        &test,
        &open_day.contributor,
        Some(open_day.event),
        vec![proposal(
            Uuid::now_v7(),
            date_window(open_day.event, 5, None),
            &[],
            "im Mai 2030",
        )],
    )
    .await;
    let mut cited = passage("20000 Besuchern");
    cited["source_version_id"] = json!(first.source_version_id.unwrap().as_uuid());
    let mut second = proposal(
        Uuid::now_v7(),
        json!({
            "kind": "set-fact", "event_id": open_day.event.as_uuid(),
            "field_id": core_field("visitor_estimate"),
            "state": {"state": "assumption",
                      "value": {"type": "quantity", "min": "20000", "max": "20000"}},
        }),
        &[],
        "Mai",
    );
    second["evidence"] = json!([cited]);
    let input: NewChangeset = serde_json::from_value(json!({
        "event_id": open_day.event.as_uuid(),
        "source_text": SOURCE,
        "proposals": [second],
    }))
    .unwrap();
    let racing = RedactBeforeInsert {
        database: &test.database,
        owner: open_day.owner.clone(),
        target: source_item(item_of(&test, first.source_version_id.unwrap()).await),
    };
    let stores = ProposeStores {
        proposals: &racing,
        ..propose_stores(&test)
    };

    let result = create_changeset(&open_day.contributor, input, stores, &FixedClock).await;
    let Err(ProposeError::Invalid(errors)) = result else {
        panic!("not refused: {result:?}");
    };
    assert_eq!(
        errors
            .iter()
            .map(|error| (error.field.as_ref(), error.code))
            .collect::<Vec<_>>(),
        [("proposals/0/evidence/0", "no-text")]
    );
    // Nothing of the second changeset went in, and no copy of the quote exists.
    assert_eq!(count(&test, "changeset").await, 1);
    assert_eq!(
        test.scalar::<i64>("SELECT count(*) FROM proposal_evidence WHERE quote IS NOT NULL")
            .await,
        0
    );
}

#[tokio::test]
async fn an_apply_that_read_a_quote_before_its_redaction_writes_a_tombstone() {
    let test = TestDatabase::start().await;
    let open_day = open_day(&test).await;
    let (fact, person_proposal) = (Uuid::now_v7(), Uuid::now_v7());
    let changeset = propose(
        &test,
        &open_day.contributor,
        Some(open_day.event),
        vec![
            proposal(
                fact,
                date_window(open_day.event, 5, None),
                &[],
                "im Mai 2030",
            ),
            proposal(person_proposal, person(Uuid::now_v7()), &[], "Das Open Day"),
        ],
    )
    .await;
    let racing = RedactBeforeApply {
        database: &test.database,
        owner: open_day.owner.clone(),
        target: source_item(item_of(&test, changeset.source_version_id.unwrap()).await),
    };
    let stores = ReviewStores {
        review: &racing,
        ..stores(&test)
    };

    apply_changeset(
        &open_day.manager,
        changeset.id,
        select(&[fact, person_proposal]),
        stores,
        &FixedClock,
    )
    .await
    .unwrap();
    // The fact version and the person exist; their evidence keeps the passage but not the quote.
    for table in ["evidence_link", "record_evidence"] {
        let quotes: Vec<(Option<String>, bool)> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
            "SELECT quote, redacted_at IS NOT NULL FROM {table}"
        )))
        .fetch_all(&test.database.pool)
        .await
        .unwrap();
        assert_eq!(quotes, [(None, true)], "{table}");
    }
}

#[tokio::test]
async fn an_apply_of_a_proposal_redacted_after_the_review_read_it_is_refused() {
    let test = TestDatabase::start().await;
    let open_day = open_day(&test).await;
    let id = Uuid::now_v7();
    let changeset = propose(
        &test,
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
    let racing = RedactBeforeApply {
        database: &test.database,
        owner: open_day.owner.clone(),
        target: RedactionTarget {
            kind: RedactionTargetKind::Proposal,
            id,
        },
    };
    let stores = ReviewStores {
        review: &racing,
        ..stores(&test)
    };

    let result = apply_changeset(
        &open_day.manager,
        changeset.id,
        select(&[id]),
        stores,
        &FixedClock,
    )
    .await;
    assert!(
        matches!(result, Err(ApplyError::InvalidTransition)),
        "{result:?}"
    );
    assert_eq!(count(&test, "fact_version").await, 0);
    assert_eq!(count(&test, "evidence_link").await, 0);
    // The redaction rejected the emptied proposal; the apply added no second result.
    assert_eq!(count(&test, "review_result").await, 1);
}

/// A redaction of a proposal holds its lock while an apply starts: the apply waits, then finds the proposal
/// emptied and applies nothing. Without the wait, the apply would read no review result yet and copy the quotes.
#[tokio::test]
async fn an_apply_waits_for_a_running_redaction_of_its_proposal() {
    let test = TestDatabase::start().await;
    let open_day = open_day(&test).await;
    let id = Uuid::now_v7();
    let changeset = propose(
        &test,
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
    // The first part of a redaction on its own connection: the lock of the target row.
    let mut redaction = test.database.pool.begin().await.unwrap();
    sqlx::query("SELECT 1 FROM proposal WHERE id = $1 FOR UPDATE")
        .bind(id)
        .execute(&mut *redaction)
        .await
        .unwrap();

    let database = test.database.clone();
    let manager = open_day.manager.clone();
    let applying = tokio::spawn(async move {
        let stores = ReviewStores {
            identity: &database,
            facts: &database,
            proposals: &database,
            review: &database,
            sources: &database,
            workstreams: &database,
            work: &database,
            parties: &database,
        };
        apply_changeset(&manager, changeset.id, select(&[id]), stores, &FixedClock).await
    });
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    assert!(!applying.is_finished(), "the apply waits for the redaction");

    // The rest of the redaction: the tombstone and the rejection, then the commit.
    for statement in [
        "SET LOCAL tada.redaction = 'on'",
        "UPDATE proposal SET operation = jsonb_build_object('kind', operation -> 'kind'), reason = NULL,
             manifest = NULL, lint_warnings = NULL, redacted_at = now() WHERE id = $1",
        "UPDATE proposal_evidence SET quote = NULL, redacted_at = now() WHERE proposal_id = $1",
        "INSERT INTO review_result (id, organization_id, proposal_id, result, reviewer, created_at)
         SELECT gen_random_uuid(), organization_id, id, 'rejected', '{}'::jsonb, now() FROM proposal WHERE id = $1",
    ] {
        let query = sqlx::query(statement);
        let query = if statement.contains("$1") {
            query.bind(id)
        } else {
            query
        };
        query.execute(&mut *redaction).await.unwrap();
    }
    redaction.commit().await.unwrap();

    let result = applying.await.unwrap();
    assert!(
        matches!(result, Err(ApplyError::InvalidTransition)),
        "{result:?}"
    );
    assert_eq!(count(&test, "fact_version").await, 0);
    assert_eq!(count(&test, "evidence_link").await, 0);
}
