//! The commands and queries of actions (ADR 0068).

use jiff::civil::Date;
use tada_domain::RecordVersion;
use tada_domain::ids::{ActionId, EventId, UserId, WorkstreamId};
use tada_domain::work::ActionStatus;
use uuid::Uuid;

use super::checks::{
    Checker, audit, check_owner, check_workstream, page, parse_description, parse_title, record_id,
    require_change, require_create,
};
use super::{
    ActionFields, ActionView, NewActionRecord, WorkChanged, WorkCreated, WorkCursor, WorkError,
    WorkPorts, WorkQuery, WorkStore,
};
use crate::access;
use crate::audit::AuditAction;
use crate::caller::MemberCaller;
use crate::identity::IdentityStore;
use crate::paging::Page;
use crate::problem::FieldError;

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
