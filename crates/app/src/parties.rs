//! Persons and institutions: the commands and queries, and the port (ADR 0069).

use std::fmt::Debug;

use async_trait::async_trait;
use jiff::Timestamp;
use tada_domain::RecordVersion;
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
use crate::proposals::text_error_code;
use crate::records::{Changed, Checker, NumberCursor, audit, page};
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
    ) -> Result<Changed<PersonView>, StoreError>;

    async fn person(&self, scope: OrgScope, id: PersonId)
    -> Result<Option<PersonView>, StoreError>;

    /// At most `limit` persons in the order of their numbers, after `after`.
    /// `query` is a normalized name (`normalized_name`): the persons whose name contains it.
    async fn persons(
        &self,
        scope: OrgScope,
        query: Option<&str>,
        after: Option<NumberCursor>,
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
    ) -> Result<Changed<InstitutionView>, StoreError>;

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
        after: Option<NumberCursor>,
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
    /// Invalid values. A change without a field has no field errors.
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

fn parse_name(check: &mut Checker, input: &str) -> Option<PartyName> {
    check.parse("name", input, PartyName::parse, text_error_code)
}

fn parse_kind(check: &mut Checker, input: &str) -> Option<InstitutionKind> {
    let kind = InstitutionKind::parse(input);
    if kind.is_none() {
        check.push("kind", "unknown");
    }
    kind
}

fn parse_email(check: &mut Checker, input: Option<&str>) -> Option<Email> {
    check.parse("email", input?, Email::parse, email_code)
}

fn parse_phone(check: &mut Checker, input: Option<&str>) -> Option<PhoneNumber> {
    check.parse("phone", input?, PhoneNumber::parse, text_error_code)
}

/// The record of a change, or why the store did not change it.
fn changed<T>(result: Changed<T>) -> Result<T, PartyError> {
    match result {
        Changed::Changed(view) => Ok(view),
        Changed::NotFound => Err(PartyError::NotFound),
        Changed::VersionConflict => Err(PartyError::VersionConflict),
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
    let name = parse_name(&mut check, &input.name);
    let email = parse_email(&mut check, input.email.as_deref());
    let phone = parse_phone(&mut check, input.phone.as_deref());
    if let Some(user) = input.user_id
        && identity.membership(caller.scope(), user).await?.is_none()
    {
        check.push("user_id", "unknown-member");
    }
    let name = check.finish(name).map_err(PartyError::Invalid)?;
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
    if change.name.is_none() && change.email.is_none() && change.phone.is_none() {
        return Err(PartyError::Invalid(Vec::new()));
    }
    if current.version != change.expected_version {
        return Err(PartyError::VersionConflict);
    }
    let mut check = Checker::default();
    let name = match &change.name {
        Some(name) => parse_name(&mut check, name),
        None => Some(current.name.clone()),
    };
    let email = merged(
        &change.email,
        &current.email,
        |text| parse_email(&mut check, text),
        Email::clone,
    );
    let phone = merged(
        &change.phone,
        &current.phone,
        |text| parse_phone(&mut check, text),
        PhoneNumber::clone,
    );
    let name = check.finish(name).map_err(PartyError::Invalid)?;
    let audit = audit(caller, AuditAction::PersonChange, id.as_uuid());
    let fields = PersonFields { name, email, phone };
    changed(
        store
            .change_person(
                scope,
                id,
                &fields,
                change.expected_version,
                clock.now(),
                &audit,
            )
            .await?,
    )
}

/// The persons whose name contains `query`, in the order of their numbers.
/// Each member with a role in some event, and each owner and admin, can read them.
pub async fn list_persons(
    caller: &impl Principal,
    query: Option<&str>,
    after: Option<NumberCursor>,
    limit: PageLimit,
    identity: &dyn IdentityStore,
    store: &dyn PartyStore,
) -> Result<Page<PersonView, NumberCursor>, PartyReadError> {
    require_read(caller, identity).await?;
    let query = normalized_query(query);
    let items = store
        .persons(caller.scope(), query.as_deref(), after, limit.get() + 1)
        .await?;
    Ok(page(items, limit, |person| person.local_number))
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
    let name = parse_name(&mut check, &input.name);
    let kind = parse_kind(&mut check, &input.kind);
    let email = parse_email(&mut check, input.email.as_deref());
    let phone = parse_phone(&mut check, input.phone.as_deref());
    let (name, kind) = check.finish(name.zip(kind)).map_err(PartyError::Invalid)?;
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
    if change.name.is_none()
        && change.kind.is_none()
        && change.email.is_none()
        && change.phone.is_none()
    {
        return Err(PartyError::Invalid(Vec::new()));
    }
    if current.version != change.expected_version {
        return Err(PartyError::VersionConflict);
    }
    let mut check = Checker::default();
    let name = match &change.name {
        Some(name) => parse_name(&mut check, name),
        None => Some(current.name.clone()),
    };
    let kind = match &change.kind {
        Some(kind) => parse_kind(&mut check, kind),
        None => Some(current.kind),
    };
    let email = merged(
        &change.email,
        &current.email,
        |text| parse_email(&mut check, text),
        Email::clone,
    );
    let phone = merged(
        &change.phone,
        &current.phone,
        |text| parse_phone(&mut check, text),
        PhoneNumber::clone,
    );
    let (name, kind) = check.finish(name.zip(kind)).map_err(PartyError::Invalid)?;
    let audit = audit(caller, AuditAction::InstitutionChange, id.as_uuid());
    let fields = InstitutionFields {
        name,
        kind,
        email,
        phone,
    };
    changed(
        store
            .change_institution(
                scope,
                id,
                &fields,
                change.expected_version,
                clock.now(),
                &audit,
            )
            .await?,
    )
}

/// The institutions whose name contains `query`. The permission is the one of `list_persons`.
pub async fn list_institutions(
    caller: &impl Principal,
    query: Option<&str>,
    after: Option<NumberCursor>,
    limit: PageLimit,
    identity: &dyn IdentityStore,
    store: &dyn PartyStore,
) -> Result<Page<InstitutionView, NumberCursor>, PartyReadError> {
    require_read(caller, identity).await?;
    let query = normalized_query(query);
    let items = store
        .institutions(caller.scope(), query.as_deref(), after, limit.get() + 1)
        .await?;
    Ok(page(items, limit, |institution| institution.local_number))
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

#[cfg(test)]
mod tests;
