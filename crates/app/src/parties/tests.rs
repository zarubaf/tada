use std::collections::HashMap;
use std::sync::Mutex;

use tada_domain::identity::{EventRole, OrganizationRole};
use tada_domain::ids::{EventId, OrganizationId};

use super::*;
use crate::access::SourceReach;
use crate::identity::{Membership, UserRef};
use crate::records::{EvidenceStore, RecordEvidenceView, RecordRef};

fn testwil() -> OrganizationId {
    OrganizationId::from_uuid(Uuid::from_u128(10))
}

fn open_day() -> EventId {
    EventId::from_uuid(Uuid::from_u128(20))
}

fn user(n: u128) -> UserId {
    UserId::from_uuid(Uuid::from_u128(n))
}

fn caller(user_id: UserId, role: OrganizationRole) -> MemberCaller {
    MemberCaller::new(user_id, testwil(), role)
}

/// Anna: a member who contributes to the event.
fn anna() -> MemberCaller {
    caller(user(1), OrganizationRole::Member)
}

/// Bruno: a member who views the event.
fn bruno() -> MemberCaller {
    caller(user(2), OrganizationRole::Member)
}

/// Carla: an admin.
fn carla() -> MemberCaller {
    caller(user(3), OrganizationRole::Admin)
}

/// Dino: a member without an event role.
fn dino() -> MemberCaller {
    caller(user(4), OrganizationRole::Member)
}

#[derive(Debug, Default)]
struct Memory {
    persons: Mutex<Vec<PersonView>>,
    institutions: Mutex<Vec<InstitutionView>>,
    audit: Mutex<Vec<AuditEvent>>,
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
        user_id: UserId,
    ) -> Result<Option<OrganizationRole>, StoreError> {
        assert_eq!(scope.organization_id(), testwil());
        Ok((1..=4)
            .contains(&(user_id.as_uuid().as_u128()))
            .then_some(OrganizationRole::Member))
    }

    async fn event_exists(&self, _: OrgScope, _: EventId) -> Result<bool, StoreError> {
        unreachable!()
    }

    async fn event_role(
        &self,
        _: OrgScope,
        _: EventId,
        _: UserId,
    ) -> Result<Option<EventRole>, StoreError> {
        unreachable!()
    }

    async fn event_roles_of(
        &self,
        _: OrgScope,
        user_id: UserId,
    ) -> Result<Vec<(EventId, EventRole)>, StoreError> {
        let roles: HashMap<UserId, EventRole> = HashMap::from([
            (user(1), EventRole::EventContributor),
            (user(2), EventRole::EventViewer),
        ]);
        Ok(roles
            .get(&user_id)
            .map(|role| (open_day(), *role))
            .into_iter()
            .collect())
    }
}

fn next_number<T>(items: &[T]) -> u64 {
    items.len() as u64 + 1
}

#[async_trait]
impl EvidenceStore for Memory {
    async fn evidence_of(
        &self,
        _: OrgScope,
        _: &[RecordRef],
        _: &SourceReach,
    ) -> Result<Vec<(RecordRef, RecordEvidenceView)>, StoreError> {
        Ok(Vec::new())
    }
}

#[async_trait]
impl PartyStore for Memory {
    async fn create_person(
        &self,
        _: OrgScope,
        id: PersonId,
        fields: &PersonFields,
        user_id: Option<UserId>,
        _: Timestamp,
        audit: &AuditEvent,
    ) -> Result<Created<PersonView>, StoreError> {
        let mut persons = self.persons.lock().unwrap();
        if persons.iter().any(|known| known.id == id) {
            return Ok(Created::IdTaken);
        }
        let person = PersonView {
            id,
            local_number: next_number(&persons),
            name: fields.name.clone(),
            email: fields.email.clone(),
            phone: fields.phone.clone(),
            user_id,
            version: RecordVersion::FIRST,
        };
        persons.push(person.clone());
        self.audit.lock().unwrap().push(audit.clone());
        Ok(Created::Created(person))
    }

    async fn change_person(
        &self,
        _: OrgScope,
        id: PersonId,
        fields: &PersonFields,
        expected: RecordVersion,
        _: Timestamp,
        audit: &AuditEvent,
    ) -> Result<Changed<PersonView>, StoreError> {
        let mut persons = self.persons.lock().unwrap();
        let Some(person) = persons.iter_mut().find(|person| person.id == id) else {
            return Ok(Changed::NotFound);
        };
        if person.version != expected {
            return Ok(Changed::VersionConflict);
        }
        person.name = fields.name.clone();
        person.email = fields.email.clone();
        person.phone = fields.phone.clone();
        person.version = RecordVersion::new(person.version.get() + 1).unwrap();
        self.audit.lock().unwrap().push(audit.clone());
        Ok(Changed::Changed(person.clone()))
    }

    async fn person(&self, _: OrgScope, id: PersonId) -> Result<Option<PersonView>, StoreError> {
        let persons = self.persons.lock().unwrap();
        Ok(persons.iter().find(|person| person.id == id).cloned())
    }

    async fn persons(
        &self,
        _: OrgScope,
        query: Option<&str>,
        after: Option<NumberCursor>,
        limit: u32,
    ) -> Result<Vec<PersonView>, StoreError> {
        let persons = self.persons.lock().unwrap();
        Ok(persons
            .iter()
            .filter(|person| after.is_none_or(|after| person.local_number > after.0))
            .filter(|person| {
                query.is_none_or(|query| normalized_name(person.name.as_str()).contains(query))
            })
            .take(limit as usize)
            .cloned()
            .collect())
    }

    async fn create_institution(
        &self,
        _: OrgScope,
        id: InstitutionId,
        fields: &InstitutionFields,
        _: Timestamp,
        audit: &AuditEvent,
    ) -> Result<Created<InstitutionView>, StoreError> {
        let mut institutions = self.institutions.lock().unwrap();
        if institutions.iter().any(|known| known.id == id) {
            return Ok(Created::IdTaken);
        }
        let institution = InstitutionView {
            id,
            local_number: next_number(&institutions),
            name: fields.name.clone(),
            kind: fields.kind,
            email: fields.email.clone(),
            phone: fields.phone.clone(),
            version: RecordVersion::FIRST,
        };
        institutions.push(institution.clone());
        self.audit.lock().unwrap().push(audit.clone());
        Ok(Created::Created(institution))
    }

    async fn change_institution(
        &self,
        _: OrgScope,
        id: InstitutionId,
        fields: &InstitutionFields,
        expected: RecordVersion,
        _: Timestamp,
        audit: &AuditEvent,
    ) -> Result<Changed<InstitutionView>, StoreError> {
        let mut institutions = self.institutions.lock().unwrap();
        let Some(found) = institutions.iter_mut().find(|found| found.id == id) else {
            return Ok(Changed::NotFound);
        };
        if found.version != expected {
            return Ok(Changed::VersionConflict);
        }
        found.name = fields.name.clone();
        found.kind = fields.kind;
        found.email = fields.email.clone();
        found.phone = fields.phone.clone();
        found.version = RecordVersion::new(found.version.get() + 1).unwrap();
        self.audit.lock().unwrap().push(audit.clone());
        Ok(Changed::Changed(found.clone()))
    }

    async fn institution(
        &self,
        _: OrgScope,
        id: InstitutionId,
    ) -> Result<Option<InstitutionView>, StoreError> {
        let institutions = self.institutions.lock().unwrap();
        Ok(institutions.iter().find(|found| found.id == id).cloned())
    }

    async fn institutions(
        &self,
        _: OrgScope,
        _: Option<&str>,
        after: Option<NumberCursor>,
        limit: u32,
    ) -> Result<Vec<InstitutionView>, StoreError> {
        let institutions = self.institutions.lock().unwrap();
        Ok(institutions
            .iter()
            .filter(|found| after.is_none_or(|after| found.local_number > after.0))
            .take(limit as usize)
            .cloned()
            .collect())
    }

    async fn named_like(&self, _: OrgScope, _: &str) -> Result<Vec<PartyRef>, StoreError> {
        unreachable!("the store matches names; `names_match` has its own tests")
    }
}

#[derive(Debug)]
struct FixedClock;

impl Clock for FixedClock {
    fn now(&self) -> Timestamp {
        "2030-05-18T08:00:00Z".parse().unwrap()
    }
}

fn new_person(name: &str) -> NewPerson {
    NewPerson {
        id: None,
        name: name.to_owned(),
        email: None,
        phone: None,
        user_id: None,
    }
}

fn rename(name: &str, expected: RecordVersion) -> PersonChange {
    PersonChange {
        name: Some(name.to_owned()),
        email: None,
        phone: None,
        expected_version: expected,
    }
}

async fn person(memory: &Memory, name: &str) -> PersonView {
    create_person(&anna(), new_person(name), memory, memory, &FixedClock)
        .await
        .unwrap()
        .record
}

#[tokio::test]
async fn a_contributor_creates_a_person() {
    let memory = Memory::default();
    let input = NewPerson {
        email: Some(" Beat.Muster@Example.org ".to_owned()),
        phone: Some("+41 00 000 00 00".to_owned()),
        user_id: Some(user(2)),
        ..new_person(" Beat Muster ")
    };
    let created = create_person(&anna(), input, &memory, &memory, &FixedClock)
        .await
        .unwrap()
        .record;
    assert_eq!(created.name.as_str(), "Beat Muster");
    assert_eq!(created.email.unwrap().as_str(), "beat.muster@example.org");
    assert_eq!(created.user_id, Some(user(2)));
    assert_eq!(created.version, RecordVersion::FIRST);
    let audit = memory.audit.lock().unwrap();
    assert_eq!(audit.len(), 1);
    assert_eq!(audit[0].action(), AuditAction::PersonCreate);
}

/// A client chooses the ID of a new person or institution as of a work record (ADR 0038): a UUIDv7 that is free.
#[tokio::test]
async fn a_client_chooses_the_id_of_a_new_party() {
    let memory = Memory::default();
    let id = Uuid::now_v7();
    let input = NewPerson {
        id: Some(id),
        ..new_person("Beat Muster")
    };
    let created = create_person(&anna(), input.clone(), &memory, &memory, &FixedClock)
        .await
        .unwrap()
        .record;
    assert_eq!(created.id.as_uuid(), id);
    let again = create_person(&anna(), input, &memory, &memory, &FixedClock)
        .await
        .unwrap_err();
    assert_eq!(again.field_errors(), [FieldError::new("id", "taken")]);

    let input = NewInstitution {
        id: Some(Uuid::from_u128(7)),
        name: "Testwil Generatoren AG".to_owned(),
        kind: "company".to_owned(),
        email: None,
        phone: None,
    };
    let error = create_institution(&anna(), input, &memory, &memory, &FixedClock)
        .await
        .unwrap_err();
    assert_eq!(error.field_errors(), [FieldError::new("id", "not-uuid-v7")]);
    assert_eq!(memory.persons.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn a_viewer_cannot_create_a_person() {
    let memory = Memory::default();
    for who in [bruno(), dino()] {
        let error = create_person(
            &who,
            new_person("Beat Muster"),
            &memory,
            &memory,
            &FixedClock,
        )
        .await
        .unwrap_err();
        assert_eq!(error.code(), ProblemCode::Forbidden);
    }
    assert!(memory.persons.lock().unwrap().is_empty());
}

#[tokio::test]
async fn the_first_person_is_per_001() {
    let memory = Memory::default();
    assert_eq!(person(&memory, "Beat Muster").await.local_id(), "PER-001");
    assert_eq!(person(&memory, "Anna Beispiel").await.local_id(), "PER-002");
    let institution = create_institution(
        &carla(),
        NewInstitution {
            id: None,
            name: "Testwil Generatoren AG".to_owned(),
            kind: "company".to_owned(),
            email: None,
            phone: None,
        },
        &memory,
        &memory,
        &FixedClock,
    )
    .await
    .unwrap()
    .record;
    assert_eq!(institution.local_id(), "INS-001");
}

#[tokio::test]
async fn only_an_owner_or_admin_changes_a_person() {
    let memory = Memory::default();
    let created = person(&memory, "Beat Muster").await;
    let renamed = rename("Beat Beispiel", created.version);

    let error = change_person(
        &anna(),
        created.id,
        renamed.clone(),
        &memory,
        &memory,
        &FixedClock,
    )
    .await
    .unwrap_err();
    assert_eq!(error.code(), ProblemCode::Forbidden);

    let changed = change_person(&carla(), created.id, renamed, &memory, &memory, &FixedClock)
        .await
        .unwrap()
        .record;
    assert_eq!(changed.name.as_str(), "Beat Beispiel");
    assert_eq!(changed.version.get(), 2);
    assert_eq!(
        memory.audit.lock().unwrap()[1].action(),
        AuditAction::PersonChange
    );
}

#[tokio::test]
async fn a_change_keeps_what_it_does_not_name_and_clears_what_it_empties() {
    let memory = Memory::default();
    let input = NewPerson {
        email: Some("beat@example.org".to_owned()),
        phone: Some("+41 00 000 00 00".to_owned()),
        ..new_person("Beat Muster")
    };
    let created = create_person(&anna(), input, &memory, &memory, &FixedClock)
        .await
        .unwrap()
        .record;
    let change = PersonChange {
        name: None,
        email: Some(None),
        phone: None,
        expected_version: created.version,
    };
    let changed = change_person(&carla(), created.id, change, &memory, &memory, &FixedClock)
        .await
        .unwrap()
        .record;
    assert_eq!(changed.name.as_str(), "Beat Muster");
    assert_eq!(changed.email, None);
    assert_eq!(changed.phone.unwrap().as_str(), "+41 00 000 00 00");
}

/// A change without a field is `validation-failed` without field errors, as for work records; it writes nothing.
#[tokio::test]
async fn a_change_without_a_field_is_invalid() {
    let memory = Memory::default();
    let created = person(&memory, "Beat Muster").await;
    let empty = PersonChange {
        name: None,
        email: None,
        phone: None,
        expected_version: created.version,
    };
    let error = change_person(&carla(), created.id, empty, &memory, &memory, &FixedClock)
        .await
        .unwrap_err();
    assert!(
        matches!(&error, PartyError::Invalid(errors) if errors.is_empty()),
        "{error:?}"
    );

    let institution = create_institution(
        &anna(),
        NewInstitution {
            id: None,
            name: "Testwil Generatoren AG".to_owned(),
            kind: "company".to_owned(),
            email: None,
            phone: None,
        },
        &memory,
        &memory,
        &FixedClock,
    )
    .await
    .unwrap()
    .record;
    let empty = InstitutionChange {
        name: None,
        kind: None,
        email: None,
        phone: None,
        expected_version: institution.version,
    };
    let error = change_institution(
        &carla(),
        institution.id,
        empty,
        &memory,
        &memory,
        &FixedClock,
    )
    .await
    .unwrap_err();
    assert!(
        matches!(&error, PartyError::Invalid(errors) if errors.is_empty()),
        "{error:?}"
    );
    assert_eq!(memory.audit.lock().unwrap().len(), 2);
}

#[tokio::test]
async fn a_stale_version_conflicts() {
    let memory = Memory::default();
    let created = person(&memory, "Beat Muster").await;
    change_person(
        &carla(),
        created.id,
        rename("Beat Beispiel", created.version),
        &memory,
        &memory,
        &FixedClock,
    )
    .await
    .unwrap();
    let error = change_person(
        &carla(),
        created.id,
        rename("Beat Zweiter", created.version),
        &memory,
        &memory,
        &FixedClock,
    )
    .await
    .unwrap_err();
    assert_eq!(error.code(), ProblemCode::RecordVersionConflict);
}

#[tokio::test]
async fn invalid_values_name_their_fields() {
    let memory = Memory::default();
    let input = NewPerson {
        email: Some("no address".to_owned()),
        phone: Some("1".repeat(51)),
        user_id: Some(user(99)),
        ..new_person("  ")
    };
    let error = create_person(&anna(), input, &memory, &memory, &FixedClock)
        .await
        .unwrap_err();
    assert_eq!(error.code(), ProblemCode::ValidationFailed);
    let fields: Vec<_> = error
        .field_errors()
        .iter()
        .map(|error| (error.field.as_ref(), error.code))
        .collect();
    assert_eq!(
        fields,
        [
            ("name", "empty"),
            ("email", "whitespace"),
            ("phone", "too-long"),
            ("user_id", "unknown-member"),
        ]
    );
}

#[tokio::test]
async fn an_institution_of_an_unknown_kind_is_refused() {
    let memory = Memory::default();
    let input = NewInstitution {
        id: None,
        name: "Testwil Generatoren AG".to_owned(),
        kind: "bank".to_owned(),
        email: None,
        phone: None,
    };
    let error = create_institution(&anna(), input, &memory, &memory, &FixedClock)
        .await
        .unwrap_err();
    assert_eq!(error.field_errors(), [FieldError::new("kind", "unknown")]);
}

#[tokio::test]
async fn each_member_with_an_event_role_reads_but_a_member_without_one_does_not() {
    let memory = Memory::default();
    let created = person(&memory, "Beat Muster").await;
    for who in [anna(), bruno(), carla()] {
        let found = get_person(&who, created.id, &memory, &memory)
            .await
            .unwrap()
            .record;
        assert_eq!(found, created);
    }
    let error = get_person(&dino(), created.id, &memory, &memory)
        .await
        .unwrap_err();
    assert_eq!(error.code(), ProblemCode::Forbidden);
    let missing = PersonId::from_uuid(Uuid::now_v7());
    let error = get_person(&anna(), missing, &memory, &memory)
        .await
        .unwrap_err();
    assert_eq!(error.code(), ProblemCode::NotFound);
}

#[tokio::test]
async fn the_list_filters_by_name_and_pages_by_number() {
    let memory = Memory::default();
    for name in ["Beat Müller", "Anna Beispiel", "Clara MÜLLER"] {
        person(&memory, name).await;
    }
    let all = list_persons(
        &anna(),
        None,
        None,
        PageLimit::new(2).unwrap(),
        &memory,
        &memory,
    )
    .await
    .unwrap();
    assert_eq!(all.items.len(), 2);
    assert_eq!(all.next, Some(NumberCursor(2)));
    let rest = list_persons(
        &anna(),
        None,
        all.next,
        PageLimit::DEFAULT,
        &memory,
        &memory,
    )
    .await
    .unwrap();
    assert_eq!(rest.items.len(), 1);
    assert_eq!(rest.next, None);

    let found = list_persons(
        &anna(),
        Some(" muller "),
        None,
        PageLimit::DEFAULT,
        &memory,
        &memory,
    )
    .await
    .unwrap();
    let names: Vec<_> = found.items.iter().map(|p| p.record.name.as_str()).collect();
    assert_eq!(names, ["Beat Müller", "Clara MÜLLER"]);
}

#[test]
fn names_match_on_equal_contained_and_shared_words() {
    assert!(names_match("muller ag", "muller ag"));
    assert!(names_match("muller ag", "muller"));
    assert!(names_match("muller", "muller ag"));
    assert!(names_match("generatoren ag", "testwil generatoren"));
    // A shared word of three characters is too short.
    assert!(!names_match("hans ott", "eva ott"));
    assert!(!names_match("beat muster", "anna beispiel"));
    assert!(!names_match("", "anna"));
    assert!(!names_match("anna", ""));
}
