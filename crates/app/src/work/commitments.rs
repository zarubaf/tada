//! The commands and queries of commitments (ADR 0068).

use jiff::civil::Date;
use tada_domain::RecordVersion;
use tada_domain::ids::{CommitmentId, EventId, UserId, WorkstreamId};
use tada_domain::parties::Party;
use tada_domain::work::{CommitmentStatus, ConditionText, FirmReason};
use uuid::Uuid;

use super::checks::{
    changed, check_owner, check_promisor, check_workstream, created, may_change, parse_text,
    require_change, require_create, show, show_one,
};
use super::{
    CommitmentFields, CommitmentView, NewCommitmentRecord, WorkError, WorkPorts, WorkQuery,
};
use crate::access::{self, EventAccess, Principal};
use crate::audit::AuditAction;
use crate::caller::MemberCaller;
use crate::paging::Page;
use crate::problem::FieldError;
use crate::proposals::text_error_code;
use crate::records::{Checker, NumberCursor, Shown, audit, page, record_id};

/// The input of `create_commitment`, as the caller gives it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewCommitment {
    /// The ID of the new commitment, a UUIDv7 (ADR 0038). Without it, the command chooses one. An ID that a record holds is `taken`, also on a retry.
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

/// Creates a commitment. A contributor or a manager of the event can do it.
pub async fn create_commitment(
    caller: &MemberCaller,
    event: EventId,
    input: NewCommitment,
    ports: WorkPorts<'_>,
) -> Result<Shown<CommitmentView>, WorkError> {
    let access = require_create(caller, event, ports.identity).await?;
    let scope = caller.scope();
    let id = CommitmentId::from_uuid(
        record_id(input.id).map_err(|error| WorkError::Invalid(vec![error]))?,
    );
    let mut check = Checker::default();
    let text = parse_text(&mut check, &input.text);
    let condition = match input.condition.as_deref() {
        None => Some(None),
        Some(text) => check
            .parse("condition", text, ConditionText::parse, text_error_code)
            .map(Some),
    };
    check_promisor(&mut check, scope, input.promisor, ports.parties).await?;
    check_owner(&mut check, scope, event, input.owner, ports.identity).await?;
    if let Some(workstream) = input.workstream {
        check_workstream(&mut check, scope, event, workstream, ports.workstreams).await?;
    }
    let (text, condition) = check
        .finish(text.zip(condition))
        .map_err(WorkError::Invalid)?;
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
    let commitment = created(
        ports
            .work
            .create_commitment(scope, &commitment, ports.clock.now(), &audit)
            .await?,
    )?;
    let can_change = may_change(
        caller,
        access,
        event,
        commitment.fields.owner,
        commitment.fields.workstream_id,
        ports.workstreams,
    )
    .await?;
    Ok(Shown::created(commitment, can_change))
}

/// The current commitment and the access of the caller, if the caller can change the commitment.
async fn changeable_commitment(
    caller: &MemberCaller,
    event: EventId,
    id: CommitmentId,
    ports: WorkPorts<'_>,
) -> Result<(EventAccess, CommitmentView), WorkError> {
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
    Ok((access, current))
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
    changed(
        ports
            .work
            .change_commitment(
                caller.scope(),
                event,
                id,
                fields,
                expected,
                ports.clock.now(),
                &audit,
            )
            .await?,
    )
}

/// Changes a commitment: its owner, the lead of its workstream or an event manager can do it.
/// It cannot make a commitment firm: only `make_commitment_firm` does that (ADR 0068).
pub async fn change_commitment(
    caller: &MemberCaller,
    event: EventId,
    id: CommitmentId,
    change: CommitmentChange,
    ports: WorkPorts<'_>,
) -> Result<Shown<CommitmentView>, WorkError> {
    let (access, current) = changeable_commitment(caller, event, id, ports).await?;
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
    let text = check.finish(text).map_err(WorkError::Invalid)?;
    let status = match status {
        Some(next) => old.status.change_directly(next)?,
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
    let commitment = store_commitment_change(
        caller,
        event,
        id,
        &fields,
        expected_version,
        AuditAction::CommitmentChange,
        ports,
    )
    .await?;
    Ok(show_one(caller, access, event, commitment, ports).await?)
}

/// Makes a conditional commitment firm, with a reason. The condition stays as history (ADR 0068).
/// The commitment keeps the reason; the audit event records who and when.
pub async fn make_commitment_firm(
    caller: &MemberCaller,
    event: EventId,
    id: CommitmentId,
    input: FirmInput,
    ports: WorkPorts<'_>,
) -> Result<Shown<CommitmentView>, WorkError> {
    let (access, current) = changeable_commitment(caller, event, id, ports).await?;
    let reason = FirmReason::parse(&input.reason).map_err(|error| {
        WorkError::Invalid(vec![FieldError::new("reason", text_error_code(error))])
    })?;
    if current.version != input.expected_version {
        return Err(WorkError::VersionConflict);
    }
    let fields = CommitmentFields {
        status: current.fields.status.change_to(CommitmentStatus::Firm)?,
        firm_reason: Some(reason),
        ..current.fields
    };
    let commitment = store_commitment_change(
        caller,
        event,
        id,
        &fields,
        input.expected_version,
        AuditAction::CommitmentFirm,
        ports,
    )
    .await?;
    Ok(show_one(caller, access, event, commitment, ports).await?)
}

/// One commitment of the event, with its evidence. Each reader of the event sees it.
pub async fn get_commitment(
    caller: &impl Principal,
    event: EventId,
    id: CommitmentId,
    ports: WorkPorts<'_>,
) -> Result<Shown<CommitmentView>, WorkError> {
    let access = access::event_access(caller, event, ports.identity).await?;
    let commitment = ports
        .work
        .commitment(caller.scope(), event, id)
        .await?
        .ok_or(WorkError::NotFound)?;
    Ok(show_one(caller, access, event, commitment, ports).await?)
}

/// The commitments of the event that match `query`, in the order of their numbers, with their evidence.
pub async fn list_commitments(
    caller: &impl Principal,
    event: EventId,
    query: WorkQuery<CommitmentStatus>,
    ports: WorkPorts<'_>,
) -> Result<Page<Shown<CommitmentView>, NumberCursor>, WorkError> {
    let access = access::event_access(caller, event, ports.identity).await?;
    let items = ports
        .work
        .commitments(caller.scope(), event, &query.filter())
        .await?;
    let Page { items, next } = page(items, query.limit, |commitment| commitment.local_number);
    let items = show(caller, access, event, items, ports).await?;
    Ok(Page { items, next })
}
