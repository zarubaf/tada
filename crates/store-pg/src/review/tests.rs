use jiff::Timestamp;
use serde_json::{Value, json};
use sqlx::types::Uuid;
use tada_app::caller::{MemberCaller, OrganizationRole};
use tada_app::clock::Clock;
use tada_app::domain::facts::core_catalog;
use tada_app::domain::identity::{DisplayName, Email, EventRole};
use tada_app::domain::ids::{ChangesetId, EventId, OrganizationId, ProposalId, UserId};
use tada_app::paging::PageLimit;
use tada_app::proposals::ProposalStore;
use tada_app::proposals::{
    Changeset, Created, FactStateInput, NewChangeset, ProposeStores, ValueInput, create_changeset,
};
use tada_app::review::{
    Applied, ApplyError, ApplyInput, Edit, ProposalStatus, ReviewQueryError, ReviewStore,
    ReviewStores, apply_changeset, list_open_changesets, reject_proposals, status,
};

use crate::actor;
use crate::testing::TestDatabase;

mod drafts;

const SOURCE: &str = "Das Open Day findet im Mai 2030 statt. Wir rechnen mit 20000 Besuchern.";

#[derive(Debug)]
struct FixedClock;

impl Clock for FixedClock {
    fn now(&self) -> Timestamp {
        "2030-05-18T08:00:00.123456Z".parse().unwrap()
    }
}

fn core_field(key: &str) -> Uuid {
    core_catalog()
        .into_iter()
        .find(|field| field.key.as_str() == key)
        .unwrap()
        .id
        .as_uuid()
}

fn passage(quote: &str) -> Value {
    let byte = SOURCE.find(quote).unwrap();
    let start = SOURCE[..byte].chars().count();
    json!({"start": start, "end": start + quote.chars().count(), "quote": quote})
}

fn propose_stores(test: &TestDatabase) -> ProposeStores<'_> {
    ProposeStores {
        identity: &test.database,
        facts: &test.database,
        proposals: &test.database,
        sources: &test.database,
        documents: &test.database,
    }
}

fn stores(test: &TestDatabase) -> ReviewStores<'_> {
    ReviewStores {
        identity: &test.database,
        facts: &test.database,
        proposals: &test.database,
        review: &test.database,
        sources: &test.database,
    }
}

/// An organization with one event, its event manager Mia, the contributor Anna and the owner Olga.
struct OpenDay {
    organization: OrganizationId,
    event: EventId,
    manager: MemberCaller,
    contributor: MemberCaller,
    owner: MemberCaller,
}

async fn user(test: &TestDatabase, organization: OrganizationId, name: &str) -> UserId {
    let email = format!("{}@example.org", name.to_lowercase());
    let user = test
        .create_user(
            &DisplayName::parse(name).unwrap(),
            &Email::parse(&email).unwrap(),
        )
        .await;
    test.add_membership(organization, user, OrganizationRole::Member)
        .await;
    user
}

async fn add_event_role(test: &TestDatabase, open_day: &OpenDay, user: UserId, role: EventRole) {
    sqlx::query(
        "INSERT INTO event_membership (organization_id, event_id, user_id, event_role, version, created_at)
         VALUES ($1, $2, $3, $4, 1, now())",
    )
    .bind(open_day.organization.as_uuid())
    .bind(open_day.event.as_uuid())
    .bind(user.as_uuid())
    .bind(role.as_str())
    .execute(&test.database.pool)
    .await
    .unwrap();
}

async fn open_day(test: &TestDatabase) -> OpenDay {
    let organization = test.create_organization("testwil").await;
    let event = test.create_event(organization, "OPEN30").await;
    let mia = user(test, organization, "Mia").await;
    let anna = user(test, organization, "Anna").await;
    let olga = test
        .create_user(
            &DisplayName::parse("Olga").unwrap(),
            &Email::parse("olga@example.org").unwrap(),
        )
        .await;
    test.add_membership(organization, olga, OrganizationRole::Owner)
        .await;
    let open_day = OpenDay {
        organization,
        event,
        manager: MemberCaller::new(mia, organization, OrganizationRole::Member),
        contributor: MemberCaller::new(anna, organization, OrganizationRole::Member),
        owner: MemberCaller::new(olga, organization, OrganizationRole::Owner),
    };
    add_event_role(test, &open_day, mia, EventRole::EventManager).await;
    add_event_role(test, &open_day, anna, EventRole::EventContributor).await;
    open_day
}

fn proposal(id: Uuid, operation: Value, depends_on: &[Uuid], quote: &str) -> Value {
    json!({
        "id": id,
        "operation": operation,
        "depends_on": depends_on,
        "evidence": [passage(quote)],
        "reason": "The member says so.",
    })
}

fn date_window(event: EventId, month: u8, expected_version: Option<i64>) -> Value {
    json!({
        "kind": "set-fact", "event_id": event.as_uuid(), "field_id": core_field("date_window"),
        "state": {"state": "accepted",
                  "value": {"type": "date-window", "start": format!("2030-{month:02}-01"),
                            "end": format!("2030-{month:02}-28"), "granularity": "month"}},
        "expected_version": expected_version,
    })
}

fn question(event: EventId, id: Uuid, owner: UserId) -> Value {
    json!({
        "kind": "create-open-question", "id": id, "event_id": event.as_uuid(),
        "text": "Welcher Samstag?", "owner": owner.as_uuid(),
    })
}

async fn propose(
    test: &TestDatabase,
    caller: &MemberCaller,
    event: Option<EventId>,
    proposals: Vec<Value>,
) -> Changeset {
    let input: NewChangeset = serde_json::from_value(json!({
        "event_id": event.map(EventId::as_uuid),
        "source_text": SOURCE,
        "proposals": proposals,
    }))
    .unwrap();
    match create_changeset(caller, input, propose_stores(test), &FixedClock)
        .await
        .unwrap()
    {
        Created::New(changeset) => changeset,
        Created::Existing(_) => panic!("not new"),
    }
}

fn select(ids: &[Uuid]) -> ApplyInput {
    ApplyInput {
        selected: ids.iter().copied().map(ProposalId::from_uuid).collect(),
        edits: Vec::new(),
    }
}

async fn apply(
    test: &TestDatabase,
    caller: &MemberCaller,
    changeset: &Changeset,
    input: ApplyInput,
) -> Result<Applied, ApplyError> {
    apply_changeset(caller, changeset.id, input, stores(test), &FixedClock).await
}

async fn status_of(
    test: &TestDatabase,
    open_day: &OpenDay,
    changeset: ChangesetId,
    id: Uuid,
) -> ProposalStatus {
    let results = test
        .database
        .results(open_day.owner.scope(), changeset)
        .await
        .unwrap();
    let own: Vec<_> = results
        .into_iter()
        .filter(|record| record.proposal_id.as_uuid() == id)
        .collect();
    status(&own)
}

async fn count(test: &TestDatabase, table: &str) -> i64 {
    test.scalar(&format!("SELECT count(*) FROM {table}")).await
}

/// The number, state and value of each version of the fact of `field` in the event.
async fn fact_versions(
    test: &TestDatabase,
    event: EventId,
    field: Uuid,
) -> Vec<(i64, String, Option<Value>)> {
    sqlx::query_as(
        "SELECT v.number, v.state, v.value FROM fact_version v
         JOIN fact f ON f.id = v.fact_id
         WHERE f.event_id = $1 AND f.field_id = $2 ORDER BY v.number",
    )
    .bind(event.as_uuid())
    .bind(field)
    .fetch_all(&test.database.pool)
    .await
    .unwrap()
}

/// Acceptance (4): a changed accepted record causes an old proposal to conflict rather than overwrite it.
#[tokio::test]
async fn an_applied_fact_makes_an_older_proposal_of_the_same_field_conflict() {
    let test = TestDatabase::start().await;
    let open_day = open_day(&test).await;
    let event = open_day.event;
    let (older, newer) = (Uuid::now_v7(), Uuid::now_v7());
    let first = propose(
        &test,
        &open_day.contributor,
        Some(event),
        vec![proposal(
            older,
            date_window(event, 5, None),
            &[],
            "im Mai 2030",
        )],
    )
    .await;
    let second = propose(
        &test,
        &open_day.contributor,
        Some(event),
        vec![proposal(
            newer,
            date_window(event, 6, None),
            &[],
            "im Mai 2030",
        )],
    )
    .await;

    let applied = apply(&test, &open_day.manager, &second, select(&[newer]))
        .await
        .unwrap();
    assert_eq!(
        applied.proposals,
        [(ProposalId::from_uuid(newer), ProposalStatus::Accepted)]
    );
    let field = core_field("date_window");
    let versions = fact_versions(&test, event, field).await;
    assert_eq!(versions.len(), 1);
    assert_eq!(versions[0].0, 1);
    assert_eq!(versions[0].2.as_ref().unwrap()["start"], "2030-06-01");
    let (source, quote, accepted_by, proposal_id): (Uuid, String, Value, Option<Uuid>) =
        sqlx::query_as(
            "SELECT e.source_version_id, e.quote, v.accepted_by, v.proposal_id
             FROM evidence_link e JOIN fact_version v ON v.id = e.fact_version_id",
        )
        .fetch_one(&test.database.pool)
        .await
        .unwrap();
    assert_eq!(
        (source, quote.as_str()),
        (second.source_version_id.as_uuid(), "im Mai 2030")
    );
    assert_eq!(
        actor::from_json(&accepted_by).unwrap(),
        open_day.manager.actor()
    );
    assert_eq!(proposal_id, Some(newer));
    assert_eq!(
        status_of(&test, &open_day, second.id, newer).await,
        ProposalStatus::Accepted
    );

    let result = apply(&test, &open_day.manager, &first, select(&[older])).await;
    let Err(ApplyError::Conflict(conflicts)) = result else {
        panic!("not a conflict: {result:?}");
    };
    assert_eq!(conflicts, [ProposalId::from_uuid(older)]);
    assert_eq!(fact_versions(&test, event, field).await, versions);
    assert_eq!(count(&test, "evidence_link").await, 1);
    assert_eq!(
        status_of(&test, &open_day, first.id, older).await,
        ProposalStatus::Conflict
    );
    assert_eq!(
        count(&test, "audit_event WHERE action = 'proposal.conflict'").await,
        1
    );

    // A conflicting proposal does not apply.
    let again = apply(&test, &open_day.manager, &first, select(&[older])).await;
    assert!(
        matches!(again, Err(ApplyError::InvalidTransition)),
        "{again:?}"
    );
}

#[tokio::test]
async fn selecting_a_fact_that_depends_on_a_new_field_also_applies_the_field() {
    let test = TestDatabase::start().await;
    let open_day = open_day(&test).await;
    let event = open_day.event;
    let ids = [Uuid::now_v7(), Uuid::now_v7(), Uuid::now_v7()];
    let field = Uuid::now_v7();
    let changeset = propose(
        &test,
        &open_day.contributor,
        Some(event),
        vec![
            proposal(
                ids[0],
                json!({
                    "kind": "add-field-definition", "id": field, "event_id": event.as_uuid(),
                    "key": "visitors_total", "label": "Besucher total",
                    "value_type": {"type": "quantity", "unit": "person"},
                    "description": "The expected number of visitors of the whole event.",
                    "module": "open_day",
                }),
                &[],
                "Besuchern",
            ),
            proposal(
                ids[1],
                json!({
                    "kind": "set-fact", "event_id": event.as_uuid(), "field_id": field,
                    "state": {"state": "assumption", "approximate": true,
                              "value": {"type": "quantity", "min": "20000", "max": "20000"}},
                }),
                &[ids[0]],
                "20000 Besuchern",
            ),
            proposal(ids[2], date_window(event, 5, None), &[], "im Mai 2030"),
        ],
    )
    .await;

    let applied = apply(&test, &open_day.manager, &changeset, select(&[ids[1]]))
        .await
        .unwrap();
    let order: Vec<Uuid> = applied
        .proposals
        .iter()
        .map(|(id, _)| id.as_uuid())
        .collect();
    assert_eq!(order, [ids[0], ids[1]], "the field applies first");
    let key: String = sqlx::query_scalar(
        "SELECT key FROM field_definition WHERE id = $1 AND event_id = $2 AND status = 'active'",
    )
    .bind(field)
    .bind(event.as_uuid())
    .fetch_one(&test.database.pool)
    .await
    .unwrap();
    assert_eq!(key, "visitors_total");
    let versions = fact_versions(&test, event, field).await;
    assert_eq!(versions.len(), 1);
    assert_eq!(versions[0].1, "assumption");
    for (id, expected) in [
        (ids[0], ProposalStatus::Accepted),
        (ids[1], ProposalStatus::Accepted),
        (ids[2], ProposalStatus::Open),
    ] {
        assert_eq!(
            status_of(&test, &open_day, changeset.id, id).await,
            expected
        );
    }
    assert_eq!(
        count(&test, "audit_event WHERE action = 'proposal.accept'").await,
        2
    );
}

#[tokio::test]
async fn an_edit_is_the_evidence_of_its_value_and_keeps_the_proposal() {
    let test = TestDatabase::start().await;
    let open_day = open_day(&test).await;
    let event = open_day.event;
    let id = Uuid::now_v7();
    let changeset = propose(
        &test,
        &open_day.contributor,
        Some(event),
        vec![proposal(
            id,
            date_window(event, 5, None),
            &[],
            "im Mai 2030",
        )],
    )
    .await;
    let edit: FactStateInput = serde_json::from_value(json!({
        "state": "assumption",
        "value": {"type": "date-window", "start": "2030-06-01", "end": "2030-06-30", "granularity": "month"},
    }))
    .unwrap();
    let input = ApplyInput {
        selected: vec![ProposalId::from_uuid(id)],
        edits: vec![Edit {
            proposal_id: ProposalId::from_uuid(id),
            state: edit,
        }],
    };
    let applied = apply(&test, &open_day.manager, &changeset, input)
        .await
        .unwrap();
    assert_eq!(
        applied.proposals,
        [(ProposalId::from_uuid(id), ProposalStatus::AcceptedWithEdit)]
    );
    let versions = fact_versions(&test, event, core_field("date_window")).await;
    assert_eq!(versions[0].1, "assumption");
    assert_eq!(versions[0].2.as_ref().unwrap()["start"], "2030-06-01");

    let (review, kind, author, text, item_event): (Uuid, String, Value, String, Option<Uuid>) =
        sqlx::query_as(
            "SELECT v.id, v.kind, v.author_actor, v.text, i.event_id
             FROM source_version v JOIN source_item i ON i.id = v.source_item_id
             WHERE v.kind = 'review'",
        )
        .fetch_one(&test.database.pool)
        .await
        .unwrap();
    assert_eq!(kind, "review");
    assert_eq!(item_event, Some(event.as_uuid()));
    assert_eq!(actor::from_json(&author).unwrap(), open_day.manager.actor());
    assert!(text.contains("2030-06-01"), "{text}");
    let (source, quote, start, end): (Uuid, String, i32, i32) = sqlx::query_as(
        "SELECT source_version_id, quote, start_offset, end_offset FROM evidence_link",
    )
    .fetch_one(&test.database.pool)
    .await
    .unwrap();
    assert_eq!((source, quote.as_str()), (review, text.as_str()));
    assert_eq!(
        (start, end),
        (0, i32::try_from(text.chars().count()).unwrap())
    );
    let edit_source: Option<Uuid> = sqlx::query_scalar(
        "SELECT edit_source_version_id FROM review_result WHERE result = 'accepted-with-edit'",
    )
    .fetch_one(&test.database.pool)
    .await
    .unwrap();
    assert_eq!(edit_source, Some(review));

    // The proposal and its evidence stay as they were.
    let (stored, _) = test
        .database
        .get(open_day.owner.scope(), changeset.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(stored.proposals, changeset.proposals);
    assert_eq!(
        status_of(&test, &open_day, changeset.id, id).await,
        ProposalStatus::AcceptedWithEdit
    );
}

#[tokio::test]
async fn an_edit_must_match_the_value_type_of_its_field() {
    let test = TestDatabase::start().await;
    let open_day = open_day(&test).await;
    let event = open_day.event;
    let id = Uuid::now_v7();
    let changeset = propose(
        &test,
        &open_day.contributor,
        Some(event),
        vec![proposal(
            id,
            date_window(event, 5, None),
            &[],
            "im Mai 2030",
        )],
    )
    .await;
    let input = ApplyInput {
        selected: vec![ProposalId::from_uuid(id)],
        edits: vec![Edit {
            proposal_id: ProposalId::from_uuid(id),
            state: FactStateInput::Accepted {
                value: ValueInput::Boolean { value: true },
                approximate: false,
            },
        }],
    };
    let result = apply(&test, &open_day.manager, &changeset, input).await;
    let Err(ApplyError::Invalid(errors)) = result else {
        panic!("not invalid: {result:?}");
    };
    assert_eq!(
        (errors[0].field.as_ref(), errors[0].code),
        ("edits/0/state/value", "type-mismatch")
    );
    assert_eq!(count(&test, "fact_version").await, 0);
    assert_eq!(count(&test, "review_result").await, 0);
}

#[tokio::test]
async fn a_contributor_cannot_review() {
    let test = TestDatabase::start().await;
    let open_day = open_day(&test).await;
    let event = open_day.event;
    let id = Uuid::now_v7();
    let changeset = propose(
        &test,
        &open_day.contributor,
        Some(event),
        vec![proposal(
            id,
            date_window(event, 5, None),
            &[],
            "im Mai 2030",
        )],
    )
    .await;
    let contributor = &open_day.contributor;
    let applied = apply(&test, contributor, &changeset, select(&[id])).await;
    assert!(matches!(applied, Err(ApplyError::Forbidden)), "{applied:?}");
    let rejected = reject_proposals(
        contributor,
        changeset.id,
        vec![ProposalId::from_uuid(id)],
        stores(&test),
        &FixedClock,
    )
    .await;
    assert!(
        matches!(rejected, Err(ApplyError::Forbidden)),
        "{rejected:?}"
    );
    let listed = list_open_changesets(
        contributor,
        Some(event),
        None,
        PageLimit::DEFAULT,
        &test.database,
        &test.database,
    )
    .await;
    assert!(
        matches!(listed, Err(ReviewQueryError::Forbidden)),
        "{listed:?}"
    );
    let inbox = list_open_changesets(
        contributor,
        None,
        None,
        PageLimit::DEFAULT,
        &test.database,
        &test.database,
    )
    .await
    .unwrap();
    assert!(inbox.items.is_empty());

    // An owner of another organization does not find the changeset.
    let elsewhere = test.create_organization("musterhausen").await;
    let stranger = MemberCaller::new(open_day.owner.user_id(), elsewhere, OrganizationRole::Owner);
    let applied = apply(&test, &stranger, &changeset, select(&[id])).await;
    assert!(matches!(applied, Err(ApplyError::NotFound)), "{applied:?}");
    assert_eq!(count(&test, "review_result").await, 0);
}

#[tokio::test]
async fn event_local_ids_increase_and_a_failed_apply_takes_none() {
    let test = TestDatabase::start().await;
    let open_day = open_day(&test).await;
    let event = open_day.event;
    let mia = open_day.manager.user_id();
    let (q1, q2, q3, fact) = (
        Uuid::now_v7(),
        Uuid::now_v7(),
        Uuid::now_v7(),
        Uuid::now_v7(),
    );
    let (p1, p2, p3) = (Uuid::now_v7(), Uuid::now_v7(), Uuid::now_v7());
    let first = propose(
        &test,
        &open_day.contributor,
        Some(event),
        vec![proposal(p1, question(event, q1, mia), &[], "Das Open Day")],
    )
    .await;
    // The fact expects a version that does not exist, so an apply with it fails.
    let second = propose(
        &test,
        &open_day.contributor,
        Some(event),
        vec![
            proposal(p2, question(event, q2, mia), &[], "Das Open Day"),
            proposal(fact, date_window(event, 5, Some(3)), &[], "im Mai 2030"),
        ],
    )
    .await;
    let third = propose(
        &test,
        &open_day.contributor,
        Some(event),
        vec![proposal(p3, question(event, q3, mia), &[], "Das Open Day")],
    )
    .await;

    let applied = apply(&test, &open_day.manager, &first, select(&[p1]))
        .await
        .unwrap();
    assert_eq!(applied.local_ids.len(), 1);
    assert_eq!(applied.local_ids[0].local_number, 1);
    let failed = apply(&test, &open_day.manager, &second, select(&[p2, fact])).await;
    assert!(matches!(failed, Err(ApplyError::Conflict(_))), "{failed:?}");
    assert_eq!(count(&test, "open_question").await, 1);
    assert_eq!(
        status_of(&test, &open_day, second.id, p2).await,
        ProposalStatus::Open
    );
    let numbers = [
        apply(&test, &open_day.manager, &second, select(&[p2])).await,
        apply(&test, &open_day.manager, &third, select(&[p3])).await,
    ]
    .map(|applied| applied.unwrap().local_ids[0].local_number);
    assert_eq!(numbers, [2, 3]);

    let stored: Vec<(Uuid, i64, Uuid, String)> = sqlx::query_as(
        "SELECT id, local_number, owner_user_id, status FROM open_question ORDER BY local_number",
    )
    .fetch_all(&test.database.pool)
    .await
    .unwrap();
    let expected: Vec<(Uuid, i64, Uuid, String)> = [(q1, 1), (q2, 2), (q3, 3)]
        .into_iter()
        .map(|(id, number)| (id, number, mia.as_uuid(), "open".to_owned()))
        .collect();
    assert_eq!(stored, expected);
    let next: i64 = test
        .scalar("SELECT next FROM local_id_counter WHERE kind = 'QST'")
        .await;
    assert_eq!(next, 4);
}

#[tokio::test]
async fn an_unknown_applies_as_a_fact_version_without_a_value() {
    let test = TestDatabase::start().await;
    let open_day = open_day(&test).await;
    let event = open_day.event;
    let id = Uuid::now_v7();
    let unknown = json!({
        "kind": "set-fact", "event_id": event.as_uuid(), "field_id": core_field("date_window"),
        "state": {"state": "unknown"},
    });
    let changeset = propose(
        &test,
        &open_day.contributor,
        Some(event),
        vec![proposal(id, unknown, &[], "im Mai 2030")],
    )
    .await;
    apply(&test, &open_day.manager, &changeset, select(&[id]))
        .await
        .unwrap();
    assert_eq!(
        fact_versions(&test, event, core_field("date_window")).await,
        [(1, "unknown".to_owned(), None)]
    );
}

/// An organization changeset of the owner: a new event and a fact of the new event.
async fn new_event(
    test: &TestDatabase,
    open_day: &OpenDay,
    key: &str,
) -> (Changeset, EventId, [Uuid; 2]) {
    let event = EventId::from_uuid(Uuid::now_v7());
    let ids = [Uuid::now_v7(), Uuid::now_v7()];
    let changeset = propose(
        test,
        &open_day.owner,
        None,
        vec![
            proposal(
                ids[0],
                json!({"kind": "create-event", "id": event.as_uuid(), "key": key, "name": "Open Day 2031"}),
                &[],
                "Das Open Day",
            ),
            proposal(ids[1], date_window(event, 5, None), &[ids[0]], "im Mai 2030"),
        ],
    )
    .await;
    (changeset, event, ids)
}

/// The IDs of the Review Inbox of `caller`.
async fn inbox(test: &TestDatabase, caller: &MemberCaller) -> Vec<ChangesetId> {
    list_open_changesets(
        caller,
        None,
        None,
        PageLimit::DEFAULT,
        &test.database,
        &test.database,
    )
    .await
    .unwrap()
    .items
    .into_iter()
    .map(|changeset| changeset.id)
    .collect()
}

#[tokio::test]
async fn the_review_inbox_shows_organization_changesets_to_owners_and_admins_only() {
    let test = TestDatabase::start().await;
    let open_day = open_day(&test).await;
    let event = open_day.event;
    let id = Uuid::now_v7();
    let of_event = propose(
        &test,
        &open_day.contributor,
        Some(event),
        vec![proposal(
            id,
            date_window(event, 5, None),
            &[],
            "im Mai 2030",
        )],
    )
    .await;
    let (of_organization, _, _) = new_event(&test, &open_day, "OPEN31").await;

    assert_eq!(inbox(&test, &open_day.manager).await, [of_event.id]);
    assert_eq!(
        inbox(&test, &open_day.owner).await,
        [of_event.id, of_organization.id],
        "oldest first"
    );
    let admin = MemberCaller::new(
        open_day.manager.user_id(),
        open_day.organization,
        OrganizationRole::Admin,
    );
    assert_eq!(
        inbox(&test, &admin).await,
        [of_event.id, of_organization.id]
    );
    let listed = list_open_changesets(
        &open_day.manager,
        Some(event),
        None,
        PageLimit::DEFAULT,
        &test.database,
        &test.database,
    )
    .await
    .unwrap()
    .items;
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].open_proposals, 1);
    assert_eq!(listed[0].event_id, Some(event));
    assert_eq!(listed[0].author, open_day.contributor.actor());

    // A changeset without open proposals leaves the inbox.
    apply(&test, &open_day.manager, &of_event, select(&[id]))
        .await
        .unwrap();
    assert!(inbox(&test, &open_day.manager).await.is_empty());
    // The manager of an event does not review the changesets of the organization.
    let result = apply(
        &test,
        &open_day.manager,
        &of_organization,
        select(&[of_organization.proposals[0].id.as_uuid()]),
    )
    .await;
    assert!(matches!(result, Err(ApplyError::NotFound)), "{result:?}");
}

#[tokio::test]
async fn a_fact_applies_after_an_earlier_apply_accepted_its_new_event() {
    let test = TestDatabase::start().await;
    let open_day = open_day(&test).await;
    let (changeset, event, ids) = new_event(&test, &open_day, "OPEN31").await;
    let first = apply(&test, &open_day.owner, &changeset, select(&[ids[0]]))
        .await
        .unwrap();
    assert_eq!(
        first.proposals,
        [(ProposalId::from_uuid(ids[0]), ProposalStatus::Accepted)]
    );
    let second = apply(&test, &open_day.owner, &changeset, select(&[ids[1]]))
        .await
        .unwrap();
    assert_eq!(
        second.proposals,
        [(ProposalId::from_uuid(ids[1]), ProposalStatus::Accepted)]
    );
    assert_eq!(
        fact_versions(&test, event, core_field("date_window"))
            .await
            .len(),
        1
    );
    // An accepted proposal does not apply twice.
    let again = apply(&test, &open_day.owner, &changeset, select(&[ids[0]])).await;
    assert!(
        matches!(again, Err(ApplyError::InvalidTransition)),
        "{again:?}"
    );
}

#[tokio::test]
async fn an_owner_applies_a_new_event_and_becomes_its_event_manager() {
    let test = TestDatabase::start().await;
    let open_day = open_day(&test).await;
    let (changeset, event, ids) = new_event(&test, &open_day, "OPEN31").await;
    let applied = apply(&test, &open_day.owner, &changeset, select(&[ids[1]]))
        .await
        .unwrap();
    assert_eq!(applied.proposals.len(), 2);
    let (key, version): (String, i64) =
        sqlx::query_as("SELECT key, version FROM event WHERE id = $1")
            .bind(event.as_uuid())
            .fetch_one(&test.database.pool)
            .await
            .unwrap();
    assert_eq!((key.as_str(), version), ("OPEN31", 1));
    let role: String = sqlx::query_scalar(
        "SELECT event_role FROM event_membership WHERE event_id = $1 AND user_id = $2",
    )
    .bind(event.as_uuid())
    .bind(open_day.owner.user_id().as_uuid())
    .fetch_one(&test.database.pool)
    .await
    .unwrap();
    assert_eq!(role, "event-manager");
    assert_eq!(
        fact_versions(&test, event, core_field("date_window"))
            .await
            .len(),
        1
    );
    for action in ["event.create", "event_membership.add"] {
        let sql = format!("audit_event WHERE action = '{action}'");
        assert_eq!(count(&test, &sql).await, 1, "{action}");
    }
}

#[tokio::test]
async fn a_new_event_with_a_taken_key_does_not_apply() {
    let test = TestDatabase::start().await;
    let open_day = open_day(&test).await;
    let (changeset, event, ids) = new_event(&test, &open_day, "OPEN30").await;
    let result = apply(&test, &open_day.owner, &changeset, select(&[ids[0]])).await;
    let Err(ApplyError::Invalid(errors)) = result else {
        panic!("not invalid: {result:?}");
    };
    assert_eq!((errors[0].field.as_ref(), errors[0].code), ("key", "taken"));
    let events: i64 = sqlx::query_scalar("SELECT count(*) FROM event WHERE id = $1")
        .bind(event.as_uuid())
        .fetch_one(&test.database.pool)
        .await
        .unwrap();
    assert_eq!(events, 0);
    assert_eq!(count(&test, "review_result").await, 0);
}

#[tokio::test]
async fn the_rejection_of_a_new_event_rejects_its_facts() {
    let test = TestDatabase::start().await;
    let open_day = open_day(&test).await;
    let (changeset, _, ids) = new_event(&test, &open_day, "OPEN31").await;
    let rejected = reject_proposals(
        &open_day.owner,
        changeset.id,
        vec![ProposalId::from_uuid(ids[0])],
        stores(&test),
        &FixedClock,
    )
    .await
    .unwrap();
    let rejected: Vec<Uuid> = rejected.proposals.iter().map(|id| id.as_uuid()).collect();
    assert_eq!(rejected, ids);
    for id in ids {
        assert_eq!(
            status_of(&test, &open_day, changeset.id, id).await,
            ProposalStatus::Rejected
        );
    }
    assert_eq!(
        count(&test, "audit_event WHERE action = 'proposal.reject'").await,
        2
    );
    let applied = apply(&test, &open_day.owner, &changeset, select(&[ids[1]])).await;
    assert!(
        matches!(applied, Err(ApplyError::InvalidTransition)),
        "{applied:?}"
    );
    let again = reject_proposals(
        &open_day.owner,
        changeset.id,
        vec![ProposalId::from_uuid(ids[1])],
        stores(&test),
        &FixedClock,
    )
    .await;
    assert!(
        matches!(again, Err(ApplyError::InvalidTransition)),
        "{again:?}"
    );
    assert!(
        list_open_changesets(
            &open_day.owner,
            None,
            None,
            PageLimit::DEFAULT,
            &test.database,
            &test.database,
        )
        .await
        .unwrap()
        .items
        .is_empty()
    );
}

#[tokio::test]
async fn the_new_record_ids_of_proposals_are_taken() {
    let test = TestDatabase::start().await;
    let open_day = open_day(&test).await;
    let (_, event, _) = new_event(&test, &open_day, "OPEN31").await;
    let taken = test.database.taken_ids(&[event.as_uuid()]).await.unwrap();
    assert_eq!(taken, [event.as_uuid()]);
    // A second changeset with the same new event fails at its creation, not at its apply.
    let input: NewChangeset = serde_json::from_value(json!({
        "source_text": SOURCE,
        "proposals": [proposal(
            Uuid::now_v7(),
            json!({"kind": "create-event", "id": event.as_uuid(), "key": "OPEN32", "name": "Open Day 2032"}),
            &[],
            "Das Open Day",
        )],
    }))
    .unwrap();
    let result = create_changeset(&open_day.owner, input, propose_stores(&test), &FixedClock).await;
    let Err(tada_app::proposals::ProposeError::Invalid(errors)) = result else {
        panic!("not invalid");
    };
    assert_eq!(
        (errors[0].field.as_ref(), errors[0].code),
        ("proposals/0/operation/id", "taken")
    );
}

/// A field of the event with the key `visitors_total`, written directly, as a concurrent apply would.
async fn insert_visitors_field(test: &TestDatabase, open_day: &OpenDay) {
    sqlx::query(
        "INSERT INTO field_definition (id, organization_id, event_id, key, label_text, value_type,
                                       description, module, status, created_at)
         VALUES ($1, $2, $3, 'visitors_total', 'Besucher', '{\"type\": \"boolean\"}', 'Taken.', 'open_day',
                 'active', now())",
    )
    .bind(Uuid::now_v7())
    .bind(open_day.organization.as_uuid())
    .bind(open_day.event.as_uuid())
    .execute(&test.database.pool)
    .await
    .unwrap();
}

fn visitors_field(event: EventId, id: Uuid) -> Value {
    json!({
        "kind": "add-field-definition", "id": id, "event_id": event.as_uuid(),
        "key": "visitors_total", "label": "Besucher total",
        "value_type": {"type": "quantity", "unit": "person"},
        "description": "The expected number of visitors of the whole event.",
        "module": "open_day",
    })
}

/// I3: a step that fails after earlier steps wrote leaves nothing: no record, no counter, no result, no audit event.
#[tokio::test]
async fn a_step_that_fails_after_earlier_writes_leaves_nothing() {
    let test = TestDatabase::start().await;
    let open_day = open_day(&test).await;
    let event = open_day.event;
    let mia = open_day.manager.user_id();
    // The proposal IDs give the order of the steps: the question, then the fact, then the field.
    let (pq, pf, pd) = (Uuid::now_v7(), Uuid::now_v7(), Uuid::now_v7());
    let changeset = propose(
        &test,
        &open_day.contributor,
        Some(event),
        vec![
            proposal(
                pq,
                question(event, Uuid::now_v7(), mia),
                &[],
                "Das Open Day",
            ),
            proposal(pf, date_window(event, 5, None), &[], "im Mai 2030"),
            proposal(pd, visitors_field(event, Uuid::now_v7()), &[], "Besuchern"),
        ],
    )
    .await;
    insert_visitors_field(&test, &open_day).await;

    let result = apply(&test, &open_day.manager, &changeset, select(&[pq, pf, pd])).await;
    let Err(ApplyError::Invalid(errors)) = result else {
        panic!("not invalid: {result:?}");
    };
    assert_eq!((errors[0].field.as_ref(), errors[0].code), ("key", "taken"));
    for table in [
        "open_question",
        "local_id_counter",
        "fact",
        "fact_version",
        "evidence_link",
        "review_result",
        "audit_event WHERE action <> 'changeset.create'",
    ] {
        assert_eq!(count(&test, table).await, 0, "{table}");
    }

    let applied = apply(&test, &open_day.manager, &changeset, select(&[pq]))
        .await
        .unwrap();
    assert_eq!(applied.local_ids[0].local_number, 1);
}

/// I3: a fact that a concurrent apply commits between the version check and the write breaks the unique
/// constraint of the fact; the apply returns a conflict and changes nothing.
#[tokio::test]
async fn a_fact_created_by_a_concurrent_apply_after_the_check_is_a_conflict() {
    let test = TestDatabase::start().await;
    let open_day = open_day(&test).await;
    let event = open_day.event;
    let mia = open_day.manager.user_id();
    let (pq, pf) = (Uuid::now_v7(), Uuid::now_v7());
    let changeset = propose(
        &test,
        &open_day.contributor,
        Some(event),
        vec![
            proposal(
                pq,
                question(event, Uuid::now_v7(), mia),
                &[],
                "Das Open Day",
            ),
            proposal(pf, date_window(event, 5, None), &[], "im Mai 2030"),
        ],
    )
    .await;
    // The write of the question, the first step, inserts the fact as the concurrent apply would.
    let race = format!(
        "CREATE FUNCTION test_race() RETURNS trigger LANGUAGE plpgsql AS $$
         BEGIN
             INSERT INTO fact (id, organization_id, event_id, field_id, version)
             VALUES (gen_random_uuid(), NEW.organization_id, NEW.event_id, '{}', 1);
             RETURN NEW;
         END;
         $$;
         CREATE TRIGGER test_race AFTER INSERT ON open_question
             FOR EACH ROW EXECUTE FUNCTION test_race();",
        core_field("date_window"),
    );
    sqlx::raw_sql(sqlx::AssertSqlSafe(race))
        .execute(&test.database.pool)
        .await
        .unwrap();

    let result = apply(&test, &open_day.manager, &changeset, select(&[pq, pf])).await;
    let Err(ApplyError::Conflict(conflicts)) = result else {
        panic!("not a conflict: {result:?}");
    };
    assert_eq!(conflicts, [ProposalId::from_uuid(pf)]);
    for table in ["open_question", "fact", "fact_version"] {
        assert_eq!(count(&test, table).await, 0, "{table}");
    }
    assert_eq!(
        status_of(&test, &open_day, changeset.id, pf).await,
        ProposalStatus::Conflict
    );
    assert_eq!(
        status_of(&test, &open_day, changeset.id, pq).await,
        ProposalStatus::Open
    );
}

fn duration(event: EventId, days: &str, expected_version: Option<i64>) -> Value {
    json!({
        "kind": "set-fact", "event_id": event.as_uuid(), "field_id": core_field("duration_days"),
        "state": {"state": "accepted", "value": {"type": "quantity", "min": days, "max": days}},
        "expected_version": expected_version,
    })
}

/// Two applies that change the same two facts in opposite orders do not deadlock: one applies, and the other
/// waits for it and then conflicts.
///
/// A third transaction holds the duration fact until both applies wait for a lock. With locks in plan order,
/// A (duration first) waits on the third transaction, and B locks the date fact and then waits on duration.
/// After the release, A gets duration and waits on the date fact that B holds: a certain deadlock.
/// With locks in the order of the fact IDs, the second apply waits for the first one, whatever the IDs are.
#[tokio::test]
async fn two_applies_of_the_same_facts_in_opposite_orders_do_not_deadlock() {
    let test = TestDatabase::start().await;
    let open_day = open_day(&test).await;
    let event = open_day.event;
    let (d0, w0) = (Uuid::now_v7(), Uuid::now_v7());
    let setup = propose(
        &test,
        &open_day.contributor,
        Some(event),
        vec![
            proposal(d0, duration(event, "1", None), &[], "Das Open Day"),
            proposal(w0, date_window(event, 5, None), &[], "im Mai 2030"),
        ],
    )
    .await;
    apply(&test, &open_day.manager, &setup, select(&[d0, w0]))
        .await
        .unwrap();

    // The proposal IDs give the order of the steps: duration then date in A, date then duration in B.
    let (a1, a2) = (Uuid::now_v7(), Uuid::now_v7());
    let a = propose(
        &test,
        &open_day.contributor,
        Some(event),
        vec![
            proposal(a1, duration(event, "2", Some(1)), &[], "Das Open Day"),
            proposal(a2, date_window(event, 6, Some(1)), &[], "im Mai 2030"),
        ],
    )
    .await;
    let (b1, b2) = (Uuid::now_v7(), Uuid::now_v7());
    let b = propose(
        &test,
        &open_day.contributor,
        Some(event),
        vec![
            proposal(b1, date_window(event, 7, Some(1)), &[], "im Mai 2030"),
            proposal(b2, duration(event, "3", Some(1)), &[], "Das Open Day"),
        ],
    )
    .await;
    let mut holder = test.database.pool.begin().await.unwrap();
    sqlx::query("SELECT id FROM fact WHERE event_id = $1 AND field_id = $2 FOR UPDATE")
        .bind(event.as_uuid())
        .bind(core_field("duration_days"))
        .fetch_one(&mut *holder)
        .await
        .unwrap();
    let release = async {
        let waiting = "SELECT count(DISTINCT pid) FROM pg_locks WHERE NOT granted";
        let wait_for_both = async {
            while test.scalar::<i64>(waiting).await < 2 {
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
        };
        tokio::time::timeout(std::time::Duration::from_secs(30), wait_for_both)
            .await
            .expect("both applies wait for a lock");
        holder.commit().await.unwrap();
    };
    let (first, second, ()) = tokio::join!(
        apply(&test, &open_day.manager, &a, select(&[a1, a2])),
        apply(&test, &open_day.manager, &b, select(&[b1, b2])),
        release,
    );
    let outcomes = [&first, &second].map(|result| match result {
        Ok(_) => "applied",
        Err(ApplyError::Conflict(_)) => "conflict",
        Err(error) => panic!("neither applied nor a conflict: {error:?}"),
    });
    let mut sorted = outcomes;
    sorted.sort_unstable();
    assert_eq!(sorted, ["applied", "conflict"]);
    assert_eq!(count(&test, "fact_version").await, 4);
}

/// A deprecated field takes no new choice, and a second deprecation conflicts (ADR 0049).
#[tokio::test]
async fn a_deprecated_field_takes_no_choice_and_no_second_deprecation() {
    let test = TestDatabase::start().await;
    let open_day = open_day(&test).await;
    let event = open_day.event.as_uuid();
    let (field, add) = (Uuid::now_v7(), Uuid::now_v7());
    let new_field = json!({
        "kind": "add-field-definition", "id": field, "event_id": event,
        "key": "runway_surface", "label": "Pistenbelag",
        "value_type": {"type": "choice", "values": [{"key": "grass", "label": "Gras"}]},
        "description": "The surface of the runway.", "module": "aviation",
    });
    let setup = propose(
        &test,
        &open_day.contributor,
        Some(open_day.event),
        vec![proposal(add, new_field, &[], "Das Open Day")],
    )
    .await;
    apply(&test, &open_day.manager, &setup, select(&[add]))
        .await
        .unwrap();

    let deprecate = json!({"kind": "deprecate-field", "event_id": event, "field_id": field});
    let choice = json!({
        "kind": "add-choice-value", "event_id": event, "field_id": field,
        "key": "asphalt", "label": "Asphalt",
    });
    let mut changesets = Vec::new();
    for operation in [deprecate.clone(), deprecate, choice] {
        let id = Uuid::now_v7();
        let changeset = propose(
            &test,
            &open_day.contributor,
            Some(open_day.event),
            vec![proposal(id, operation, &[], "Das Open Day")],
        )
        .await;
        changesets.push((changeset, id));
    }
    let (first, id) = &changesets[0];
    apply(&test, &open_day.manager, first, select(&[*id]))
        .await
        .unwrap();
    for (changeset, id) in &changesets[1..] {
        let result = apply(&test, &open_day.manager, changeset, select(&[*id])).await;
        assert!(matches!(result, Err(ApplyError::Conflict(_))), "{result:?}");
    }
}

/// A proposal cites a passage of an earlier source version of its event, not of its own source text.
/// The fact version links the earlier source version, and the review shows the excerpt of that text.
#[tokio::test]
async fn a_fact_applies_with_the_evidence_of_another_source_version_of_its_event() {
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
    let earlier = first.source_version_id;
    let mut cited = passage("20000 Besuchern");
    cited["source_version_id"] = json!(earlier.as_uuid());
    let id = Uuid::now_v7();
    let mut second = proposal(
        id,
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
    let changeset = propose(
        &test,
        &open_day.contributor,
        Some(open_day.event),
        vec![second],
    )
    .await;
    assert_eq!(
        changeset.proposals[0].evidence[0].source_version_id,
        earlier
    );

    let stored = test
        .database
        .get(open_day.owner.scope(), changeset.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(stored.0.proposals, changeset.proposals);
    let review = tada_app::review::get_changeset(
        &open_day.manager,
        changeset.id,
        stores(&test),
        &FixedClock,
    )
    .await
    .unwrap();
    assert_eq!(review.proposals[0].excerpts[0].quote, "20000 Besuchern");

    apply(&test, &open_day.manager, &changeset, select(&[id]))
        .await
        .unwrap();
    let links: Vec<(Uuid, String)> = sqlx::query_as(
        "SELECT e.source_version_id, e.quote FROM evidence_link e
         JOIN fact_version v ON v.id = e.fact_version_id
         WHERE v.proposal_id = $1",
    )
    .bind(id)
    .fetch_all(&test.database.pool)
    .await
    .unwrap();
    assert_eq!(links, [(earlier.as_uuid(), "20000 Besuchern".to_owned())]);
}

/// Source items, source versions, fact versions and evidence links never change, and each fact has its current
/// version (ADR 0050).
#[tokio::test]
async fn sources_and_fact_versions_never_change() {
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
    apply(&test, &open_day.manager, &changeset, select(&[id]))
        .await
        .unwrap();
    for statement in [
        "UPDATE source_item SET kind = kind",
        "DELETE FROM source_item",
        "TRUNCATE source_item CASCADE",
        "UPDATE source_version SET captured_at = now()",
        "DELETE FROM source_version",
        "TRUNCATE source_version CASCADE",
        "UPDATE fact_version SET approximate = approximate",
        "DELETE FROM fact_version",
        "TRUNCATE fact_version CASCADE",
        "UPDATE evidence_link SET quote = 'changed'",
        "DELETE FROM evidence_link",
        "TRUNCATE evidence_link",
    ] {
        let error = sqlx::query(sqlx::AssertSqlSafe(statement))
            .execute(&test.database.pool)
            .await
            .unwrap_err();
        assert_eq!(crate::testing::sqlstate(&error), "23001", "{statement}");
    }
    for table in [
        "source_item",
        "source_version",
        "fact_version",
        "evidence_link",
    ] {
        assert_eq!(count(&test, table).await, 1, "{table}");
    }

    // A fact without its current version fails at the commit.
    let error = sqlx::query("UPDATE fact SET version = 2 WHERE organization_id = $1")
        .bind(open_day.organization.as_uuid())
        .execute(&test.database.pool)
        .await
        .unwrap_err();
    assert_eq!(crate::testing::sqlstate(&error), "23503");
    // A fact version names a proposal of its organization, not one of another organization.
    let musterhausen = test.create_organization("musterhausen").await;
    let fly_in = test.create_event(musterhausen, "FLY31").await;
    let otto = test
        .create_user(
            &DisplayName::parse("Otto Owner").unwrap(),
            &Email::parse("otto@example.org").unwrap(),
        )
        .await;
    test.add_membership(musterhausen, otto, OrganizationRole::Owner)
        .await;
    let otto = MemberCaller::new(otto, musterhausen, OrganizationRole::Owner);
    let foreign = propose(
        &test,
        &otto,
        Some(fly_in),
        vec![proposal(
            Uuid::now_v7(),
            date_window(fly_in, 5, None),
            &[],
            "im Mai 2030",
        )],
    )
    .await;
    let error = sqlx::query(
        "INSERT INTO fact_version
             (id, organization_id, fact_id, number, state, approximate, created_at, accepted_by, proposal_id)
         SELECT $1, organization_id, fact_id, 2, 'unknown', false, now(), accepted_by, $2 FROM fact_version",
    )
    .bind(Uuid::now_v7())
    .bind(foreign.proposals[0].id.as_uuid())
    .execute(&test.database.pool)
    .await
    .unwrap_err();
    assert_eq!(crate::testing::sqlstate(&error), "23503");
}
