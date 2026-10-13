//! Milestones (ADR 0080): the dated points of the plan of an event, with their direct commands.
//! They follow the rules of actions: a contributor or a manager creates one, and its owner, the lead of its
//! workstream or an event manager changes it.

use std::fmt::Debug;

use async_trait::async_trait;
use jiff::Timestamp;
use jiff::civil::Date;
use tada_domain::RecordVersion;
use tada_domain::ids::{ActionId, EventId, LocalIdKind, MilestoneId, UserId, WorkstreamId};
use tada_domain::records::{MilestoneStatus, MilestoneTitle};
use tada_domain::redaction::Redactable;
use tada_domain::work::RecordDescription;
use uuid::Uuid;

use super::checks::{
    Assignment, WorkRecord, changed, check_assignment, created, may_change, parse_description,
    require_any_change, require_change, require_create, show, show_one, writable_access,
};
use super::{WorkError, WorkFilter, WorkPorts, WorkQuery};
use crate::access::{self, Principal};
use crate::audit::{AuditAction, AuditEvent};
use crate::caller::{MemberCaller, OrgScope};
use crate::paging::Page;
use crate::records::text_error_code;
use crate::records::{
    Changed, Checker, Created, NumberCursor, RecordRef, Shown, audit, page, record_id,
};
use crate::store::StoreError;

/// A milestone of an event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MilestoneView {
    pub id: MilestoneId,
    /// The number in `MIL-001`, local to the event (ADR 0038).
    pub local_number: u64,
    pub event_id: EventId,
    pub fields: MilestoneFields,
    pub version: RecordVersion,
    /// The warnings at the time of the read. tada calculates them and does not store them (ADR 0080).
    pub warnings: Vec<MilestoneWarning>,
}

impl MilestoneView {
    /// The readable ID, for example `MIL-001`.
    pub fn local_id(&self) -> String {
        LocalIdKind::Milestone.readable_id(self.local_number)
    }
}

/// The values of a milestone that a change replaces.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MilestoneFields {
    /// A legal redaction removes the title and the description (ADR 0072).
    pub title: Redactable<MilestoneTitle>,
    pub description: Option<RecordDescription>,
    pub date: Date,
    /// A member of the event with the contributor or manager role.
    pub owner: UserId,
    pub workstream_id: Option<WorkstreamId>,
    pub status: MilestoneStatus,
}

/// A warning about a milestone (ADR 0080).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MilestoneWarning {
    /// The milestone is open and its date is before today in the time zone of the event.
    Overdue,
    /// An action of the milestone is open, in progress or blocked, and it is due after the date of the milestone.
    LateAction(ActionId),
}

/// A new milestone, checked, before the store gives it its number.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewMilestoneRecord {
    pub id: MilestoneId,
    pub event_id: EventId,
    pub fields: MilestoneFields,
}

/// The repository port for milestones. Each method stays inside `scope`.
/// A create or a change records its audit event in the same transaction.
/// Each view carries its warnings: `at` or `now` gives "today" in the time zone of the event.
#[async_trait]
pub trait MilestoneStore: Debug + Send + Sync {
    /// Inserts a milestone with the version 1 and gives it the next number of its event (ADR 0038).
    async fn create_milestone(
        &self,
        scope: OrgScope,
        milestone: &NewMilestoneRecord,
        at: Timestamp,
        audit: &AuditEvent,
    ) -> Result<Created<MilestoneView>, StoreError>;

    /// Replaces the values of a milestone if its version is `expected`, and counts the version up.
    #[expect(
        clippy::too_many_arguments,
        reason = "a write port takes the scope, the record, the values, the version, the time and the audit event"
    )]
    async fn change_milestone(
        &self,
        scope: OrgScope,
        event: EventId,
        id: MilestoneId,
        fields: &MilestoneFields,
        expected: RecordVersion,
        at: Timestamp,
        audit: &AuditEvent,
    ) -> Result<Changed<MilestoneView>, StoreError>;

    async fn milestone(
        &self,
        scope: OrgScope,
        event: EventId,
        id: MilestoneId,
        now: Timestamp,
    ) -> Result<Option<MilestoneView>, StoreError>;

    /// At most `filter.limit` milestones of the event in the order of their numbers.
    async fn milestones(
        &self,
        scope: OrgScope,
        event: EventId,
        filter: &WorkFilter<MilestoneStatus>,
        now: Timestamp,
    ) -> Result<Vec<MilestoneView>, StoreError>;
}

impl WorkRecord for MilestoneView {
    fn record_ref(&self) -> RecordRef {
        RecordRef::Milestone(self.id)
    }

    fn owner(&self) -> UserId {
        self.fields.owner
    }

    fn workstream(&self) -> Option<WorkstreamId> {
        self.fields.workstream_id
    }
}

impl Shown<MilestoneView> {
    /// The statuses that the caller can set with a change now: none without the right to change the milestone.
    pub fn next_statuses(&self) -> Vec<MilestoneStatus> {
        if self.can_change {
            self.record.fields.status.next_statuses()
        } else {
            Vec::new()
        }
    }
}

/// The input of `create_milestone`, as the caller gives it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewMilestone {
    /// The ID of the new milestone, a UUIDv7 (ADR 0038). Without it, the command chooses one. An ID that a record holds is `taken`, also on a retry.
    pub id: Option<Uuid>,
    pub title: String,
    pub description: Option<String>,
    pub date: Date,
    pub owner: UserId,
    pub workstream: Option<WorkstreamId>,
}

/// The input of `change_milestone`. A field that is `None` stays as it is; `Some(None)` clears it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MilestoneChange {
    pub title: Option<String>,
    pub description: Option<Option<String>>,
    pub date: Option<Date>,
    pub owner: Option<UserId>,
    pub workstream: Option<Option<WorkstreamId>>,
    pub status: Option<MilestoneStatus>,
    pub expected_version: RecordVersion,
}

fn parse_title(check: &mut Checker, input: &str) -> Option<MilestoneTitle> {
    check.parse("title", input, MilestoneTitle::parse, text_error_code)
}

/// Creates a milestone. A contributor or a manager of the event can do it.
pub async fn create_milestone(
    caller: &MemberCaller,
    event: EventId,
    input: NewMilestone,
    ports: WorkPorts<'_>,
) -> Result<Shown<MilestoneView>, WorkError> {
    let access = require_create(caller, event, ports.identity).await?;
    let scope = caller.scope();
    let id = MilestoneId::from_uuid(
        record_id(input.id).map_err(|error| WorkError::Invalid(vec![error]))?,
    );
    let mut check = Checker::default();
    let title = parse_title(&mut check, &input.title);
    let description = parse_description(&mut check, input.description.as_deref());
    let assigned = Assignment::new(input.owner, input.workstream);
    check_assignment(&mut check, scope, event, None, assigned, ports).await?;
    let (title, description) = check
        .finish(title.zip(description))
        .map_err(WorkError::Invalid)?;
    let milestone = NewMilestoneRecord {
        id,
        event_id: event,
        fields: MilestoneFields {
            title: Redactable::Present(title),
            description,
            date: input.date,
            owner: input.owner,
            workstream_id: input.workstream,
            status: MilestoneStatus::Open,
        },
    };
    let audit = audit(caller, AuditAction::MilestoneCreate, id.as_uuid());
    let milestone = created(
        ports
            .milestones
            .create_milestone(scope, &milestone, ports.clock.now(), &audit)
            .await?,
    )?;
    let can_change = may_change(
        caller,
        access,
        event,
        milestone.fields.owner,
        milestone.fields.workstream_id,
        ports.workstreams,
    )
    .await?;
    Ok(Shown::created(milestone, can_change))
}

/// Changes a milestone: its owner, the lead of its workstream or an event manager can do it.
pub async fn change_milestone(
    caller: &MemberCaller,
    event: EventId,
    id: MilestoneId,
    change: MilestoneChange,
    ports: WorkPorts<'_>,
) -> Result<Shown<MilestoneView>, WorkError> {
    let access = writable_access(caller, event, ports.identity).await?;
    let scope = caller.scope();
    let now = ports.clock.now();
    let current = ports
        .milestones
        .milestone(scope, event, id, now)
        .await?
        .ok_or(WorkError::NotFound)?;
    let old = &current.fields;
    // A redacted milestone has no content to change (ADR 0072).
    let Redactable::Present(old_title) = &old.title else {
        return Err(WorkError::InvalidTransition);
    };
    require_change(
        caller,
        access,
        event,
        old.owner,
        old.workstream_id,
        ports.workstreams,
    )
    .await?;
    let MilestoneChange {
        title,
        description,
        date,
        owner,
        workstream,
        status,
        expected_version,
    } = change;
    let any = title.is_some()
        || description.is_some()
        || date.is_some()
        || owner.is_some()
        || workstream.is_some()
        || status.is_some();
    require_any_change(any, current.version, expected_version)?;
    let mut check = Checker::default();
    let title = match &title {
        Some(text) => parse_title(&mut check, text),
        None => Some(old_title.clone()),
    };
    let description = match &description {
        Some(text) => parse_description(&mut check, text.as_deref()),
        None => Some(old.description.clone()),
    };
    let owner = owner.unwrap_or(old.owner);
    let workstream_id = workstream.unwrap_or(old.workstream_id);
    let assigned = Assignment::new(owner, workstream_id);
    let before = Assignment::new(old.owner, old.workstream_id);
    check_assignment(&mut check, scope, event, Some(before), assigned, ports).await?;
    let (title, description) = check
        .finish(title.zip(description))
        .map_err(WorkError::Invalid)?;
    let status = match status {
        Some(next) => old.status.change_to(next)?,
        None => old.status,
    };
    let fields = MilestoneFields {
        title: Redactable::Present(title),
        description,
        date: date.unwrap_or(old.date),
        owner,
        workstream_id,
        status,
    };
    let audit = audit(caller, AuditAction::MilestoneChange, id.as_uuid());
    let milestone = changed(
        ports
            .milestones
            .change_milestone(scope, event, id, &fields, expected_version, now, &audit)
            .await?,
    )?;
    Ok(show_one(caller, access, event, milestone, ports).await?)
}

/// One milestone of the event, with its evidence and its warnings. Each reader of the event sees it.
pub async fn get_milestone(
    caller: &impl Principal,
    event: EventId,
    id: MilestoneId,
    ports: WorkPorts<'_>,
) -> Result<Shown<MilestoneView>, WorkError> {
    let access = access::event_access(caller, event, ports.identity).await?;
    let milestone = ports
        .milestones
        .milestone(caller.scope(), event, id, ports.clock.now())
        .await?
        .ok_or(WorkError::NotFound)?;
    Ok(show_one(caller, access, event, milestone, ports).await?)
}

/// The milestones of the event that match `query`, in the order of their numbers, with their evidence and warnings.
pub async fn list_milestones(
    caller: &impl Principal,
    event: EventId,
    query: WorkQuery<MilestoneStatus>,
    ports: WorkPorts<'_>,
) -> Result<Page<Shown<MilestoneView>, NumberCursor>, WorkError> {
    let access = access::event_access(caller, event, ports.identity).await?;
    let items = ports
        .milestones
        .milestones(caller.scope(), event, &query.filter(), ports.clock.now())
        .await?;
    let Page { items, next } = page(items, query.limit, |milestone| milestone.local_number);
    let items = show(caller, access, event, items, ports).await?;
    Ok(Page { items, next })
}
