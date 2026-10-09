//! Workstreams (ADR 0067): named parts of an event with a lead. Event managers manage them.

use std::fmt::Debug;

use async_trait::async_trait;
use jiff::Timestamp;
use tada_domain::RecordVersion;
use tada_domain::ids::{EventId, UserId, WorkstreamId};
use tada_domain::work::{WorkstreamName, WorkstreamStatus};
use uuid::Uuid;

use crate::access::{self, AccessError, EventAccess, Principal};
use crate::audit::{AuditAction, AuditEvent};
use crate::caller::{MemberCaller, OrgScope};
use crate::clock::Clock;
use crate::identity::IdentityStore;
use crate::problem::{CommandError, FieldError, ProblemCode};
use crate::proposals::text_error_code;
use crate::records;
use crate::store::StoreError;

/// A workstream of an event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Workstream {
    pub id: WorkstreamId,
    pub event_id: EventId,
    pub name: WorkstreamName,
    /// A member of the event with the contributor or manager role.
    pub lead: UserId,
    pub status: WorkstreamStatus,
    pub version: RecordVersion,
}

/// The checked fields of a change. A field that is `None` stays as it is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkstreamUpdate {
    pub name: Option<WorkstreamName>,
    pub lead: Option<UserId>,
    pub status: Option<WorkstreamStatus>,
}

/// The result of `WorkstreamStore::create`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Created {
    Created,
    /// A record with this ID exists, in this organization or in another one.
    IdTaken,
    /// A workstream of the event has this name, whatever its case.
    NameTaken,
}

/// The result of `WorkstreamStore::change`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Changed {
    Changed(Workstream),
    /// The event has no workstream with this ID.
    NotFound,
    /// The workstream has another version.
    VersionConflict,
    /// Another workstream of the event has this name, whatever its case.
    NameTaken,
}

/// The repository port for workstreams. Each method stays inside `scope`.
/// Each change records its audit event in the same transaction.
#[async_trait]
pub trait WorkstreamStore: Debug + Send + Sync {
    async fn create(
        &self,
        scope: OrgScope,
        workstream: &Workstream,
        now: Timestamp,
        audit: &AuditEvent,
    ) -> Result<Created, StoreError>;

    /// Applies `update` if the workstream has the version `expected`.
    #[expect(
        clippy::too_many_arguments,
        reason = "a write port takes the scope, the record, the values, the version, the time and the audit event"
    )]
    async fn change(
        &self,
        scope: OrgScope,
        event: EventId,
        id: WorkstreamId,
        update: &WorkstreamUpdate,
        expected: RecordVersion,
        now: Timestamp,
        audit: &AuditEvent,
    ) -> Result<Changed, StoreError>;

    async fn get(
        &self,
        scope: OrgScope,
        event: EventId,
        id: WorkstreamId,
    ) -> Result<Option<Workstream>, StoreError>;

    /// The workstreams of the event in the order of their names.
    async fn list(&self, scope: OrgScope, event: EventId) -> Result<Vec<Workstream>, StoreError>;
}

/// The input of `create_workstream`, as the caller gives it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewWorkstream {
    /// The ID of the new workstream. A client that sends it can retry safely (ADR 0038).
    pub id: Option<Uuid>,
    pub name: String,
    pub lead: UserId,
}

/// The input of `change_workstream`. A field that is `None` stays as it is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkstreamChange {
    pub name: Option<String>,
    pub lead: Option<UserId>,
    pub status: Option<WorkstreamStatus>,
    pub expected_version: RecordVersion,
}

#[derive(Debug, thiserror::Error)]
pub enum WorkstreamError {
    /// The event, or for a change the workstream, does not exist or the caller cannot see it.
    #[error("the record does not exist or the caller cannot see it")]
    NotFound,
    #[error("only event managers manage workstreams")]
    Forbidden,
    #[error("invalid values")]
    Invalid(Vec<FieldError>),
    #[error("the workstream changed after the caller read it")]
    VersionConflict,
    #[error(transparent)]
    Store(#[from] StoreError),
}

impl WorkstreamError {
    /// All codes that the workstream commands can return, for the API contract (ADR 0037).
    pub const CODES: &[ProblemCode] = &[
        ProblemCode::NotFound,
        ProblemCode::Forbidden,
        ProblemCode::ValidationFailed,
        ProblemCode::RecordVersionConflict,
        ProblemCode::Unavailable,
        ProblemCode::Internal,
    ];
}

impl CommandError for WorkstreamError {
    fn code(&self) -> ProblemCode {
        match self {
            Self::NotFound => ProblemCode::NotFound,
            Self::Forbidden => ProblemCode::Forbidden,
            Self::Invalid(_) => ProblemCode::ValidationFailed,
            Self::VersionConflict => ProblemCode::RecordVersionConflict,
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

impl From<AccessError> for WorkstreamError {
    fn from(error: AccessError) -> Self {
        match error {
            AccessError::NotFound => Self::NotFound,
            AccessError::Store(error) => Self::Store(error),
        }
    }
}

fn invalid(field: &'static str, code: &'static str) -> WorkstreamError {
    WorkstreamError::Invalid(vec![FieldError::new(field, code)])
}

/// The caller must manage the event.
async fn require_manager(
    caller: &MemberCaller,
    event: EventId,
    identity: &dyn IdentityStore,
) -> Result<(), WorkstreamError> {
    match access::event_access(caller, event, identity).await? {
        EventAccess::Manager => Ok(()),
        _ => Err(WorkstreamError::Forbidden),
    }
}

/// The lead must be a contributor or a manager of the event (ADR 0067).
async fn check_lead(
    scope: OrgScope,
    event: EventId,
    lead: UserId,
    identity: &dyn IdentityStore,
) -> Result<(), WorkstreamError> {
    match access::member_access(scope, event, lead, identity).await? {
        Some(access) if access.can_propose() => Ok(()),
        _ => Err(invalid("lead", "unknown-member")),
    }
}

fn parse_name(name: &str) -> Result<WorkstreamName, WorkstreamError> {
    WorkstreamName::parse(name).map_err(|error| invalid("name", text_error_code(error)))
}

/// Creates a workstream in the event. Only event managers do this.
pub async fn create_workstream(
    caller: &MemberCaller,
    event: EventId,
    input: NewWorkstream,
    identity: &dyn IdentityStore,
    store: &dyn WorkstreamStore,
    clock: &dyn Clock,
) -> Result<Workstream, WorkstreamError> {
    require_manager(caller, event, identity).await?;
    let id = WorkstreamId::from_uuid(
        records::record_id(input.id).map_err(|error| WorkstreamError::Invalid(vec![error]))?,
    );
    let name = parse_name(&input.name)?;
    check_lead(caller.scope(), event, input.lead, identity).await?;
    let workstream = Workstream {
        id,
        event_id: event,
        name,
        lead: input.lead,
        status: WorkstreamStatus::Active,
        version: RecordVersion::FIRST,
    };
    let audit = records::audit(caller, AuditAction::WorkstreamCreate, id.as_uuid());
    match store
        .create(caller.scope(), &workstream, clock.now(), &audit)
        .await?
    {
        Created::Created => Ok(workstream),
        Created::IdTaken => Err(invalid("id", "taken")),
        Created::NameTaken => Err(invalid("name", "taken")),
    }
}

/// Renames a workstream, changes its lead or opens or closes it. Only event managers do this.
pub async fn change_workstream(
    caller: &MemberCaller,
    event: EventId,
    id: WorkstreamId,
    change: WorkstreamChange,
    identity: &dyn IdentityStore,
    store: &dyn WorkstreamStore,
    clock: &dyn Clock,
) -> Result<Workstream, WorkstreamError> {
    require_manager(caller, event, identity).await?;
    let name = change.name.as_deref().map(parse_name).transpose()?;
    if let Some(lead) = change.lead {
        check_lead(caller.scope(), event, lead, identity).await?;
    }
    let update = WorkstreamUpdate {
        name,
        lead: change.lead,
        status: change.status,
    };
    let audit = records::audit(caller, AuditAction::WorkstreamChange, id.as_uuid());
    match store
        .change(
            caller.scope(),
            event,
            id,
            &update,
            change.expected_version,
            clock.now(),
            &audit,
        )
        .await?
    {
        Changed::Changed(workstream) => Ok(workstream),
        Changed::NotFound => Err(WorkstreamError::NotFound),
        Changed::VersionConflict => Err(WorkstreamError::VersionConflict),
        Changed::NameTaken => Err(invalid("name", "taken")),
    }
}

/// The workstreams of the event. Each reader of the event sees them.
pub async fn list_workstreams(
    caller: &impl Principal,
    event: EventId,
    identity: &dyn IdentityStore,
    store: &dyn WorkstreamStore,
) -> Result<Vec<Workstream>, WorkstreamError> {
    access::event_access(caller, event, identity).await?;
    Ok(store.list(caller.scope(), event).await?)
}

/// Why a work record cannot name a workstream.
#[derive(Debug, thiserror::Error)]
pub enum ActiveWorkstreamError {
    /// The workstream is missing in the event (`unknown-record`) or closed (`closed`).
    #[error("the workstream cannot take new records")]
    Refused(FieldError),
    #[error(transparent)]
    Store(#[from] StoreError),
}

/// The workstream `id` of `event`, if it takes new records (ADR 0067).
/// A workstream of another event or organization is missing here, as is an unknown ID.
/// The field of the refusal is `workstream`.
pub async fn active_workstream(
    store: &dyn WorkstreamStore,
    scope: OrgScope,
    event: EventId,
    id: WorkstreamId,
) -> Result<Workstream, ActiveWorkstreamError> {
    let refuse = |code| ActiveWorkstreamError::Refused(FieldError::new("workstream", code));
    match store.get(scope, event, id).await? {
        None => Err(refuse("unknown-record")),
        Some(workstream) if workstream.status == WorkstreamStatus::Closed => Err(refuse("closed")),
        Some(workstream) => Ok(workstream),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::Mutex;

    use tada_domain::identity::{EventRole, OrganizationRole};
    use tada_domain::ids::OrganizationId;

    use super::*;
    use crate::identity::{Membership, UserRef};

    fn testwil() -> OrganizationId {
        OrganizationId::from_uuid(Uuid::from_u128(10))
    }

    fn open_day() -> EventId {
        EventId::from_uuid(Uuid::from_u128(20))
    }

    fn other_event() -> EventId {
        EventId::from_uuid(Uuid::from_u128(21))
    }

    fn user(number: u128) -> UserId {
        UserId::from_uuid(Uuid::from_u128(number))
    }

    fn caller(user: UserId) -> MemberCaller {
        MemberCaller::new(user, testwil(), OrganizationRole::Member)
    }

    const MANAGER: u128 = 1;
    const CONTRIBUTOR: u128 = 2;
    const VIEWER: u128 = 3;

    /// Three members of Testwil with an event role in `open_day`, and a store of workstreams.
    #[derive(Debug, Default)]
    struct Memory {
        workstreams: Mutex<Vec<Workstream>>,
    }

    fn roles() -> HashMap<UserId, EventRole> {
        HashMap::from([
            (user(MANAGER), EventRole::EventManager),
            (user(CONTRIBUTOR), EventRole::EventContributor),
            (user(VIEWER), EventRole::EventViewer),
        ])
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
            _: OrgScope,
            user: UserId,
        ) -> Result<Option<OrganizationRole>, StoreError> {
            Ok(roles()
                .contains_key(&user)
                .then_some(OrganizationRole::Member))
        }

        async fn event_exists(&self, _: OrgScope, event: EventId) -> Result<bool, StoreError> {
            Ok(event == open_day() || event == other_event())
        }

        async fn event_role(
            &self,
            _: OrgScope,
            event: EventId,
            user: UserId,
        ) -> Result<Option<EventRole>, StoreError> {
            Ok((event == open_day())
                .then(|| roles().get(&user).copied())
                .flatten())
        }

        async fn event_roles_of(
            &self,
            _: OrgScope,
            _: UserId,
        ) -> Result<Vec<(EventId, EventRole)>, StoreError> {
            unreachable!()
        }
    }

    fn same_name(a: &WorkstreamName, b: &WorkstreamName) -> bool {
        a.as_str().to_lowercase() == b.as_str().to_lowercase()
    }

    #[async_trait]
    impl WorkstreamStore for Memory {
        async fn create(
            &self,
            _: OrgScope,
            workstream: &Workstream,
            _: Timestamp,
            _: &AuditEvent,
        ) -> Result<Created, StoreError> {
            let mut all = self.workstreams.lock().unwrap();
            if all.iter().any(|known| known.id == workstream.id) {
                return Ok(Created::IdTaken);
            }
            if all.iter().any(|known| {
                known.event_id == workstream.event_id && same_name(&known.name, &workstream.name)
            }) {
                return Ok(Created::NameTaken);
            }
            all.push(workstream.clone());
            Ok(Created::Created)
        }

        async fn change(
            &self,
            _: OrgScope,
            event: EventId,
            id: WorkstreamId,
            update: &WorkstreamUpdate,
            expected: RecordVersion,
            _: Timestamp,
            _: &AuditEvent,
        ) -> Result<Changed, StoreError> {
            let mut all = self.workstreams.lock().unwrap();
            let Some(position) = all
                .iter()
                .position(|known| known.id == id && known.event_id == event)
            else {
                return Ok(Changed::NotFound);
            };
            if all[position].version != expected {
                return Ok(Changed::VersionConflict);
            }
            if let Some(name) = &update.name
                && all.iter().any(|known| {
                    known.id != id && known.event_id == event && same_name(&known.name, name)
                })
            {
                return Ok(Changed::NameTaken);
            }
            let known = &mut all[position];
            if let Some(name) = &update.name {
                known.name = name.clone();
            }
            if let Some(lead) = update.lead {
                known.lead = lead;
            }
            if let Some(status) = update.status {
                known.status = status;
            }
            known.version = RecordVersion::new(known.version.get() + 1).unwrap();
            Ok(Changed::Changed(known.clone()))
        }

        async fn get(
            &self,
            _: OrgScope,
            event: EventId,
            id: WorkstreamId,
        ) -> Result<Option<Workstream>, StoreError> {
            let all = self.workstreams.lock().unwrap();
            Ok(all
                .iter()
                .find(|known| known.id == id && known.event_id == event)
                .cloned())
        }

        async fn list(&self, _: OrgScope, event: EventId) -> Result<Vec<Workstream>, StoreError> {
            let all = self.workstreams.lock().unwrap();
            Ok(all
                .iter()
                .filter(|known| known.event_id == event)
                .cloned()
                .collect())
        }
    }

    #[derive(Debug)]
    struct FixedClock;

    impl Clock for FixedClock {
        fn now(&self) -> Timestamp {
            "2030-05-18T08:00:00Z".parse().unwrap()
        }
    }

    fn new(name: &str, lead: u128) -> NewWorkstream {
        NewWorkstream {
            id: None,
            name: name.to_owned(),
            lead: user(lead),
        }
    }

    async fn create(
        memory: &Memory,
        by: u128,
        input: NewWorkstream,
    ) -> Result<Workstream, WorkstreamError> {
        create_workstream(
            &caller(user(by)),
            open_day(),
            input,
            memory,
            memory,
            &FixedClock,
        )
        .await
    }

    fn fields(result: Result<Workstream, WorkstreamError>) -> Vec<(String, &'static str)> {
        match result {
            Err(WorkstreamError::Invalid(errors)) => errors
                .into_iter()
                .map(|error| (error.field.into_owned(), error.code))
                .collect(),
            other => panic!("not invalid: {other:?}"),
        }
    }

    #[tokio::test]
    async fn only_an_event_manager_creates_a_workstream() {
        let memory = Memory::default();
        for by in [CONTRIBUTOR, VIEWER] {
            let result = create(&memory, by, new("Gelände", CONTRIBUTOR)).await;
            assert!(matches!(result, Err(WorkstreamError::Forbidden)), "{by}");
        }
        let stranger = create(&memory, 99, new("Gelände", CONTRIBUTOR)).await;
        assert!(matches!(stranger, Err(WorkstreamError::NotFound)));
        assert!(memory.workstreams.lock().unwrap().is_empty());

        let created = create(&memory, MANAGER, new("Gelände", CONTRIBUTOR))
            .await
            .unwrap();
        assert_eq!(created.status, WorkstreamStatus::Active);
        assert_eq!(created.version, RecordVersion::FIRST);
        assert_eq!(created.lead, user(CONTRIBUTOR));
    }

    #[tokio::test]
    async fn an_owner_acts_as_event_manager() {
        let memory = Memory::default();
        let owner = MemberCaller::new(user(50), testwil(), OrganizationRole::Owner);
        let created = create_workstream(
            &owner,
            open_day(),
            new("Gelände", MANAGER),
            &memory,
            &memory,
            &FixedClock,
        )
        .await;
        assert!(created.is_ok());
    }

    #[tokio::test]
    async fn the_lead_must_be_a_contributor_or_manager_of_the_event() {
        let memory = Memory::default();
        let viewer = create(&memory, MANAGER, new("Gelände", VIEWER)).await;
        assert_eq!(fields(viewer), [("lead".to_owned(), "unknown-member")]);
        let stranger = create(&memory, MANAGER, new("Gelände", 99)).await;
        assert_eq!(fields(stranger), [("lead".to_owned(), "unknown-member")]);
        assert!(
            create(&memory, MANAGER, new("Gelände", MANAGER))
                .await
                .is_ok()
        );

        // A change of the lead obeys the same rule.
        let id = memory.workstreams.lock().unwrap()[0].id;
        let change = WorkstreamChange {
            name: None,
            lead: Some(user(VIEWER)),
            status: None,
            expected_version: RecordVersion::FIRST,
        };
        let result = change_workstream(
            &caller(user(MANAGER)),
            open_day(),
            id,
            change,
            &memory,
            &memory,
            &FixedClock,
        )
        .await;
        assert_eq!(fields(result), [("lead".to_owned(), "unknown-member")]);
    }

    #[tokio::test]
    async fn workstream_names_are_unique_in_the_event() {
        let memory = Memory::default();
        create(&memory, MANAGER, new("Gelände", MANAGER))
            .await
            .unwrap();
        let again = create(&memory, MANAGER, new("GELÄNDE", MANAGER)).await;
        assert_eq!(fields(again), [("name".to_owned(), "taken")]);

        let other = create(&memory, MANAGER, new("Küche", MANAGER))
            .await
            .unwrap();
        let rename = WorkstreamChange {
            name: Some("gelände".to_owned()),
            lead: None,
            status: None,
            expected_version: RecordVersion::FIRST,
        };
        let result = change_workstream(
            &caller(user(MANAGER)),
            open_day(),
            other.id,
            rename,
            &memory,
            &memory,
            &FixedClock,
        )
        .await;
        assert_eq!(fields(result), [("name".to_owned(), "taken")]);
    }

    #[tokio::test]
    async fn a_change_needs_the_current_version() {
        let memory = Memory::default();
        let created = create(&memory, MANAGER, new("Gelände", MANAGER))
            .await
            .unwrap();
        let change = |expected_version| WorkstreamChange {
            name: Some("Aussengelände".to_owned()),
            lead: None,
            status: None,
            expected_version,
        };
        let manager = caller(user(MANAGER));
        let run = |change| {
            change_workstream(
                &manager,
                open_day(),
                created.id,
                change,
                &memory,
                &memory,
                &FixedClock,
            )
        };
        let changed = run(change(RecordVersion::FIRST)).await.unwrap();
        assert_eq!(changed.name.as_str(), "Aussengelände");
        assert_eq!(changed.version.get(), 2);
        let stale = run(change(RecordVersion::FIRST)).await;
        assert!(matches!(stale, Err(WorkstreamError::VersionConflict)));
    }

    #[tokio::test]
    async fn a_closed_workstream_refuses_new_records() {
        let memory = Memory::default();
        let scope = caller(user(MANAGER)).scope();
        let created = create(&memory, MANAGER, new("Gelände", MANAGER))
            .await
            .unwrap();
        assert!(
            active_workstream(&memory, scope, open_day(), created.id)
                .await
                .is_ok()
        );

        let close = WorkstreamChange {
            name: None,
            lead: None,
            status: Some(WorkstreamStatus::Closed),
            expected_version: RecordVersion::FIRST,
        };
        change_workstream(
            &caller(user(MANAGER)),
            open_day(),
            created.id,
            close,
            &memory,
            &memory,
            &FixedClock,
        )
        .await
        .unwrap();
        let refused = |result| match result {
            Err(ActiveWorkstreamError::Refused(error)) => (error.field.into_owned(), error.code),
            other => panic!("not refused: {other:?}"),
        };
        assert_eq!(
            refused(active_workstream(&memory, scope, open_day(), created.id).await),
            ("workstream".to_owned(), "closed")
        );
        // A workstream of another event is as unknown as an ID that does not exist.
        assert_eq!(
            refused(active_workstream(&memory, scope, other_event(), created.id).await),
            ("workstream".to_owned(), "unknown-record")
        );
    }

    #[tokio::test]
    async fn each_reader_of_the_event_lists_the_workstreams() {
        let memory = Memory::default();
        create(&memory, MANAGER, new("Gelände", MANAGER))
            .await
            .unwrap();
        let listed = list_workstreams(&caller(user(VIEWER)), open_day(), &memory, &memory).await;
        assert_eq!(listed.unwrap().len(), 1);
        let stranger = list_workstreams(&caller(user(99)), open_day(), &memory, &memory).await;
        assert!(matches!(stranger, Err(WorkstreamError::NotFound)));
    }
}
