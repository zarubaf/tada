//! Decisions (ADR 0081): what the club decided, when, who decided it, and what replaced it.
//!
//! Only an event manager creates, supersedes or revokes a decision, or changes its owner or workstream.
//! A contributor can own a decision and follow it up. The wording, the date and the parties never change:
//! a changed decision is a new decision that supersedes the old one.

use std::fmt::Debug;

use async_trait::async_trait;
use jiff::Timestamp;
use jiff::civil::Date;
use tada_domain::RecordVersion;
use tada_domain::ids::{DecisionId, EventId, LocalIdKind, UserId, WorkstreamId};
use tada_domain::parties::Party;
use tada_domain::records::{
    DecisionChange, DecisionParties, DecisionStatus, DecisionWording, RecordReason,
};
use tada_domain::redaction::Redactable;
use uuid::Uuid;

use super::checks::{
    Assignment, WorkRecord, changed, check_assignment, require_any_change, show, show_created,
    show_one, writable_access,
};
use super::{WorkError, WorkFilter, WorkPorts, WorkQuery};
use crate::access::{self, EventAccess, Principal};
use crate::audit::{AuditAction, AuditEvent};
use crate::caller::{MemberCaller, OrgScope};
use crate::paging::Page;
use crate::parties::PartyRef;
use crate::records::{
    Changed, Checker, Created, NumberCursor, RecordRef, Shown, audit, page, parties_code,
    record_id, text_error_code,
};
use crate::store::StoreError;

/// A decision of an event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecisionView {
    pub id: DecisionId,
    /// The number in `DEC-001`, local to the event (ADR 0038).
    pub local_number: u64,
    pub event_id: EventId,
    /// A legal redaction removes the wording (ADR 0072).
    pub wording: Redactable<DecisionWording>,
    pub decided_on: Date,
    /// The persons and institutions that decided, in their order. A legal redaction of a party removes its name.
    pub parties: Vec<PartyRef>,
    pub fields: DecisionFields,
    pub state: DecisionState,
    pub version: RecordVersion,
}

impl DecisionView {
    /// The readable ID, for example `DEC-001`.
    pub fn local_id(&self) -> String {
        LocalIdKind::Decision.readable_id(self.local_number)
    }
}

/// The values of a decision that a change replaces: only the owner and the workstream.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DecisionFields {
    /// A member of the event with the contributor or manager role, who follows the decision up.
    pub owner: UserId,
    pub workstream_id: Option<WorkstreamId>,
}

/// The status of a decision with what the status needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecisionState {
    InForce,
    /// A newer decision of the same event replaced it.
    Superseded {
        by: DecisionId,
    },
    /// A legal redaction removes the reason (ADR 0072).
    Revoked {
        reason: Redactable<RecordReason>,
    },
}

impl DecisionState {
    pub fn status(&self) -> DecisionStatus {
        match self {
            Self::InForce => DecisionStatus::InForce,
            Self::Superseded { .. } => DecisionStatus::Superseded,
            Self::Revoked { .. } => DecisionStatus::Revoked,
        }
    }
}

/// A new decision, checked, before the store gives it its number. It starts `in-force`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewDecisionRecord {
    pub id: DecisionId,
    pub event_id: EventId,
    pub wording: DecisionWording,
    pub decided_on: Date,
    pub parties: DecisionParties,
    pub fields: DecisionFields,
}

/// The old decision and its successor after a supersede.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Superseded {
    pub old: DecisionView,
    pub successor: DecisionView,
}

/// The repository port for decisions. Each method stays inside `scope`.
/// A create or a change records its audit events in the same transaction.
#[async_trait]
pub trait DecisionStore: Debug + Send + Sync {
    /// Inserts a decision in force with the version 1 and its parties, and gives it the next number of its event.
    async fn create_decision(
        &self,
        scope: OrgScope,
        decision: &NewDecisionRecord,
        at: Timestamp,
        audit: &AuditEvent,
    ) -> Result<Created<DecisionView>, StoreError>;

    /// Replaces the owner and the workstream of a decision if its version is `expected`, and counts the version up.
    #[expect(
        clippy::too_many_arguments,
        reason = "a write port takes the scope, the record, the values, the version, the time and the audit event"
    )]
    async fn change_decision(
        &self,
        scope: OrgScope,
        event: EventId,
        id: DecisionId,
        fields: &DecisionFields,
        expected: RecordVersion,
        at: Timestamp,
        audit: &AuditEvent,
    ) -> Result<Changed<DecisionView>, StoreError>;

    /// In one transaction: inserts `successor` like `create_decision`, then supersedes `old` with it if `old` is in
    /// force and has the version `expected`. If `old` does not change, the successor does not stay either.
    /// `audit` holds the events of both.
    #[expect(
        clippy::too_many_arguments,
        reason = "a write port takes the scope, the record, the values, the version, the time and the audit events"
    )]
    async fn supersede_decision(
        &self,
        scope: OrgScope,
        event: EventId,
        old: DecisionId,
        successor: &NewDecisionRecord,
        expected: RecordVersion,
        at: Timestamp,
        audit: &[AuditEvent],
    ) -> Result<Created<Changed<Superseded>>, StoreError>;

    /// Revokes a decision in force if its version is `expected`, and counts the version up.
    #[expect(
        clippy::too_many_arguments,
        reason = "a write port takes the scope, the record, the values, the version, the time and the audit event"
    )]
    async fn revoke_decision(
        &self,
        scope: OrgScope,
        event: EventId,
        id: DecisionId,
        reason: &RecordReason,
        expected: RecordVersion,
        at: Timestamp,
        audit: &AuditEvent,
    ) -> Result<Changed<DecisionView>, StoreError>;

    async fn decision(
        &self,
        scope: OrgScope,
        event: EventId,
        id: DecisionId,
    ) -> Result<Option<DecisionView>, StoreError>;

    /// At most `filter.limit` decisions of the event in the order of their numbers.
    async fn decisions(
        &self,
        scope: OrgScope,
        event: EventId,
        filter: &WorkFilter<DecisionStatus>,
    ) -> Result<Vec<DecisionView>, StoreError>;
}

impl WorkRecord for DecisionView {
    fn managers_only(&self) -> bool {
        true
    }

    fn record_ref(&self) -> RecordRef {
        RecordRef::Decision(self.id)
    }

    fn owner(&self) -> UserId {
        self.fields.owner
    }

    fn workstream(&self) -> Option<WorkstreamId> {
        self.fields.workstream_id
    }
}

impl Shown<DecisionView> {
    /// True if the caller can supersede or revoke the decision now: an event manager, and a decision in force.
    pub fn can_end(&self) -> bool {
        self.can_change && self.record.state == DecisionState::InForce
    }
}

/// The input of `create_decision`, and the successor of `supersede_decision`, as the caller gives it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewDecision {
    /// The ID of the new decision, a UUIDv7 (ADR 0038). Without it, the command chooses one. An ID that a record holds
    /// is `taken`, also on a retry.
    pub id: Option<Uuid>,
    /// The words of the decision. It can have line breaks.
    pub wording: String,
    pub decided_on: Date,
    /// The persons and institutions of the organization that decided.
    pub decided_by: Vec<Party>,
    pub owner: UserId,
    pub workstream: Option<WorkstreamId>,
}

/// The input of `change_decision`. A field that is `None` stays as it is; `Some(None)` clears the workstream.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DecisionUpdate {
    pub owner: Option<UserId>,
    pub workstream: Option<Option<WorkstreamId>>,
    pub expected_version: RecordVersion,
}

/// The input of `revoke_decision`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Revocation {
    /// Why the decision ends: 1 to 500 characters.
    pub reason: String,
    pub expected_version: RecordVersion,
}

/// The caller must be an event manager of an event that takes writes (ADR 0081). Returns the access of the caller.
async fn require_manager(
    caller: &MemberCaller,
    event: EventId,
    ports: WorkPorts<'_>,
) -> Result<EventAccess, WorkError> {
    let access = writable_access(caller, event, ports.identity).await?;
    if access.can_review() {
        Ok(access)
    } else {
        Err(WorkError::Forbidden)
    }
}

/// Checks the input of a new decision like each command that creates one: the wording, the parties of the
/// organization, the owner and the workstream.
async fn check_new_decision(
    scope: OrgScope,
    event: EventId,
    input: NewDecision,
    ports: WorkPorts<'_>,
) -> Result<NewDecisionRecord, WorkError> {
    let id = DecisionId::from_uuid(
        record_id(input.id).map_err(|error| WorkError::Invalid(vec![error]))?,
    );
    let mut check = Checker::default();
    let wording = check.parse(
        "wording",
        &input.wording,
        DecisionWording::parse,
        text_error_code,
    );
    let parties = match DecisionParties::new(input.decided_by) {
        Ok(parties) => Some(parties),
        Err(error) => {
            check.push("decided_by", parties_code(error));
            None
        }
    };
    if let Some(parties) = &parties
        && !all_parties_exist(scope, parties, ports).await?
    {
        check.push("decided_by", "unknown-record");
    }
    let assigned = Assignment::new(input.owner, input.workstream);
    check_assignment(&mut check, scope, event, None, assigned, ports).await?;
    let (wording, parties) = check
        .finish(wording.zip(parties))
        .map_err(WorkError::Invalid)?;
    Ok(NewDecisionRecord {
        id,
        event_id: event,
        wording,
        decided_on: input.decided_on,
        parties,
        fields: DecisionFields {
            owner: input.owner,
            workstream_id: input.workstream,
        },
    })
}

/// True if each party is a person or an institution of the organization (ADR 0069).
async fn all_parties_exist(
    scope: OrgScope,
    parties: &DecisionParties,
    ports: WorkPorts<'_>,
) -> Result<bool, StoreError> {
    for party in parties.as_slice() {
        let known = match *party {
            Party::Person(id) => ports.parties.person(scope, id).await?.is_some(),
            Party::Institution(id) => ports.parties.institution(scope, id).await?.is_some(),
        };
        if !known {
            return Ok(false);
        }
    }
    Ok(true)
}

/// The current decision for a change, or why it cannot change: unknown, redacted (ADR 0072), or another version.
async fn current_decision(
    caller: &MemberCaller,
    event: EventId,
    id: DecisionId,
    expected: RecordVersion,
    ports: WorkPorts<'_>,
) -> Result<DecisionView, WorkError> {
    let current = ports
        .decisions
        .decision(caller.scope(), event, id)
        .await?
        .ok_or(WorkError::NotFound)?;
    if current.wording.as_present().is_none() {
        return Err(WorkError::InvalidTransition);
    }
    if current.version != expected {
        return Err(WorkError::VersionConflict);
    }
    Ok(current)
}

/// Records a decision. Only an event manager can do it.
pub async fn create_decision(
    caller: &MemberCaller,
    event: EventId,
    input: NewDecision,
    ports: WorkPorts<'_>,
) -> Result<Shown<DecisionView>, WorkError> {
    let access = require_manager(caller, event, ports).await?;
    let scope = caller.scope();
    let decision = check_new_decision(scope, event, input, ports).await?;
    let audit = audit(caller, AuditAction::DecisionCreate, decision.id.as_uuid());
    let result = ports
        .decisions
        .create_decision(scope, &decision, ports.clock.now(), &audit)
        .await?;
    show_created(caller, access, event, result, ports).await
}

/// Changes the owner or the workstream of a decision. Only an event manager can do it.
/// The wording, the date and the parties never change.
pub async fn change_decision(
    caller: &MemberCaller,
    event: EventId,
    id: DecisionId,
    update: DecisionUpdate,
    ports: WorkPorts<'_>,
) -> Result<Shown<DecisionView>, WorkError> {
    let access = require_manager(caller, event, ports).await?;
    let scope = caller.scope();
    let current = ports
        .decisions
        .decision(scope, event, id)
        .await?
        .ok_or(WorkError::NotFound)?;
    // A redacted decision has no content to change (ADR 0072).
    if current.wording.as_present().is_none() {
        return Err(WorkError::InvalidTransition);
    }
    let DecisionUpdate {
        owner,
        workstream,
        expected_version,
    } = update;
    require_any_change(
        owner.is_some() || workstream.is_some(),
        current.version,
        expected_version,
    )?;
    let old = current.fields;
    let fields = DecisionFields {
        owner: owner.unwrap_or(old.owner),
        workstream_id: workstream.unwrap_or(old.workstream_id),
    };
    let mut check = Checker::default();
    let assigned = Assignment::new(fields.owner, fields.workstream_id);
    let before = Assignment::new(old.owner, old.workstream_id);
    check_assignment(&mut check, scope, event, Some(before), assigned, ports).await?;
    check.finish(Some(())).map_err(WorkError::Invalid)?;
    let audit = audit(caller, AuditAction::DecisionChange, id.as_uuid());
    let decision = changed(
        ports
            .decisions
            .change_decision(
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
    Ok(show_one(caller, access, event, decision, ports).await?)
}

/// Records a new decision that replaces the decision `id`, in one step. Only an event manager can do it.
/// The old decision must be in force. Returns the new decision.
pub async fn supersede_decision(
    caller: &MemberCaller,
    event: EventId,
    id: DecisionId,
    successor: NewDecision,
    expected_version: RecordVersion,
    ports: WorkPorts<'_>,
) -> Result<Shown<DecisionView>, WorkError> {
    let access = require_manager(caller, event, ports).await?;
    let scope = caller.scope();
    let current = current_decision(caller, event, id, expected_version, ports).await?;
    let successor = check_new_decision(scope, event, successor, ports).await?;
    current.state.status().apply(&DecisionChange::Supersede {
        successor: successor.id,
    })?;
    let audit = [
        audit(caller, AuditAction::DecisionCreate, successor.id.as_uuid()),
        audit(caller, AuditAction::DecisionSupersede, id.as_uuid()),
    ];
    let result = ports
        .decisions
        .supersede_decision(
            scope,
            event,
            id,
            &successor,
            expected_version,
            ports.clock.now(),
            &audit,
        )
        .await?;
    let superseded = match result {
        Created::Created(result) => Created::Created(changed(result)?.successor),
        Created::IdTaken => Created::IdTaken,
    };
    show_created(caller, access, event, superseded, ports).await
}

/// Revokes a decision in force for a reason. Only an event manager can do it.
pub async fn revoke_decision(
    caller: &MemberCaller,
    event: EventId,
    id: DecisionId,
    revocation: Revocation,
    ports: WorkPorts<'_>,
) -> Result<Shown<DecisionView>, WorkError> {
    let access = require_manager(caller, event, ports).await?;
    let scope = caller.scope();
    let mut check = Checker::default();
    let reason = check.parse(
        "reason",
        &revocation.reason,
        RecordReason::parse,
        text_error_code,
    );
    let reason = check.finish(reason).map_err(WorkError::Invalid)?;
    let current = current_decision(caller, event, id, revocation.expected_version, ports).await?;
    current.state.status().apply(&DecisionChange::Revoke {
        reason: reason.clone(),
    })?;
    let audit = audit(caller, AuditAction::DecisionRevoke, id.as_uuid());
    let decision = changed(
        ports
            .decisions
            .revoke_decision(
                scope,
                event,
                id,
                &reason,
                revocation.expected_version,
                ports.clock.now(),
                &audit,
            )
            .await?,
    )?;
    Ok(show_one(caller, access, event, decision, ports).await?)
}

/// One decision of the event, with its evidence. Each reader of the event sees it.
pub async fn get_decision(
    caller: &impl Principal,
    event: EventId,
    id: DecisionId,
    ports: WorkPorts<'_>,
) -> Result<Shown<DecisionView>, WorkError> {
    let access = access::event_access(caller, event, ports.identity).await?;
    let decision = ports
        .decisions
        .decision(caller.scope(), event, id)
        .await?
        .ok_or(WorkError::NotFound)?;
    Ok(show_one(caller, access, event, decision, ports).await?)
}

/// The decisions of the event that match `query`, in the order of their numbers, with their evidence.
pub async fn list_decisions(
    caller: &impl Principal,
    event: EventId,
    query: WorkQuery<DecisionStatus>,
    ports: WorkPorts<'_>,
) -> Result<Page<Shown<DecisionView>, NumberCursor>, WorkError> {
    let access = access::event_access(caller, event, ports.identity).await?;
    let items = ports
        .decisions
        .decisions(caller.scope(), event, &query.filter())
        .await?;
    let Page { items, next } = page(items, query.limit, |decision| decision.local_number);
    let items = show(caller, access, event, items, ports).await?;
    Ok(Page { items, next })
}
