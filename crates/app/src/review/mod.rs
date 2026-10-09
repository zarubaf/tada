//! Review and apply (ADR 0050): event managers apply a selection of a changeset all or nothing,
//! edit values with their own evidence, and reject proposals.
//!
//! Review results are append-only. The status of a proposal comes from its latest review result.
//! Only a `MemberCaller` reviews: an AI client cannot accept proposals (ADR 0010, ADR 0039).

mod read;

use std::collections::{HashMap, HashSet};
use std::fmt::Debug;

use async_trait::async_trait;
use jiff::{SignedDuration, Timestamp};
use tada_domain::facts::{ChoiceValue, FactState, Label, ValueType, Valued};
use tada_domain::ids::{
    ChangesetId, DocumentId, EventId, FieldDefinitionId, OpenQuestionId, ProposalId, UserId,
};
use tada_domain::proposals::{Operation, Proposal};
use tada_domain::sources::{Evidence, SourceText};

pub use self::read::{
    ChangesetReview, ConflictReason, EXCERPT_CONTEXT, ProposalReview, get_changeset,
};
use crate::access::{self, AccessError};
use crate::audit::{AuditAction, AuditEvent};
use crate::caller::{Actor, MemberCaller, OrgScope};
use crate::clock::Clock;
use crate::facts::FactStore;
use crate::identity::IdentityStore;
use crate::paging::{Page, PageLimit};
use crate::problem::{CommandError, FieldError, ProblemCode};
use crate::proposals::{
    Changeset, FactStateInput, ProposalStore, state_from_input, value_error_code,
};
use crate::sources::SourceStore;
use crate::store::StoreError;

/// An open proposal older than this shows as stale (ADR 0050). It does not change.
pub const STALE_AFTER: SignedDuration = SignedDuration::from_hours(14 * 24);

/// The result of one review of one proposal (ADR 0050).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReviewOutcome {
    Accepted,
    AcceptedWithEdit,
    Rejected,
    /// The target record changed after the proposal, so the proposal cannot apply.
    Conflict,
    Withdrawn,
}

impl ReviewOutcome {
    pub fn as_str(self) -> &'static str {
        ProposalStatus::from(self).as_str()
    }

    pub fn parse(name: &str) -> Option<Self> {
        [
            Self::Accepted,
            Self::AcceptedWithEdit,
            Self::Rejected,
            Self::Conflict,
            Self::Withdrawn,
        ]
        .into_iter()
        .find(|outcome| outcome.as_str() == name)
    }
}

/// The status of a proposal, derived from its latest review result. Without one, the proposal is open.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProposalStatus {
    Open,
    Accepted,
    AcceptedWithEdit,
    Rejected,
    Conflict,
    Withdrawn,
}

impl ProposalStatus {
    /// The API value (ADR 0044).
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Accepted => "accepted",
            Self::AcceptedWithEdit => "accepted-with-edit",
            Self::Rejected => "rejected",
            Self::Conflict => "conflict",
            Self::Withdrawn => "withdrawn",
        }
    }

    /// True if the proposal is open and older than `STALE_AFTER`. A stale proposal still waits for a review.
    pub fn is_stale(self, created_at: Timestamp, now: Timestamp) -> bool {
        self == Self::Open && created_at < now - STALE_AFTER
    }
}

impl From<ReviewOutcome> for ProposalStatus {
    fn from(outcome: ReviewOutcome) -> Self {
        match outcome {
            ReviewOutcome::Accepted => Self::Accepted,
            ReviewOutcome::AcceptedWithEdit => Self::AcceptedWithEdit,
            ReviewOutcome::Rejected => Self::Rejected,
            ReviewOutcome::Conflict => Self::Conflict,
            ReviewOutcome::Withdrawn => Self::Withdrawn,
        }
    }
}

/// One stored review result of a proposal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewRecord {
    pub proposal_id: ProposalId,
    pub outcome: ReviewOutcome,
    pub created_at: Timestamp,
}

/// The status of a proposal from its review results: the latest result, or `Open` without one.
/// Of two results with the same time, the later one in `results` counts.
pub fn status(results: &[ReviewRecord]) -> ProposalStatus {
    results
        .iter()
        .enumerate()
        .max_by_key(|(position, record)| (record.created_at, *position))
        .map_or(ProposalStatus::Open, |(_, record)| record.outcome.into())
}

/// A changeset with at least one open proposal, for the Review Inbox.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenChangeset {
    pub id: ChangesetId,
    /// `None` for a changeset of the organization, for example one that creates an event.
    pub event_id: Option<EventId>,
    pub author: Actor,
    pub created_at: Timestamp,
    /// The number of its open proposals.
    pub open_proposals: u32,
}

impl OpenChangeset {
    /// True if its open proposals are stale. All proposals of a changeset have its creation time.
    pub fn is_stale(&self, now: Timestamp) -> bool {
        ProposalStatus::Open.is_stale(self.created_at, now)
    }
}

/// The evidence of a fact version that an apply creates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StepEvidence {
    /// The evidence of the proposal.
    Proposal(Vec<Evidence>),
    /// The reviewer edited the value. The store keeps the edited state as a source version of the kind
    /// `review`, with the reviewer as author, and links it as the evidence (ADR 0050).
    Edit,
}

/// One proposal of an apply, with the operation to apply. An edit changes the state of a `SetFact`.
#[derive(Clone, PartialEq, Eq)]
pub struct ApplyStep {
    pub proposal_id: ProposalId,
    pub operation: Operation,
    pub evidence: StepEvidence,
}

/// An operation can hold an edited value with personal data, so `Debug` names the kind of the operation only (ADR 0035).
impl Debug for ApplyStep {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let operation = match self.operation {
            Operation::CreateEvent { .. } => "CreateEvent",
            Operation::SetFact { .. } => "SetFact",
            Operation::AddFieldDefinition { .. } => "AddFieldDefinition",
            Operation::AddChoiceValue { .. } => "AddChoiceValue",
            Operation::DeprecateField { .. } => "DeprecateField",
            Operation::CreateOpenQuestion { .. } => "CreateOpenQuestion",
            Operation::CreateDocumentDraft { .. } => "CreateDocumentDraft",
        };
        f.debug_struct("ApplyStep")
            .field("proposal_id", &self.proposal_id)
            .field("operation", &operation)
            .field("evidence", &self.evidence)
            .finish()
    }
}

/// All writes of one apply. The store applies them in one transaction, or nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplyPlan {
    pub changeset_id: ChangesetId,
    /// The reviewer: the author of the review results, the fact versions and the review source versions.
    pub reviewer: Actor,
    /// The member who becomes the event manager of each new event (ADR 0052),
    /// and who adds each draft version and owns each new document of a draft (ADR 0051).
    pub manager: UserId,
    pub now: Timestamp,
    /// Each step follows the steps of its dependencies.
    pub steps: Vec<ApplyStep>,
    pub audit: Vec<AuditEvent>,
}

/// The local number that an apply gave a new record (ADR 0038).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NewLocalId {
    pub record: LocalRecord,
    pub local_number: u64,
}

/// A new record with a local number.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocalRecord {
    /// An open question: `QST-<n>`, local to its event.
    OpenQuestion(OpenQuestionId),
    /// A document: `DOC-<n>`, local to the organization.
    Document(DocumentId),
}

/// The result of `ReviewStore::apply`. Each result other than `Applied` changed nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApplyOutcome {
    Applied(Vec<NewLocalId>),
    /// A proposal of the plan has a review result.
    NotOpen,
    /// The target records of these proposals do not have the expected versions.
    Conflict(Vec<ProposalId>),
    /// Another event of the organization, or another field of the event, has the key of a new record.
    KeyTaken,
}

/// The review result that a batch appends without an apply. Only an apply accepts, so a batch cannot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BatchOutcome {
    Rejected,
    Conflict,
}

impl BatchOutcome {
    pub fn review_outcome(self) -> ReviewOutcome {
        match self {
            Self::Rejected => ReviewOutcome::Rejected,
            Self::Conflict => ReviewOutcome::Conflict,
        }
    }

    fn audit_action(self) -> AuditAction {
        match self {
            Self::Rejected => AuditAction::ProposalReject,
            Self::Conflict => AuditAction::ProposalConflict,
        }
    }
}

/// Review results to append without an apply: one rejection or one conflict for each proposal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewBatch {
    pub changeset_id: ChangesetId,
    pub proposals: Vec<ProposalId>,
    pub outcome: BatchOutcome,
    /// The author of the review results.
    pub reviewer: Actor,
    pub now: Timestamp,
    pub audit: Vec<AuditEvent>,
}

/// The result of `ReviewStore::record`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Recorded {
    Recorded,
    /// A proposal has a review result now. The store changed nothing.
    NotOpen,
}

/// The repository port for review results and applies. Each method stays inside `scope`.
#[async_trait]
pub trait ReviewStore: Debug + Send + Sync {
    /// The review results of the proposals of the changeset, oldest first.
    async fn results(
        &self,
        scope: OrgScope,
        changeset: ChangesetId,
    ) -> Result<Vec<ReviewRecord>, StoreError>;

    /// Applies `plan` in one transaction, or nothing (ADR 0050).
    /// It locks the changeset and checks that each proposal is open and that each target has the expected version.
    /// Then it creates the records and fact versions with their evidence, assigns the local IDs,
    /// adds the draft versions with the provenance manifests that their proposals fixed (ADR 0051),
    /// appends `accepted` or `accepted-with-edit` for each step, and records the audit events of the plan.
    async fn apply(&self, scope: OrgScope, plan: &ApplyPlan) -> Result<ApplyOutcome, StoreError>;

    /// Appends the review results and the audit events of `batch` in one transaction, if each proposal is open.
    async fn record(&self, scope: OrgScope, batch: &ReviewBatch) -> Result<Recorded, StoreError>;

    /// The changesets with at least one open proposal, oldest first.
    /// `Some(event)` gives the changesets of the event only; `None` gives all changesets of the organization.
    async fn open_changesets(
        &self,
        scope: OrgScope,
        event: Option<EventId>,
    ) -> Result<Vec<OpenChangeset>, StoreError>;
}

/// The ports that a review reads and writes.
#[derive(Debug, Clone, Copy)]
pub struct ReviewStores<'a> {
    pub identity: &'a dyn IdentityStore,
    pub facts: &'a dyn FactStore,
    pub proposals: &'a dyn ProposalStore,
    pub review: &'a dyn ReviewStore,
    /// The source versions that the evidence of a proposal cites besides the source text of its changeset.
    pub sources: &'a dyn SourceStore,
}

/// The input of an apply: the selected proposals and the edited values.
#[derive(Debug, Clone)]
pub struct ApplyInput {
    pub selected: Vec<ProposalId>,
    pub edits: Vec<Edit>,
}

/// A value that the reviewer changes before the acceptance. Only a proposal that sets a fact has a value.
#[derive(Clone)]
pub struct Edit {
    pub proposal_id: ProposalId,
    pub state: FactStateInput,
}

/// A value can contain personal data, so `Debug` shows the proposal only (ADR 0035).
impl Debug for Edit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Edit")
            .field("proposal_id", &self.proposal_id)
            .finish_non_exhaustive()
    }
}

/// The result of a successful apply.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Applied {
    /// The applied proposals with their new status, in the order of the apply.
    pub proposals: Vec<(ProposalId, ProposalStatus)>,
    /// The local IDs of the new open questions and documents.
    pub local_ids: Vec<NewLocalId>,
}

#[derive(Debug, thiserror::Error)]
pub enum ApplyError {
    /// The changeset is not in the caller's organization, or the caller cannot see its event.
    #[error("the changeset does not exist or the caller cannot see it")]
    NotFound,
    /// Only event managers review (ADR 0052).
    #[error("the caller cannot review this changeset")]
    Forbidden,
    #[error("invalid values")]
    Invalid(Vec<FieldError>),
    /// A selected proposal is not open.
    #[error("a selected proposal is not open")]
    InvalidTransition,
    /// The target records of these proposals changed after the proposal. Their status is `conflict` now.
    #[error("a target record changed after the proposal")]
    Conflict(Vec<ProposalId>),
    #[error(transparent)]
    Store(#[from] StoreError),
}

impl ApplyError {
    /// All codes that the apply and the rejection can return, for the API contract (ADR 0037).
    pub const CODES: &[ProblemCode] = &[
        ProblemCode::Forbidden,
        ProblemCode::NotFound,
        ProblemCode::ValidationFailed,
        ProblemCode::InvalidTransition,
        ProblemCode::RecordVersionConflict,
        ProblemCode::Unavailable,
        ProblemCode::Internal,
    ];
}

impl From<ReviewQueryError> for ApplyError {
    fn from(error: ReviewQueryError) -> Self {
        match error {
            ReviewQueryError::NotFound => Self::NotFound,
            ReviewQueryError::Forbidden => Self::Forbidden,
            ReviewQueryError::Store(error) => Self::Store(error),
        }
    }
}

impl CommandError for ApplyError {
    fn code(&self) -> ProblemCode {
        match self {
            Self::NotFound => ProblemCode::NotFound,
            Self::Forbidden => ProblemCode::Forbidden,
            Self::Invalid(_) => ProblemCode::ValidationFailed,
            Self::InvalidTransition => ProblemCode::InvalidTransition,
            Self::Conflict(_) => ProblemCode::RecordVersionConflict,
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

/// Applies a selection of the proposals of a changeset, all or nothing (ADR 0050).
///
/// 1. The caller reviews the event of the changeset; owners and admins review a changeset of the organization.
/// 2. The selection includes the dependencies of each selected proposal, except those that an earlier apply accepted.
/// 3. Each proposal of the selection is open, else `invalid-transition`.
/// 4. Each target has the expected version. Else nothing changes, a separate transaction appends
///    `conflict` for the proposals concerned, and the result is `record-version-conflict`.
/// 5. A successful apply writes the records, the fact versions with their evidence, the draft versions with their
///    provenance manifests, the local IDs, the review results and the audit events in one transaction.
///
/// The caller is a `MemberCaller`, so an AI client cannot apply (ADR 0039).
/// Code that has an `AiCaller` does not compile, because the caller type does not match:
///
/// ```compile_fail,E0308
/// use tada_app::caller::AiCaller;
/// use tada_app::clock::Clock;
/// use tada_app::domain::ids::ChangesetId;
/// use tada_app::review::{ApplyInput, ReviewStores, apply_changeset};
///
/// async fn apply_as_ai_client(
///     caller: &AiCaller,
///     changeset: ChangesetId,
///     input: ApplyInput,
///     stores: ReviewStores<'_>,
///     clock: &dyn Clock,
/// ) {
///     let _ = apply_changeset(caller, changeset, input, stores, clock).await;
/// }
/// ```
pub async fn apply_changeset(
    caller: &MemberCaller,
    changeset_id: ChangesetId,
    input: ApplyInput,
    stores: ReviewStores<'_>,
    clock: &dyn Clock,
) -> Result<Applied, ApplyError> {
    let scope = caller.scope();
    let (changeset, _) = reviewable(caller, changeset_id, stores).await?;
    let results = stores.review.results(scope, changeset_id).await?;

    let given = selection(&changeset, &input.selected, "selected")?;
    let selected = to_apply(&changeset.proposals, &given, &results)?;
    let edits = parse_edits(&changeset, &selected, input.edits)?;
    check_edits(scope, &changeset, &selected, &edits, stores.facts).await?;

    let now = clock.now();
    let steps: Vec<ApplyStep> = apply_order(&changeset.proposals, &selected)
        .into_iter()
        .map(|proposal| step(proposal, &edits))
        .collect();
    let audit = steps
        .iter()
        .flat_map(|step| audit_of(caller, step))
        .collect();
    let plan = ApplyPlan {
        changeset_id,
        reviewer: caller.actor(),
        manager: caller.user_id(),
        now,
        steps,
        audit,
    };
    match stores.review.apply(scope, &plan).await? {
        ApplyOutcome::Applied(local_ids) => Ok(Applied {
            proposals: plan
                .steps
                .iter()
                .map(|step| (step.proposal_id, step_status(step)))
                .collect(),
            local_ids,
        }),
        ApplyOutcome::NotOpen => Err(ApplyError::InvalidTransition),
        ApplyOutcome::KeyTaken => Err(invalid("key", "taken")),
        ApplyOutcome::Conflict(proposals) => {
            // The conflict stays visible: a separate transaction records it (ADR 0050).
            let batch = review_batch(
                caller,
                changeset_id,
                &proposals,
                BatchOutcome::Conflict,
                now,
            );
            // A concurrent review may have closed a proposal first. Then its status stays as it is.
            // If the record fails, the caller still learns of the conflict: the proposals stay open,
            // and the next apply finds the same conflict and records it then.
            let _recorded = stores.review.record(scope, &batch).await;
            Err(ApplyError::Conflict(proposals))
        }
    }
}

/// The result of a successful rejection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rejected {
    /// The rejected proposals: the given ones and their open dependents.
    pub proposals: Vec<ProposalId>,
}

/// Rejects proposals of a changeset. It also rejects the open proposals that depend on them, because
/// these can never apply: an apply selects the dependencies of each proposal (ADR 0050).
/// For example, the rejection of a new event rejects the facts of that event.
/// Each given proposal must be open, else `invalid-transition`.
///
/// A conflict does not close the dependents of a proposal in the same way: the conflict is a fact about the
/// target record, not a decision of the reviewer. The dependents stay open until a reviewer rejects them.
pub async fn reject_proposals(
    caller: &MemberCaller,
    changeset_id: ChangesetId,
    ids: Vec<ProposalId>,
    stores: ReviewStores<'_>,
    clock: &dyn Clock,
) -> Result<Rejected, ApplyError> {
    let scope = caller.scope();
    let (changeset, _) = reviewable(caller, changeset_id, stores).await?;
    let results = stores.review.results(scope, changeset_id).await?;

    let given = selection(&changeset, &ids, "proposal_ids")?;
    let is_open = |id: ProposalId| proposal_status(&results, id) == ProposalStatus::Open;
    if !given.iter().all(|id| is_open(*id)) {
        return Err(ApplyError::InvalidTransition);
    }
    let rejected: Vec<ProposalId> = with_dependents(&changeset.proposals, &given)
        .into_iter()
        .filter(|id| is_open(*id))
        .collect();
    let batch = review_batch(
        caller,
        changeset_id,
        &rejected,
        BatchOutcome::Rejected,
        clock.now(),
    );
    match stores.review.record(scope, &batch).await? {
        Recorded::Recorded => Ok(Rejected {
            proposals: rejected,
        }),
        Recorded::NotOpen => Err(ApplyError::InvalidTransition),
    }
}

/// The error of a review query, and of the access check of each review.
#[derive(Debug, thiserror::Error)]
pub enum ReviewQueryError {
    /// The event or the changeset is not in the caller's organization, or the caller cannot see it.
    #[error("the event or the changeset does not exist or the caller cannot see it")]
    NotFound,
    /// Only event managers review (ADR 0052).
    #[error("the caller cannot review this event")]
    Forbidden,
    #[error(transparent)]
    Store(#[from] StoreError),
}

impl ReviewQueryError {
    /// All codes that the query can return, for the API contract (ADR 0037).
    pub const CODES: &[ProblemCode] = &[
        ProblemCode::Forbidden,
        ProblemCode::NotFound,
        ProblemCode::Unavailable,
        ProblemCode::Internal,
    ];
}

impl From<AccessError> for ReviewQueryError {
    fn from(error: AccessError) -> Self {
        match error {
            AccessError::NotFound => Self::NotFound,
            AccessError::Store(error) => Self::Store(error),
        }
    }
}

impl CommandError for ReviewQueryError {
    fn code(&self) -> ProblemCode {
        match self {
            Self::NotFound => ProblemCode::NotFound,
            Self::Forbidden => ProblemCode::Forbidden,
            Self::Store(error) => error.code(),
        }
    }

    fn store_error(&self) -> Option<&StoreError> {
        match self {
            Self::Store(error) => Some(error),
            _ => None,
        }
    }
}

/// The position of an open changeset in the list of open changesets, which is oldest first (ADR 0044).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChangesetCursor {
    pub created_at: Timestamp,
    pub id: ChangesetId,
}

/// One page of the changesets with open proposals that the caller can review, oldest first.
///
/// `Some(event)` lists the changesets of one event and needs the right to review it.
/// `None` is the Review Inbox: the changesets of each event that the caller reviews and, for owners and admins,
/// the changesets of the organization, for example a new event.
pub async fn list_open_changesets(
    caller: &MemberCaller,
    event_id: Option<EventId>,
    after: Option<ChangesetCursor>,
    limit: PageLimit,
    identity: &dyn IdentityStore,
    store: &dyn ReviewStore,
) -> Result<Page<OpenChangeset, ChangesetCursor>, ReviewQueryError> {
    let visible = visible_open_changesets(caller, event_id, identity, store).await?;
    Ok(page(visible, after, limit))
}

async fn visible_open_changesets(
    caller: &MemberCaller,
    event_id: Option<EventId>,
    identity: &dyn IdentityStore,
    store: &dyn ReviewStore,
) -> Result<Vec<OpenChangeset>, ReviewQueryError> {
    let scope = caller.scope();
    if let Some(event) = event_id {
        review_access(caller, Some(event), identity).await?;
        return Ok(store.open_changesets(scope, Some(event)).await?);
    }
    let mut reviews: HashMap<Option<EventId>, bool> = HashMap::new();
    let mut visible = Vec::new();
    for changeset in store.open_changesets(scope, None).await? {
        let allowed = match reviews.get(&changeset.event_id) {
            Some(allowed) => *allowed,
            None => {
                let allowed = can_review(caller, changeset.event_id, identity).await?;
                reviews.insert(changeset.event_id, allowed);
                allowed
            }
        };
        if allowed {
            visible.push(changeset);
        }
    }
    Ok(visible)
}

/// The page after `after` of `changesets`, which are oldest first.
/// The access of the caller filters the changesets, so the page comes after the filter.
/// The open changesets of an organization are few, so the store gives them all.
fn page(
    changesets: Vec<OpenChangeset>,
    after: Option<ChangesetCursor>,
    limit: PageLimit,
) -> Page<OpenChangeset, ChangesetCursor> {
    let limit = limit.get() as usize;
    let mut items: Vec<OpenChangeset> = changesets
        .into_iter()
        .filter(|changeset| {
            after.is_none_or(|after| {
                (changeset.created_at, changeset.id) > (after.created_at, after.id)
            })
        })
        .take(limit + 1)
        .collect();
    let more = items.len() > limit;
    items.truncate(limit);
    let next = more
        .then(|| items.last())
        .flatten()
        .map(|last| ChangesetCursor {
            created_at: last.created_at,
            id: last.id,
        });
    Page { items, next }
}

/// The one review rule (ADR 0052): the caller reviews the changesets of an event in which it is event manager,
/// and, for `None`, the changesets of the organization only as owner or admin.
///
/// `Forbidden` means that the caller sees the event but does not review it.
/// `NotFound` means that the caller cannot see the event, or that it cannot see the changesets of the organization.
async fn review_access(
    caller: &MemberCaller,
    event: Option<EventId>,
    identity: &dyn IdentityStore,
) -> Result<(), ReviewQueryError> {
    match event {
        Some(event) => {
            if access::event_access(caller, event, identity)
                .await?
                .can_review()
            {
                Ok(())
            } else {
                Err(ReviewQueryError::Forbidden)
            }
        }
        None if access::sees_all_events(caller) => Ok(()),
        None => Err(ReviewQueryError::NotFound),
    }
}

/// True if the caller reviews the event, or for `None`, the changesets of the organization.
/// A list hides what the caller cannot review, so a refusal is `false` here, not an error.
async fn can_review(
    caller: &MemberCaller,
    event: Option<EventId>,
    identity: &dyn IdentityStore,
) -> Result<bool, StoreError> {
    match review_access(caller, event, identity).await {
        Ok(()) => Ok(true),
        Err(ReviewQueryError::NotFound | ReviewQueryError::Forbidden) => Ok(false),
        Err(ReviewQueryError::Store(error)) => Err(error),
    }
}

/// The changeset with the text of its source version, if the caller can review it.
async fn reviewable(
    caller: &MemberCaller,
    id: ChangesetId,
    stores: ReviewStores<'_>,
) -> Result<(Changeset, SourceText), ReviewQueryError> {
    let Some((changeset, source)) = stores.proposals.get(caller.scope(), id).await? else {
        return Err(ReviewQueryError::NotFound);
    };
    review_access(caller, changeset.event_id, stores.identity).await?;
    Ok((changeset, source))
}

fn invalid(field: &'static str, code: &'static str) -> ApplyError {
    ApplyError::Invalid(vec![FieldError::new(field, code)])
}

fn finish(errors: Vec<FieldError>) -> Result<(), ApplyError> {
    if errors.is_empty() {
        Ok(())
    } else {
        Err(ApplyError::Invalid(errors))
    }
}

fn proposal_status(results: &[ReviewRecord], id: ProposalId) -> ProposalStatus {
    let own: Vec<ReviewRecord> = results
        .iter()
        .filter(|record| record.proposal_id == id)
        .cloned()
        .collect();
    status(&own)
}

/// Checks that `ids` are proposals of the changeset. `field` names the list in the errors.
fn selection(
    changeset: &Changeset,
    ids: &[ProposalId],
    field: &str,
) -> Result<HashSet<ProposalId>, ApplyError> {
    if ids.is_empty() {
        return Err(ApplyError::Invalid(vec![FieldError::new(
            field.to_owned(),
            "empty",
        )]));
    }
    let known: HashSet<ProposalId> = changeset.proposals.iter().map(|p| p.id).collect();
    finish(
        ids.iter()
            .enumerate()
            .filter(|(_, id)| !known.contains(id))
            .map(|(index, _)| FieldError::new(format!("{field}/{index}"), "unknown"))
            .collect(),
    )?;
    Ok(ids.iter().copied().collect())
}

/// The proposals of `selected` and all their dependencies.
fn with_dependencies(
    proposals: &[Proposal],
    selected: &HashSet<ProposalId>,
) -> HashSet<ProposalId> {
    let depends_on: HashMap<ProposalId, &[ProposalId]> = proposals
        .iter()
        .map(|proposal| (proposal.id, proposal.depends_on.as_slice()))
        .collect();
    closure(selected, |id| {
        depends_on.get(&id).copied().unwrap_or_default().to_vec()
    })
}

/// The proposals to apply: the selected ones and their dependencies, without the dependencies that an earlier apply
/// accepted, for example the new event of a fact. Each of them must be open, else `invalid-transition`.
fn to_apply(
    proposals: &[Proposal],
    given: &HashSet<ProposalId>,
    results: &[ReviewRecord],
) -> Result<HashSet<ProposalId>, ApplyError> {
    let mut selected = HashSet::new();
    for id in with_dependencies(proposals, given) {
        match proposal_status(results, id) {
            ProposalStatus::Open => {
                selected.insert(id);
            }
            ProposalStatus::Accepted | ProposalStatus::AcceptedWithEdit if !given.contains(&id) => {
            }
            _ => return Err(ApplyError::InvalidTransition),
        }
    }
    Ok(selected)
}

/// The proposals of `given` and all proposals that depend on them.
fn with_dependents(proposals: &[Proposal], given: &HashSet<ProposalId>) -> Vec<ProposalId> {
    let mut dependents: HashMap<ProposalId, Vec<ProposalId>> = HashMap::new();
    for proposal in proposals {
        for dependency in &proposal.depends_on {
            dependents.entry(*dependency).or_default().push(proposal.id);
        }
    }
    let all = closure(given, |id| dependents.get(&id).cloned().unwrap_or_default());
    // In the order of the changeset, so the result does not depend on a hash order.
    proposals
        .iter()
        .map(|proposal| proposal.id)
        .filter(|id| all.contains(id))
        .collect()
}

fn closure(
    start: &HashSet<ProposalId>,
    next: impl Fn(ProposalId) -> Vec<ProposalId>,
) -> HashSet<ProposalId> {
    let mut all = start.clone();
    let mut todo: Vec<ProposalId> = start.iter().copied().collect();
    while let Some(id) = todo.pop() {
        for other in next(id) {
            if all.insert(other) {
                todo.push(other);
            }
        }
    }
    all
}

/// The selected proposals in an order where each proposal follows its dependencies.
/// The dependencies of a stored changeset have no cycle (ADR 0050). Ties keep the order of the changeset.
fn apply_order<'a>(proposals: &'a [Proposal], selected: &HashSet<ProposalId>) -> Vec<&'a Proposal> {
    let mut pending: Vec<&Proposal> = proposals
        .iter()
        .filter(|proposal| selected.contains(&proposal.id))
        .collect();
    let mut done = HashSet::new();
    let mut order = Vec::new();
    while !pending.is_empty() {
        let before = pending.len();
        pending.retain(|proposal| {
            // A dependency outside the selection was accepted by an earlier apply.
            let ready = proposal
                .depends_on
                .iter()
                .all(|id| done.contains(id) || !selected.contains(id));
            if ready {
                done.insert(proposal.id);
                order.push(*proposal);
            }
            !ready
        });
        assert!(
            pending.len() < before,
            "the dependencies of a changeset have no cycle"
        );
    }
    order
}

/// The edited states by proposal, each with the position of its edit in the input.
type Edits = HashMap<ProposalId, (usize, FactState<Valued>)>;

/// Maps the edits to fact states. Each edit names a selected proposal that sets a fact, at most once.
fn parse_edits(
    changeset: &Changeset,
    selected: &HashSet<ProposalId>,
    edits: Vec<Edit>,
) -> Result<Edits, ApplyError> {
    let mut errors = Vec::new();
    let mut parsed = HashMap::new();
    for (index, edit) in edits.into_iter().enumerate() {
        let path = |field: &str| format!("edits/{index}/{field}");
        let proposal = changeset
            .proposals
            .iter()
            .find(|proposal| proposal.id == edit.proposal_id);
        let code = match proposal {
            None => Some("unknown"),
            Some(_) if !selected.contains(&edit.proposal_id) => Some("not-selected"),
            Some(proposal) if !matches!(proposal.operation, Operation::SetFact { .. }) => {
                Some("not-editable")
            }
            Some(_) if parsed.contains_key(&edit.proposal_id) => Some("duplicate"),
            Some(_) => None,
        };
        if let Some(code) = code {
            errors.push(FieldError::new(path("proposal_id"), code));
            continue;
        }
        match state_from_input(edit.state) {
            Ok(state) => {
                parsed.insert(edit.proposal_id, (index, state));
            }
            Err(nested) => {
                errors.extend(nested.into_iter().map(|error| {
                    FieldError::new(path(&format!("state/{}", error.field)), error.code)
                }))
            }
        }
    }
    finish(errors)?;
    Ok(parsed)
}

/// Checks each edited value against the value type of its field: a field of the catalog of the event,
/// or a new field of the selection, with the new choices of the selection.
async fn check_edits(
    scope: OrgScope,
    changeset: &Changeset,
    selected: &HashSet<ProposalId>,
    edits: &Edits,
    facts: &dyn FactStore,
) -> Result<(), ApplyError> {
    let mut value_types: HashMap<FieldDefinitionId, ValueType> = HashMap::new();
    let mut events = HashSet::new();
    for proposal in &changeset.proposals {
        if edits.contains_key(&proposal.id) {
            events.insert(proposal.operation.event_id());
        }
    }
    for event in events {
        for field in facts.catalog(scope, event).await? {
            value_types.insert(field.id, field.value_type);
        }
    }
    let selection = || {
        changeset
            .proposals
            .iter()
            .filter(|proposal| selected.contains(&proposal.id))
    };
    for proposal in selection() {
        if let Operation::AddFieldDefinition { id, value_type, .. } = &proposal.operation {
            value_types.insert(*id, value_type.clone());
        }
    }
    for proposal in selection() {
        if let Operation::AddChoiceValue {
            field_id,
            key,
            label,
            ..
        } = &proposal.operation
            && let Some(ValueType::Choice { values, .. }) = value_types.get_mut(field_id)
        {
            values.push(ChoiceValue {
                key: key.clone(),
                label: Label::Text(label.clone()),
            });
        }
    }
    let mut errors = Vec::new();
    for proposal in &changeset.proposals {
        let (Some((index, state)), Operation::SetFact { field_id, .. }) =
            (edits.get(&proposal.id), &proposal.operation)
        else {
            continue;
        };
        let value = match state {
            FactState::Accepted(valued) | FactState::Assumption(valued) => &valued.value,
            FactState::Unknown => continue,
        };
        let checked = match value_types.get(field_id) {
            Some(value_type) => value.check(value_type).map_err(value_error_code),
            None => Err("unknown-field"),
        };
        if let Err(code) = checked {
            errors.push(FieldError::new(format!("edits/{index}/state/value"), code));
        }
    }
    finish(errors)
}

fn step(proposal: &Proposal, edits: &Edits) -> ApplyStep {
    match (&proposal.operation, edits.get(&proposal.id)) {
        (
            Operation::SetFact {
                event_id,
                field_id,
                expected_version,
                ..
            },
            Some((_, state)),
        ) => ApplyStep {
            proposal_id: proposal.id,
            operation: Operation::SetFact {
                event_id: *event_id,
                field_id: *field_id,
                state: state.clone(),
                expected_version: *expected_version,
            },
            evidence: StepEvidence::Edit,
        },
        _ => ApplyStep {
            proposal_id: proposal.id,
            operation: proposal.operation.clone(),
            evidence: StepEvidence::Proposal(proposal.evidence.clone()),
        },
    }
}

fn step_status(step: &ApplyStep) -> ProposalStatus {
    match step.evidence {
        StepEvidence::Edit => ProposalStatus::AcceptedWithEdit,
        StepEvidence::Proposal(_) => ProposalStatus::Accepted,
    }
}

/// The audit events of one step: the acceptance and, for a new event, the event and its first event manager.
fn audit_of(caller: &MemberCaller, step: &ApplyStep) -> Vec<AuditEvent> {
    let scope = Some(caller.scope());
    let mut events = vec![AuditEvent::new(
        caller.actor(),
        AuditAction::ProposalAccept,
        Some(step.proposal_id.as_uuid()),
        scope,
    )];
    if let Operation::CreateEvent { id, .. } = step.operation {
        events.extend(crate::events::creation_audit(caller, id));
    }
    events
}

/// A rejection or a conflict of `proposals` by the caller, with one audit event for each proposal.
fn review_batch(
    caller: &MemberCaller,
    changeset_id: ChangesetId,
    proposals: &[ProposalId],
    outcome: BatchOutcome,
    now: Timestamp,
) -> ReviewBatch {
    let action = outcome.audit_action();
    ReviewBatch {
        changeset_id,
        proposals: proposals.to_vec(),
        outcome,
        reviewer: caller.actor(),
        now,
        audit: proposals
            .iter()
            .map(|id| {
                AuditEvent::new(
                    caller.actor(),
                    action,
                    Some(id.as_uuid()),
                    Some(caller.scope()),
                )
            })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use tada_domain::proposals::Reason;
    use uuid::Uuid;

    use super::*;

    fn id(n: u128) -> ProposalId {
        ProposalId::from_uuid(Uuid::from_u128(n))
    }

    fn at(time: &str) -> Timestamp {
        time.parse().unwrap()
    }

    fn record(outcome: ReviewOutcome, time: &str) -> ReviewRecord {
        ReviewRecord {
            proposal_id: id(1),
            outcome,
            created_at: at(time),
        }
    }

    /// A proposal that only matters for its dependencies.
    fn proposal(n: u128, depends_on: &[u128]) -> Proposal {
        Proposal {
            id: id(n),
            operation: Operation::DeprecateField {
                event_id: EventId::from_uuid(Uuid::from_u128(20)),
                field_id: FieldDefinitionId::from_uuid(Uuid::from_u128(30)),
            },
            depends_on: depends_on.iter().map(|n| id(*n)).collect(),
            evidence: Vec::new(),
            reason: Reason::parse("test").unwrap(),
        }
    }

    fn set(ids: &[u128]) -> HashSet<ProposalId> {
        ids.iter().map(|n| id(*n)).collect()
    }

    #[test]
    fn the_latest_result_gives_the_status() {
        assert_eq!(status(&[]), ProposalStatus::Open);
        let results = [
            record(ReviewOutcome::Conflict, "2030-05-02T00:00:00Z"),
            record(ReviewOutcome::Withdrawn, "2030-05-01T00:00:00Z"),
        ];
        assert_eq!(status(&results), ProposalStatus::Conflict);
        let same_time = [
            record(ReviewOutcome::Rejected, "2030-05-01T00:00:00Z"),
            record(ReviewOutcome::AcceptedWithEdit, "2030-05-01T00:00:00Z"),
        ];
        assert_eq!(status(&same_time), ProposalStatus::AcceptedWithEdit);
    }

    #[test]
    fn an_open_proposal_is_stale_after_fourteen_days() {
        let created = at("2030-05-01T08:00:00Z");
        assert!(!ProposalStatus::Open.is_stale(created, at("2030-05-15T08:00:00Z")));
        assert!(ProposalStatus::Open.is_stale(created, at("2030-05-15T08:00:01Z")));
        assert!(!ProposalStatus::Conflict.is_stale(created, at("2030-06-01T00:00:00Z")));
    }

    #[test]
    fn each_outcome_has_the_name_of_its_status() {
        for (outcome, name) in [
            (ReviewOutcome::Accepted, "accepted"),
            (ReviewOutcome::AcceptedWithEdit, "accepted-with-edit"),
            (ReviewOutcome::Rejected, "rejected"),
            (ReviewOutcome::Conflict, "conflict"),
            (ReviewOutcome::Withdrawn, "withdrawn"),
        ] {
            assert_eq!(outcome.as_str(), name);
            assert_eq!(ReviewOutcome::parse(name), Some(outcome));
        }
        assert_eq!(ReviewOutcome::parse("open"), None);
    }

    #[test]
    fn a_selection_includes_all_dependencies_and_a_rejection_all_dependents() {
        // 3 depends on 2, 2 on 1; 4 is alone.
        let proposals = [
            proposal(1, &[]),
            proposal(2, &[1]),
            proposal(3, &[2]),
            proposal(4, &[]),
        ];
        assert_eq!(with_dependencies(&proposals, &set(&[3])), set(&[1, 2, 3]));
        assert_eq!(with_dependencies(&proposals, &set(&[4])), set(&[4]));
        assert_eq!(
            with_dependents(&proposals, &set(&[1])),
            [id(1), id(2), id(3)]
        );
        assert_eq!(with_dependents(&proposals, &set(&[3])), [id(3)]);
    }

    #[test]
    fn debug_hides_the_value_of_a_step() {
        let value = tada_domain::facts::ShortText::parse("Anna Muster").unwrap();
        let step = ApplyStep {
            proposal_id: id(1),
            operation: Operation::SetFact {
                event_id: EventId::from_uuid(Uuid::from_u128(20)),
                field_id: FieldDefinitionId::from_uuid(Uuid::from_u128(30)),
                state: FactState::Accepted(Valued {
                    value: tada_domain::facts::FactValue::Text(value),
                    approximate: false,
                }),
                expected_version: None,
            },
            evidence: StepEvidence::Edit,
        };
        let debug = format!("{step:?}");
        assert!(debug.contains("SetFact"), "{debug}");
        assert!(!debug.contains("Anna"), "{debug}");
    }

    fn open(n: u128, time: &str) -> OpenChangeset {
        OpenChangeset {
            id: ChangesetId::from_uuid(Uuid::from_u128(n)),
            event_id: None,
            author: Actor::restore(
                crate::caller::ActorKind::Member,
                Uuid::from_u128(9),
                None,
                crate::caller::Channel::Web,
                None,
            ),
            created_at: at(time),
            open_proposals: 1,
        }
    }

    #[test]
    fn a_page_of_open_changesets_continues_after_its_cursor() {
        let all = || {
            vec![
                open(1, "2030-05-01T00:00:00Z"),
                open(2, "2030-05-01T00:00:00Z"),
                open(3, "2030-05-02T00:00:00Z"),
            ]
        };
        let two = PageLimit::new(2).unwrap();
        let first = page(all(), None, two);
        assert_eq!(first.items, all()[..2]);
        let cursor = first.next.unwrap();
        assert_eq!(cursor.id, ChangesetId::from_uuid(Uuid::from_u128(2)));
        let second = page(all(), Some(cursor), two);
        assert_eq!(second.items, all()[2..]);
        assert_eq!(second.next, None);
    }

    #[test]
    fn each_proposal_applies_after_its_dependencies() {
        // The changeset is in the order of the IDs, so a dependency can come later.
        let proposals = [proposal(1, &[3]), proposal(2, &[]), proposal(3, &[2])];
        let order: Vec<ProposalId> = apply_order(&proposals, &set(&[1, 2, 3]))
            .into_iter()
            .map(|proposal| proposal.id)
            .collect();
        assert_eq!(order, [id(2), id(3), id(1)]);
    }

    #[test]
    fn each_error_gives_a_code_of_its_list() {
        for error in [
            ApplyError::NotFound,
            ApplyError::Forbidden,
            ApplyError::Invalid(Vec::new()),
            ApplyError::InvalidTransition,
            ApplyError::Conflict(Vec::new()),
            ApplyError::Store(StoreError::Internal("test".into())),
            ApplyError::Store(StoreError::Unavailable("test".into())),
        ] {
            assert!(ApplyError::CODES.contains(&error.code()), "{error:?}");
        }
        for error in [
            ReviewQueryError::NotFound,
            ReviewQueryError::Forbidden,
            ReviewQueryError::Store(StoreError::Internal("test".into())),
            ReviewQueryError::Store(StoreError::Unavailable("test".into())),
        ] {
            assert!(ReviewQueryError::CODES.contains(&error.code()), "{error:?}");
        }
    }
}
