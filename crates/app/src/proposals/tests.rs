use std::sync::Mutex;

use serde_json::{Value, json};
use tada_domain::RecordVersion;
use tada_domain::facts::{FieldDefinition, core_catalog};
use tada_domain::identity::{EventRole, OrganizationRole};
use tada_domain::ids::{ApiTokenId, DocumentId, DocumentVersionId, FactId, OrganizationId, UserId};

use super::*;
use crate::access::SourceReach;
use crate::caller::AiCaller;
use crate::documents::{
    DocumentCursor, DocumentView, NewUpload, Published, StoredVersion, VersionContent, VersionView,
};
use crate::drafts::{CitedFact, LintKind, LintWarning};
use crate::facts::{EventProfile, FactVersionRef};
use crate::identity::{Membership, UserRef};
use crate::sources::{SourceHit, SourceVersionRef, SourceVersionText};
use crate::tokens::TokenScope;

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
    /// The fact versions of Testwil, with their events.
    fact_versions: Mutex<Vec<(EventId, FactId, RecordVersion)>>,
    /// The source versions of Testwil.
    sources: Mutex<Vec<(Option<EventId>, SourceVersionText)>>,
    /// The documents of Testwil, with their events.
    documents: Mutex<Vec<(DocumentId, EventId)>>,
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

    async fn event_roles_of(
        &self,
        scope: OrgScope,
        user: UserId,
    ) -> Result<Vec<(EventId, EventRole)>, StoreError> {
        let found = scope.organization_id() == testwil() && user == anna();
        Ok(found
            .then(|| *self.role.lock().unwrap())
            .flatten()
            .map(|role| (open_day(), role))
            .into_iter()
            .collect())
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

    async fn existing_versions(
        &self,
        scope: OrgScope,
        event: EventId,
        versions: &[(FactId, RecordVersion)],
    ) -> Result<Vec<(FactId, RecordVersion)>, StoreError> {
        if scope.organization_id() != testwil() {
            return Ok(Vec::new());
        }
        let stored = self.fact_versions.lock().unwrap();
        Ok(versions
            .iter()
            .filter(|(fact, number)| stored.contains(&(event, *fact, *number)))
            .copied()
            .collect())
    }
}

#[async_trait]
impl SourceStore for Memory {
    async fn add_member_text(
        &self,
        _: OrgScope,
        _: EventId,
        _: &SourceText,
        _: &Actor,
        _: Timestamp,
    ) -> Result<SourceVersionRef, StoreError> {
        unreachable!()
    }

    async fn search(
        &self,
        _: OrgScope,
        _: &SourceReach,
        _: &str,
        _: u32,
    ) -> Result<Vec<SourceHit>, StoreError> {
        unreachable!()
    }

    async fn readable_text(
        &self,
        _: OrgScope,
        _: &SourceReach,
        _: SourceVersionId,
    ) -> Result<Option<String>, StoreError> {
        unreachable!()
    }

    /// The evidence clause of the reach is tested with the store, so this memory knows no evidence.
    async fn texts(
        &self,
        scope: OrgScope,
        reach: &SourceReach,
        ids: &[SourceVersionId],
    ) -> Result<Vec<SourceVersionText>, StoreError> {
        if scope.organization_id() != testwil() {
            return Ok(Vec::new());
        }
        let sources = self.sources.lock().unwrap();
        Ok(sources
            .iter()
            .filter(|(event, source)| {
                ids.contains(&source.id)
                    && match reach {
                        SourceReach::Organization => true,
                        SourceReach::Events(events) => event.is_some_and(|e| events.contains(&e)),
                    }
            })
            .map(|(_, source)| source.clone())
            .collect())
    }
}

#[async_trait]
impl DocumentStore for Memory {
    async fn publish(
        &self,
        _: OrgScope,
        _: &NewUpload,
        _: &AuditEvent,
    ) -> Result<Published, StoreError> {
        unreachable!()
    }

    async fn list(
        &self,
        _: OrgScope,
        _: EventId,
        _: Option<&str>,
        _: Option<DocumentCursor>,
        _: u32,
    ) -> Result<Vec<DocumentView>, StoreError> {
        unreachable!()
    }

    async fn get(
        &self,
        scope: OrgScope,
        id: DocumentId,
    ) -> Result<Option<DocumentView>, StoreError> {
        if scope.organization_id() != testwil() {
            return Ok(None);
        }
        let documents = self.documents.lock().unwrap();
        Ok(documents
            .iter()
            .find(|(document, _)| *document == id)
            .map(|(document, event)| DocumentView {
                id: *document,
                event_id: *event,
                local_number: 1,
                name: "Konzept".to_owned(),
                owner: anna(),
                created_at: Timestamp::UNIX_EPOCH,
                version: RecordVersion::FIRST,
                newest_version: VersionView {
                    id: DocumentVersionId::from_uuid(Uuid::now_v7()),
                    document_id: *document,
                    number: 1,
                    sha256: [0; 32],
                    uploaded_by: anna(),
                    created_at: Timestamp::UNIX_EPOCH,
                    content: VersionContent::Draft {
                        status: crate::documents::DraftStatus::Draft,
                    },
                },
            }))
    }

    async fn versions(&self, _: OrgScope, _: DocumentId) -> Result<Vec<VersionView>, StoreError> {
        unreachable!()
    }

    async fn version(
        &self,
        _: OrgScope,
        _: DocumentVersionId,
    ) -> Result<Option<StoredVersion>, StoreError> {
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
        sources: memory,
        documents: memory,
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
        "kind": "set-fact",
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
    assert_eq!(
        proposal.evidence[0].passage.quote,
        "20000 Besuchern pro Tag"
    );
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

fn ai(member: MemberCaller, scope: TokenScope) -> AiCaller {
    AiCaller::new(member, ApiTokenId::from_uuid(Uuid::now_v7()), scope)
}

#[tokio::test]
async fn a_read_token_cannot_create_a_changeset() {
    let memory = Memory::default();
    let client = ai(contributor(&memory), TokenScope::Read);
    let result = create_changeset(&client, one_fact(), stores(&memory), &FixedClock).await;
    assert!(matches!(result, Err(ProposeError::Forbidden)), "{result:?}");
    assert!(memory.inserted.lock().unwrap().is_empty());
}

#[tokio::test]
async fn a_propose_token_creates_a_changeset_as_ai_for_its_member() {
    let memory = Memory::default();
    let client = ai(contributor(&memory), TokenScope::Propose);
    let changeset = create_changeset(&client, one_fact(), stores(&memory), &FixedClock)
        .await
        .unwrap();
    let changeset = changeset_of(changeset);
    assert_eq!(changeset.author, client.actor());
    assert_eq!(changeset.author.kind(), crate::caller::ActorKind::Ai);
    assert_eq!(changeset.author.principal(), Some(anna().as_uuid()));
    let inserted = memory.inserted.lock().unwrap();
    assert_eq!(inserted[0].2.actor(), &client.actor());
}

#[tokio::test]
async fn a_propose_token_of_a_viewer_cannot_create_a_changeset() {
    let memory = Memory::default();
    *memory.role.lock().unwrap() = Some(EventRole::EventViewer);
    let client = ai(caller(OrganizationRole::Member), TokenScope::Propose);
    let result = create_changeset(&client, one_fact(), stores(&memory), &FixedClock).await;
    assert!(matches!(result, Err(ProposeError::Forbidden)), "{result:?}");
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
        "kind": "add-field-definition",
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
    json!({"kind": "create-event", "id": id, "key": "OPEN30", "name": "Open Day Testwil"})
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
                    "kind": "create-open-question",
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
                json!({"kind": "add-choice-value", "event_id": open_day().as_uuid(), "field_id": field, "key": "asphalt", "label": "Asphalt"}),
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
                json!({"kind": "deprecate-field", "event_id": open_day().as_uuid(), "field_id": core_field("audience")}),
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
        "create-event",
        "set-fact",
        "add-field-definition",
        "add-choice-value",
        "deprecate-field",
        "create-open-question",
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

// Document drafts (ADR 0051).

/// The member text of the other event: Anna has no role there.
fn other_event() -> EventId {
    EventId::from_uuid(Uuid::from_u128(21))
}

const FACT: u128 = 0x0190_f3a2_7b1c_7d4e_8f00_0000_0000_0001;
const LEAFLET: &str = "Flyer: Das Flugfeld öffnet um 9 Uhr.";

/// A memory with fact version 1 of the open day and three source versions:
/// one of the open day, one of the other event and one of the organization.
fn memory_with_targets() -> (Memory, [SourceVersionId; 3]) {
    let memory = Memory::default();
    memory.fact_versions.lock().unwrap().push((
        open_day(),
        FactId::from_uuid(Uuid::from_u128(FACT)),
        RecordVersion::FIRST,
    ));
    let ids = [
        SourceVersionId::from_uuid(Uuid::now_v7()),
        SourceVersionId::from_uuid(Uuid::now_v7()),
        SourceVersionId::from_uuid(Uuid::now_v7()),
    ];
    let events = [Some(open_day()), Some(other_event()), None];
    for (id, event) in ids.into_iter().zip(events) {
        memory.sources.lock().unwrap().push((
            event,
            SourceVersionText {
                id,
                text: Some(SourceText::normalize(LEAFLET)),
            },
        ));
    }
    (memory, ids)
}

fn fact_link(version: i64) -> String {
    format!("[](tada:fact/{}?v={version})", Uuid::from_u128(FACT))
}

fn source_link(source: SourceVersionId, start: u32, end: u32) -> String {
    format!("[das Flugfeld](tada:source/{source}#{start}-{end})")
}

fn draft(document: Value, markdown: &str) -> NewChangeset {
    changeset(
        Some(open_day()),
        vec![proposal(
            Uuid::now_v7(),
            json!({
                "kind": "create-document-draft",
                "event_id": open_day().as_uuid(),
                "document": document,
                "markdown": markdown,
            }),
            &[],
            "Open Day",
        )],
    )
}

fn new_document() -> Value {
    json!({"new": {"id": Uuid::now_v7(), "name": "Konzept Open Day"}})
}

#[tokio::test]
async fn a_draft_proposal_fixes_its_manifest_and_its_lint_warnings() {
    let (memory, [source, ..]) = memory_with_targets();
    let anna = contributor(&memory);
    let markdown = format!(
        "# Konzept\n\nDas Fest ist am {}.\n{} öffnet früh.\nWir erwarten 20000 Gäste.\n",
        fact_link(1),
        source_link(source, 7, 19),
    );
    let changeset = changeset_of(
        create_changeset(
            &anna,
            draft(new_document(), &markdown),
            stores(&memory),
            &FixedClock,
        )
        .await
        .unwrap(),
    );

    let [draft] = changeset.drafts.as_slice() else {
        panic!("not one draft: {:?}", changeset.drafts);
    };
    assert_eq!(draft.proposal_id, changeset.proposals[0].id);
    assert_eq!(
        draft.manifest.facts,
        [CitedFact {
            fact_id: FactId::from_uuid(Uuid::from_u128(FACT)),
            version: RecordVersion::FIRST,
        }]
    );
    let [cited] = draft.manifest.sources.as_slice() else {
        panic!("not one source");
    };
    assert_eq!(cited.source_version_id, source);
    assert_eq!(cited.passage.quote, "Das Flugfeld");
    assert_eq!(
        draft.lint_warnings,
        [LintWarning {
            line: 5,
            kind: LintKind::Number
        }]
    );
    assert_eq!(
        memory.inserted.lock().unwrap()[0].0.drafts,
        changeset.drafts
    );
}

async fn draft_errors(
    memory: &Memory,
    caller: &MemberCaller,
    input: NewChangeset,
) -> Vec<(String, &'static str)> {
    let result = create_changeset(caller, input, stores(memory), &FixedClock).await;
    let errors = invalid_fields(result);
    assert!(memory.inserted.lock().unwrap().is_empty());
    errors
}

fn markdown_error(code: &'static str) -> Vec<(String, &'static str)> {
    vec![("proposals/0/operation/markdown".to_owned(), code)]
}

#[tokio::test]
async fn a_draft_link_to_a_missing_fact_version_is_rejected() {
    let (memory, _) = memory_with_targets();
    let anna = contributor(&memory);
    // Version 2 does not exist yet: an open proposal can propose it, but a draft cannot cite it.
    let input = draft(new_document(), &format!("Am {}.\n", fact_link(2)));
    assert_eq!(
        draft_errors(&memory, &anna, input).await,
        markdown_error("link-not-found")
    );
}

#[tokio::test]
async fn a_draft_cannot_cite_a_source_that_its_author_cannot_see() {
    let (memory, [_, other, organization]) = memory_with_targets();
    let anna = contributor(&memory);
    for source in [other, organization] {
        let input = draft(new_document(), &format!("{}\n", source_link(source, 7, 19)));
        assert_eq!(
            draft_errors(&memory, &anna, input).await,
            markdown_error("link-not-found")
        );
    }
    // An owner reaches each source of the organization (ADR 0052).
    let owner = caller(OrganizationRole::Owner);
    let input = draft(
        new_document(),
        &format!("{}\n", source_link(organization, 7, 19)),
    );
    create_changeset(&owner, input, stores(&memory), &FixedClock)
        .await
        .unwrap();
}

#[tokio::test]
async fn a_draft_cannot_cite_a_source_without_text() {
    let (memory, _) = memory_with_targets();
    let pdf = SourceVersionId::from_uuid(Uuid::now_v7());
    memory.sources.lock().unwrap().push((
        Some(open_day()),
        SourceVersionText {
            id: pdf,
            text: None,
        },
    ));
    let anna = contributor(&memory);
    let input = draft(new_document(), &format!("{}\n", source_link(pdf, 7, 19)));
    assert_eq!(
        draft_errors(&memory, &anna, input).await,
        markdown_error("no-text")
    );
}

#[tokio::test]
async fn a_draft_link_outside_the_source_text_is_rejected() {
    let (memory, [source, ..]) = memory_with_targets();
    let anna = contributor(&memory);
    let input = draft(
        new_document(),
        &format!("{}\n", source_link(source, 7, 999)),
    );
    assert_eq!(
        draft_errors(&memory, &anna, input).await,
        markdown_error("out-of-range")
    );
}

#[tokio::test]
async fn a_draft_that_breaks_a_link_rule_is_rejected() {
    let (memory, _) = memory_with_targets();
    let anna = contributor(&memory);
    let input = draft(new_document(), "![Logo](https://example.org/logo.png)\n");
    assert_eq!(
        draft_errors(&memory, &anna, input).await,
        markdown_error("image-not-allowed")
    );
}

#[tokio::test]
async fn a_draft_for_a_document_of_another_event_is_rejected() {
    let (memory, _) = memory_with_targets();
    let anna = contributor(&memory);
    let elsewhere = DocumentId::from_uuid(Uuid::now_v7());
    memory
        .documents
        .lock()
        .unwrap()
        .push((elsewhere, other_event()));
    let existing = json!({"existing": {"document_id": elsewhere.as_uuid(), "expected_version": 1}});
    assert_eq!(
        draft_errors(&memory, &anna, draft(existing, "Text.\n")).await,
        [(
            "proposals/0/operation/document/existing/document_id".to_owned(),
            "unknown-document"
        )]
    );

    let own = DocumentId::from_uuid(Uuid::now_v7());
    memory.documents.lock().unwrap().push((own, open_day()));
    let existing = json!({"existing": {"document_id": own.as_uuid(), "expected_version": 1}});
    create_changeset(
        &anna,
        draft(existing, "Text.\n"),
        stores(&memory),
        &FixedClock,
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn a_draft_needs_a_name_a_version_and_text() {
    let (memory, _) = memory_with_targets();
    let anna = contributor(&memory);
    let unnamed = json!({"new": {"id": Uuid::now_v7(), "name": " "}});
    assert_eq!(
        draft_errors(&memory, &anna, draft(unnamed, " \n")).await,
        [
            (
                "proposals/0/operation/document/new/name".to_owned(),
                "empty"
            ),
            ("proposals/0/operation/markdown".to_owned(), "empty"),
        ]
    );
    let unversioned = json!({"existing": {"document_id": Uuid::now_v7(), "expected_version": 0}});
    assert_eq!(
        draft_errors(&memory, &anna, draft(unversioned, "Text.\n")).await,
        [(
            "proposals/0/operation/document/existing/expected_version".to_owned(),
            "invalid"
        )]
    );
}

/// A fact of the open day whose evidence is the passage "Das Flugfeld" of the source version `source`.
fn fact_citing(source: SourceVersionId, quote: &str) -> NewChangeset {
    let mut input = one_fact();
    input.proposals[0].evidence = vec![
        serde_json::from_value(json!({
            "source_version_id": source.as_uuid(),
            "start": 7,
            "end": 19,
            "quote": quote,
        }))
        .unwrap(),
    ];
    input
}

#[tokio::test]
async fn a_fact_cites_a_passage_of_a_text_file_of_its_event() {
    let (memory, [source, ..]) = memory_with_targets();
    let anna = contributor(&memory);
    let created = create_changeset(
        &anna,
        fact_citing(source, "Das Flugfeld"),
        stores(&memory),
        &FixedClock,
    )
    .await;
    let changeset = changeset_of(created.unwrap());
    let evidence = &changeset.proposals[0].evidence[0];
    assert_eq!(evidence.source_version_id, source);
    assert_eq!(evidence.passage.quote, "Das Flugfeld");
}

#[tokio::test]
async fn a_fact_cannot_cite_a_source_of_another_event_or_without_text() {
    let (memory, [_, elsewhere, organization]) = memory_with_targets();
    let pdf = SourceVersionId::from_uuid(Uuid::now_v7());
    memory.sources.lock().unwrap().push((
        Some(open_day()),
        SourceVersionText {
            id: pdf,
            text: None,
        },
    ));
    let anna = contributor(&memory);
    let unknown = SourceVersionId::from_uuid(Uuid::now_v7());
    for (source, code) in [
        (elsewhere, "unknown-source"),
        (organization, "unknown-source"),
        (unknown, "unknown-source"),
        (pdf, "no-text"),
    ] {
        let result = create_changeset(
            &anna,
            fact_citing(source, "Das Flugfeld"),
            stores(&memory),
            &FixedClock,
        )
        .await;
        assert_eq!(
            invalid_fields(result),
            [("proposals/0/evidence/0".to_owned(), code)]
        );
    }
    assert!(memory.inserted.lock().unwrap().is_empty());
}

#[tokio::test]
async fn a_cited_passage_must_match_its_source_version() {
    let (memory, [source, ..]) = memory_with_targets();
    let anna = contributor(&memory);
    let result = create_changeset(
        &anna,
        fact_citing(source, "Das Festzelt"),
        stores(&memory),
        &FixedClock,
    )
    .await;
    assert_eq!(
        invalid_fields(result),
        [("proposals/0/evidence/0".to_owned(), "quote-mismatch")]
    );
}
