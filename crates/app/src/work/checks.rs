//! The checks that the commands of actions and commitments share.

use std::collections::HashMap;

use jiff::Timestamp;
use jiff::civil::Date;
use tada_domain::RecordVersion;
use tada_domain::ids::{EventId, MilestoneId, UserId, WorkstreamId};
use tada_domain::parties::Party;
use tada_domain::records::MilestoneStatus;
use tada_domain::work::{ActionTitle, CommitmentText, RecordDescription, dates_in_order};

use super::{ActionView, CommitmentView, MilestoneStore, WorkError, WorkPorts, may_change_work};
use crate::access::{self, EventAccess, EventLevel, Principal};
use crate::caller::{MemberCaller, OrgScope};
use crate::identity::IdentityStore;
use crate::parties::PartyStore;
use crate::problem::FieldError;
use crate::records::{Changed, Checker, Created, RecordRef, Shown, shown, shown_one};
use crate::records::{START_AFTER_DUE, text_error_code};
use crate::store::StoreError;
use crate::workstreams::{ActiveWorkstreamError, WorkstreamStore, active_workstream};

pub(super) fn parse_title(check: &mut Checker, input: &str) -> Option<ActionTitle> {
    check.parse("title", input, ActionTitle::parse, text_error_code)
}

pub(super) fn parse_description(
    check: &mut Checker,
    input: Option<&str>,
) -> Option<Option<RecordDescription>> {
    match input {
        None => Some(None),
        Some(text) => check
            .parse(
                "description",
                text,
                RecordDescription::parse,
                text_error_code,
            )
            .map(Some),
    }
}

pub(super) fn parse_text(check: &mut Checker, input: &str) -> Option<CommitmentText> {
    check.parse("text", input, CommitmentText::parse, text_error_code)
}

/// The owner and the workstream of a work record.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) struct Assignment {
    owner: UserId,
    workstream: Option<WorkstreamId>,
}

impl Assignment {
    pub(super) fn new(owner: UserId, workstream: Option<WorkstreamId>) -> Self {
        Self { owner, workstream }
    }
}

/// Checks the owner and the workstream of a work record: all of a new record (`old` is `None`), and only the changed
/// ones of a change. An owner who lost the role or a workstream that closed after it was set stays.
pub(super) async fn check_assignment(
    check: &mut Checker,
    scope: OrgScope,
    event: EventId,
    old: Option<Assignment>,
    new: Assignment,
    ports: WorkPorts<'_>,
) -> Result<(), StoreError> {
    if old.is_none_or(|old| old.owner != new.owner) {
        check_owner(check, scope, event, new.owner, ports.identity).await?;
    }
    if let Some(workstream) = new.workstream
        && old.is_none_or(|old| old.workstream != new.workstream)
    {
        check_workstream(check, scope, event, workstream, ports.workstreams).await?;
    }
    Ok(())
}

/// Refuses a change that changes no field, then a change of another version than the current one.
pub(super) fn require_any_change(
    any: bool,
    current: RecordVersion,
    expected: RecordVersion,
) -> Result<(), WorkError> {
    if !any {
        return Err(WorkError::Invalid(Vec::new()));
    }
    if current != expected {
        return Err(WorkError::VersionConflict);
    }
    Ok(())
}

/// The owner of a work record must be a contributor or a manager of the event (ADR 0068).
async fn check_owner(
    check: &mut Checker,
    scope: OrgScope,
    event: EventId,
    owner: UserId,
    identity: &dyn IdentityStore,
) -> Result<(), StoreError> {
    if !is_possible_owner(scope, event, owner, identity).await? {
        check.push("owner", "unknown-member");
    }
    Ok(())
}

/// True if `user` can own a work record of the event: a contributor or a manager of the event (ADR 0068).
pub(crate) async fn is_possible_owner(
    scope: OrgScope,
    event: EventId,
    user: UserId,
    identity: &dyn IdentityStore,
) -> Result<bool, StoreError> {
    Ok(access::member_access(scope, event, user, identity)
        .await?
        .is_some_and(EventLevel::can_propose))
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
            check.add(error);
            Ok(())
        }
        Err(ActiveWorkstreamError::Store(error)) => Err(error),
    }
}

/// The start date of an action is not after its due date (ADR 0080).
pub(super) fn check_dates(start: Option<Date>, due: Option<Date>) -> Result<(), FieldError> {
    if dates_in_order(start, due) {
        Ok(())
    } else {
        Err(FieldError::new("start_date", START_AFTER_DUE))
    }
}

/// A new milestone of an action must be a milestone of the event that is not canceled (ADR 0080).
pub(super) async fn check_milestone(
    check: &mut Checker,
    scope: OrgScope,
    event: EventId,
    id: MilestoneId,
    ports: WorkPorts<'_>,
) -> Result<(), StoreError> {
    if let Some(code) =
        milestone_refusal(scope, event, id, ports.milestones, ports.clock.now()).await?
    {
        check.push("milestone", code);
    }
    Ok(())
}

/// The code that refuses `id` as the milestone of an action of the event (`unknown-record`), or `None`:
/// a milestone of another event, an unknown one and a canceled one are refused alike.
pub(crate) async fn milestone_refusal(
    scope: OrgScope,
    event: EventId,
    id: MilestoneId,
    milestones: &dyn MilestoneStore,
    now: Timestamp,
) -> Result<Option<&'static str>, StoreError> {
    let usable = milestones
        .milestone(scope, event, id, now)
        .await?
        .is_some_and(|milestone| milestone.fields.status != MilestoneStatus::Canceled);
    Ok((!usable).then_some("unknown-record"))
}

/// The promisor must be a person or an institution of the organization (ADR 0069).
pub(super) async fn check_promisor(
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

/// The access of the caller to an event that takes writes (ADR 0071).
pub(super) async fn writable_access(
    caller: &MemberCaller,
    event: EventId,
    identity: &dyn IdentityStore,
) -> Result<EventAccess, WorkError> {
    let access = access::event_access(caller, event, identity).await?;
    access::require_writable(&access)?;
    Ok(access)
}

/// The caller must be a contributor or a manager of the event. Returns the access of the caller.
pub(super) async fn require_create(
    caller: &MemberCaller,
    event: EventId,
    identity: &dyn IdentityStore,
) -> Result<EventAccess, WorkError> {
    let access = writable_access(caller, event, identity).await?;
    if access.can_propose() {
        Ok(access)
    } else {
        Err(WorkError::Forbidden)
    }
}

/// The caller must be allowed to change a record with the owner `owner` in the workstream `workstream`.
pub(super) async fn require_change(
    caller: &MemberCaller,
    access: EventAccess,
    event: EventId,
    owner: UserId,
    workstream: Option<WorkstreamId>,
    workstreams: &dyn WorkstreamStore,
) -> Result<(), WorkError> {
    if may_change(caller, access, event, owner, workstream, workstreams).await? {
        Ok(())
    } else {
        Err(WorkError::Forbidden)
    }
}

/// True if the caller can change a record with the owner `owner` in the workstream `workstream` (`may_change_work`).
pub(super) async fn may_change(
    caller: &impl Principal,
    access: EventAccess,
    event: EventId,
    owner: UserId,
    workstream: Option<WorkstreamId>,
    workstreams: &dyn WorkstreamStore,
) -> Result<bool, StoreError> {
    Ok(ChangeRights::of_event(caller, access, event, workstreams)
        .await?
        .allow(owner, workstream))
}

/// What a caller can change in one event: the input of `may_change_work` for each record of the event.
#[derive(Debug)]
pub(crate) struct ChangeRights {
    caller: UserId,
    access: EventAccess,
    /// The lead of each workstream of the event.
    leads: HashMap<WorkstreamId, UserId>,
}

impl ChangeRights {
    pub(crate) async fn of_event(
        caller: &impl Principal,
        access: EventAccess,
        event: EventId,
        workstreams: &dyn WorkstreamStore,
    ) -> Result<Self, StoreError> {
        let leads = workstreams
            .list(caller.scope(), event)
            .await?
            .into_iter()
            .map(|workstream| (workstream.id, workstream.lead))
            .collect();
        Ok(Self {
            caller: caller.user_id(),
            access,
            leads,
        })
    }

    /// True if the caller can change a record with the owner `owner` in the workstream `workstream`.
    pub(crate) fn allow(&self, owner: UserId, workstream: Option<WorkstreamId>) -> bool {
        let lead = workstream.and_then(|id| self.leads.get(&id).copied());
        self.access.is_writable() && may_change_work(self.access.level(), self.caller, owner, lead)
    }
}

/// The parts of an action or a commitment that its view for a caller needs.
pub(crate) trait WorkRecord {
    fn record_ref(&self) -> RecordRef;
    fn owner(&self) -> UserId;
    fn workstream(&self) -> Option<WorkstreamId>;
}

impl WorkRecord for ActionView {
    fn record_ref(&self) -> RecordRef {
        RecordRef::Action(self.id)
    }

    fn owner(&self) -> UserId {
        self.fields.owner
    }

    fn workstream(&self) -> Option<WorkstreamId> {
        self.fields.workstream_id
    }
}

impl WorkRecord for CommitmentView {
    fn record_ref(&self) -> RecordRef {
        RecordRef::Commitment(self.id)
    }

    fn owner(&self) -> UserId {
        self.fields.owner
    }

    fn workstream(&self) -> Option<WorkstreamId> {
        self.fields.workstream_id
    }
}

/// The records of one event as the caller reads them: with their evidence and the right to change each.
pub(super) async fn show<T: WorkRecord>(
    caller: &impl Principal,
    access: EventAccess,
    event: EventId,
    records: Vec<T>,
    ports: WorkPorts<'_>,
) -> Result<Vec<Shown<T>>, StoreError> {
    let rights = ChangeRights::of_event(caller, access, event, ports.workstreams).await?;
    shown(
        caller,
        records,
        WorkRecord::record_ref,
        |record| rights.allow(record.owner(), record.workstream()),
        ports.identity,
        ports.work,
    )
    .await
}

/// One record of an event as the caller reads it (`show`).
pub(super) async fn show_one<T: WorkRecord>(
    caller: &impl Principal,
    access: EventAccess,
    event: EventId,
    record: T,
    ports: WorkPorts<'_>,
) -> Result<Shown<T>, StoreError> {
    let can_change = may_change(
        caller,
        access,
        event,
        record.owner(),
        record.workstream(),
        ports.workstreams,
    )
    .await?;
    let record_ref = record.record_ref();
    shown_one(
        caller,
        record,
        record_ref,
        can_change,
        ports.identity,
        ports.work,
    )
    .await
}

/// The record of a create, or `taken` for its ID.
pub(super) fn created<T>(result: Created<T>) -> Result<T, WorkError> {
    match result {
        Created::Created(view) => Ok(view),
        Created::IdTaken => Err(WorkError::Invalid(vec![FieldError::new("id", "taken")])),
    }
}

/// The record of a change, or why the store did not change it.
pub(super) fn changed<T>(result: Changed<T>) -> Result<T, WorkError> {
    match result {
        Changed::Changed(view) => Ok(view),
        Changed::NotFound => Err(WorkError::NotFound),
        Changed::VersionConflict => Err(WorkError::VersionConflict),
    }
}
