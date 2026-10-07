use std::sync::Mutex;

use serde_json::{Value, json};
use tada_domain::facts::{FieldDefinition, core_catalog};
use tada_domain::identity::{EventRole, OrganizationRole};
use tada_domain::ids::{OrganizationId, UserId};

use super::*;
use crate::facts::{EventProfile, FactVersionRef};
use crate::identity::{Membership, UserRef};

const SOURCE: &str =
    "Das Open Day findet im Mai oder Juni 2030 statt.\r\nWir rechnen mit 20000 Besuchern pro Tag.";

fn testwil() -> OrganizationId {
    OrganizationId::from_uuid(Uuid::from_u128(10))
}

fn open_day() -> EventId {
    EventId::from_uuid(Uuid::from_u128(20))
}

fn anna() -> UserId {
    UserId::from_uuid(Uuid::from_u128(1))
}

/// Bruno: a member of the organization without an event role.
fn bruno() -> UserId {
    UserId::from_uuid(Uuid::from_u128(2))
}

/// Carla: an owner of the organization.
fn carla() -> UserId {
    UserId::from_uuid(Uuid::from_u128(3))
}

/// One event of one organization with the shipped catalog. It records each insert.
/// Anna has the event role `role`.
#[derive(Debug, Default)]
struct Memory {
    role: Mutex<Option<EventRole>>,
    taken: Mutex<Vec<Uuid>>,
    inserted: Mutex<Vec<(Changeset, SourceText, AuditEvent)>>,
    /// The stored changesets, also the ones of a concurrent request.
    stored: Mutex<Vec<(Changeset, SourceText)>>,
    /// The next insert loses a race: a concurrent request stores this changeset first.
    race: Mutex<Option<Changeset>>,
}

#[async_trait]
impl IdentityStore for Memory {
    async fn user(&self, _: UserId) -> Result<Option<UserRef>, StoreError> {
        unreachable!()
    }

    async fn memberships_of(&self, _: UserId) -> Result<Vec<Membership>, StoreError> {
        unreachable!()
    }

    async fn membership(
        &self,
        scope: OrgScope,
        user: UserId,
    ) -> Result<Option<OrganizationRole>, StoreError> {
        if scope.organization_id() != testwil() {
            return Ok(None);
        }
        Ok(if user == anna() || user == bruno() {
            Some(OrganizationRole::Member)
        } else if user == carla() {
            Some(OrganizationRole::Owner)
        } else {
            None
        })
    }

    async fn event_exists(&self, scope: OrgScope, event: EventId) -> Result<bool, StoreError> {
        Ok(scope.organization_id() == testwil() && event == open_day())
    }

    async fn event_role(
        &self,
        scope: OrgScope,
        event: EventId,
        user: UserId,
    ) -> Result<Option<EventRole>, StoreError> {
        let found = self.event_exists(scope, event).await? && user == anna();
        Ok(found.then(|| *self.role.lock().unwrap()).flatten())
    }
}

#[async_trait]
impl FactStore for Memory {
    async fn catalog(&self, _: OrgScope, _: EventId) -> Result<Vec<FieldDefinition>, StoreError> {
        Ok(core_catalog())
    }

    async fn profile(&self, _: OrgScope, _: EventId) -> Result<EventProfile, StoreError> {
        unreachable!()
    }

    async fn current_version(
        &self,
        _: OrgScope,
        _: EventId,
        _: FieldDefinitionId,
    ) -> Result<Option<FactVersionRef>, StoreError> {
        unreachable!()
    }
}

#[async_trait]
impl ProposalStore for Memory {
    async fn taken_ids(&self, ids: &[Uuid]) -> Result<Vec<Uuid>, StoreError> {
        let taken = self.taken.lock().unwrap();
        Ok(ids
            .iter()
            .filter(|id| taken.contains(id))
            .copied()
            .collect())
    }

    async fn insert(
        &self,
        scope: OrgScope,
        changeset: &Changeset,
        source: &SourceText,
        audit: &AuditEvent,
    ) -> Result<Inserted, StoreError> {
        assert_eq!(scope.organization_id(), testwil());
        if let Some(winner) = self.race.lock().unwrap().take() {
            self.stored.lock().unwrap().push((winner, source.clone()));
            return Ok(Inserted::IdTaken);
        }
        self.inserted
            .lock()
            .unwrap()
            .push((changeset.clone(), source.clone(), audit.clone()));
        self.stored
            .lock()
            .unwrap()
            .push((changeset.clone(), source.clone()));
        Ok(Inserted::Inserted)
    }

    async fn get(
        &self,
        scope: OrgScope,
        id: ChangesetId,
    ) -> Result<Option<(Changeset, SourceText)>, StoreError> {
        assert_eq!(scope.organization_id(), testwil());
        let stored = self.stored.lock().unwrap();
        Ok(stored
            .iter()
            .find(|(changeset, _)| changeset.id == id)
            .map(|(changeset, source)| {
                let mut changeset = changeset.clone();
                changeset.proposals.sort_by_key(|proposal| proposal.id);
                (changeset, source.clone())
            }))
    }
}

/// The changeset of a successful create, new or existing.
fn changeset_of(created: Created) -> Changeset {
    match created {
        Created::New(changeset) | Created::Existing(changeset) => changeset,
    }
}

#[derive(Debug)]
struct FixedClock;

impl Clock for FixedClock {
    fn now(&self) -> Timestamp {
        "2030-05-18T08:00:00Z".parse().unwrap()
    }
}

fn stores(memory: &Memory) -> ProposeStores<'_> {
    ProposeStores {
        identity: memory,
        facts: memory,
        proposals: memory,
    }
}

fn caller(role: OrganizationRole) -> MemberCaller {
    MemberCaller::new(anna(), testwil(), role)
}

/// A contributor of the open day.
fn contributor(memory: &Memory) -> MemberCaller {
    *memory.role.lock().unwrap() = Some(EventRole::EventContributor);
    caller(OrganizationRole::Member)
}

fn core_field(key: &str) -> Uuid {
    core_catalog()
        .into_iter()
        .find(|field| field.key.as_str() == key)
        .unwrap()
        .id
        .as_uuid()
}

/// The passage of the first occurrence of `quote` in the normalized source text.
fn passage(quote: &str) -> Value {
    let text = SourceText::normalize(SOURCE);
    let byte = text.as_str().find(quote).unwrap();
    let start = text.as_str()[..byte].chars().count();
    json!({"start": start, "end": start + quote.chars().count(), "quote": quote})
}

fn proposal(id: Uuid, operation: Value, depends_on: &[Uuid], quote: &str) -> Value {
    json!({
        "id": id,
        "operation": operation,
        "depends_on": depends_on,
        "evidence": [passage(quote)],
        "reason": "The member wrote it.",
    })
}

fn visitors(field: Uuid, value: Value) -> Value {
    json!({
        "kind": "set_fact",
        "event_id": open_day().as_uuid(),
        "field_id": field,
        "state": {"state": "assumption", "value": value, "approximate": true},
    })
}

fn quantity(number: &str) -> Value {
    json!({"type": "quantity", "min": number, "max": number})
}

fn changeset(event: Option<EventId>, proposals: Vec<Value>) -> NewChangeset {
    serde_json::from_value(json!({
        "event_id": event.map(EventId::as_uuid),
        "source_text": SOURCE,
        "proposals": proposals,
    }))
    .unwrap()
}

fn one_fact() -> NewChangeset {
    changeset(
        Some(open_day()),
        vec![proposal(
            Uuid::now_v7(),
            visitors(core_field("visitor_estimate"), quantity("20000")),
            &[],
            "20000 Besuchern pro Tag",
        )],
    )
}

fn invalid_fields(result: Result<Created, ProposeError>) -> Vec<(String, &'static str)> {
    let Err(ProposeError::Invalid(errors)) = result else {
        panic!("not invalid: {result:?}");
    };
    errors
        .into_iter()
        .map(|error| (error.field.into_owned(), error.code))
        .collect()
}

#[tokio::test]
async fn a_contributor_creates_a_changeset_with_its_source_and_an_audit_event() {
    let memory = Memory::default();
    let anna = contributor(&memory);
    let changeset = create_changeset(&anna, one_fact(), stores(&memory), &FixedClock)
        .await
        .unwrap();
    let changeset = changeset_of(changeset);

    let inserted = memory.inserted.lock().unwrap();
    let (stored, source, audit) = &inserted[0];
    assert_eq!(stored, &changeset);
    assert_eq!(changeset.event_id, Some(open_day()));
    assert_eq!(changeset.author, anna.actor());
    assert_eq!(changeset.created_at, FixedClock.now());
    assert!(ids::is_record_id(changeset.id.as_uuid()));
    assert_eq!(source, &SourceText::normalize(SOURCE));
    assert_eq!(audit.action(), AuditAction::ChangesetCreate);
    assert_eq!(audit.record_kind(), "changeset");
    assert_eq!(audit.record_id(), Some(changeset.id.as_uuid()));
    assert_eq!(audit.organization_id(), Some(testwil()));
    let [proposal] = changeset.proposals.as_slice() else {
        panic!("not one proposal");
    };
    assert_eq!(proposal.evidence[0].quote, "20000 Besuchern pro Tag");
}

#[tokio::test]
async fn a_viewer_cannot_propose() {
    let memory = Memory::default();
    *memory.role.lock().unwrap() = Some(EventRole::EventViewer);
    let viewer = caller(OrganizationRole::Member);
    let result = create_changeset(&viewer, one_fact(), stores(&memory), &FixedClock).await;
    assert!(matches!(result, Err(ProposeError::Forbidden)), "{result:?}");
    assert_eq!(ProposeError::Forbidden.code(), ProblemCode::Forbidden);
    assert!(memory.inserted.lock().unwrap().is_empty());
}

#[tokio::test]
async fn a_member_without_an_event_role_does_not_find_the_event() {
    let memory = Memory::default();
    let member = caller(OrganizationRole::Member);
    let result = create_changeset(&member, one_fact(), stores(&memory), &FixedClock).await;
    assert!(matches!(result, Err(ProposeError::NotFound)), "{result:?}");
}

#[tokio::test]
async fn a_proposal_without_evidence_is_rejected() {
    let memory = Memory::default();
    let anna = contributor(&memory);
    let mut input = one_fact();
    input.proposals[0].evidence.clear();
    let result = create_changeset(&anna, input, stores(&memory), &FixedClock).await;
    assert_eq!(
        invalid_fields(result),
        [("proposals/0/evidence".to_owned(), "evidence-missing")]
    );
    assert!(memory.inserted.lock().unwrap().is_empty());
}

#[tokio::test]
async fn a_quote_that_does_not_match_its_range_is_rejected() {
    let memory = Memory::default();
    let anna = contributor(&memory);
    let mut input = one_fact();
    input.proposals[0].evidence[0].quote = "30000 Besuchern pro Tag".to_owned();
    let result = create_changeset(&anna, input, stores(&memory), &FixedClock).await;
    assert_eq!(
        invalid_fields(result),
        [("proposals/0/evidence/0".to_owned(), "quote-mismatch")]
    );
    assert!(memory.inserted.lock().unwrap().is_empty());
}

#[tokio::test]
async fn a_cycle_is_rejected_and_nothing_is_stored() {
    let memory = Memory::default();
    let anna = contributor(&memory);
    let (a, b) = (Uuid::now_v7(), Uuid::now_v7());
    let input = changeset(
        Some(open_day()),
        vec![
            proposal(
                a,
                visitors(core_field("visitor_estimate"), quantity("20000")),
                &[b],
                "20000 Besuchern",
            ),
            proposal(
                b,
                visitors(core_field("duration_days"), quantity("2")),
                &[a],
                "Open Day",
            ),
        ],
    );
    let result = create_changeset(&anna, input, stores(&memory), &FixedClock).await;
    assert_eq!(
        invalid_fields(result),
        [("proposals/0/depends_on".to_owned(), "cycle")]
    );
    assert!(memory.inserted.lock().unwrap().is_empty());
}

fn new_field(id: Uuid, value_type: Value) -> Value {
    json!({
        "kind": "add_field_definition",
        "id": id,
        "event_id": open_day().as_uuid(),
        "key": "visitors_saturday",
        "label": "Besucher am Samstag",
        "value_type": value_type,
        "description": "The expected number of visitors on the Saturday.",
        "module": "open_day",
    })
}

#[tokio::test]
async fn a_fact_on_a_new_field_of_the_changeset_has_the_value_type_of_the_new_field() {
    let memory = Memory::default();
    let anna = contributor(&memory);
    let (field, define, fact) = (Uuid::now_v7(), Uuid::now_v7(), Uuid::now_v7());
    let input = |value: Value| {
        changeset(
            Some(open_day()),
            vec![
                proposal(
                    define,
                    new_field(field, json!({"type": "quantity", "unit": "person"})),
                    &[],
                    "Besuchern",
                ),
                proposal(fact, visitors(field, value), &[define], "20000"),
            ],
        )
    };

    let wrong = input(json!({"type": "text", "text": "viele"}));
    let result = create_changeset(&anna, wrong, stores(&memory), &FixedClock).await;
    assert_eq!(
        invalid_fields(result),
        [(
            "proposals/1/operation/state/value".to_owned(),
            "type-mismatch"
        )]
    );

    let right = input(quantity("20000"));
    let changeset = create_changeset(&anna, right, stores(&memory), &FixedClock)
        .await
        .unwrap();
    let changeset = changeset_of(changeset);
    assert_eq!(
        changeset.proposals[1].depends_on,
        [ProposalId::from_uuid(define)]
    );
}

#[tokio::test]
async fn a_proposal_that_uses_a_new_field_depends_on_its_definition() {
    let memory = Memory::default();
    let anna = contributor(&memory);
    let field = Uuid::now_v7();
    let input = changeset(
        Some(open_day()),
        vec![
            proposal(
                Uuid::now_v7(),
                new_field(field, json!({"type": "quantity", "unit": "person"})),
                &[],
                "Besuchern",
            ),
            proposal(
                Uuid::now_v7(),
                visitors(field, quantity("20000")),
                &[],
                "20000",
            ),
        ],
    );
    let result = create_changeset(&anna, input, stores(&memory), &FixedClock).await;
    assert_eq!(
        invalid_fields(result),
        [("proposals/1/depends_on".to_owned(), "dependency-missing")]
    );
}

#[tokio::test]
async fn rejects_a_value_of_another_type_than_its_shipped_field_and_an_unknown_field() {
    let memory = Memory::default();
    let anna = contributor(&memory);
    let input = changeset(
        Some(open_day()),
        vec![
            proposal(
                Uuid::now_v7(),
                visitors(
                    core_field("visitor_estimate"),
                    json!({"type": "boolean", "value": true}),
                ),
                &[],
                "20000",
            ),
            proposal(
                Uuid::now_v7(),
                visitors(Uuid::now_v7(), quantity("1")),
                &[],
                "20000",
            ),
        ],
    );
    let result = create_changeset(&anna, input, stores(&memory), &FixedClock).await;
    assert_eq!(
        invalid_fields(result),
        [
            (
                "proposals/0/operation/state/value".to_owned(),
                "type-mismatch"
            ),
            ("proposals/1/operation/field_id".to_owned(), "unknown-field"),
        ]
    );
}

#[tokio::test]
async fn rejects_ids_that_are_taken_or_not_uuid_v7() {
    let memory = Memory::default();
    let anna = contributor(&memory);
    let mut input = one_fact();
    memory.taken.lock().unwrap().push(input.proposals[0].id);
    let result = create_changeset(&anna, input.clone(), stores(&memory), &FixedClock).await;
    assert_eq!(
        invalid_fields(result),
        [("proposals/0/id".to_owned(), "taken")]
    );

    input.proposals[0].id = Uuid::from_u128(7);
    let result = create_changeset(&anna, input, stores(&memory), &FixedClock).await;
    assert_eq!(
        invalid_fields(result),
        [("proposals/0/id".to_owned(), "not-uuid-v7")]
    );
}

fn new_event(id: Uuid) -> Value {
    json!({"kind": "create_event", "id": id, "key": "OPEN30", "name": "Open Day Testwil"})
}

#[tokio::test]
async fn only_owners_and_admins_propose_a_new_event() {
    let memory = Memory::default();
    let (event, create, fact) = (Uuid::now_v7(), Uuid::now_v7(), Uuid::now_v7());
    let input = || {
        let mut set_fact = visitors(core_field("visitor_estimate"), quantity("20000"));
        set_fact["event_id"] = json!(event);
        changeset(
            None,
            vec![
                proposal(create, new_event(event), &[], "Open Day"),
                proposal(fact, set_fact, &[create], "20000"),
            ],
        )
    };

    let member = contributor(&memory);
    let result = create_changeset(&member, input(), stores(&memory), &FixedClock).await;
    assert!(matches!(result, Err(ProposeError::Forbidden)), "{result:?}");

    let admin = caller(OrganizationRole::Admin);
    let changeset = create_changeset(&admin, input(), stores(&memory), &FixedClock)
        .await
        .unwrap();
    let changeset = changeset_of(changeset);
    assert_eq!(changeset.event_id, None);
    assert_eq!(changeset.proposals.len(), 2);
}

#[tokio::test]
async fn a_changeset_of_an_event_works_in_that_event_only() {
    let memory = Memory::default();
    let anna = contributor(&memory);
    let mut other_event = visitors(core_field("visitor_estimate"), quantity("20000"));
    other_event["event_id"] = json!(Uuid::now_v7());
    let input = changeset(
        Some(open_day()),
        vec![
            proposal(Uuid::now_v7(), new_event(Uuid::now_v7()), &[], "Open Day"),
            proposal(Uuid::now_v7(), other_event, &[], "20000"),
        ],
    );
    let result = create_changeset(&anna, input, stores(&memory), &FixedClock).await;
    assert_eq!(
        invalid_fields(result),
        [
            ("proposals/0/operation".to_owned(), "event-mismatch"),
            ("proposals/1/operation".to_owned(), "event-mismatch"),
        ]
    );
}

#[tokio::test]
async fn an_open_question_needs_an_owner_who_is_a_member_of_the_event() {
    let memory = Memory::default();
    let anna = contributor(&memory);
    let question = |owner: UserId| {
        changeset(
            Some(open_day()),
            vec![proposal(
                Uuid::now_v7(),
                json!({
                    "kind": "create_open_question",
                    "id": Uuid::now_v7(),
                    "event_id": open_day().as_uuid(),
                    "text": "Mai oder Juni?",
                    "owner": owner.as_uuid(),
                }),
                &[],
                "im Mai oder Juni 2030",
            )],
        )
    };
    // A stranger, and a member of the organization without an event role (ADR 0052).
    for owner in [UserId::from_uuid(Uuid::from_u128(99)), bruno()] {
        let result = create_changeset(&anna, question(owner), stores(&memory), &FixedClock).await;
        assert_eq!(
            invalid_fields(result),
            [("proposals/0/operation/owner".to_owned(), "unknown-member")]
        );
    }
    // A member with an event role, and an owner, who acts as event manager in each event.
    for owner in [anna.user_id(), carla()] {
        let result = create_changeset(&anna, question(owner), stores(&memory), &FixedClock).await;
        assert!(result.is_ok(), "{result:?}");
    }
}

#[tokio::test]
async fn a_choice_of_the_changeset_counts_and_a_shipped_field_does_not_change() {
    let memory = Memory::default();
    let anna = contributor(&memory);
    let field = Uuid::now_v7();
    let (define, choice) = (Uuid::now_v7(), Uuid::now_v7());
    let mut value_type = json!({"type": "choice", "values": [{"key": "grass", "label": "Gras"}]});
    let input = changeset(
        Some(open_day()),
        vec![
            proposal(define, new_field(field, value_type.take()), &[], "Open Day"),
            proposal(
                choice,
                json!({"kind": "add_choice_value", "event_id": open_day().as_uuid(), "field_id": field, "key": "asphalt", "label": "Asphalt"}),
                &[define],
                "Open Day",
            ),
            proposal(
                Uuid::now_v7(),
                visitors(field, json!({"type": "choice", "keys": ["asphalt"]})),
                &[define, choice],
                "Open Day",
            ),
            proposal(
                Uuid::now_v7(),
                json!({"kind": "deprecate_field", "event_id": open_day().as_uuid(), "field_id": core_field("audience")}),
                &[],
                "Open Day",
            ),
        ],
    );
    let result = create_changeset(&anna, input, stores(&memory), &FixedClock).await;
    assert_eq!(
        invalid_fields(result),
        [("proposals/3/operation/field_id".to_owned(), "shipped-field")]
    );
}

#[test]
fn the_json_schema_of_a_new_changeset_names_each_operation_kind() {
    let schema = serde_json::to_string(&schemars::schema_for!(NewChangeset)).unwrap();
    for kind in [
        "create_event",
        "set_fact",
        "add_field_definition",
        "add_choice_value",
        "deprecate_field",
        "create_open_question",
    ] {
        assert!(schema.contains(&format!("\"{kind}\"")), "{kind}");
    }
}

#[test]
fn each_error_gives_a_code_of_its_list() {
    for error in [
        ProposeError::NotFound,
        ProposeError::Forbidden,
        ProposeError::Invalid(Vec::new()),
        ProposeError::Store(StoreError::Internal("test".into())),
        ProposeError::Store(StoreError::Unavailable("test".into())),
    ] {
        assert!(ProposeError::CODES.contains(&error.code()), "{error:?}");
    }
}

/// `one_fact` with a changeset ID, as a client that retries sends it.
fn with_id(id: Uuid) -> NewChangeset {
    NewChangeset {
        id: Some(id),
        ..one_fact()
    }
}

#[tokio::test]
async fn a_retry_with_the_same_content_returns_the_stored_changeset() {
    let memory = Memory::default();
    let anna = contributor(&memory);
    let input = with_id(Uuid::now_v7());
    let Created::New(first) = create_changeset(&anna, input.clone(), stores(&memory), &FixedClock)
        .await
        .unwrap()
    else {
        panic!("not new");
    };
    // The retry comes through another request.
    let retry_caller = anna
        .clone()
        .with_request(crate::caller::Channel::Web, Some(Uuid::now_v7()));
    let retry = create_changeset(&retry_caller, input.clone(), stores(&memory), &FixedClock)
        .await
        .unwrap();
    let mut expected = first;
    expected.proposals.sort_by_key(|proposal| proposal.id);
    assert_eq!(retry, Created::Existing(expected));
    assert_eq!(
        memory.inserted.lock().unwrap().len(),
        1,
        "one insert, one audit event"
    );

    let mut changed = input;
    changed.proposals[0].reason = "Another reason.".to_owned();
    let result = create_changeset(&anna, changed, stores(&memory), &FixedClock).await;
    assert_eq!(invalid_fields(result), [("id".to_owned(), "taken")]);
}

#[tokio::test]
async fn a_request_that_loses_a_race_for_its_id_returns_the_winner_if_it_is_the_same() {
    let input = with_id(Uuid::now_v7());
    // The concurrent winner: the same intake, stored by the other request.
    let other = Memory::default();
    let winner = changeset_of(
        create_changeset(
            &contributor(&other),
            input.clone(),
            stores(&other),
            &FixedClock,
        )
        .await
        .unwrap(),
    );

    let memory = Memory::default();
    let anna = contributor(&memory);
    *memory.race.lock().unwrap() = Some(winner.clone());
    let result = create_changeset(&anna, input.clone(), stores(&memory), &FixedClock).await;
    assert_eq!(result.unwrap(), Created::Existing(winner.clone()));
    assert!(memory.inserted.lock().unwrap().is_empty());

    let memory = Memory::default();
    let anna = contributor(&memory);
    let mut different = winner;
    different.proposals[0].reason = Reason::parse("Another reason.").unwrap();
    *memory.race.lock().unwrap() = Some(different);
    let result = create_changeset(&anna, input, stores(&memory), &FixedClock).await;
    assert_eq!(invalid_fields(result), [("id".to_owned(), "taken")]);
}

#[tokio::test]
async fn a_dependency_named_twice_is_rejected() {
    let memory = Memory::default();
    let anna = contributor(&memory);
    let (a, b) = (Uuid::now_v7(), Uuid::now_v7());
    let input = changeset(
        Some(open_day()),
        vec![
            proposal(
                a,
                visitors(core_field("visitor_estimate"), quantity("20000")),
                &[],
                "20000",
            ),
            proposal(
                b,
                visitors(core_field("duration_days"), quantity("2")),
                &[a, a],
                "Open Day",
            ),
        ],
    );
    let result = create_changeset(&anna, input, stores(&memory), &FixedClock).await;
    assert_eq!(
        invalid_fields(result),
        [("proposals/1/depends_on".to_owned(), "duplicate")]
    );
    assert!(memory.inserted.lock().unwrap().is_empty());
}

#[tokio::test]
async fn a_dependency_outside_the_changeset_is_rejected() {
    let memory = Memory::default();
    let anna = contributor(&memory);
    let mut input = one_fact();
    input.proposals[0].depends_on = vec![Uuid::now_v7()];
    let result = create_changeset(&anna, input, stores(&memory), &FixedClock).await;
    assert_eq!(
        invalid_fields(result),
        [("proposals/0/depends_on".to_owned(), "outside-changeset")]
    );
}

#[tokio::test]
async fn an_organization_changeset_works_only_in_its_new_events() {
    let memory = Memory::default();
    let admin = caller(OrganizationRole::Admin);
    let (event, create, field, define) = (
        Uuid::now_v7(),
        Uuid::now_v7(),
        Uuid::now_v7(),
        Uuid::now_v7(),
    );
    let mut add_field = new_field(field, json!({"type": "boolean"}));
    add_field["event_id"] = json!(event);
    // The event of the fact is the ID of the new field, not of a new event.
    let mut set_fact = visitors(core_field("visitor_estimate"), quantity("20000"));
    set_fact["event_id"] = json!(field);
    let input = changeset(
        None,
        vec![
            proposal(create, new_event(event), &[], "Open Day"),
            proposal(define, add_field, &[create], "Besuchern"),
            proposal(Uuid::now_v7(), set_fact, &[define], "20000"),
        ],
    );
    let result = create_changeset(&admin, input, stores(&memory), &FixedClock).await;
    assert_eq!(
        invalid_fields(result),
        [("proposals/2/operation".to_owned(), "event-mismatch")]
    );
}

#[tokio::test]
async fn a_fact_of_a_new_event_expects_no_version() {
    let memory = Memory::default();
    let admin = caller(OrganizationRole::Admin);
    let (event, create) = (Uuid::now_v7(), Uuid::now_v7());
    let mut set_fact = visitors(core_field("visitor_estimate"), quantity("20000"));
    set_fact["event_id"] = json!(event);
    set_fact["expected_version"] = json!(1);
    let input = changeset(
        None,
        vec![
            proposal(create, new_event(event), &[], "Open Day"),
            proposal(Uuid::now_v7(), set_fact, &[create], "20000"),
        ],
    );
    let result = create_changeset(&admin, input, stores(&memory), &FixedClock).await;
    assert_eq!(
        invalid_fields(result),
        [(
            "proposals/1/operation/expected_version".to_owned(),
            "invalid"
        )]
    );
}

#[tokio::test]
async fn a_passage_of_a_member_text_has_no_page() {
    let memory = Memory::default();
    let anna = contributor(&memory);
    for page in [1, u32::MAX] {
        let mut input = one_fact();
        input.proposals[0].evidence[0].page = Some(page);
        let result = create_changeset(&anna, input, stores(&memory), &FixedClock).await;
        assert_eq!(
            invalid_fields(result),
            [("proposals/0/evidence/0".to_owned(), "page")]
        );
    }
}
