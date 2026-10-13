//! The read tools for the work records of an event and for the persons and institutions (ADR 0064, ADR 0069).
//!
//! They call the queries with the `AiCaller`, so the agent reads what its member reads and no more.
//! The views leave out `can_change`, `next_statuses` and `can_make_firm`: they describe what the member can do
//! in tada, and an agent changes no record directly. It proposes, and a reviewer decides.
//! The email and phone of a party are the ones that the API shows to the same member.

use std::sync::LazyLock;

use async_trait::async_trait;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::events::EventInput;
use super::{AiToolError, AiToolPorts, TypedTool, find_event};
use tada_app::ai_queries;
use tada_app::caller::AiCaller;
use tada_app::dispute_notes::{DisputeNotes, NoteKey};
use tada_app::domain::events::EventKey;
use tada_app::domain::ids::{MilestoneId, UserId, WorkstreamId};
use tada_app::domain::parties::{Party, PartyName};
use tada_app::domain::redaction::Redactable;
use tada_app::domain::work::{
    ActionStatus, ActionTitle, CommitmentStatus, CommitmentText, WorkstreamName,
};
use tada_app::paging::PageLimit;
use tada_app::parties::{InstitutionView, PartyRef, PersonView};
use tada_app::records::{RecordEvidenceView, Shown};
use tada_app::search::MAX_QUERY_CHARS;
use tada_app::views::{DisputeNoteView, note_views};
use tada_app::work::{ActionView, CommitmentView, WorkQuery};
use tada_app::workstreams::Workstream;

/// The input of the tools that list the actions of an event.
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct ActionsInput {
    /// The key of the event, for example `FLY28`, from `list_events`.
    #[schemars(regex(pattern = EventKey::PATTERN))]
    event_key: String,
    /// Only the actions with this status.
    status: Option<ActionStatusInput>,
}

/// The input of the tools that list the commitments of an event.
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct CommitmentsInput {
    /// The key of the event, for example `FLY28`, from `list_events`.
    #[schemars(regex(pattern = EventKey::PATTERN))]
    event_key: String,
    /// Only the commitments with this status.
    status: Option<CommitmentStatusInput>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
enum ActionStatusInput {
    Open,
    InProgress,
    Blocked,
    Done,
    Canceled,
}

impl From<ActionStatusInput> for ActionStatus {
    fn from(status: ActionStatusInput) -> Self {
        match status {
            ActionStatusInput::Open => Self::Open,
            ActionStatusInput::InProgress => Self::InProgress,
            ActionStatusInput::Blocked => Self::Blocked,
            ActionStatusInput::Done => Self::Done,
            ActionStatusInput::Canceled => Self::Canceled,
        }
    }
}

#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
enum CommitmentStatusInput {
    Conditional,
    Firm,
    Fulfilled,
    Broken,
    Withdrawn,
}

impl From<CommitmentStatusInput> for CommitmentStatus {
    fn from(status: CommitmentStatusInput) -> Self {
        match status {
            CommitmentStatusInput::Conditional => Self::Conditional,
            CommitmentStatusInput::Firm => Self::Firm,
            CommitmentStatusInput::Fulfilled => Self::Fulfilled,
            CommitmentStatusInput::Broken => Self::Broken,
            CommitmentStatusInput::Withdrawn => Self::Withdrawn,
        }
    }
}

/// The input of `search_parties`.
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct PartiesInput {
    /// A part of a name. The search ignores case and punctuation.
    #[schemars(length(min = 1, max = MAX_QUERY_CHARS))]
    q: String,
}

/// The text of a redactable value: empty after a legal redaction (ADR 0072).
pub(super) fn text<T>(value: &Redactable<T>, as_str: impl Fn(&T) -> &str) -> String {
    value
        .as_present()
        .map(|value| as_str(value).to_owned())
        .unwrap_or_default()
}

#[derive(Debug, Serialize, JsonSchema)]
pub(super) struct WorkstreamList {
    workstreams: Vec<WorkstreamItem>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub(super) struct WorkstreamItem {
    id: Uuid,
    /// Empty after a legal redaction (ADR 0072).
    name: String,
    /// True after a legal redaction: the texts are empty (ADR 0072).
    redacted: bool,
    /// The member who leads the workstream.
    lead_user_id: Uuid,
    /// `active` or `closed`. A closed workstream takes no new records.
    status: &'static str,
    /// The record version.
    version: i64,
}

impl From<&Workstream> for WorkstreamItem {
    fn from(workstream: &Workstream) -> Self {
        Self {
            id: workstream.id.as_uuid(),
            name: text(&workstream.name, WorkstreamName::as_str),
            redacted: workstream.name.as_present().is_none(),
            lead_user_id: workstream.lead.as_uuid(),
            status: workstream.status.as_str(),
            version: workstream.version.get(),
        }
    }
}

/// A passage that supports one version of a record, from a source that the member can read.
#[derive(Debug, Serialize, JsonSchema)]
pub(super) struct EvidenceItem {
    record_version: i64,
    source_version_id: Uuid,
    start: u32,
    end: u32,
    /// Empty after a legal redaction of the source (ADR 0072).
    quote: String,
    /// True after a legal redaction of the source: the quote is empty (ADR 0072).
    redacted: bool,
    /// The dispute notes about the source (ADR 0073), oldest first: a person objects to it. Empty if there is none.
    dispute_notes: Vec<DisputeNoteView>,
}

impl EvidenceItem {
    fn new(evidence: &RecordEvidenceView, notes: &DisputeNotes) -> Self {
        Self {
            dispute_notes: note_views(notes, &NoteKey::source_version(evidence.source_version_id)),
            record_version: evidence.record_version.get(),
            source_version_id: evidence.source_version_id.as_uuid(),
            start: evidence.start_offset,
            end: evidence.end_offset,
            quote: text(&evidence.quote, String::as_str),
            redacted: evidence.quote.as_present().is_none(),
        }
    }
}

pub(super) fn evidence<T>(shown: &Shown<T>) -> Vec<EvidenceItem> {
    shown
        .evidence
        .iter()
        .map(|evidence| EvidenceItem::new(evidence, &shown.dispute_notes))
        .collect()
}

#[derive(Debug, Serialize, JsonSchema)]
pub(super) struct ActionList {
    actions: Vec<ActionItem>,
    /// True if the event has more actions than the list shows.
    more: bool,
}

#[derive(Debug, Serialize, JsonSchema)]
pub(super) struct ActionItem {
    id: Uuid,
    /// The readable ID, for example `ACT-001`.
    local_id: String,
    /// Empty after a legal redaction (ADR 0072).
    title: String,
    /// True after a legal redaction: the texts are empty (ADR 0072).
    redacted: bool,
    description: Option<String>,
    owner_user_id: Uuid,
    workstream_id: Option<Uuid>,
    /// Not after `due_date`.
    start_date: Option<String>,
    due_date: Option<String>,
    /// The ID of the milestone of the action, from `list_milestones`.
    milestone_id: Option<Uuid>,
    /// `open`, `in-progress`, `blocked`, `done` or `canceled`.
    status: &'static str,
    version: i64,
    evidence: Vec<EvidenceItem>,
}

impl From<&Shown<ActionView>> for ActionItem {
    fn from(shown: &Shown<ActionView>) -> Self {
        let action = &shown.record;
        let fields = &action.fields;
        Self {
            id: action.id.as_uuid(),
            local_id: action.local_id(),
            title: text(&fields.title, ActionTitle::as_str),
            redacted: fields.title.as_present().is_none(),
            description: fields
                .description
                .as_ref()
                .map(|text| text.as_str().to_owned()),
            owner_user_id: fields.owner.as_uuid(),
            workstream_id: fields.workstream_id.map(WorkstreamId::as_uuid),
            start_date: fields.start_date.map(|date| date.to_string()),
            due_date: fields.due_date.map(|date| date.to_string()),
            milestone_id: fields.milestone_id.map(MilestoneId::as_uuid),
            status: fields.status.as_str(),
            version: action.version.get(),
            evidence: evidence(shown),
        }
    }
}

#[derive(Debug, Serialize, JsonSchema)]
pub(super) struct CommitmentList {
    commitments: Vec<CommitmentItem>,
    /// True if the event has more commitments than the list shows.
    more: bool,
}

/// A person or an institution of the organization: the promisor of a commitment, or a party of a decision.
#[derive(Debug, Serialize, JsonSchema)]
pub(super) struct PartyItem {
    /// `person` or `institution`.
    kind: &'static str,
    id: Uuid,
    /// The readable ID, for example `INS-001`.
    local_id: String,
    /// Empty after a legal redaction (ADR 0072).
    name: String,
    /// True after a legal redaction: the texts are empty (ADR 0072).
    redacted: bool,
}

impl From<&PartyRef> for PartyItem {
    fn from(party: &PartyRef) -> Self {
        Self {
            kind: match party.party {
                Party::Person(_) => "person",
                Party::Institution(_) => "institution",
            },
            id: party.party.as_uuid(),
            local_id: party.local_id.clone(),
            name: text(&party.name, PartyName::as_str),
            redacted: party.name.as_present().is_none(),
        }
    }
}

#[derive(Debug, Serialize, JsonSchema)]
pub(super) struct CommitmentItem {
    id: Uuid,
    /// The readable ID, for example `COM-001`.
    local_id: String,
    /// Empty after a legal redaction (ADR 0072).
    text: String,
    /// True after a legal redaction: the texts are empty (ADR 0072).
    redacted: bool,
    /// The condition of a conditional promise. It stays after the commitment becomes firm.
    /// A condition is met only when evidence says so.
    condition: Option<String>,
    promisor: PartyItem,
    owner_user_id: Uuid,
    workstream_id: Option<Uuid>,
    due_date: Option<String>,
    /// `conditional`, `firm`, `fulfilled`, `broken` or `withdrawn`.
    status: &'static str,
    /// The reason that made the commitment firm.
    firm_reason: Option<String>,
    version: i64,
    evidence: Vec<EvidenceItem>,
}

impl From<&Shown<CommitmentView>> for CommitmentItem {
    fn from(shown: &Shown<CommitmentView>) -> Self {
        let commitment = &shown.record;
        let fields = &commitment.fields;
        Self {
            id: commitment.id.as_uuid(),
            local_id: commitment.local_id(),
            text: text(&fields.text, CommitmentText::as_str),
            redacted: fields.text.as_present().is_none(),
            condition: commitment
                .condition
                .as_ref()
                .map(|text| text.as_str().to_owned()),
            promisor: PartyItem::from(&commitment.promisor),
            owner_user_id: fields.owner.as_uuid(),
            workstream_id: fields.workstream_id.map(WorkstreamId::as_uuid),
            due_date: fields.due_date.map(|date| date.to_string()),
            status: fields.status.as_str(),
            firm_reason: fields
                .firm_reason
                .as_ref()
                .map(|text| text.as_str().to_owned()),
            version: commitment.version.get(),
            evidence: evidence(shown),
        }
    }
}

#[derive(Debug, Serialize, JsonSchema)]
pub(super) struct PartyList {
    persons: Vec<PersonItem>,
    institutions: Vec<InstitutionItem>,
    /// True if more persons or institutions match than the lists show: use a longer search text.
    more: bool,
}

#[derive(Debug, Serialize, JsonSchema)]
pub(super) struct PersonItem {
    id: Uuid,
    /// The readable ID, for example `PER-001`.
    local_id: String,
    /// Empty after a legal redaction (ADR 0072).
    name: String,
    /// True after a legal redaction: the texts are empty (ADR 0072).
    redacted: bool,
    email: Option<String>,
    phone: Option<String>,
    /// The account of the person, if the person is a member.
    user_id: Option<Uuid>,
    version: i64,
    evidence: Vec<EvidenceItem>,
}

impl From<&Shown<PersonView>> for PersonItem {
    fn from(shown: &Shown<PersonView>) -> Self {
        let person = &shown.record;
        Self {
            id: person.id.as_uuid(),
            local_id: person.local_id(),
            name: text(&person.name, PartyName::as_str),
            redacted: person.name.as_present().is_none(),
            email: person.email.as_ref().map(|email| email.as_str().to_owned()),
            phone: person.phone.as_ref().map(|phone| phone.as_str().to_owned()),
            user_id: person.user_id.map(UserId::as_uuid),
            version: person.version.get(),
            evidence: evidence(shown),
        }
    }
}

#[derive(Debug, Serialize, JsonSchema)]
pub(super) struct InstitutionItem {
    id: Uuid,
    /// The readable ID, for example `INS-001`.
    local_id: String,
    /// Empty after a legal redaction (ADR 0072).
    name: String,
    /// True after a legal redaction: the texts are empty (ADR 0072).
    redacted: bool,
    /// `authority`, `company`, `club` or `other`.
    kind: &'static str,
    email: Option<String>,
    phone: Option<String>,
    version: i64,
    evidence: Vec<EvidenceItem>,
}

impl From<&Shown<InstitutionView>> for InstitutionItem {
    fn from(shown: &Shown<InstitutionView>) -> Self {
        let institution = &shown.record;
        Self {
            id: institution.id.as_uuid(),
            local_id: institution.local_id(),
            name: text(&institution.name, PartyName::as_str),
            redacted: institution.name.as_present().is_none(),
            kind: institution.kind.as_str(),
            email: institution
                .email
                .as_ref()
                .map(|email| email.as_str().to_owned()),
            phone: institution
                .phone
                .as_ref()
                .map(|phone| phone.as_str().to_owned()),
            version: institution.version.get(),
            evidence: evidence(shown),
        }
    }
}

/// The description of a list tool with a page of `PageLimit::MAX` records in the order of their numbers.
pub(super) fn list_description(what: &str, more: &str) -> String {
    list_description_in_order(what, "in the order of their numbers", more)
}

/// The description of a list tool with a page of `PageLimit::MAX` records in the order `order`.
pub(super) fn list_description_in_order(what: &str, order: &str, more: &str) -> String {
    format!(
        "{what} The list holds at most {} records, {order}, and has no next page. \
If more is true, {more}: tell the member.",
        PageLimit::MAX
    )
}

static LIST_ACTIONS: LazyLock<String> = LazyLock::new(|| {
    list_description(
        "List the actions of an event, each with its owner, status and the evidence of the accepted proposals that made it.",
        "the event has more actions than this tool can show",
    )
});

static LIST_COMMITMENTS: LazyLock<String> = LazyLock::new(|| {
    list_description(
        "List the commitments of an event: what a person or institution promised. \
A conditional commitment has a condition and stays conditional until a reviewer makes it firm. Never treat a condition as met without evidence.",
        "the event has more commitments than this tool can show",
    )
});

/// The first page of the largest size, without a filter but the status.
pub(super) fn query<S, C>(status: Option<S>) -> WorkQuery<S, C> {
    WorkQuery {
        owner: None,
        status,
        workstream: None,
        after: None,
        limit: PageLimit::LARGEST,
    }
}

/// The search text has the same bounds as the `q` of the HTTP endpoint: 1 to `MAX_QUERY_CHARS` characters.
fn check_query(q: &str) -> Result<(), AiToolError> {
    let code = match q.chars().count() {
        0 => "empty",
        n if n > MAX_QUERY_CHARS => "too-long",
        _ => return Ok(()),
    };
    Err(super::invalid("q", code))
}

pub(super) struct ListWorkstreams;

#[async_trait]
impl TypedTool for ListWorkstreams {
    type Input = EventInput;
    type Output = WorkstreamList;
    const NAME: &'static str = "list_workstreams";
    const READ_ONLY: bool = true;

    fn description() -> &'static str {
        "List the workstreams of an event: its areas of work, each with its lead. \
A work record names a workstream by its ID. Propose a record in an active workstream, or in a new workstream of the same changeset (create-workstream); a closed workstream takes no new records."
    }

    async fn run(
        &self,
        caller: &AiCaller,
        input: EventInput,
        ports: &AiToolPorts,
    ) -> Result<WorkstreamList, AiToolError> {
        let event = find_event(caller, &input.event_key, ports).await?;
        let workstreams = ai_queries::list_workstreams(caller, event.id, ports).await?;
        Ok(WorkstreamList {
            workstreams: workstreams.iter().map(WorkstreamItem::from).collect(),
        })
    }
}

pub(super) struct ListActions;

#[async_trait]
impl TypedTool for ListActions {
    type Input = ActionsInput;
    type Output = ActionList;
    const NAME: &'static str = "list_actions";
    const READ_ONLY: bool = true;

    fn description() -> &'static str {
        &LIST_ACTIONS
    }

    async fn run(
        &self,
        caller: &AiCaller,
        input: ActionsInput,
        ports: &AiToolPorts,
    ) -> Result<ActionList, AiToolError> {
        let event = find_event(caller, &input.event_key, ports).await?;
        let query = query(input.status.map(ActionStatus::from));
        let page = ai_queries::list_actions(caller, event.id, query, ports).await?;
        Ok(ActionList {
            actions: page.items.iter().map(ActionItem::from).collect(),
            more: page.next.is_some(),
        })
    }
}

pub(super) struct ListCommitments;

#[async_trait]
impl TypedTool for ListCommitments {
    type Input = CommitmentsInput;
    type Output = CommitmentList;
    const NAME: &'static str = "list_commitments";
    const READ_ONLY: bool = true;

    fn description() -> &'static str {
        &LIST_COMMITMENTS
    }

    async fn run(
        &self,
        caller: &AiCaller,
        input: CommitmentsInput,
        ports: &AiToolPorts,
    ) -> Result<CommitmentList, AiToolError> {
        let event = find_event(caller, &input.event_key, ports).await?;
        let query = query(input.status.map(CommitmentStatus::from));
        let page = ai_queries::list_commitments(caller, event.id, query, ports).await?;
        Ok(CommitmentList {
            commitments: page.items.iter().map(CommitmentItem::from).collect(),
            more: page.next.is_some(),
        })
    }
}

pub(super) struct SearchParties;

#[async_trait]
impl TypedTool for SearchParties {
    type Input = PartiesInput;
    type Output = PartyList;
    const NAME: &'static str = "search_parties";
    const READ_ONLY: bool = true;

    fn description() -> &'static str {
        "Search the persons and institutions of the organization by a part of their name. \
Search here before you propose a new person or institution: use the ID of an existing one as a promisor instead of a duplicate. \
Each list holds the first matches in the order of their numbers; if more is true, use a longer search text."
    }

    async fn run(
        &self,
        caller: &AiCaller,
        input: PartiesInput,
        ports: &AiToolPorts,
    ) -> Result<PartyList, AiToolError> {
        check_query(&input.q)?;
        let limit = PageLimit::LARGEST;
        let persons = ai_queries::list_persons(caller, Some(&input.q), None, limit, ports).await?;
        let institutions =
            ai_queries::list_institutions(caller, Some(&input.q), None, limit, ports).await?;
        Ok(PartyList {
            persons: persons.items.iter().map(PersonItem::from).collect(),
            institutions: institutions
                .items
                .iter()
                .map(InstitutionItem::from)
                .collect(),
            more: persons.next.is_some() || institutions.next.is_some(),
        })
    }
}
