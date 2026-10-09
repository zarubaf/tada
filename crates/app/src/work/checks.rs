//! The checks that the commands of actions and commitments share.

use tada_domain::ids::{self, EventId, UserId, WorkstreamId};
use tada_domain::parties::Party;
use tada_domain::work::{ActionDescription, ActionTitle, CommitmentText};
use uuid::Uuid;

use super::{WorkCursor, WorkError, may_change_work};
use crate::access::{self, EventAccess};
use crate::audit::{AuditAction, AuditEvent};
use crate::caller::{MemberCaller, OrgScope};
use crate::identity::IdentityStore;
use crate::paging::{Page, PageLimit};
use crate::parties::PartyStore;
use crate::problem::FieldError;
use crate::proposals::text_error_code;
use crate::store::StoreError;
use crate::workstreams::{ActiveWorkstreamError, WorkstreamStore, active_workstream};

/// Collects the field errors of one input, so that the caller sees all of them at once.
#[derive(Debug, Default)]
pub(super) struct Checker(Vec<FieldError>);

impl Checker {
    pub(super) fn text<T, E: Copy>(
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

    pub(super) fn push(&mut self, field: &'static str, code: &'static str) {
        self.0.push(FieldError::new(field, code));
    }

    pub(super) fn finish(self) -> Result<(), WorkError> {
        if self.0.is_empty() {
            Ok(())
        } else {
            Err(WorkError::Invalid(self.0))
        }
    }
}

pub(super) fn parse_title(check: &mut Checker, input: &str) -> Option<ActionTitle> {
    check.text("title", input, ActionTitle::parse, text_error_code)
}

pub(super) fn parse_description(
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

pub(super) fn parse_text(check: &mut Checker, input: &str) -> Option<CommitmentText> {
    check.text("text", input, CommitmentText::parse, text_error_code)
}

pub(super) fn record_id(id: Option<Uuid>) -> Result<Uuid, WorkError> {
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
pub(super) async fn check_owner(
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
        .is_some_and(EventAccess::can_propose))
}

/// A new workstream of a record must be active and in the event. A store failure stops the command.
pub(super) async fn check_workstream(
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

/// The caller must be a contributor or a manager of the event.
pub(super) async fn require_create(
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
pub(super) async fn require_change(
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

pub(super) fn audit(caller: &MemberCaller, action: AuditAction, record: Uuid) -> AuditEvent {
    AuditEvent::new(caller.actor(), action, Some(record), Some(caller.scope()))
}

pub(super) fn page<T>(
    mut items: Vec<T>,
    limit: PageLimit,
    number: impl Fn(&T) -> u64,
) -> Page<T, WorkCursor> {
    let more = items.len() > limit.get() as usize;
    items.truncate(limit.get() as usize);
    let next = more
        .then(|| items.last())
        .flatten()
        .map(|last| WorkCursor(number(last)));
    Page { items, next }
}
