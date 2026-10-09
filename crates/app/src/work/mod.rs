//! Actions and commitments (ADR 0068): the work records of an event, with their direct commands.
//! A member creates and changes them without review. Proposals come later (ADR 0050).

mod actions;
mod checks;
mod commitments;

use std::fmt::Debug;

use async_trait::async_trait;
use jiff::Timestamp;
use jiff::civil::Date;
use tada_domain::RecordVersion;
use tada_domain::events::EventKey;
use tada_domain::ids::{ActionId, CommitmentId, EventId, LocalIdKind, UserId, WorkstreamId};
use tada_domain::parties::Party;
use tada_domain::work::{
    ActionDescription, ActionStatus, ActionTitle, CommitmentStatus, CommitmentText, ConditionText,
    FirmReason, InvalidTransition,
};

pub use self::actions::{
    ActionChange, NewAction, change_action, create_action, get_action, list_actions,
};
pub(crate) use self::checks::{ChangeRights, WorkRecord, is_possible_owner};
pub use self::commitments::{
    CommitmentChange, FirmInput, NewCommitment, change_commitment, create_commitment,
    get_commitment, list_commitments, make_commitment_firm,
};
use crate::access::{AccessError, EventAccess, EventReach};
use crate::audit::AuditEvent;
use crate::caller::OrgScope;
use crate::clock::Clock;
use crate::identity::IdentityStore;
use crate::paging::PageLimit;
use crate::parties::{PartyRef, PartyStore};
use crate::problem::{CommandError, FieldError, ProblemCode};
use crate::records::{Changed, Created, EvidenceStore, NumberCursor, Shown};
use crate::store::StoreError;
use crate::workstreams::WorkstreamStore;

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

impl Shown<ActionView> {
    /// The statuses that the caller can set with a change now (ADR 0068): none without the right to change the action.
    pub fn next_statuses(&self) -> Vec<ActionStatus> {
        if self.can_change {
            self.record.fields.status.next_statuses()
        } else {
            Vec::new()
        }
    }
}

impl Shown<CommitmentView> {
    /// The statuses that the caller can set with a change now (ADR 0068): none without the right to change the
    /// commitment. `firm` is never one of them: only "make firm" sets it.
    pub fn next_statuses(&self) -> Vec<CommitmentStatus> {
        if self.can_change {
            self.record.fields.status.direct_next_statuses()
        } else {
            Vec::new()
        }
    }
    /// True if the caller can make the commitment firm now (ADR 0068): the right to change it, and a status that can
    /// change to `firm`, that is `conditional`.
    pub fn can_make_firm(&self) -> bool {
        self.can_change
            && self
                .record
                .fields
                .status
                .change_to(CommitmentStatus::Firm)
                .is_ok()
    }
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

/// The records of an event that a list shows, in the order of their numbers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WorkFilter<S> {
    pub owner: Option<UserId>,
    pub status: Option<S>,
    pub workstream: Option<WorkstreamId>,
    pub after: Option<NumberCursor>,
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

/// The current owner of each found action and commitment (`WorkStore::owners`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WorkOwners {
    pub actions: Vec<(ActionId, UserId)>,
    pub commitments: Vec<(CommitmentId, UserId)>,
}

/// The repository port for actions and commitments. Each method stays inside `scope`.
/// A create or a change records its audit event in the same transaction.
#[async_trait]
pub trait WorkStore: EvidenceStore + Debug + Send + Sync {
    /// Inserts an action with the version 1 and gives it the next number of its event (ADR 0038).
    async fn create_action(
        &self,
        scope: OrgScope,
        action: &NewActionRecord,
        at: Timestamp,
        audit: &AuditEvent,
    ) -> Result<Created<ActionView>, StoreError>;

    /// Replaces the values of an action if its version is `expected`, and counts the version up.
    #[expect(
        clippy::too_many_arguments,
        reason = "a write port takes the scope, the record, the values, the version, the time and the audit event"
    )]
    async fn change_action(
        &self,
        scope: OrgScope,
        event: EventId,
        id: ActionId,
        fields: &ActionFields,
        expected: RecordVersion,
        at: Timestamp,
        audit: &AuditEvent,
    ) -> Result<Changed<ActionView>, StoreError>;

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
    ) -> Result<Created<CommitmentView>, StoreError>;

    /// Replaces the values of a commitment if its version is `expected`, and counts the version up.
    #[expect(
        clippy::too_many_arguments,
        reason = "a write port takes the scope, the record, the values, the version, the time and the audit event"
    )]
    async fn change_commitment(
        &self,
        scope: OrgScope,
        event: EventId,
        id: CommitmentId,
        fields: &CommitmentFields,
        expected: RecordVersion,
        at: Timestamp,
        audit: &AuditEvent,
    ) -> Result<Changed<CommitmentView>, StoreError>;

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

    /// The current owners of the actions `actions` and the commitments `commitments` of the event, for the review
    /// routing (ADR 0067). A record that the event does not have is missing in the result.
    async fn owners(
        &self,
        scope: OrgScope,
        event: EventId,
        actions: &[ActionId],
        commitments: &[CommitmentId],
    ) -> Result<WorkOwners, StoreError>;

    /// The open records that `user` owns in the events of `events` (`access::readable_events`).
    async fn my_open_work(
        &self,
        scope: OrgScope,
        user: UserId,
        events: &EventReach,
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

impl From<InvalidTransition> for WorkError {
    fn from(_: InvalidTransition) -> Self {
        Self::InvalidTransition
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

/// The filter of a list as the caller gives it, with the page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WorkQuery<S> {
    pub owner: Option<UserId>,
    pub status: Option<S>,
    pub workstream: Option<WorkstreamId>,
    pub after: Option<NumberCursor>,
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
