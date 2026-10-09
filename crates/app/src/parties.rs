//! Persons and institutions: the commands and queries, and the port (ADR 0069).

use std::fmt::Debug;

use async_trait::async_trait;
use jiff::Timestamp;
use tada_domain::RecordVersion;
use tada_domain::facts::TextError;
use tada_domain::identity::{Email, EmailError};
use tada_domain::ids::{InstitutionId, LocalIdKind, PersonId, UserId};
use tada_domain::parties::{InstitutionKind, Party, PartyName, PhoneNumber, normalized_name};
use uuid::Uuid;

use crate::access::{self, Principal};
use crate::audit::{AuditAction, AuditEvent};
use crate::caller::{MemberCaller, OrgScope};
use crate::clock::Clock;
use crate::identity::IdentityStore;
use crate::paging::{Page, PageLimit};
use crate::problem::{CommandError, FieldError, ProblemCode};
use crate::store::StoreError;

/// A person of the organization, as the commands and queries show it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PersonView {
    pub id: PersonId,
    /// The number in `PER-001`, local to the organization (ADR 0038).
    pub local_number: u64,
    pub name: PartyName,
    pub email: Option<Email>,
    pub phone: Option<PhoneNumber>,
    /// The account of the person, if the person is a member.
    pub user_id: Option<UserId>,
    pub version: RecordVersion,
}

impl PersonView {
    /// The readable ID, for example `PER-001`.
    pub fn local_id(&self) -> String {
        LocalIdKind::Person.readable_id(self.local_number)
    }
}

/// An institution of the organization, as the commands and queries show it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstitutionView {
    pub id: InstitutionId,
    /// The number in `INS-001`, local to the organization (ADR 0038).
    pub local_number: u64,
    pub name: PartyName,
    pub kind: InstitutionKind,
    pub email: Option<Email>,
    pub phone: Option<PhoneNumber>,
    pub version: RecordVersion,
}

impl InstitutionView {
    /// The readable ID, for example `INS-001`.
    pub fn local_id(&self) -> String {
        LocalIdKind::Institution.readable_id(self.local_number)
    }
}

/// A person or an institution that a name can match.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartyRef {
    pub party: Party,
    pub local_id: String,
    pub name: PartyName,
}

/// The values of a person that a change replaces. The user account never changes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PersonFields {
    pub name: PartyName,
    pub email: Option<Email>,
    pub phone: Option<PhoneNumber>,
}

/// The values of an institution that a change replaces.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstitutionFields {
    pub name: PartyName,
    pub kind: InstitutionKind,
    pub email: Option<Email>,
    pub phone: Option<PhoneNumber>,
}

/// The result of a change in the store.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PartyChanged<T> {
    Changed(T),
    /// The organization has no such record.
    NotFound,
    /// The record has another version.
    VersionConflict,
}

/// The position after the last record of a page: its local number (ADR 0044).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PartyCursor(pub u64);

/// The repository port for persons and institutions. Each method stays inside `scope`.
/// A create or a change records its audit event in the same transaction.
#[async_trait]
pub trait PartyStore: Debug + Send + Sync {
    /// Inserts a person and gives it the next number of the organization (ADR 0038).
    async fn create_person(
        &self,
        scope: OrgScope,
        id: PersonId,
        fields: &PersonFields,
        user_id: Option<UserId>,
        at: Timestamp,
        audit: &AuditEvent,
    ) -> Result<PersonView, StoreError>;

    /// Replaces the values of a person if its version is `expected`, and counts the version up.
    async fn change_person(
        &self,
        scope: OrgScope,
        id: PersonId,
        fields: &PersonFields,
        expected: RecordVersion,
        at: Timestamp,
        audit: &AuditEvent,
    ) -> Result<PartyChanged<PersonView>, StoreError>;

    async fn person(&self, scope: OrgScope, id: PersonId)
    -> Result<Option<PersonView>, StoreError>;

    /// At most `limit` persons in the order of their numbers, after `after`.
    /// `query` is a normalized name (`normalized_name`): the persons whose name contains it.
    async fn persons(
        &self,
        scope: OrgScope,
        query: Option<&str>,
        after: Option<PartyCursor>,
        limit: u32,
    ) -> Result<Vec<PersonView>, StoreError>;

    /// Inserts an institution and gives it the next number of the organization (ADR 0038).
    async fn create_institution(
        &self,
        scope: OrgScope,
        id: InstitutionId,
        fields: &InstitutionFields,
        at: Timestamp,
        audit: &AuditEvent,
    ) -> Result<InstitutionView, StoreError>;

    /// Replaces the values of an institution if its version is `expected`, and counts the version up.
    async fn change_institution(
        &self,
        scope: OrgScope,
        id: InstitutionId,
        fields: &InstitutionFields,
        expected: RecordVersion,
        at: Timestamp,
        audit: &AuditEvent,
    ) -> Result<PartyChanged<InstitutionView>, StoreError>;

    async fn institution(
        &self,
        scope: OrgScope,
        id: InstitutionId,
    ) -> Result<Option<InstitutionView>, StoreError>;

    /// Like `persons`, for institutions.
    async fn institutions(
        &self,
        scope: OrgScope,
        query: Option<&str>,
        after: Option<PartyCursor>,
        limit: u32,
    ) -> Result<Vec<InstitutionView>, StoreError>;

    /// The persons and institutions whose name `names_match` the normalized name `normalized`.
    /// The review of a proposed person uses it to show possible duplicates (ADR 0050).
    async fn named_like(
        &self,
        scope: OrgScope,
        normalized: &str,
    ) -> Result<Vec<PartyRef>, StoreError>;
}

/// The shortest word that two names must share to match (ADR 0069).
const SHARED_WORD_MIN_CHARS: usize = 4;

/// True if two normalized names can name the same party: they are equal, one contains the other,
/// or they share a word of at least four characters.
pub fn names_match(a: &str, b: &str) -> bool {
    if a.is_empty() || b.is_empty() {
        return false;
    }
    if a.contains(b) || b.contains(a) {
        return true;
    }
    a.split(' ')
        .filter(|word| word.chars().count() >= SHARED_WORD_MIN_CHARS)
        .any(|word| b.split(' ').any(|other| other == word))
}

/// The input of `create_person`, as the caller gives it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewPerson {
    pub name: String,
    pub email: Option<String>,
    pub phone: Option<String>,
    /// The account of the person. It must belong to a member of the organization.
    pub user_id: Option<UserId>,
}

/// The input of `create_institution`, as the caller gives it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewInstitution {
    pub name: String,
    pub kind: String,
    pub email: Option<String>,
    pub phone: Option<String>,
}

/// A change of a person. A field that is `None` stays as it is; `Some(None)` clears email or phone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PersonChange {
    pub name: Option<String>,
    pub email: Option<Option<String>>,
    pub phone: Option<Option<String>>,
    pub expected_version: RecordVersion,
}

/// A change of an institution. A field that is `None` stays as it is; `Some(None)` clears email or phone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstitutionChange {
    pub name: Option<String>,
    pub kind: Option<String>,
    pub email: Option<Option<String>>,
    pub phone: Option<Option<String>>,
    pub expected_version: RecordVersion,
}

/// The error of a create and of a change.
#[derive(Debug, thiserror::Error)]
pub enum PartyError {
    #[error("the caller lacks the role for this command")]
    Forbidden,
    #[error("the record does not exist")]
    NotFound,
    #[error("the record changed after the caller read it")]
    VersionConflict,
    #[error("invalid values")]
    Invalid(Vec<FieldError>),
    #[error(transparent)]
    Store(#[from] StoreError),
}

impl PartyError {
    /// All codes that these commands can return, for the API contract (ADR 0037).
    pub const CODES: &[ProblemCode] = &[
        ProblemCode::Forbidden,
        ProblemCode::NotFound,
        ProblemCode::RecordVersionConflict,
        ProblemCode::ValidationFailed,
        ProblemCode::Unavailable,
        ProblemCode::Internal,
    ];
}

impl CommandError for PartyError {
    fn code(&self) -> ProblemCode {
        match self {
            Self::Forbidden => ProblemCode::Forbidden,
            Self::NotFound => ProblemCode::NotFound,
            Self::VersionConflict => ProblemCode::RecordVersionConflict,
            Self::Invalid(_) => ProblemCode::ValidationFailed,
            Self::Store(error) => error.code(),
        }
    }

    fn store_error(&self) -> Option<&StoreError> {
        match self {
            Self::Store(error) => Some(error),
            _ => None,
        }
    }

    fn field_errors(&self) -> &[FieldError] {
        match self {
            Self::Invalid(errors) => errors,
            _ => &[],
        }
    }
}

/// The error of the queries.
#[derive(Debug, thiserror::Error)]
pub enum PartyReadError {
    #[error("the caller has no role in any event")]
    Forbidden,
    #[error("the record does not exist")]
    NotFound,
    #[error(transparent)]
    Store(#[from] StoreError),
}

impl PartyReadError {
    /// All codes that these queries can return, for the API contract (ADR 0037).
    pub const CODES: &[ProblemCode] = &[
        ProblemCode::Forbidden,
        ProblemCode::NotFound,
        ProblemCode::Unavailable,
        ProblemCode::Internal,
    ];
}

impl CommandError for PartyReadError {
    fn code(&self) -> ProblemCode {
        match self {
            Self::Forbidden => ProblemCode::Forbidden,
            Self::NotFound => ProblemCode::NotFound,
            Self::Store(error) => error.code(),
        }
    }

    fn store_error(&self) -> Option<&StoreError> {
        match self {
            Self::Store(error) => Some(error),
            _ => None,
        }
    }
}

/// Collects the invalid fields of one input.
#[derive(Default)]
struct Checker(Vec<FieldError>);

impl Checker {
    fn name(&mut self, input: &str) -> Option<PartyName> {
        PartyName::parse(input)
            .map_err(|error| self.0.push(FieldError::new("name", text_code(error))))
            .ok()
    }

    fn kind(&mut self, input: &str) -> Option<InstitutionKind> {
        let kind = InstitutionKind::parse(input);
        if kind.is_none() {
            self.0.push(FieldError::new("kind", "unknown"));
        }
        kind
    }

    fn email(&mut self, input: Option<&str>) -> Option<Email> {
        let input = input?;
        Email::parse(input)
            .map_err(|error| self.0.push(FieldError::new("email", email_code(error))))
            .ok()
    }

    fn phone(&mut self, input: Option<&str>) -> Option<PhoneNumber> {
        let input = input?;
        PhoneNumber::parse(input)
            .map_err(|error| self.0.push(FieldError::new("phone", text_code(error))))
            .ok()
    }

    fn finish<T>(self, value: Option<T>) -> Result<T, PartyError> {
        match value {
            Some(value) if self.0.is_empty() => Ok(value),
            _ => Err(PartyError::Invalid(self.0)),
        }
    }
}

fn text_code(error: TextError) -> &'static str {
    match error {
        TextError::Empty => "empty",
        TextError::TooLong => "too-long",
        TextError::ControlCharacter => "control-character",
    }
}

pub(crate) fn email_code(error: EmailError) -> &'static str {
    match error {
        EmailError::Shape => "shape",
        EmailError::TooLong => "too-long",
        EmailError::ControlCharacter => "control-character",
        EmailError::Whitespace => "whitespace",
    }
}

/// The new value of an optional field: absent keeps `current`, `Some(None)` clears it.
fn merged<T: Clone, P>(
    change: &Option<Option<String>>,
    current: &Option<T>,
    parse: impl FnOnce(Option<&str>) -> Option<P>,
    keep: impl FnOnce(&T) -> P,
) -> Option<P> {
    match change {
        None => current.as_ref().map(keep),
        Some(text) => parse(text.as_deref()),
    }
}

async fn require_create(
    caller: &MemberCaller,
    identity: &dyn IdentityStore,
) -> Result<(), PartyError> {
    if access::proposes_in_some_event(caller, identity).await? {
        Ok(())
    } else {
        Err(PartyError::Forbidden)
    }
}

fn require_change(caller: &MemberCaller) -> Result<(), PartyError> {
    if access::sees_all_events(caller) {
        Ok(())
    } else {
        Err(PartyError::Forbidden)
    }
}

async fn require_read(
    caller: &impl Principal,
    identity: &dyn IdentityStore,
) -> Result<(), PartyReadError> {
    if access::reads_in_some_event(caller, identity).await? {
        Ok(())
    } else {
        Err(PartyReadError::Forbidden)
    }
}

fn audit(caller: &MemberCaller, action: AuditAction, record: Uuid) -> AuditEvent {
    AuditEvent::new(caller.actor(), action, Some(record), Some(caller.scope()))
}

/// Creates a person. A member with the contributor or manager role in any event can do it,
/// and so can an owner or an admin.
pub async fn create_person(
    caller: &MemberCaller,
    input: NewPerson,
    identity: &dyn IdentityStore,
    store: &dyn PartyStore,
    clock: &dyn Clock,
) -> Result<PersonView, PartyError> {
    require_create(caller, identity).await?;
    let mut check = Checker::default();
    let name = check.name(&input.name);
    let email = check.email(input.email.as_deref());
    let phone = check.phone(input.phone.as_deref());
    if let Some(user) = input.user_id
        && identity.membership(caller.scope(), user).await?.is_none()
    {
        check.0.push(FieldError::new("user_id", "unknown-member"));
    }
    let name = check.finish(name)?;
    let id = PersonId::from_uuid(Uuid::now_v7());
    let audit = audit(caller, AuditAction::PersonCreate, id.as_uuid());
    let fields = PersonFields { name, email, phone };
    Ok(store
        .create_person(
            caller.scope(),
            id,
            &fields,
            input.user_id,
            clock.now(),
            &audit,
        )
        .await?)
}

/// Changes a person. Only an owner or an admin can do it.
pub async fn change_person(
    caller: &MemberCaller,
    id: PersonId,
    change: PersonChange,
    store: &dyn PartyStore,
    clock: &dyn Clock,
) -> Result<PersonView, PartyError> {
    require_change(caller)?;
    let scope = caller.scope();
    let current = store.person(scope, id).await?.ok_or(PartyError::NotFound)?;
    if current.version != change.expected_version {
        return Err(PartyError::VersionConflict);
    }
    let mut check = Checker::default();
    let name = match &change.name {
        Some(name) => check.name(name),
        None => Some(current.name.clone()),
    };
    let email = merged(
        &change.email,
        &current.email,
        |text| check.email(text),
        Email::clone,
    );
    let phone = merged(
        &change.phone,
        &current.phone,
        |text| check.phone(text),
        PhoneNumber::clone,
    );
    let name = check.finish(name)?;
    let audit = audit(caller, AuditAction::PersonChange, id.as_uuid());
    let fields = PersonFields { name, email, phone };
    match store
        .change_person(
            scope,
            id,
            &fields,
            change.expected_version,
            clock.now(),
            &audit,
        )
        .await?
    {
        PartyChanged::Changed(person) => Ok(person),
        PartyChanged::NotFound => Err(PartyError::NotFound),
        PartyChanged::VersionConflict => Err(PartyError::VersionConflict),
    }
}

/// The persons whose name contains `query`, in the order of their numbers.
/// Each member with a role in some event, and each owner and admin, can read them.
pub async fn list_persons(
    caller: &impl Principal,
    query: Option<&str>,
    after: Option<PartyCursor>,
    limit: PageLimit,
    identity: &dyn IdentityStore,
    store: &dyn PartyStore,
) -> Result<Page<PersonView, PartyCursor>, PartyReadError> {
    require_read(caller, identity).await?;
    let query = normalized_query(query);
    let (items, next) = page(
        store
            .persons(caller.scope(), query.as_deref(), after, limit.get() + 1)
            .await?,
        limit,
        |person| person.local_number,
    );
    Ok(Page { items, next })
}

pub async fn get_person(
    caller: &impl Principal,
    id: PersonId,
    identity: &dyn IdentityStore,
    store: &dyn PartyStore,
) -> Result<PersonView, PartyReadError> {
    require_read(caller, identity).await?;
    store
        .person(caller.scope(), id)
        .await?
        .ok_or(PartyReadError::NotFound)
}

/// Creates an institution. The permission is the one of `create_person`.
pub async fn create_institution(
    caller: &MemberCaller,
    input: NewInstitution,
    identity: &dyn IdentityStore,
    store: &dyn PartyStore,
    clock: &dyn Clock,
) -> Result<InstitutionView, PartyError> {
    require_create(caller, identity).await?;
    let mut check = Checker::default();
    let name = check.name(&input.name);
    let kind = check.kind(&input.kind);
    let email = check.email(input.email.as_deref());
    let phone = check.phone(input.phone.as_deref());
    let (name, kind) = check.finish(name.zip(kind))?;
    let id = InstitutionId::from_uuid(Uuid::now_v7());
    let audit = audit(caller, AuditAction::InstitutionCreate, id.as_uuid());
    let fields = InstitutionFields {
        name,
        kind,
        email,
        phone,
    };
    Ok(store
        .create_institution(caller.scope(), id, &fields, clock.now(), &audit)
        .await?)
}

/// Changes an institution. Only an owner or an admin can do it.
pub async fn change_institution(
    caller: &MemberCaller,
    id: InstitutionId,
    change: InstitutionChange,
    store: &dyn PartyStore,
    clock: &dyn Clock,
) -> Result<InstitutionView, PartyError> {
    require_change(caller)?;
    let scope = caller.scope();
    let current = store
        .institution(scope, id)
        .await?
        .ok_or(PartyError::NotFound)?;
    if current.version != change.expected_version {
        return Err(PartyError::VersionConflict);
    }
    let mut check = Checker::default();
    let name = match &change.name {
        Some(name) => check.name(name),
        None => Some(current.name.clone()),
    };
    let kind = match &change.kind {
        Some(kind) => check.kind(kind),
        None => Some(current.kind),
    };
    let email = merged(
        &change.email,
        &current.email,
        |text| check.email(text),
        Email::clone,
    );
    let phone = merged(
        &change.phone,
        &current.phone,
        |text| check.phone(text),
        PhoneNumber::clone,
    );
    let (name, kind) = check.finish(name.zip(kind))?;
    let audit = audit(caller, AuditAction::InstitutionChange, id.as_uuid());
    let fields = InstitutionFields {
        name,
        kind,
        email,
        phone,
    };
    match store
        .change_institution(
            scope,
            id,
            &fields,
            change.expected_version,
            clock.now(),
            &audit,
        )
        .await?
    {
        PartyChanged::Changed(institution) => Ok(institution),
        PartyChanged::NotFound => Err(PartyError::NotFound),
        PartyChanged::VersionConflict => Err(PartyError::VersionConflict),
    }
}

/// The institutions whose name contains `query`. The permission is the one of `list_persons`.
pub async fn list_institutions(
    caller: &impl Principal,
    query: Option<&str>,
    after: Option<PartyCursor>,
    limit: PageLimit,
    identity: &dyn IdentityStore,
    store: &dyn PartyStore,
) -> Result<Page<InstitutionView, PartyCursor>, PartyReadError> {
    require_read(caller, identity).await?;
    let query = normalized_query(query);
    let (items, next) = page(
        store
            .institutions(caller.scope(), query.as_deref(), after, limit.get() + 1)
            .await?,
        limit,
        |institution| institution.local_number,
    );
    Ok(Page { items, next })
}

pub async fn get_institution(
    caller: &impl Principal,
    id: InstitutionId,
    identity: &dyn IdentityStore,
    store: &dyn PartyStore,
) -> Result<InstitutionView, PartyReadError> {
    require_read(caller, identity).await?;
    store
        .institution(caller.scope(), id)
        .await?
        .ok_or(PartyReadError::NotFound)
}

/// The normalized search text, or `None` if it has no characters.
fn normalized_query(query: Option<&str>) -> Option<String> {
    query.map(normalized_name).filter(|text| !text.is_empty())
}

/// Cuts the extra record that shows a next page, and gives the cursor of that page.
fn page<T>(
    mut items: Vec<T>,
    limit: PageLimit,
    number: impl Fn(&T) -> u64,
) -> (Vec<T>, Option<PartyCursor>) {
    let more = items.len() > limit.get() as usize;
    items.truncate(limit.get() as usize);
    let next = more
        .then(|| items.last())
        .flatten()
        .map(|last| PartyCursor(number(last)));
    (items, next)
}

#[cfg(test)]
mod tests;
