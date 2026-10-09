//! Review and apply (ADR 0050): reviewers apply a selection of a changeset all or nothing,
//! edit values with their own evidence, and reject proposals.
//! The review routing decides who reviews each proposal (ADR 0067).
//!
//! Review results are append-only. The status of a proposal comes from its latest review result.
//! Only a `MemberCaller` reviews: an AI client cannot accept proposals (ADR 0010, ADR 0039).

mod checks;
mod edits;
mod links;
mod read;
pub mod routing;
mod selection;
mod steps;

use std::collections::HashMap;
use std::fmt::Debug;

use async_trait::async_trait;
use jiff::{SignedDuration, Timestamp};
use tada_domain::ids::{
    ActionId, ChangesetId, CommitmentId, DocumentId, EventId, InstitutionId, OpenQuestionId,
    PersonId, ProposalId, UserId,
};
use tada_domain::proposals::Operation;
use tada_domain::sources::{Evidence, SourceText};
use tada_domain::work::FirmReason;

pub use self::edits::RecordEditInput;
pub use self::links::{Link, LinkedRecord, MAX_DUPLICATES};
pub use self::read::{
    ChangesetReview, ConflictReason, EXCERPT_CONTEXT, ProposalReview, get_changeset,
};
use crate::access::{self, AccessError, EventAccess, Principal};
use crate::audit::{AuditAction, AuditEvent};
use crate::caller::{Actor, MemberCaller, OrgScope};
use crate::clock::Clock;
use crate::facts::FactStore;
use crate::identity::IdentityStore;
use crate::paging::{Page, PageLimit};
use crate::parties::PartyStore;
use crate::problem::{CommandError, FieldError, ProblemCode};
use crate::proposals::{Changeset, FactStateInput, ProposalStore};
use crate::sources::SourceStore;
use crate::store::StoreError;
use crate::work::WorkStore;
use crate::workstreams::WorkstreamStore;

use self::checks::{check_edits, parse_edits, stale_owners};
use self::links::check_links;
use self::routing::{ChangesetRoutes, RoutedProposal, RoutingFacts, RoutingPorts};
use self::selection::{apply_order, proposal_status, selection, to_apply, with_dependents};
use self::steps::{audit_of, step, step_status};

/// An open proposal older than this shows as stale (ADR 0050). It does not change.
pub const STALE_AFTER: SignedDuration = SignedDuration::from_hours(14 * 24);

/// An open proposal older than this is overdue: the Review Inbox of the event managers shows it, even if it goes
/// to another reviewer (ADR 0067). The value is provisional, like `STALE_AFTER`.
pub const OVERDUE_AFTER: SignedDuration = SignedDuration::from_hours(3 * 24);

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

    /// True if the proposal is open and older than `OVERDUE_AFTER` (ADR 0067).
    pub fn is_overdue(self, created_at: Timestamp, now: Timestamp) -> bool {
        self == Self::Open && created_at < now - OVERDUE_AFTER
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
    /// Its open proposals, in the order of their IDs.
    pub proposals: Vec<OpenProposal>,
}

impl OpenChangeset {
    /// True if its open proposals are stale. All proposals of a changeset have its creation time.
    pub fn is_stale(&self, now: Timestamp) -> bool {
        ProposalStatus::Open.is_stale(self.created_at, now)
    }

    /// True if its open proposals are overdue (`OVERDUE_AFTER`).
    pub fn is_overdue(&self, now: Timestamp) -> bool {
        ProposalStatus::Open.is_overdue(self.created_at, now)
    }

    /// The reviewers of its open proposals.
    fn routes(&self, lookup: &RoutingFacts) -> ChangesetRoutes {
        ChangesetRoutes::new(
            self.proposals.iter().map(|proposal| RoutedProposal {
                id: proposal.id,
                operation: &proposal.operation,
                depends_on: &proposal.depends_on,
                open: true,
            }),
            self.event_id,
            lookup,
        )
    }
}

/// An open proposal of an open changeset: what the review routing needs of it.
#[derive(Clone, PartialEq, Eq)]
pub struct OpenProposal {
    pub id: ProposalId,
    pub operation: Operation,
    pub depends_on: Vec<ProposalId>,
}

/// An operation can hold personal data, so `Debug` shows the IDs only (ADR 0035).
impl Debug for OpenProposal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OpenProposal")
            .field("id", &self.id)
            .field("depends_on", &self.depends_on)
            .finish_non_exhaustive()
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
    /// The reviewer edited fields of a new record. The store keeps the edited operation as a source version of the
    /// kind `review`, with the reviewer as author. The record keeps the evidence of the proposal and the review text.
    RecordEdit(Vec<Evidence>),
    /// The reviewer linked a proposed person or institution to an existing record (ADR 0069).
    /// The store creates no record. It keeps `LinkedRecord::review_text` as a source version of the kind `review`,
    /// the evidence of the decision.
    Link(LinkedRecord),
}

/// One proposal of an apply, with the operation to apply. An edit changes the state of a `SetFact`
/// or the fields of a new record.
#[derive(Clone, PartialEq, Eq)]
pub struct ApplyStep {
    pub proposal_id: ProposalId,
    pub operation: Operation,
    pub evidence: StepEvidence,
    /// The reason of a proposal that makes a commitment firm. The commitment keeps it (ADR 0068).
    pub firm_reason: Option<FirmReason>,
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
            Operation::CreatePerson { .. } => "CreatePerson",
            Operation::CreateInstitution { .. } => "CreateInstitution",
            Operation::CreateAction { .. } => "CreateAction",
            Operation::CreateCommitment { .. } => "CreateCommitment",
            Operation::ChangeActionStatus { .. } => "ChangeActionStatus",
            Operation::ChangeActionDue { .. } => "ChangeActionDue",
            Operation::ChangeCommitmentStatus { .. } => "ChangeCommitmentStatus",
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
    /// The event of the changeset, `None` for a changeset of the organization.
    /// The review text of an edited person or institution belongs to it, so the members of the event read it.
    pub event_id: Option<EventId>,
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
    /// A person: `PER-<n>`, local to the organization.
    Person(PersonId),
    /// An institution: `INS-<n>`, local to the organization.
    Institution(InstitutionId),
    /// An action: `ACT-<n>`, local to its event.
    Action(ActionId),
    /// A commitment: `COM-<n>`, local to its event.
    Commitment(CommitmentId),
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

    /// The changesets with at least one open proposal, oldest first, each with its open proposals.
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
    /// The workstreams that an edit gives a new record, and the leads that the routing reads.
    pub workstreams: &'a dyn WorkstreamStore,
    /// The owners of actions and commitments that the routing reads (ADR 0067).
    pub work: &'a dyn WorkStore,
    /// The existing persons and institutions that a review shows as duplicates and links to (ADR 0069).
    pub parties: &'a dyn PartyStore,
}

/// The input of an apply: the selected proposals, the edited values and the links to existing records.
#[derive(Debug, Clone)]
pub struct ApplyInput {
    pub selected: Vec<ProposalId>,
    pub edits: Vec<Edit>,
    /// Proposed persons and institutions that the reviewer replaces with existing records (ADR 0069).
    pub links: Vec<Link>,
}

/// A change of the reviewer before the acceptance: the state of a proposal that sets a fact,
/// or the fields of a proposal that creates a work record or a party.
#[derive(Clone)]
pub struct Edit {
    pub proposal_id: ProposalId,
    pub state: Option<FactStateInput>,
    pub fields: Option<RecordEditInput>,
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
    /// The local IDs of the new records.
    pub local_ids: Vec<NewLocalId>,
}

#[derive(Debug, thiserror::Error)]
pub enum ApplyError {
    /// The changeset is not in the caller's organization, or the caller cannot see its event.
    #[error("the changeset does not exist or the caller cannot see it")]
    NotFound,
    /// The caller does not review the changeset, or not each proposal of the selection (ADR 0067).
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
///    The caller may review each of them (ADR 0067), else `forbidden`.
/// 4. Each link names a selected proposal that creates a person or an institution, and an existing record of the same
///    kind in the organization (ADR 0069). The other selected proposals use the linked record instead of the proposed one.
/// 5. Each target has the expected version. Else nothing changes, a separate transaction appends
///    `conflict` for the proposals concerned, and the result is `record-version-conflict`.
/// 6. A successful apply writes the records, the fact versions with their evidence, the draft versions with their
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
    let reviewable = reviewable(caller, changeset_id, stores).await?;
    let given = selection(&reviewable.changeset, &input.selected, "selected")?;
    let selected = to_apply(&reviewable.changeset.proposals, &given, &reviewable.results)?;
    reviewable.require(caller, &selected)?;
    let changeset = reviewable.changeset;
    let edits = parse_edits(&changeset, &selected, input.edits)?;
    check_edits(scope, &changeset, &selected, &edits, stores).await?;
    let links = check_links(
        scope,
        &changeset,
        &selected,
        &reviewable.results,
        &edits,
        input.links,
        stores.parties,
    )
    .await?;

    let now = clock.now();
    let steps: Vec<ApplyStep> = apply_order(&changeset.proposals, &selected)
        .into_iter()
        .map(|proposal| step(proposal, &edits, &links))
        .collect::<Result<_, _>>()?;
    // The owner of a new work record can lose the event role after the proposal (ADR 0068).
    let stale = stale_owners(scope, &steps, stores.identity).await?;
    if !stale.is_empty() {
        return Err(conflict(caller, changeset_id, stale, now, stores).await);
    }
    let audit = steps
        .iter()
        .flat_map(|step| audit_of(caller, step))
        .collect();
    let plan = ApplyPlan {
        changeset_id,
        event_id: changeset.event_id,
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
            Err(conflict(caller, changeset_id, proposals, now, stores).await)
        }
    }
}

/// Records the conflict of `proposals` and returns the error of the apply.
/// The conflict stays visible: a separate transaction records it (ADR 0050).
async fn conflict(
    caller: &MemberCaller,
    changeset_id: ChangesetId,
    proposals: Vec<ProposalId>,
    now: Timestamp,
    stores: ReviewStores<'_>,
) -> ApplyError {
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
    let _recorded = stores.review.record(caller.scope(), &batch).await;
    ApplyError::Conflict(proposals)
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
/// The caller may review each rejected proposal (ADR 0067), else `forbidden`.
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
    let reviewable = reviewable(caller, changeset_id, stores).await?;
    let given = selection(&reviewable.changeset, &ids, "proposal_ids")?;
    let is_open = |id: ProposalId| proposal_status(&reviewable.results, id) == ProposalStatus::Open;
    if !given.iter().all(|id| is_open(*id)) {
        return Err(ApplyError::InvalidTransition);
    }
    let rejected: Vec<ProposalId> = with_dependents(&reviewable.changeset.proposals, &given)
        .into_iter()
        .filter(|id| is_open(*id))
        .collect();
    reviewable.require(caller, &rejected)?;
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
    /// The caller sees the event, but reviews none of the proposals (ADR 0067).
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

/// One page of the changesets of the Review Inbox of the caller, oldest first: the changesets with at least one open
/// proposal that the review routing shows the caller (ADR 0067).
///
/// `Some(event)` lists the changesets of one event; a viewer of the event gets `forbidden`.
/// `None` lists the changesets of each event of the caller and, for owners and admins,
/// the changesets of the organization, for example a new event.
pub async fn list_open_changesets(
    caller: &MemberCaller,
    event_id: Option<EventId>,
    after: Option<ChangesetCursor>,
    limit: PageLimit,
    stores: ReviewStores<'_>,
    clock: &dyn Clock,
) -> Result<Page<InboxChangeset, ChangesetCursor>, ReviewQueryError> {
    if let Some(event) = event_id
        && !access::event_access(caller, event, stores.identity)
            .await?
            .can_propose()
    {
        return Err(ReviewQueryError::Forbidden);
    }
    let inbox = inbox(caller, event_id, stores.into(), clock.now()).await?;
    Ok(page(inbox, after, limit))
}

/// A changeset of the Review Inbox of the caller.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InboxChangeset {
    pub changeset: OpenChangeset,
    /// The number of its open proposals in the Review Inbox of the caller, at least one.
    /// "My Work" counts the same proposals.
    pub in_inbox: u32,
}

/// The ports that the Review Inbox reads.
#[derive(Debug, Clone, Copy)]
pub struct InboxPorts<'a> {
    pub identity: &'a dyn IdentityStore,
    pub review: &'a dyn ReviewStore,
    pub work: &'a dyn WorkStore,
    pub workstreams: &'a dyn WorkstreamStore,
}

impl<'a> InboxPorts<'a> {
    fn routing(self) -> RoutingPorts<'a> {
        RoutingPorts {
            identity: self.identity,
            work: self.work,
            workstreams: self.workstreams,
        }
    }
}

impl<'a> From<ReviewStores<'a>> for InboxPorts<'a> {
    fn from(stores: ReviewStores<'a>) -> Self {
        Self {
            identity: stores.identity,
            review: stores.review,
            work: stores.work,
            workstreams: stores.workstreams,
        }
    }
}

/// The number of open proposals in the Review Inbox of the caller, for "My Work".
pub async fn review_count(
    caller: &impl Principal,
    ports: InboxPorts<'_>,
    now: Timestamp,
) -> Result<u32, StoreError> {
    Ok(inbox(caller, None, ports, now)
        .await?
        .into_iter()
        .map(|item| item.in_inbox)
        .sum())
}

/// The open changesets of `event`, or of the organization for `None`, that have open proposals in the Review Inbox
/// of the caller, each with the number of these proposals. It reads the access of the caller and the current owners
/// and leads once for each event.
async fn inbox(
    caller: &impl Principal,
    event: Option<EventId>,
    ports: InboxPorts<'_>,
    now: Timestamp,
) -> Result<Vec<InboxChangeset>, StoreError> {
    let scope = caller.scope();
    let changesets = ports.review.open_changesets(scope, event).await?;
    let mut events: Vec<Option<EventId>> = changesets.iter().map(|c| c.event_id).collect();
    events.sort();
    events.dedup();
    let mut routing: HashMap<Option<EventId>, (EventAccess, RoutingFacts)> = HashMap::new();
    for event in events {
        let access = match changeset_access(caller, event, ports.identity).await {
            Ok(access) if access.can_propose() => access,
            Ok(_) | Err(ReviewQueryError::NotFound | ReviewQueryError::Forbidden) => continue,
            Err(ReviewQueryError::Store(error)) => return Err(error),
        };
        let facts = match event {
            Some(event) => {
                let operations = changesets
                    .iter()
                    .filter(|changeset| changeset.event_id == Some(event))
                    .flat_map(|changeset| &changeset.proposals)
                    .map(|proposal| &proposal.operation);
                RoutingFacts::load(scope, event, operations, ports.routing()).await?
            }
            None => RoutingFacts::default(),
        };
        routing.insert(event, (access, facts));
    }
    let user = caller.user_id();
    Ok(changesets
        .into_iter()
        .filter_map(|changeset| {
            let (access, facts) = routing.get(&changeset.event_id)?;
            let routes = changeset.routes(facts);
            let overdue = changeset.is_overdue(now);
            let count = changeset
                .proposals
                .iter()
                .filter(|proposal| routes.in_inbox_of(user, *access, proposal.id, overdue))
                .count();
            let in_inbox = u32::try_from(count).unwrap_or(u32::MAX);
            (in_inbox > 0).then_some(InboxChangeset {
                changeset,
                in_inbox,
            })
        })
        .collect())
}

/// The page after `after` of `changesets`, which are oldest first.
/// The access of the caller filters the changesets, so the page comes after the filter.
/// The open changesets of an organization are few, so the store gives them all.
fn page(
    changesets: Vec<InboxChangeset>,
    after: Option<ChangesetCursor>,
    limit: PageLimit,
) -> Page<InboxChangeset, ChangesetCursor> {
    let limit = limit.get() as usize;
    let mut items: Vec<InboxChangeset> = changesets
        .into_iter()
        .filter(|item| {
            let changeset = &item.changeset;
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
            created_at: last.changeset.created_at,
            id: last.changeset.id,
        });
    Page { items, next }
}

/// The access of the caller to the changesets of `event`, or for `None`, to the changesets of the organization:
/// owners and admins act as event managers there (ADR 0052). Other members cannot see them.
async fn changeset_access(
    caller: &impl Principal,
    event: Option<EventId>,
    identity: &dyn IdentityStore,
) -> Result<EventAccess, ReviewQueryError> {
    match event {
        Some(event) => Ok(access::event_access(caller, event, identity).await?),
        None if access::sees_all_events(caller) => Ok(EventAccess::Manager),
        None => Err(ReviewQueryError::NotFound),
    }
}

/// A changeset that the caller reviews at least in part, with its review results and the reviewers of each proposal.
struct Reviewable {
    changeset: Changeset,
    source: SourceText,
    results: Vec<ReviewRecord>,
    access: EventAccess,
    routes: ChangesetRoutes,
}

impl Reviewable {
    fn may_review(&self, caller: &MemberCaller, id: ProposalId) -> bool {
        self.routes.may_review(caller.user_id(), self.access, id)
    }

    fn routed_to(&self, caller: &MemberCaller, id: ProposalId) -> bool {
        self.routes.routed_to(caller.user_id(), self.access, id)
    }

    /// True if the caller reviews the proposal now: it is open and in the Review Inbox of the caller (rule 6).
    /// Apply and reject then accept it, because the inbox shows only proposals that the caller may review.
    fn can_review(&self, caller: &MemberCaller, id: ProposalId, now: Timestamp) -> bool {
        let status = proposal_status(&self.results, id);
        let overdue = status.is_overdue(self.changeset.created_at, now);
        status == ProposalStatus::Open
            && self
                .routes
                .in_inbox_of(caller.user_id(), self.access, id, overdue)
    }

    /// The caller must review each of `ids` (ADR 0067, rule 7). Else the review changes nothing.
    fn require<'a>(
        &self,
        caller: &MemberCaller,
        ids: impl IntoIterator<Item = &'a ProposalId>,
    ) -> Result<(), ApplyError> {
        if ids.into_iter().all(|id| self.may_review(caller, *id)) {
            Ok(())
        } else {
            Err(ApplyError::Forbidden)
        }
    }
}

/// The changeset with the text of its source version, if the caller reviews at least one of its proposals.
///
/// `NotFound` means that the caller cannot see the changeset. `Forbidden` means that the caller sees its event,
/// but reviews none of its proposals.
async fn reviewable(
    caller: &MemberCaller,
    id: ChangesetId,
    stores: ReviewStores<'_>,
) -> Result<Reviewable, ReviewQueryError> {
    let scope = caller.scope();
    let Some((changeset, source)) = stores.proposals.get(scope, id).await? else {
        return Err(ReviewQueryError::NotFound);
    };
    let access = changeset_access(caller, changeset.event_id, stores.identity).await?;
    let results = stores.review.results(scope, id).await?;
    let facts = match changeset.event_id {
        Some(event) => {
            let operations = changeset.proposals.iter().map(|p| &p.operation);
            let ports = InboxPorts::from(stores).routing();
            RoutingFacts::load(scope, event, operations, ports).await?
        }
        None => RoutingFacts::default(),
    };
    let routes = ChangesetRoutes::new(
        changeset.proposals.iter().map(|proposal| RoutedProposal {
            id: proposal.id,
            operation: &proposal.operation,
            depends_on: &proposal.depends_on,
            open: proposal_status(&results, proposal.id) == ProposalStatus::Open,
        }),
        changeset.event_id,
        &facts,
    );
    let reviewable = Reviewable {
        changeset,
        source,
        results,
        access,
        routes,
    };
    let reviews_some = reviewable
        .changeset
        .proposals
        .iter()
        .any(|proposal| reviewable.may_review(caller, proposal.id));
    if reviews_some {
        Ok(reviewable)
    } else {
        Err(ReviewQueryError::Forbidden)
    }
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
mod tests;
