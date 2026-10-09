//! Actions and commitments (ADR 0068): the work records of an event, with their direct commands.
//! A member creates and changes them without review. Proposals come later (ADR 0050).

use std::fmt::Debug;

use async_trait::async_trait;
use jiff::Timestamp;
use jiff::civil::Date;
use tada_domain::RecordVersion;
use tada_domain::events::EventKey;
use tada_domain::ids::{
    self, ActionId, CommitmentId, EventId, LocalIdKind, ProposalId, SourceVersionId, UserId,
    WorkstreamId,
};
use tada_domain::parties::Party;
use tada_domain::work::{
    ActionDescription, ActionStatus, ActionTitle, CommitmentStatus, CommitmentText, ConditionText,
    FirmReason,
};
use uuid::Uuid;

use crate::access::{self, AccessError, EventAccess};
use crate::audit::{AuditAction, AuditEvent};
use crate::caller::{MemberCaller, OrgScope};
use crate::clock::Clock;
use crate::identity::IdentityStore;
use crate::paging::{Page, PageLimit};
use crate::parties::{PartyRef, PartyStore};
use crate::problem::{CommandError, FieldError, ProblemCode};
use crate::proposals::text_error_code;
use crate::store::StoreError;
use crate::workstreams::{ActiveWorkstreamError, WorkstreamStore, active_workstream};

#[cfg(test)]
mod tests;

/// An action of an event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionView {
    pub id: ActionId,
    /// The number in `ACT-001`, local to the event (ADR 0038).
    pub local_number: u64,
    pub event_id: EventId,
    pub fields: ActionFields,
    pub version: RecordVersion,
}

impl ActionView {
    /// The readable ID, for example `ACT-001`.
    pub fn local_id(&self) -> String {
        LocalIdKind::Action.readable_id(self.local_number)
    }
}

/// The values of an action that a change replaces.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionFields {
    pub title: ActionTitle,
    pub description: Option<ActionDescription>,
    /// A member of the event with the contributor or manager role.
    pub owner: UserId,
    pub workstream_id: Option<WorkstreamId>,
    pub due_date: Option<Date>,
    pub status: ActionStatus,
}

/// A commitment of an event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitmentView {
    pub id: CommitmentId,
    /// The number in `COM-001`, local to the event (ADR 0038).
    pub local_number: u64,
    pub event_id: EventId,
    /// The condition never changes after the creation. It stays as history after "make firm".
    pub condition: Option<ConditionText>,
    pub promisor: PartyRef,
    pub fields: CommitmentFields,
    pub version: RecordVersion,
    /// The evidence of the accepted proposals that created or changed the commitment.
    pub evidence: Vec<RecordEvidenceView>,
}

impl CommitmentView {
    /// The readable ID, for example `COM-001`.
    pub fn local_id(&self) -> String {
        LocalIdKind::Commitment.readable_id(self.local_number)
    }
}

/// The values of a commitment that a change replaces.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitmentFields {
    pub text: CommitmentText,
    pub owner: UserId,
    pub workstream_id: Option<WorkstreamId>,
    pub due_date: Option<Date>,
    pub status: CommitmentStatus,
    /// The reason that made the commitment firm. A conditional commitment has none.
    pub firm_reason: Option<FirmReason>,
}

/// A passage that supports one version of a work record (ADR 0068).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordEvidenceView {
    /// The record version that the accepted change produced.
    pub record_version: RecordVersion,
    pub proposal_id: ProposalId,
    pub source_version_id: SourceVersionId,
    /// The capture time of the source version.
    pub captured_at: Timestamp,
    pub start_offset: u32,
    pub end_offset: u32,
    pub quote: String,
    pub page: Option<u32>,
}

/// A new action, checked, before the store gives it its number.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewActionRecord {
    pub id: ActionId,
    pub event_id: EventId,
    pub fields: ActionFields,
}

/// A new commitment, checked, before the store gives it its number.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewCommitmentRecord {
    pub id: CommitmentId,
    pub event_id: EventId,
    pub condition: Option<ConditionText>,
    pub promisor: Party,
    pub fields: CommitmentFields,
}

/// The result of a create in the store.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkCreated<T> {
    Created(T),
    /// A record with this ID exists, in this organization or in another one.
    IdTaken,
}

/// The result of a change in the store.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkChanged<T> {
    Changed(T),
    /// The event has no such record.
    NotFound,
    /// The record has another version.
    VersionConflict,
}

/// The position after the last record of a page: its local number (ADR 0044).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WorkCursor(pub u64);

/// The records of an event that a list shows, in the order of their numbers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WorkFilter<S> {
    pub owner: Option<UserId>,
    pub status: Option<S>,
    pub workstream: Option<WorkstreamId>,
    pub after: Option<WorkCursor>,
    pub limit: u32,
}

/// A work record with the key of its event: the full reference is the key and the readable ID,
/// for example `FLY28/ACT-042` (ADR 0038).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InEvent<T> {
    pub event_key: EventKey,
    pub record: T,
}

/// The open actions and commitments that a member owns in the events where the member has a role.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MyWork {
    /// The actions with the status `open`, `in-progress` or `blocked`.
    /// Due date first, a record without a due date last, then event key and number.
    pub actions: Vec<InEvent<ActionView>>,
    /// The commitments with the status `conditional` or `firm`, in the same order.
    pub commitments: Vec<InEvent<CommitmentView>>,
}

/// What "My Work" shows (spec 2a, section 4).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MyWorkView {
    pub work: MyWork,
    /// The proposals that the caller reviews. Zero until the review routing feeds it.
    pub review_count: u32,
}

/// The repository port for actions and commitments. Each method stays inside `scope`.
/// A create or a change records its audit event in the same transaction.
#[async_trait]
pub trait WorkStore: Debug + Send + Sync {
    /// Inserts an action with the version 1 and gives it the next number of its event (ADR 0038).
    async fn create_action(
        &self,
        scope: OrgScope,
        action: &NewActionRecord,
        at: Timestamp,
        audit: &AuditEvent,
    ) -> Result<WorkCreated<ActionView>, StoreError>;

    /// Replaces the values of an action if its version is `expected`, and counts the version up.
    /// The store stamps the change time.
    async fn change_action(
        &self,
        scope: OrgScope,
        event: EventId,
        id: ActionId,
        fields: &ActionFields,
        expected: RecordVersion,
        audit: &AuditEvent,
    ) -> Result<WorkChanged<ActionView>, StoreError>;

    async fn action(
        &self,
        scope: OrgScope,
        event: EventId,
        id: ActionId,
    ) -> Result<Option<ActionView>, StoreError>;

    /// At most `filter.limit` actions of the event in the order of their numbers.
    async fn actions(
        &self,
        scope: OrgScope,
        event: EventId,
        filter: &WorkFilter<ActionStatus>,
    ) -> Result<Vec<ActionView>, StoreError>;

    /// Inserts a commitment with the version 1 and gives it the next number of its event (ADR 0038).
    async fn create_commitment(
        &self,
        scope: OrgScope,
        commitment: &NewCommitmentRecord,
        at: Timestamp,
        audit: &AuditEvent,
    ) -> Result<WorkCreated<CommitmentView>, StoreError>;

    /// Replaces the values of a commitment if its version is `expected`, and counts the version up.
    /// The store stamps the change time.
    async fn change_commitment(
        &self,
        scope: OrgScope,
        event: EventId,
        id: CommitmentId,
        fields: &CommitmentFields,
        expected: RecordVersion,
        audit: &AuditEvent,
    ) -> Result<WorkChanged<CommitmentView>, StoreError>;

    async fn commitment(
        &self,
        scope: OrgScope,
        event: EventId,
        id: CommitmentId,
    ) -> Result<Option<CommitmentView>, StoreError>;

    /// At most `filter.limit` commitments of the event in the order of their numbers.
    async fn commitments(
        &self,
        scope: OrgScope,
        event: EventId,
        filter: &WorkFilter<CommitmentStatus>,
    ) -> Result<Vec<CommitmentView>, StoreError>;

    /// The open records that `user` owns, in the events that `user` can read now: the events with
    /// an event role of the user, or all events of the organization if `all_events` is set
    /// (owners and admins, ADR 0052). A record of an event that the user left does not count.
    async fn my_open_work(
        &self,
        scope: OrgScope,
        user: UserId,
        all_events: bool,
    ) -> Result<MyWork, StoreError>;
}

/// The ports that the work commands need.
#[derive(Debug, Clone, Copy)]
pub struct WorkPorts<'a> {
    pub identity: &'a dyn IdentityStore,
    pub work: &'a dyn WorkStore,
    pub workstreams: &'a dyn WorkstreamStore,
    pub parties: &'a dyn PartyStore,
    pub clock: &'a dyn Clock,
}

/// The input of `create_action`, as the caller gives it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewAction {
    /// The ID of the new action. A client that sends it can retry safely (ADR 0038).
    pub id: Option<Uuid>,
    pub title: String,
    pub description: Option<String>,
    pub owner: UserId,
    pub workstream: Option<WorkstreamId>,
    pub due_date: Option<Date>,
}

/// The input of `change_action`. A field that is `None` stays as it is; `Some(None)` clears it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionChange {
    pub title: Option<String>,
    pub description: Option<Option<String>>,
    pub owner: Option<UserId>,
    pub workstream: Option<Option<WorkstreamId>>,
    pub due_date: Option<Option<Date>>,
    pub status: Option<ActionStatus>,
    pub expected_version: RecordVersion,
}

/// The input of `create_commitment`, as the caller gives it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewCommitment {
    /// The ID of the new commitment. A client that sends it can retry safely (ADR 0038).
    pub id: Option<Uuid>,
    pub text: String,
    /// The condition. A commitment with a condition starts `conditional`, else `firm`.
    pub condition: Option<String>,
    pub promisor: Party,
    pub owner: UserId,
    pub workstream: Option<WorkstreamId>,
    pub due_date: Option<Date>,
}

/// The input of `change_commitment`. It has no condition: the condition is fixed at the creation.
/// A field that is `None` stays as it is; `Some(None)` clears it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitmentChange {
    pub text: Option<String>,
    pub owner: Option<UserId>,
    pub workstream: Option<Option<WorkstreamId>>,
    pub due_date: Option<Option<Date>>,
    /// Any status but `firm`: only `make_commitment_firm` makes a commitment firm.
    pub status: Option<CommitmentStatus>,
    pub expected_version: RecordVersion,
}

/// The input of `make_commitment_firm`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FirmInput {
    pub reason: String,
    pub expected_version: RecordVersion,
}

#[derive(Debug, thiserror::Error)]
pub enum WorkError {
    /// The event or the record does not exist, or the caller cannot see it.
    #[error("the record does not exist or the caller cannot see it")]
    NotFound,
    #[error("the caller cannot do this")]
    Forbidden,
    /// Invalid values. A change without a field has no field errors.
    #[error("invalid values")]
    Invalid(Vec<FieldError>),
    #[error("the status cannot change to this status")]
    InvalidTransition,
    #[error("the record changed after the caller read it")]
    VersionConflict,
    #[error(transparent)]
    Store(#[from] StoreError),
}

impl WorkError {
    /// All codes that the work commands can return, for the API contract (ADR 0037).
    pub const CODES: &[ProblemCode] = &[
        ProblemCode::NotFound,
        ProblemCode::Forbidden,
        ProblemCode::ValidationFailed,
        ProblemCode::InvalidTransition,
        ProblemCode::RecordVersionConflict,
        ProblemCode::Unavailable,
        ProblemCode::Internal,
    ];

    /// The codes of the reads.
    pub const READ_CODES: &[ProblemCode] = &[
        ProblemCode::NotFound,
        ProblemCode::Unavailable,
        ProblemCode::Internal,
    ];
}

impl CommandError for WorkError {
    fn code(&self) -> ProblemCode {
        match self {
            Self::NotFound => ProblemCode::NotFound,
            Self::Forbidden => ProblemCode::Forbidden,
            Self::Invalid(_) => ProblemCode::ValidationFailed,
            Self::InvalidTransition => ProblemCode::InvalidTransition,
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

impl From<AccessError> for WorkError {
    fn from(error: AccessError) -> Self {
        match error {
            AccessError::NotFound => Self::NotFound,
            AccessError::Store(error) => Self::Store(error),
        }
    }
}

/// True if `caller` can change a work record with the owner `owner` in a workstream with the lead `lead` (ADR 0067):
/// its owner, the lead of its workstream, or an event manager. A viewer changes nothing.
pub fn may_change_work(
    access: EventAccess,
    caller: UserId,
    owner: UserId,
    lead: Option<UserId>,
) -> bool {
    match access {
        EventAccess::Manager => true,
        EventAccess::Contributor => caller == owner || lead == Some(caller),
        EventAccess::Viewer => false,
    }
}

/// Collects the field errors of one input, so that the caller sees all of them at once.
#[derive(Debug, Default)]
struct Checker(Vec<FieldError>);

impl Checker {
    fn text<T, E: Copy>(
        &mut self,
        field: &'static str,
        input: &str,
        parse: impl FnOnce(&str) -> Result<T, E>,
        code: impl FnOnce(E) -> &'static str,
    ) -> Option<T> {
        parse(input)
            .map_err(|error| self.0.push(FieldError::new(field, code(error))))
            .ok()
    }

    fn push(&mut self, field: &'static str, code: &'static str) {
        self.0.push(FieldError::new(field, code));
    }

    fn finish(self) -> Result<(), WorkError> {
        if self.0.is_empty() {
            Ok(())
        } else {
            Err(WorkError::Invalid(self.0))
        }
    }
}

fn parse_title(check: &mut Checker, input: &str) -> Option<ActionTitle> {
    check.text("title", input, ActionTitle::parse, text_error_code)
}

fn parse_description(
    check: &mut Checker,
    input: Option<&str>,
) -> Option<Option<ActionDescription>> {
    match input {
        None => Some(None),
        Some(text) => check
            .text(
                "description",
                text,
                ActionDescription::parse,
                text_error_code,
            )
            .map(Some),
    }
}

fn parse_text(check: &mut Checker, input: &str) -> Option<CommitmentText> {
    check.text("text", input, CommitmentText::parse, text_error_code)
}

fn record_id(id: Option<Uuid>) -> Result<Uuid, WorkError> {
    match id {
        Some(id) if !ids::is_record_id(id) => Err(WorkError::Invalid(vec![FieldError::new(
            "id",
            "not-uuid-v7",
        )])),
        Some(id) => Ok(id),
        None => Ok(Uuid::now_v7()),
    }
}

/// The owner of a work record must be a contributor or a manager of the event (ADR 0068).
async fn check_owner(
    check: &mut Checker,
    scope: OrgScope,
    event: EventId,
    owner: UserId,
    identity: &dyn IdentityStore,
) -> Result<(), StoreError> {
    match access::member_access(scope, event, owner, identity).await? {
        Some(access) if access.can_propose() => {}
        _ => check.push("owner", "unknown-member"),
    }
    Ok(())
}

/// A new workstream of a record must be active and in the event. A store failure stops the command.
async fn check_workstream(
    check: &mut Checker,
    scope: OrgScope,
    event: EventId,
    workstream: WorkstreamId,
    workstreams: &dyn WorkstreamStore,
) -> Result<(), StoreError> {
    match active_workstream(workstreams, scope, event, workstream).await {
        Ok(_) => Ok(()),
        Err(ActiveWorkstreamError::Refused(error)) => {
            check.0.push(error);
            Ok(())
        }
        Err(ActiveWorkstreamError::Store(error)) => Err(error),
    }
}

/// The promisor must be a person or an institution of the organization (ADR 0069).
async fn check_promisor(
    check: &mut Checker,
    scope: OrgScope,
    promisor: Party,
    parties: &dyn PartyStore,
) -> Result<(), StoreError> {
    let known = match promisor {
        Party::Person(id) => parties.person(scope, id).await?.is_some(),
        Party::Institution(id) => parties.institution(scope, id).await?.is_some(),
    };
    if !known {
        check.push("promisor", "unknown-record");
    }
    Ok(())
}

/// The caller must be a contributor or a manager of the event.
async fn require_create(
    caller: &MemberCaller,
    event: EventId,
    identity: &dyn IdentityStore,
) -> Result<(), WorkError> {
    if access::event_access(caller, event, identity)
        .await?
        .can_propose()
    {
        Ok(())
    } else {
        Err(WorkError::Forbidden)
    }
}

/// The caller must be allowed to change a record with the owner `owner` in the workstream `workstream`.
async fn require_change(
    caller: &MemberCaller,
    access: EventAccess,
    event: EventId,
    owner: UserId,
    workstream: Option<WorkstreamId>,
    workstreams: &dyn WorkstreamStore,
) -> Result<(), WorkError> {
    let lead = match workstream {
        Some(id) => workstreams
            .get(caller.scope(), event, id)
            .await?
            .map(|workstream| workstream.lead),
        None => None,
    };
    if may_change_work(access, caller.user_id(), owner, lead) {
        Ok(())
    } else {
        Err(WorkError::Forbidden)
    }
}

fn audit(caller: &MemberCaller, action: AuditAction, record: Uuid) -> AuditEvent {
    AuditEvent::new(caller.actor(), action, Some(record), Some(caller.scope()))
}

fn page<T>(mut items: Vec<T>, limit: PageLimit, number: impl Fn(&T) -> u64) -> Page<T, WorkCursor> {
    let more = items.len() > limit.get() as usize;
    items.truncate(limit.get() as usize);
    let next = more
        .then(|| items.last())
        .flatten()
        .map(|last| WorkCursor(number(last)));
    Page { items, next }
}

/// The filter of a list as the caller gives it, with the page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WorkQuery<S> {
    pub owner: Option<UserId>,
    pub status: Option<S>,
    pub workstream: Option<WorkstreamId>,
    pub after: Option<WorkCursor>,
    pub limit: PageLimit,
}

impl<S> WorkQuery<S> {
    /// The store filter with one more record, to see if a next page exists.
    fn filter(self) -> WorkFilter<S> {
        WorkFilter {
            owner: self.owner,
            status: self.status,
            workstream: self.workstream,
            after: self.after,
            limit: self.limit.get() + 1,
        }
    }
}

/// Creates an action. A contributor or a manager of the event can do it.
pub async fn create_action(
    caller: &MemberCaller,
    event: EventId,
    input: NewAction,
    ports: WorkPorts<'_>,
) -> Result<ActionView, WorkError> {
    require_create(caller, event, ports.identity).await?;
    let scope = caller.scope();
    let id = ActionId::from_uuid(record_id(input.id)?);
    let mut check = Checker::default();
    let title = parse_title(&mut check, &input.title);
    let description = parse_description(&mut check, input.description.as_deref());
    check_owner(&mut check, scope, event, input.owner, ports.identity).await?;
    if let Some(workstream) = input.workstream {
        check_workstream(&mut check, scope, event, workstream, ports.workstreams).await?;
    }
    check.finish()?;
    let (Some(title), Some(description)) = (title, description) else {
        unreachable!("a checker without errors has all values")
    };
    let action = NewActionRecord {
        id,
        event_id: event,
        fields: ActionFields {
            title,
            description,
            owner: input.owner,
            workstream_id: input.workstream,
            due_date: input.due_date,
            status: ActionStatus::Open,
        },
    };
    let audit = audit(caller, AuditAction::ActionCreate, id.as_uuid());
    match ports
        .work
        .create_action(scope, &action, ports.clock.now(), &audit)
        .await?
    {
        WorkCreated::Created(view) => Ok(view),
        WorkCreated::IdTaken => Err(WorkError::Invalid(vec![FieldError::new("id", "taken")])),
    }
}

/// Changes an action: its owner, the lead of its workstream or an event manager can do it.
pub async fn change_action(
    caller: &MemberCaller,
    event: EventId,
    id: ActionId,
    change: ActionChange,
    ports: WorkPorts<'_>,
) -> Result<ActionView, WorkError> {
    let access = access::event_access(caller, event, ports.identity).await?;
    let scope = caller.scope();
    let current = ports
        .work
        .action(scope, event, id)
        .await?
        .ok_or(WorkError::NotFound)?;
    let old = &current.fields;
    require_change(
        caller,
        access,
        event,
        old.owner,
        old.workstream_id,
        ports.workstreams,
    )
    .await?;
    let ActionChange {
        title,
        description,
        owner,
        workstream,
        due_date,
        status,
        expected_version,
    } = change;
    if title.is_none()
        && description.is_none()
        && owner.is_none()
        && workstream.is_none()
        && due_date.is_none()
        && status.is_none()
    {
        return Err(WorkError::Invalid(Vec::new()));
    }
    if current.version != expected_version {
        return Err(WorkError::VersionConflict);
    }
    let mut check = Checker::default();
    let title = match &title {
        Some(text) => parse_title(&mut check, text),
        None => Some(old.title.clone()),
    };
    let description = match &description {
        Some(text) => parse_description(&mut check, text.as_deref()),
        None => Some(old.description.clone()),
    };
    let owner = owner.unwrap_or(old.owner);
    if owner != old.owner {
        check_owner(&mut check, scope, event, owner, ports.identity).await?;
    }
    let workstream_id = workstream.unwrap_or(old.workstream_id);
    if let Some(new) = workstream_id
        && workstream_id != old.workstream_id
    {
        check_workstream(&mut check, scope, event, new, ports.workstreams).await?;
    }
    check.finish()?;
    let (Some(title), Some(description)) = (title, description) else {
        unreachable!("a checker without errors has all values")
    };
    let status = match status {
        Some(next) if next != old.status && !old.status.can_change_to(next) => {
            return Err(WorkError::InvalidTransition);
        }
        Some(next) => next,
        None => old.status,
    };
    let fields = ActionFields {
        title,
        description,
        owner,
        workstream_id,
        due_date: due_date.unwrap_or(old.due_date),
        status,
    };
    let audit = audit(caller, AuditAction::ActionChange, id.as_uuid());
    match ports
        .work
        .change_action(scope, event, id, &fields, expected_version, &audit)
        .await?
    {
        WorkChanged::Changed(view) => Ok(view),
        WorkChanged::NotFound => Err(WorkError::NotFound),
        WorkChanged::VersionConflict => Err(WorkError::VersionConflict),
    }
}

/// One action of the event. Each reader of the event sees it.
pub async fn get_action(
    caller: &MemberCaller,
    event: EventId,
    id: ActionId,
    identity: &dyn IdentityStore,
    work: &dyn WorkStore,
) -> Result<ActionView, WorkError> {
    access::event_access(caller, event, identity).await?;
    work.action(caller.scope(), event, id)
        .await?
        .ok_or(WorkError::NotFound)
}

/// The actions of the event that match `query`, in the order of their numbers.
pub async fn list_actions(
    caller: &MemberCaller,
    event: EventId,
    query: WorkQuery<ActionStatus>,
    identity: &dyn IdentityStore,
    work: &dyn WorkStore,
) -> Result<Page<ActionView, WorkCursor>, WorkError> {
    access::event_access(caller, event, identity).await?;
    let items = work.actions(caller.scope(), event, &query.filter()).await?;
    Ok(page(items, query.limit, |action| action.local_number))
}

/// The open records of the caller in the events where the caller has a role (spec 2a, section 4).
pub async fn my_work(caller: &MemberCaller, work: &dyn WorkStore) -> Result<MyWorkView, WorkError> {
    let all_events = access::sees_all_events(caller);
    let work = work
        .my_open_work(caller.scope(), caller.user_id(), all_events)
        .await?;
    Ok(MyWorkView {
        work,
        review_count: 0,
    })
}

/// Creates a commitment. A contributor or a manager of the event can do it.
pub async fn create_commitment(
    caller: &MemberCaller,
    event: EventId,
    input: NewCommitment,
    ports: WorkPorts<'_>,
) -> Result<CommitmentView, WorkError> {
    require_create(caller, event, ports.identity).await?;
    let scope = caller.scope();
    let id = CommitmentId::from_uuid(record_id(input.id)?);
    let mut check = Checker::default();
    let text = parse_text(&mut check, &input.text);
    let condition = match input.condition.as_deref() {
        None => Some(None),
        Some(text) => check
            .text("condition", text, ConditionText::parse, text_error_code)
            .map(Some),
    };
    check_promisor(&mut check, scope, input.promisor, ports.parties).await?;
    check_owner(&mut check, scope, event, input.owner, ports.identity).await?;
    if let Some(workstream) = input.workstream {
        check_workstream(&mut check, scope, event, workstream, ports.workstreams).await?;
    }
    check.finish()?;
    let (Some(text), Some(condition)) = (text, condition) else {
        unreachable!("a checker without errors has all values")
    };
    let status = CommitmentStatus::initial(condition.as_ref());
    let commitment = NewCommitmentRecord {
        id,
        event_id: event,
        condition,
        promisor: input.promisor,
        fields: CommitmentFields {
            text,
            owner: input.owner,
            workstream_id: input.workstream,
            due_date: input.due_date,
            status,
            firm_reason: None,
        },
    };
    let audit = audit(caller, AuditAction::CommitmentCreate, id.as_uuid());
    match ports
        .work
        .create_commitment(scope, &commitment, ports.clock.now(), &audit)
        .await?
    {
        WorkCreated::Created(view) => Ok(view),
        WorkCreated::IdTaken => Err(WorkError::Invalid(vec![FieldError::new("id", "taken")])),
    }
}

/// The current commitment, if the caller can change it.
async fn changeable_commitment(
    caller: &MemberCaller,
    event: EventId,
    id: CommitmentId,
    ports: WorkPorts<'_>,
) -> Result<CommitmentView, WorkError> {
    let access = access::event_access(caller, event, ports.identity).await?;
    let current = ports
        .work
        .commitment(caller.scope(), event, id)
        .await?
        .ok_or(WorkError::NotFound)?;
    require_change(
        caller,
        access,
        event,
        current.fields.owner,
        current.fields.workstream_id,
        ports.workstreams,
    )
    .await?;
    Ok(current)
}

async fn store_commitment_change(
    caller: &MemberCaller,
    event: EventId,
    id: CommitmentId,
    fields: &CommitmentFields,
    expected: RecordVersion,
    action: AuditAction,
    ports: WorkPorts<'_>,
) -> Result<CommitmentView, WorkError> {
    let audit = audit(caller, action, id.as_uuid());
    match ports
        .work
        .change_commitment(caller.scope(), event, id, fields, expected, &audit)
        .await?
    {
        WorkChanged::Changed(view) => Ok(view),
        WorkChanged::NotFound => Err(WorkError::NotFound),
        WorkChanged::VersionConflict => Err(WorkError::VersionConflict),
    }
}

/// Changes a commitment: its owner, the lead of its workstream or an event manager can do it.
/// It cannot make a commitment firm: only `make_commitment_firm` does that (ADR 0068).
pub async fn change_commitment(
    caller: &MemberCaller,
    event: EventId,
    id: CommitmentId,
    change: CommitmentChange,
    ports: WorkPorts<'_>,
) -> Result<CommitmentView, WorkError> {
    let current = changeable_commitment(caller, event, id, ports).await?;
    let old = &current.fields;
    let CommitmentChange {
        text,
        owner,
        workstream,
        due_date,
        status,
        expected_version,
    } = change;
    if text.is_none()
        && owner.is_none()
        && workstream.is_none()
        && due_date.is_none()
        && status.is_none()
    {
        return Err(WorkError::Invalid(Vec::new()));
    }
    if current.version != expected_version {
        return Err(WorkError::VersionConflict);
    }
    let scope = caller.scope();
    let mut check = Checker::default();
    let text = match &text {
        Some(text) => parse_text(&mut check, text),
        None => Some(old.text.clone()),
    };
    let owner = owner.unwrap_or(old.owner);
    if owner != old.owner {
        check_owner(&mut check, scope, event, owner, ports.identity).await?;
    }
    let workstream_id = workstream.unwrap_or(old.workstream_id);
    if let Some(new) = workstream_id
        && workstream_id != old.workstream_id
    {
        check_workstream(&mut check, scope, event, new, ports.workstreams).await?;
    }
    check.finish()?;
    let Some(text) = text else {
        unreachable!("a checker without errors has all values")
    };
    let status = match status {
        Some(CommitmentStatus::Firm) => return Err(WorkError::InvalidTransition),
        Some(next) if next != old.status && !old.status.can_change_to(next) => {
            return Err(WorkError::InvalidTransition);
        }
        Some(next) => next,
        None => old.status,
    };
    let fields = CommitmentFields {
        text,
        owner,
        workstream_id,
        due_date: due_date.unwrap_or(old.due_date),
        status,
        firm_reason: old.firm_reason.clone(),
    };
    store_commitment_change(
        caller,
        event,
        id,
        &fields,
        expected_version,
        AuditAction::CommitmentChange,
        ports,
    )
    .await
}

/// Makes a conditional commitment firm, with a reason. The condition stays as history (ADR 0068).
/// The commitment keeps the reason; the audit event records who and when.
pub async fn make_commitment_firm(
    caller: &MemberCaller,
    event: EventId,
    id: CommitmentId,
    input: FirmInput,
    ports: WorkPorts<'_>,
) -> Result<CommitmentView, WorkError> {
    let current = changeable_commitment(caller, event, id, ports).await?;
    let reason = FirmReason::parse(&input.reason).map_err(|error| {
        WorkError::Invalid(vec![FieldError::new("reason", text_error_code(error))])
    })?;
    if current.version != input.expected_version {
        return Err(WorkError::VersionConflict);
    }
    if !current.fields.status.can_change_to(CommitmentStatus::Firm) {
        return Err(WorkError::InvalidTransition);
    }
    let fields = CommitmentFields {
        status: CommitmentStatus::Firm,
        firm_reason: Some(reason),
        ..current.fields
    };
    store_commitment_change(
        caller,
        event,
        id,
        &fields,
        input.expected_version,
        AuditAction::CommitmentFirm,
        ports,
    )
    .await
}

/// One commitment of the event, with its evidence. Each reader of the event sees it.
pub async fn get_commitment(
    caller: &MemberCaller,
    event: EventId,
    id: CommitmentId,
    identity: &dyn IdentityStore,
    work: &dyn WorkStore,
) -> Result<CommitmentView, WorkError> {
    access::event_access(caller, event, identity).await?;
    work.commitment(caller.scope(), event, id)
        .await?
        .ok_or(WorkError::NotFound)
}

/// The commitments of the event that match `query`, in the order of their numbers.
pub async fn list_commitments(
    caller: &MemberCaller,
    event: EventId,
    query: WorkQuery<CommitmentStatus>,
    identity: &dyn IdentityStore,
    work: &dyn WorkStore,
) -> Result<Page<CommitmentView, WorkCursor>, WorkError> {
    access::event_access(caller, event, identity).await?;
    let items = work
        .commitments(caller.scope(), event, &query.filter())
        .await?;
    Ok(page(items, query.limit, |commitment| {
        commitment.local_number
    }))
}
