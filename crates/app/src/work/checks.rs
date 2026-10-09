//! The checks that the commands of actions and commitments share.

use tada_domain::ids::{EventId, UserId, WorkstreamId};
use tada_domain::parties::Party;
use tada_domain::work::{ActionDescription, ActionTitle, CommitmentText};

use super::{WorkError, may_change_work};
use crate::access::{self, EventAccess};
use crate::caller::{MemberCaller, OrgScope};
use crate::identity::IdentityStore;
use crate::parties::PartyStore;
use crate::problem::FieldError;
use crate::proposals::text_error_code;
use crate::records::{Changed, Checker, Created};
use crate::store::StoreError;
use crate::workstreams::{ActiveWorkstreamError, WorkstreamStore, active_workstream};

pub(super) fn parse_title(check: &mut Checker, input: &str) -> Option<ActionTitle> {
    check.parse("title", input, ActionTitle::parse, text_error_code)
}

pub(super) fn parse_description(
    check: &mut Checker,
    input: Option<&str>,
) -> Option<Option<ActionDescription>> {
    match input {
        None => Some(None),
        Some(text) => check
            .parse(
                "description",
                text,
                ActionDescription::parse,
                text_error_code,
            )
            .map(Some),
    }
}

pub(super) fn parse_text(check: &mut Checker, input: &str) -> Option<CommitmentText> {
    check.parse("text", input, CommitmentText::parse, text_error_code)
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
            check.add(error);
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
