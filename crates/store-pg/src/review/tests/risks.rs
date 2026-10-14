//! The apply of proposals that create or change risks (ADR 0081).

use tada_app::access::SourceReach;
use tada_app::caller::TestScope;
use tada_app::domain::ids::RiskId;
use tada_app::records::{EvidenceStore, RecordRef};
use tada_app::review::{LocalRecord, NewLocalId};

use super::work::{audit_actions, edit, insert_workstream, invalid, record_evidence};
use super::*;

fn risk(event: EventId, id: Uuid, owner: UserId) -> Value {
    json!({
        "kind": "create-risk", "id": id, "event_id": event.as_uuid(),
        "title": "Starkregen am Festtag", "description": "Das Zelt steht\nauf der Wiese.",
        "likelihood": "medium", "impact": "high", "mitigation": "Zweiter Eingang",
        "owner": owner.as_uuid(),
    })
}

fn rating(event: EventId, risk: Uuid, expected_version: i64) -> Value {
    json!({
        "kind": "change-risk-rating", "event_id": event.as_uuid(), "risk_id": risk,
        "likelihood": "high", "impact": "high", "expected_version": expected_version,
    })
}

fn risk_status(event: EventId, risk: Uuid, status: &str, expected_version: i64) -> Value {
    json!({
        "kind": "change-risk-status", "event_id": event.as_uuid(), "risk_id": risk,
        "status": status, "expected_version": expected_version,
    })
}

/// The rating, the status and the version of a risk.
async fn risk_state(test: &TestDatabase, id: Uuid) -> (String, String, String, i64) {
    sqlx::query_as("SELECT likelihood, impact, status, version FROM risk WHERE id = $1")
        .bind(id)
        .fetch_one(&test.database.pool)
        .await
        .unwrap()
}

/// A new risk of the contributor, applied by the manager: it has no workstream, so the managers review it.
async fn applied_risk(test: &TestDatabase, open_day: &OpenDay) -> Uuid {
    let (risk_id, first) = (Uuid::now_v7(), Uuid::now_v7());
    let anna = open_day.contributor.user_id();
    let changeset = propose(
        test,
        &open_day.contributor,
        Some(open_day.event),
        vec![proposal(
            first,
            risk(open_day.event, risk_id, anna),
            &[],
            "im Mai 2030",
        )],
    )
    .await;
    apply(test, &open_day.manager, &changeset, select(&[first]))
        .await
        .unwrap();
    risk_id
}

#[tokio::test]
async fn accepted_risk_gets_a_readable_id_and_evidence() {
    let test = TestDatabase::start().await;
    let open_day = open_day(&test).await;
    let anna = open_day.contributor.user_id();
    let [risk_id, first, second, third] = [(); 4].map(|()| Uuid::now_v7());
    let changeset = propose(
        &test,
        &open_day.contributor,
        Some(open_day.event),
        vec![proposal(
            first,
            risk(open_day.event, risk_id, anna),
            &[],
            "im Mai 2030",
        )],
    )
    .await;
    // The contributor owns the risk, but a risk without a workstream goes to the managers (ADR 0067, rule 3).
    let refused = apply(&test, &open_day.contributor, &changeset, select(&[first])).await;
    assert!(matches!(refused, Err(ApplyError::Forbidden)), "{refused:?}");
    let applied = apply(&test, &open_day.manager, &changeset, select(&[first]))
        .await
        .unwrap();
    assert_eq!(
        applied.local_ids,
        [NewLocalId {
            record: LocalRecord::Risk(RiskId::from_uuid(risk_id)),
            local_number: 1,
        }]
    );
    assert_eq!(
        risk_state(&test, risk_id).await,
        ("medium".into(), "high".into(), "open".into(), 1)
    );
    let (description, mitigation): (String, String) =
        sqlx::query_as("SELECT description, mitigation FROM risk WHERE id = $1")
            .bind(risk_id)
            .fetch_one(&test.database.pool)
            .await
            .unwrap();
    assert_eq!(
        (description.as_str(), mitigation.as_str()),
        ("Das Zelt steht\nauf der Wiese.", "Zweiter Eingang")
    );
    assert_eq!(
        record_evidence(&test, "risk_id", risk_id).await,
        [(1, first, "im Mai 2030".to_owned())]
    );
    assert_eq!(audit_actions(&test, risk_id).await, ["risk.create"]);

    // A rating change goes to the owner and keeps the status; a status change keeps the rating.
    let changeset = propose(
        &test,
        &open_day.contributor,
        Some(open_day.event),
        vec![
            proposal(
                second,
                rating(open_day.event, risk_id, 1),
                &[],
                "Das Open Day",
            ),
            proposal(
                third,
                risk_status(open_day.event, risk_id, "mitigated", 2),
                &[second],
                "im Mai 2030",
            ),
        ],
    )
    .await;
    apply(&test, &open_day.contributor, &changeset, select(&[second]))
        .await
        .unwrap();
    assert_eq!(
        risk_state(&test, risk_id).await,
        ("high".into(), "high".into(), "open".into(), 2)
    );
    apply(&test, &open_day.contributor, &changeset, select(&[third]))
        .await
        .unwrap();
    assert_eq!(
        risk_state(&test, risk_id).await,
        ("high".into(), "high".into(), "mitigated".into(), 3)
    );
    assert_eq!(
        record_evidence(&test, "risk_id", risk_id).await,
        [
            (1, first, "im Mai 2030".to_owned()),
            (2, second, "Das Open Day".to_owned()),
            (3, third, "im Mai 2030".to_owned()),
        ]
    );
    assert_eq!(
        audit_actions(&test, risk_id).await,
        ["risk.create", "risk.change", "risk.change"]
    );
    let evidence = test
        .database
        .evidence_of(
            open_day.manager.scope(),
            &[RecordRef::Risk(RiskId::from_uuid(risk_id))],
            &SourceReach::Organization,
        )
        .await
        .unwrap();
    assert_eq!(evidence.len(), 3);
}

/// A review edit of a new risk sets its texts, its owner and its workstream; a level that the edit leaves out stays
/// as proposed (medium likelihood, high impact).
/// The edit is applied: without the arm of the risk in the edit, the fields would be dropped.
#[tokio::test]
async fn an_edited_risk_takes_its_texts_owner_and_workstream() {
    let test = TestDatabase::start().await;
    let open_day = open_day(&test).await;
    let workstream = insert_workstream(&test, &open_day, "active").await;
    let (risk_id, first) = (Uuid::now_v7(), Uuid::now_v7());
    let anna = open_day.contributor.user_id();
    let mia = open_day.manager.user_id();
    let changeset = propose(
        &test,
        &open_day.contributor,
        Some(open_day.event),
        vec![proposal(
            first,
            risk(open_day.event, risk_id, anna),
            &[],
            "im Mai 2030",
        )],
    )
    .await;
    let input = |fields: Value| ApplyInput {
        edits: vec![edit(first, fields)],
        ..select(&[first])
    };
    // The text of another kind, a date and a too long title are refused; nothing is applied.
    let refused = apply(
        &test,
        &open_day.manager,
        &changeset,
        input(json!({"text": "Anderer Text", "date": "2030-05-01", "title": "a".repeat(201)})),
    )
    .await;
    assert_eq!(
        invalid(refused),
        [
            ("edits/0/fields/date".to_owned(), "not-editable"),
            ("edits/0/fields/text".to_owned(), "not-editable"),
            ("edits/0/fields/title".to_owned(), "too-long"),
        ]
    );
    assert_eq!(count(&test, "risk").await, 0);

    apply(
        &test,
        &open_day.manager,
        &changeset,
        input(json!({
            "title": "Starkregen am Samstag", "description": null, "mitigation": "Kies auf dem Weg",
            "owner": mia.as_uuid(), "workstream": workstream,
        })),
    )
    .await
    .unwrap();
    type Row = (
        String,
        Option<String>,
        Option<String>,
        String,
        Uuid,
        Option<Uuid>,
    );
    let row: Row = sqlx::query_as(
        "SELECT title, description, mitigation, likelihood || ' ' || impact, owner_user_id, workstream_id
         FROM risk WHERE id = $1",
    )
    .bind(risk_id)
    .fetch_one(&test.database.pool)
    .await
    .unwrap();
    assert_eq!(
        row,
        (
            "Starkregen am Samstag".to_owned(),
            None,
            Some("Kies auf dem Weg".to_owned()),
            "medium high".to_owned(),
            mia.as_uuid(),
            Some(workstream),
        )
    );
}

/// A reviewer who knows the rating is wrong edits it: the stored risk takes the edited level, and the other level
/// stays as proposed.
#[tokio::test]
async fn an_edited_risk_takes_its_edited_rating() {
    let test = TestDatabase::start().await;
    let open_day = open_day(&test).await;
    let anna = open_day.contributor.user_id();
    let mut stored = Vec::new();
    // The proposal is medium x high. One level, both levels.
    for (fields, expected) in [
        (json!({"likelihood": "high"}), ("high", "high")),
        (json!({"impact": "low"}), ("medium", "low")),
        (
            json!({"likelihood": "low", "impact": "medium"}),
            ("low", "medium"),
        ),
    ] {
        let (risk_id, first) = (Uuid::now_v7(), Uuid::now_v7());
        let changeset = propose(
            &test,
            &open_day.contributor,
            Some(open_day.event),
            vec![proposal(
                first,
                risk(open_day.event, risk_id, anna),
                &[],
                "im Mai 2030",
            )],
        )
        .await;
        let input = ApplyInput {
            edits: vec![edit(first, fields)],
            ..select(&[first])
        };
        apply(&test, &open_day.manager, &changeset, input)
            .await
            .unwrap();
        let row: (String, String) =
            sqlx::query_as("SELECT likelihood, impact FROM risk WHERE id = $1")
                .bind(risk_id)
                .fetch_one(&test.database.pool)
                .await
                .unwrap();
        stored.push((row, expected));
    }
    for ((likelihood, impact), (want_likelihood, want_impact)) in stored {
        assert_eq!(
            (likelihood.as_str(), impact.as_str()),
            (want_likelihood, want_impact)
        );
    }
}

/// A refused change changes nothing: another version, or a status that the stored risk cannot take.
#[tokio::test]
async fn a_refused_risk_change_leaves_the_risk_as_it_is() {
    let test = TestDatabase::start().await;
    let open_day = open_day(&test).await;
    let risk_id = applied_risk(&test, &open_day).await;
    let (stale, rejected, ok) = (Uuid::now_v7(), Uuid::now_v7(), Uuid::now_v7());
    let changeset = propose(
        &test,
        &open_day.contributor,
        Some(open_day.event),
        vec![
            proposal(
                stale,
                rating(open_day.event, risk_id, 7),
                &[],
                "im Mai 2030",
            ),
            proposal(
                rejected,
                risk_status(open_day.event, risk_id, "mitigated", 1),
                &[],
                "Das Open Day",
            ),
            proposal(ok, rating(open_day.event, risk_id, 1), &[], "im Mai 2030"),
        ],
    )
    .await;
    let before = risk_state(&test, risk_id).await;
    let result = apply(&test, &open_day.contributor, &changeset, select(&[stale])).await;
    assert!(matches!(result, Err(ApplyError::Conflict(_))), "{result:?}");
    // The risk closed after the proposal: a status of the proposal that the stored status cannot take conflicts.
    sqlx::query("UPDATE risk SET status = 'closed' WHERE id = $1")
        .bind(risk_id)
        .execute(&test.database.pool)
        .await
        .unwrap();
    let result = apply(
        &test,
        &open_day.contributor,
        &changeset,
        select(&[rejected]),
    )
    .await;
    assert!(matches!(result, Err(ApplyError::Conflict(_))), "{result:?}");
    assert_eq!(
        risk_state(&test, risk_id).await,
        (before.0, before.1, "closed".to_owned(), before.3)
    );
    assert_eq!(
        record_evidence(&test, "risk_id", risk_id).await.len(),
        1,
        "only the creation has evidence"
    );
    // A change that fits still applies.
    apply(&test, &open_day.contributor, &changeset, select(&[ok]))
        .await
        .unwrap();
    assert_eq!(risk_state(&test, risk_id).await.3, 2);
}

/// Two applies of changes with the same expected version: one winner and a conflict, never an internal error.
#[tokio::test]
async fn concurrent_applies_on_one_risk_give_one_winner() {
    let test = TestDatabase::start().await;
    let open_day = open_day(&test).await;
    let risk_id = applied_risk(&test, &open_day).await;
    let mut changesets = Vec::new();
    for status in ["mitigated", "closed"] {
        let id = Uuid::now_v7();
        let changeset = propose(
            &test,
            &open_day.contributor,
            Some(open_day.event),
            vec![proposal(
                id,
                risk_status(open_day.event, risk_id, status, 1),
                &[],
                "im Mai 2030",
            )],
        )
        .await;
        changesets.push((changeset, id));
    }
    let (a, b) = tokio::join!(
        apply(
            &test,
            &open_day.contributor,
            &changesets[0].0,
            select(&[changesets[0].1])
        ),
        apply(
            &test,
            &open_day.contributor,
            &changesets[1].0,
            select(&[changesets[1].1])
        ),
    );
    assert_eq!(
        [&a, &b].iter().filter(|result| result.is_ok()).count(),
        1,
        "{a:?} {b:?}"
    );
    assert!(
        [&a, &b]
            .iter()
            .any(|result| matches!(result, Err(ApplyError::Conflict(_)))),
        "{a:?} {b:?}"
    );
    assert_eq!(risk_state(&test, risk_id).await.3, 2);
}

/// A risk with the ID of the proposal appears after the proposal: the apply conflicts and the risk stays.
#[tokio::test]
async fn an_apply_of_a_risk_whose_id_was_taken_meanwhile_conflicts() {
    let test = TestDatabase::start().await;
    let open_day = open_day(&test).await;
    let anna = open_day.contributor.user_id();
    let (risk_id, first) = (Uuid::now_v7(), Uuid::now_v7());
    let changeset = propose(
        &test,
        &open_day.contributor,
        Some(open_day.event),
        vec![proposal(
            first,
            risk(open_day.event, risk_id, anna),
            &[],
            "im Mai 2030",
        )],
    )
    .await;
    sqlx::query(
        "INSERT INTO risk (id, organization_id, event_id, local_number, title, likelihood, impact,
                           owner_user_id, status, version, created_at, updated_at)
         VALUES ($1, $2, $3, 1, 'Hagel', 'low', 'low', $4, 'open', 1, now(), now())",
    )
    .bind(risk_id)
    .bind(open_day.organization.as_uuid())
    .bind(open_day.event.as_uuid())
    .bind(anna.as_uuid())
    .execute(&test.database.pool)
    .await
    .unwrap();
    let result = apply(&test, &open_day.manager, &changeset, select(&[first])).await;
    assert!(matches!(result, Err(ApplyError::Conflict(_))), "{result:?}");
    assert_eq!(count(&test, "risk").await, 1);
    assert!(record_evidence(&test, "risk_id", risk_id).await.is_empty());
}
