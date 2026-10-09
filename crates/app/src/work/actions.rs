//! The commands and queries of actions (ADR 0068).

use jiff::civil::Date;
use tada_domain::RecordVersion;
use tada_domain::ids::{ActionId, EventId, UserId, WorkstreamId};
use tada_domain::work::ActionStatus;
use uuid::Uuid;

use super::checks::{
    changed, check_owner, check_workstream, created, may_change, parse_description, parse_title,
    require_change, require_create, show, show_one,
};
use super::{ActionFields, ActionView, NewActionRecord, WorkError, WorkPorts, WorkQuery};
use crate::access::{self, Principal};
use crate::audit::AuditAction;
use crate::caller::MemberCaller;
use crate::paging::Page;
use crate::records::{Checker, NumberCursor, Shown, audit, page, record_id};

/// The input of `create_action`, as the caller gives it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewAction {
    /// The ID of the new action, a UUIDv7 (ADR 0038). Without it, the command chooses one. An ID that a record holds is `taken`, also on a retry.
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
) -> Result<Shown<ActionView>, WorkError> {
    let access = require_create(caller, event, ports.identity).await?;
    let scope = caller.scope();
    let id =
        ActionId::from_uuid(record_id(input.id).map_err(|error| WorkError::Invalid(vec![error]))?);
    let mut check = Checker::default();
    let title = parse_title(&mut check, &input.title);
    let description = parse_description(&mut check, input.description.as_deref());
    check_owner(&mut check, scope, event, input.owner, ports.identity).await?;
    if let Some(workstream) = input.workstream {
        check_workstream(&mut check, scope, event, workstream, ports.workstreams).await?;
    }
    let (title, description) = check
        .finish(title.zip(description))
        .map_err(WorkError::Invalid)?;
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
    let action = created(
        ports
            .work
            .create_action(scope, &action, ports.clock.now(), &audit)
            .await?,
    )?;
    let can_change = may_change(
        caller,
        access,
        event,
        action.fields.owner,
        action.fields.workstream_id,
        ports.workstreams,
    )
    .await?;
    Ok(Shown::created(action, can_change))
}

/// Changes an action: its owner, the lead of its workstream or an event manager can do it.
pub async fn change_action(
    caller: &MemberCaller,
    event: EventId,
    id: ActionId,
    change: ActionChange,
    ports: WorkPorts<'_>,
) -> Result<Shown<ActionView>, WorkError> {
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
    let (title, description) = check
        .finish(title.zip(description))
        .map_err(WorkError::Invalid)?;
    let status = match status {
        Some(next) => old.status.change_to(next)?,
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
    let action = changed(
        ports
            .work
            .change_action(
                scope,
                event,
                id,
                &fields,
                expected_version,
                ports.clock.now(),
                &audit,
            )
            .await?,
    )?;
    Ok(show_one(caller, access, event, action, ports).await?)
}

/// One action of the event, with its evidence. Each reader of the event sees it.
pub async fn get_action(
    caller: &impl Principal,
    event: EventId,
    id: ActionId,
    ports: WorkPorts<'_>,
) -> Result<Shown<ActionView>, WorkError> {
    let access = access::event_access(caller, event, ports.identity).await?;
    let action = ports
        .work
        .action(caller.scope(), event, id)
        .await?
        .ok_or(WorkError::NotFound)?;
    Ok(show_one(caller, access, event, action, ports).await?)
}

/// The actions of the event that match `query`, in the order of their numbers, with their evidence.
pub async fn list_actions(
    caller: &impl Principal,
    event: EventId,
    query: WorkQuery<ActionStatus>,
    ports: WorkPorts<'_>,
) -> Result<Page<Shown<ActionView>, NumberCursor>, WorkError> {
    let access = access::event_access(caller, event, ports.identity).await?;
    let items = ports
        .work
        .actions(caller.scope(), event, &query.filter())
        .await?;
    let Page { items, next } = page(items, query.limit, |action| action.local_number);
    let items = show(caller, access, event, items, ports).await?;
    Ok(Page { items, next })
}
