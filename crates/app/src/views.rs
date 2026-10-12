//! The JSON views of the read models for the AI clients of members (ADR 0040).
//!
//! The domain crate has no `serde` and no `schemars`, so this code module maps the read models to
//! types that serialize and that give the JSON Schemas of the MCP tools.
//! A fact value has the shape of `ValueInput`, so an agent reads values in the shape that it proposes them.

use jiff::Timestamp;
use schemars::JsonSchema;
use serde::Serialize;
use serde_json::Value;
use tada_domain::events::Event;
use tada_domain::facts::{
    CORE_CATALOG_VERSION, FactState, FieldDefinition, FieldStatus, Label, Valued,
};
use tada_domain::redaction::Redactable;
use tada_domain::work::{ActionStatus, CommitmentStatus, WorkstreamStatus};
use uuid::Uuid;

use crate::caller::Actor;
use crate::dispute_notes::{DisputeNotes, NoteKey};
use crate::documents::{DocumentView, StoredDraft, VersionContent, VersionView};
use crate::event_members::EventMember;
use crate::facts::{
    DatedEvidence, EventProfile, OpenProposalRef, OpenQuestionRef, ProfileEntry, value_schema,
};
use crate::proposals::ValueInput;
use crate::sources::{SourceHit, SourcePassage};

/// A dispute note: the objection of a person to a fact version or to a source (ADR 0073).
#[derive(Clone, Serialize, JsonSchema)]
pub struct DisputeNoteView {
    pub text: String,
    /// RFC 3339.
    #[schemars(with = "String")]
    pub created_at: Timestamp,
}

/// The text can name a person, so `Debug` shows the time only (ADR 0035).
impl std::fmt::Debug for DisputeNoteView {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DisputeNoteView")
            .field("created_at", &self.created_at)
            .finish_non_exhaustive()
    }
}

/// The notes of `key` as a view shows them.
pub fn note_views(notes: &DisputeNotes, key: &NoteKey) -> Vec<DisputeNoteView> {
    notes
        .of(key)
        .iter()
        .map(|note| DisputeNoteView {
            text: note.text.as_str().to_owned(),
            created_at: note.created_at,
        })
        .collect()
}

/// An event that the caller can read.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct EventView {
    pub event_id: Uuid,
    /// The short key of the event, for example `FLY28`. The other tools take it.
    pub event_key: String,
    pub name: String,
    /// The IANA time zone of the event.
    pub time_zone: String,
}

impl From<&Event> for EventView {
    fn from(event: &Event) -> Self {
        Self {
            event_id: event.id.as_uuid(),
            event_key: event.key.as_str().to_owned(),
            name: event.name.as_str().to_owned(),
            time_zone: event.time_zone.as_str().to_owned(),
        }
    }
}

/// The events that the caller can read, in the order of their keys.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct EventList {
    /// The user ID of the member who uses the tool. Use it as `owner` to assign work to this member;
    /// `list_members` gives the user IDs of the other members of an event.
    pub caller_user_id: Uuid,
    pub events: Vec<EventView>,
    /// True if the caller can read more events than the list shows.
    pub more: bool,
}

/// A member of an event: what each reader of the event also sees in the web client.
/// It has no email address.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct EventMemberView {
    /// The user ID. Use it as `owner` of a question, action or commitment.
    pub user_id: Uuid,
    pub display_name: String,
    /// `event-manager`, `event-contributor` or `event-viewer`. A member with one of the first two can own work; an owner or admin of the organization can also own work without an event role.
    pub event_role: String,
}

impl From<&EventMember> for EventMemberView {
    fn from(member: &EventMember) -> Self {
        Self {
            user_id: member.user_id.as_uuid(),
            display_name: member.display_name.as_str().to_owned(),
            event_role: member.event_role.as_str().to_owned(),
        }
    }
}

/// The members of an event, in the order of their display names.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct EventMemberList {
    pub members: Vec<EventMemberView>,
}

/// A kind of record of tada, for the schema of an event.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct EntityView {
    pub name: &'static str,
    pub description: &'static str,
    /// The statuses of a record of this kind. It is absent for a kind without a status.
    #[serde(skip_serializing_if = "<[_]>::is_empty")]
    pub statuses: &'static [&'static str],
}

const ACTION_STATUSES: [&str; 5] = [
    ActionStatus::Open.as_str(),
    ActionStatus::InProgress.as_str(),
    ActionStatus::Blocked.as_str(),
    ActionStatus::Done.as_str(),
    ActionStatus::Canceled.as_str(),
];

const COMMITMENT_STATUSES: [&str; 5] = [
    CommitmentStatus::Conditional.as_str(),
    CommitmentStatus::Firm.as_str(),
    CommitmentStatus::Fulfilled.as_str(),
    CommitmentStatus::Broken.as_str(),
    CommitmentStatus::Withdrawn.as_str(),
];

const WORKSTREAM_STATUSES: [&str; 2] = [
    WorkstreamStatus::Active.as_str(),
    WorkstreamStatus::Closed.as_str(),
];

/// The kinds of records that an agent can read (ADR 0049, ADR 0068, ADR 0069).
const ENTITIES: [EntityView; 9] = [
    EntityView {
        name: "event",
        description: "One occurrence of an event, with its key, name and time zone. Facts, open questions, documents, workstreams, actions and commitments belong to it.",
        statuses: &[],
    },
    EntityView {
        name: "fact",
        description: "The value of one field in one event, with a state: accepted, assumption or unknown. Each fact version cites the source passages that support it.",
        statuses: &[],
    },
    EntityView {
        name: "open_question",
        description: "A question that the team must answer, with an owner and an event-local ID such as QST-001.",
        statuses: &[],
    },
    EntityView {
        name: "source_version",
        description: "An immutable text, for example the words of a member or a document. Evidence and citations point to a range of characters in it.",
        statuses: &[],
    },
    EntityView {
        name: "workstream",
        description: "A part of the work of an event with a lead, for example ground operations. A closed workstream takes no new actions or commitments.",
        statuses: &WORKSTREAM_STATUSES,
    },
    EntityView {
        name: "action",
        description: "A task of an event with one owner, an optional workstream and an optional due date, and an event-local ID such as ACT-001. Transitions: open and in-progress change into each other; open and in-progress change to blocked; blocked changes to open or in-progress; open, in-progress and blocked change to done or canceled; done changes to open; canceled is final.",
        statuses: &ACTION_STATUSES,
    },
    EntityView {
        name: "commitment",
        description: "A promise of one person or one institution in an event, with the owner who follows it up and an event-local ID such as COM-001. With a condition it starts conditional, else firm. Only evidence that the condition is met makes it firm; never invent a signature or an approval. Conditional changes to firm; conditional and firm change to fulfilled, broken or withdrawn, which are final.",
        statuses: &COMMITMENT_STATUSES,
    },
    EntityView {
        name: "person",
        description: "A person of the organization that a commitment names, with an ID such as PER-001. Search the persons before you propose a new one.",
        statuses: &[],
    },
    EntityView {
        name: "institution",
        description: "An authority, a company, a club or another body of the organization that a commitment names, with an ID such as INS-001. Search the institutions before you propose a new one.",
        statuses: &[],
    },
];

/// The field catalog of an event and the kinds of records, for an agent that reads or proposes facts.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct EventSchema {
    pub event_key: String,
    /// The version of the shipped `core` catalog.
    pub catalog_version: u32,
    pub entities: Vec<EntityView>,
    /// The active fields. A deprecated field is closed for new facts, so the list leaves it out.
    pub fields: Vec<FieldView>,
}

/// One active field of the catalog of an event.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct FieldView {
    pub field_id: Uuid,
    /// The stable `snake_case` key, for example `visitor_estimate`.
    pub key: String,
    /// The German label of a field of the event. A shipped field has its label in the web client only.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// The meaning of the field, in English.
    pub description: String,
    pub module: String,
    /// The JSON Schema of the `value` of a fact of this field.
    pub value_schema: Value,
}

impl EventSchema {
    pub fn new(event: &Event, catalog: &[FieldDefinition]) -> Self {
        Self {
            event_key: event.key.as_str().to_owned(),
            catalog_version: CORE_CATALOG_VERSION,
            entities: ENTITIES.to_vec(),
            fields: catalog
                .iter()
                .filter(|field| field.status == FieldStatus::Active)
                .map(FieldView::from)
                .collect(),
        }
    }
}

impl From<&FieldDefinition> for FieldView {
    fn from(field: &FieldDefinition) -> Self {
        Self {
            field_id: field.id.as_uuid(),
            key: field.key.as_str().to_owned(),
            label: match &field.label {
                Label::Text(text) => Some(text.as_str().to_owned()),
                Label::Builtin(_) => None,
            },
            description: field.description.as_str().to_owned(),
            module: field.module.as_str().to_owned(),
            value_schema: value_schema(&field.value_type),
        }
    }
}

/// The event profile with each state in its own list (ADR 0049, ADR 0050).
/// Only `accepted` is confirmed state. Open proposals are not accepted state.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct ProfileView {
    pub event_key: String,
    /// The facts that the team confirmed.
    pub accepted: Vec<FactView>,
    /// The facts that the team uses for planning but did not confirm.
    pub assumptions: Vec<FactView>,
    /// The facts that nobody knows yet. Never fill in a value for them.
    /// A field without a fact has no entry here; compare the list with `get_event_schema`.
    pub unknowns: Vec<UnknownView>,
    pub open_questions: Vec<OpenQuestionView>,
    /// The proposals without a review. They are not accepted state.
    pub open_proposals: Vec<OpenProposalView>,
}

/// The current version of a fact with a value.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct FactView {
    pub field_key: String,
    pub fact_id: Uuid,
    /// The number of the current fact version.
    pub version: i64,
    /// Absent after a legal redaction (ADR 0072).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<ValueInput>,
    /// True for an approximate value, for example "about 20,000".
    pub approximate: bool,
    /// True after a legal redaction: the value is gone (ADR 0072).
    pub redacted: bool,
    pub evidence: Vec<EvidenceView>,
    /// The dispute notes about it (ADR 0073), oldest first: a person objects to it. Absent if there is none.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub dispute_notes: Vec<DisputeNoteView>,
    /// The reviewer who accepted the current fact version.
    pub accepted_by: AuthorView,
    /// RFC 3339. The time of the acceptance of the current fact version.
    #[schemars(with = "String")]
    pub accepted_at: Timestamp,
}

/// The current version of a fact whose value nobody knows.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct UnknownView {
    pub field_key: String,
    pub fact_id: Uuid,
    pub version: i64,
    pub evidence: Vec<EvidenceView>,
    /// The dispute notes about it (ADR 0073), oldest first: a person objects to it. Absent if there is none.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub dispute_notes: Vec<DisputeNoteView>,
    /// The reviewer who accepted the current fact version.
    pub accepted_by: AuthorView,
    /// RFC 3339. The time of the acceptance of the current fact version.
    #[schemars(with = "String")]
    pub accepted_at: Timestamp,
}

/// Who did something, for which member and through which channel (ADR 0039).
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct AuthorView {
    /// `member`, `service` or `ai`. The list of kinds is open.
    pub kind: &'static str,
    /// The user ID of a member or an AI client, or the ID of a service identity.
    pub id: Uuid,
    /// The member for whom the author acts, if the author is not that member.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub principal_id: Option<Uuid>,
    /// `web`, `telegram`, `job`, `api-token` or `cli`. The list of channels is open.
    pub channel: &'static str,
}

impl From<Actor> for AuthorView {
    fn from(actor: Actor) -> Self {
        Self {
            kind: actor.kind().as_str(),
            id: actor.id(),
            principal_id: actor.principal(),
            channel: actor.channel().as_str(),
        }
    }
}

/// A passage of a source version that supports a fact version.
#[derive(Clone, Serialize, JsonSchema)]
pub struct EvidenceView {
    pub source_version_id: Uuid,
    /// The offset of the first character, in characters of the normalized text.
    pub start: u32,
    /// The offset after the last character.
    pub end: u32,
    /// Empty after a legal redaction of the source (ADR 0072).
    pub quote: String,
    /// True after a legal redaction of the source: the quote is empty (ADR 0072).
    pub redacted: bool,
    /// The page of a PDF, from 1.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page: Option<u32>,
    /// RFC 3339. The time when tada captured the source version.
    #[schemars(with = "String")]
    pub captured_at: Timestamp,
    /// The dispute notes about it (ADR 0073), oldest first: a person objects to it. Absent if there is none.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub dispute_notes: Vec<DisputeNoteView>,
}

/// The quote can contain personal data, so `Debug` shows the range only (ADR 0035).
impl std::fmt::Debug for EvidenceView {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EvidenceView")
            .field("source_version_id", &self.source_version_id)
            .field("start", &self.start)
            .field("end", &self.end)
            .finish_non_exhaustive()
    }
}

/// An open question of the event.
#[derive(Clone, Serialize, JsonSchema)]
pub struct OpenQuestionView {
    pub open_question_id: Uuid,
    /// The event-local ID, for example `QST-001` (ADR 0038).
    pub readable_id: String,
    /// Empty after a legal redaction (ADR 0072).
    pub text: String,
    /// True after a legal redaction: the text is gone (ADR 0072).
    pub redacted: bool,
    /// The user ID of the member who owns the question.
    pub owner_user_id: Uuid,
    pub version: i64,
}

/// The text can contain personal data, so `Debug` shows the IDs only (ADR 0035).
impl std::fmt::Debug for OpenQuestionView {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OpenQuestionView")
            .field("open_question_id", &self.open_question_id)
            .field("readable_id", &self.readable_id)
            .finish_non_exhaustive()
    }
}

/// An open proposal that sets the fact of a field. It is not accepted state.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct OpenProposalView {
    pub proposal_id: Uuid,
    pub changeset_id: Uuid,
    pub field_id: Uuid,
    /// The key of the field, if the catalog of the event has it. A field that the same changeset proposes has none yet.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub field_key: Option<String>,
    /// The proposed state: `accepted`, `assumption` or `unknown`.
    pub proposed_state: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<ValueInput>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub approximate: Option<bool>,
    /// The fact version that the proposal expects. Absent if the field had no fact.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_version: Option<i64>,
    /// RFC 3339.
    #[schemars(with = "String")]
    pub created_at: Timestamp,
}

impl ProfileView {
    /// The view of `profile` of `event`. `catalog` gives the keys of the fields of the open proposals.
    pub fn new(event: &Event, profile: &EventProfile, catalog: &[FieldDefinition]) -> Self {
        let mut view = Self {
            event_key: event.key.as_str().to_owned(),
            accepted: Vec::new(),
            assumptions: Vec::new(),
            unknowns: Vec::new(),
            open_questions: profile.open_questions.iter().map(question_view).collect(),
            open_proposals: profile
                .proposals
                .iter()
                .map(|proposal| proposal_view(proposal, catalog))
                .collect(),
        };
        for entry in &profile.fields {
            match &entry.state {
                FactState::Accepted(valued) => {
                    view.accepted
                        .push(fact_view(entry, valued, &profile.dispute_notes));
                }
                FactState::Assumption(valued) => {
                    view.assumptions
                        .push(fact_view(entry, valued, &profile.dispute_notes));
                }
                FactState::Unknown => view.unknowns.push(UnknownView {
                    field_key: entry.field.key.as_str().to_owned(),
                    fact_id: entry.fact_id.as_uuid(),
                    version: entry.version.get(),
                    evidence: evidence_views(entry, &profile.dispute_notes),
                    dispute_notes: note_views(
                        &profile.dispute_notes,
                        &NoteKey::fact_number(entry.fact_id, entry.version),
                    ),
                    accepted_by: entry.accepted_by.into(),
                    accepted_at: entry.accepted_at,
                }),
            }
        }
        view
    }
}

fn fact_view(entry: &ProfileEntry, valued: &Redactable<Valued>, notes: &DisputeNotes) -> FactView {
    FactView {
        field_key: entry.field.key.as_str().to_owned(),
        fact_id: entry.fact_id.as_uuid(),
        version: entry.version.get(),
        value: valued
            .as_present()
            .map(|valued| ValueInput::from(&valued.value)),
        approximate: valued.as_present().is_some_and(|valued| valued.approximate),
        redacted: valued.as_present().is_none(),
        evidence: evidence_views(entry, notes),
        dispute_notes: note_views(notes, &NoteKey::fact_number(entry.fact_id, entry.version)),
        accepted_by: entry.accepted_by.into(),
        accepted_at: entry.accepted_at,
    }
}

fn evidence_views(entry: &ProfileEntry, notes: &DisputeNotes) -> Vec<EvidenceView> {
    entry
        .evidence
        .iter()
        .map(|dated| evidence_view(dated, notes))
        .collect()
}

/// The one mapping of an evidence link to its view.
fn evidence_view(dated: &DatedEvidence, notes: &DisputeNotes) -> EvidenceView {
    let evidence = &dated.evidence;
    EvidenceView {
        source_version_id: evidence.source_version_id.as_uuid(),
        start: evidence.passage.start,
        end: evidence.passage.end,
        quote: evidence
            .passage
            .quote
            .as_present()
            .cloned()
            .unwrap_or_default(),
        redacted: evidence.passage.quote == Redactable::Redacted,
        page: evidence.passage.page,
        captured_at: dated.captured_at,
        dispute_notes: note_views(notes, &NoteKey::source_version(evidence.source_version_id)),
    }
}

fn question_view(question: &OpenQuestionRef) -> OpenQuestionView {
    OpenQuestionView {
        open_question_id: question.id.as_uuid(),
        readable_id: question.readable_id(),
        text: question
            .text
            .as_present()
            .map(|text| text.as_str().to_owned())
            .unwrap_or_default(),
        redacted: question.text == Redactable::Redacted,
        owner_user_id: question.owner.as_uuid(),
        version: question.version.get(),
    }
}

fn proposal_view(proposal: &OpenProposalRef, catalog: &[FieldDefinition]) -> OpenProposalView {
    let (proposed_state, valued) = match &proposal.state {
        FactState::Accepted(valued) => ("accepted", Some(valued)),
        FactState::Assumption(valued) => ("assumption", Some(valued)),
        FactState::Unknown => ("unknown", None),
    };
    OpenProposalView {
        proposal_id: proposal.proposal_id.as_uuid(),
        changeset_id: proposal.changeset_id.as_uuid(),
        field_id: proposal.field_id.as_uuid(),
        field_key: catalog
            .iter()
            .find(|field| field.id == proposal.field_id)
            .map(|field| field.key.as_str().to_owned()),
        proposed_state,
        value: valued.map(|valued| ValueInput::from(&valued.value)),
        approximate: valued.map(|valued| valued.approximate),
        expected_version: proposal.expected_version.map(|version| version.get()),
        created_at: proposal.created_at,
    }
}

/// The hits of a search, the best matches first.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct SearchResult {
    pub hits: Vec<SearchHitView>,
}

/// A source version that contains the words of the search.
#[derive(Clone, Serialize, JsonSchema)]
pub struct SearchHitView {
    pub source_version_id: Uuid,
    /// RFC 3339.
    #[schemars(with = "String")]
    pub captured_at: Timestamp,
    /// The text from `start` to `end`. Cite it as a passage of the source version.
    pub snippet: String,
    pub start: u32,
    pub end: u32,
}

/// The snippet can contain personal data, so `Debug` shows the range only (ADR 0035).
impl std::fmt::Debug for SearchHitView {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SearchHitView")
            .field("source_version_id", &self.source_version_id)
            .field("start", &self.start)
            .field("end", &self.end)
            .finish_non_exhaustive()
    }
}

impl From<&SourceHit> for SearchHitView {
    fn from(hit: &SourceHit) -> Self {
        Self {
            source_version_id: hit.source_version_id.as_uuid(),
            captured_at: hit.captured_at,
            snippet: hit.snippet.clone(),
            start: hit.start,
            end: hit.end,
        }
    }
}

/// The exact text of a range of a source version, for a citation.
#[derive(Clone, Serialize, JsonSchema)]
pub struct PassageView {
    pub source_version_id: Uuid,
    pub start: u32,
    pub end: u32,
    pub quote: String,
}

/// The quote can contain personal data, so `Debug` shows the range only (ADR 0035).
impl std::fmt::Debug for PassageView {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PassageView")
            .field("source_version_id", &self.source_version_id)
            .field("start", &self.start)
            .field("end", &self.end)
            .finish_non_exhaustive()
    }
}

impl From<&SourcePassage> for PassageView {
    fn from(passage: &SourcePassage) -> Self {
        Self {
            source_version_id: passage.source_version_id.as_uuid(),
            start: passage.start,
            end: passage.end,
            quote: passage.quote.clone(),
        }
    }
}

/// The documents of an event, the newest first.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct DocumentList {
    pub documents: Vec<DocumentSummaryView>,
    /// True if the event has older documents than the list shows. The list has no next page.
    pub more: bool,
}

/// A document with its newest version.
#[derive(Clone, Serialize, JsonSchema)]
pub struct DocumentSummaryView {
    pub document_id: Uuid,
    /// The readable ID, for example `DOC-001`.
    pub readable_id: String,
    /// Empty after a legal redaction (ADR 0072).
    pub name: String,
    /// True after a legal redaction: the name is gone (ADR 0072).
    pub redacted: bool,
    /// The record version of the document. A draft of this document expects it as `expected_version`.
    pub version: i64,
    pub newest_version: VersionSummaryView,
}

/// The name can contain personal data, so `Debug` leaves it out (ADR 0035).
impl std::fmt::Debug for DocumentSummaryView {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DocumentSummaryView")
            .field("document_id", &self.document_id)
            .field("version", &self.version)
            .finish_non_exhaustive()
    }
}

impl From<&DocumentView> for DocumentSummaryView {
    fn from(document: &DocumentView) -> Self {
        Self {
            document_id: document.id.as_uuid(),
            readable_id: document.readable_id(),
            name: document.name.as_present().cloned().unwrap_or_default(),
            redacted: document.name.as_present().is_none(),
            version: document.version.get(),
            newest_version: VersionSummaryView::from(&document.newest_version),
        }
    }
}

/// One version of a document. Only a draft has a status and Markdown; an upload is a file.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct VersionSummaryView {
    pub version_id: Uuid,
    pub number: u32,
    /// `upload` or `draft`.
    pub kind: &'static str,
    /// The status of a draft: `draft`, `review`, `approved`, `superseded` or `archived`. Absent for an upload.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<&'static str>,
    /// True after a legal redaction: the file or the Markdown is gone (ADR 0072).
    pub redacted: bool,
}

impl From<&VersionView> for VersionSummaryView {
    fn from(version: &VersionView) -> Self {
        let (kind, status) = match &version.content {
            VersionContent::Upload(_) => ("upload", None),
            VersionContent::Draft { status } => ("draft", Some(status.as_str())),
        };
        Self {
            version_id: version.id.as_uuid(),
            number: version.number,
            kind,
            status,
            redacted: version.redacted,
        }
    }
}

/// A draft version with its Markdown and its provenance manifest, so that an agent can write the next version.
#[derive(Clone, Serialize, JsonSchema)]
pub struct DraftVersionView {
    pub version_id: Uuid,
    pub document_id: Uuid,
    pub number: u32,
    /// `draft`, `review`, `approved`, `superseded` or `archived`. An approved version never changes.
    pub status: &'static str,
    /// Empty after a legal redaction (ADR 0072).
    pub markdown: String,
    /// True after a legal redaction: the Markdown is gone (ADR 0072).
    pub redacted: bool,
    pub manifest: ManifestView,
    /// The dispute notes about the version (ADR 0073), oldest first: a person objects to it. Absent if there is none.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub dispute_notes: Vec<DisputeNoteView>,
}

/// The Markdown is document content, so `Debug` leaves it out (ADR 0035).
impl std::fmt::Debug for DraftVersionView {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DraftVersionView")
            .field("version_id", &self.version_id)
            .field("number", &self.number)
            .field("status", &self.status)
            .finish_non_exhaustive()
    }
}

/// The fact versions and source passages that a draft version cites (ADR 0051).
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct ManifestView {
    pub facts: Vec<CitedFactView>,
    pub sources: Vec<CitedPassageView>,
}

/// An exact fact version that a draft cites. get_event_profile gives the current version of the fact.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct CitedFactView {
    pub fact_id: Uuid,
    pub version: i64,
    /// The dispute notes about the fact version (ADR 0073), oldest first: a person objects to it. Absent if there is none.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub dispute_notes: Vec<DisputeNoteView>,
}

/// A passage that a draft cites. get_source_passage gives its text.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct CitedPassageView {
    pub source_version_id: Uuid,
    pub start: u32,
    pub end: u32,
}

impl From<&StoredDraft> for DraftVersionView {
    fn from(draft: &StoredDraft) -> Self {
        Self {
            version_id: draft.version.id.as_uuid(),
            document_id: draft.version.document_id.as_uuid(),
            number: draft.version.number,
            status: draft.status.as_str(),
            markdown: draft
                .markdown
                .as_present()
                .map(|markdown| markdown.as_str().to_owned())
                .unwrap_or_default(),
            redacted: draft.version.redacted,
            manifest: ManifestView {
                facts: draft
                    .manifest
                    .facts
                    .iter()
                    .map(|fact| CitedFactView {
                        fact_id: fact.fact_id.as_uuid(),
                        version: fact.version.get(),
                        dispute_notes: note_views(
                            &draft.dispute_notes,
                            &NoteKey::fact_number(fact.fact_id, fact.version),
                        ),
                    })
                    .collect(),
                sources: draft
                    .manifest
                    .sources
                    .iter()
                    .map(|source| CitedPassageView {
                        source_version_id: source.source_version_id.as_uuid(),
                        start: source.passage.start,
                        end: source.passage.end,
                    })
                    .collect(),
            },
            dispute_notes: note_views(
                &draft.version.dispute_notes,
                &NoteKey::document_version(draft.version.id),
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use jiff::civil::date;
    use serde_json::json;
    use tada_domain::RecordVersion;
    use tada_domain::events::{EventKey, EventName, EventStatus, EventTimeZone};
    use tada_domain::facts::{
        DateWindow, FactValue, Granularity, ReferenceTarget, ShortText, ValueType, Valued,
        core_catalog,
    };
    use tada_domain::ids::{
        ChangesetId, EventId, FactId, OpenQuestionId, OrganizationId, ProposalId, SourceVersionId,
        UserId,
    };
    use tada_domain::proposals::QuestionText;
    use tada_domain::sources::Passage;

    use super::*;
    use tada_domain::sources::Evidence;

    use crate::caller::{MemberCaller, OrganizationRole};

    fn field(key: &str) -> FieldDefinition {
        core_catalog()
            .into_iter()
            .find(|field| field.key.as_str() == key)
            .unwrap()
    }

    #[test]
    fn a_value_schema_names_the_choices_the_unit_and_the_currency_of_the_field() {
        let audience = value_schema(&field("audience").value_type);
        let keys = &audience["properties"]["keys"];
        assert_eq!(
            keys["items"]["enum"],
            json!(["public", "members", "invited"])
        );
        assert_eq!(keys["maxItems"], 1, "one choice only");
        let components = value_schema(&field("components").value_type);
        assert!(
            components["properties"]["keys"]["maxItems"].is_null(),
            "several choices"
        );

        let visitors = value_schema(&field("visitor_estimate").value_type);
        assert!(
            visitors["properties"]["min"]["description"]
                .as_str()
                .unwrap()
                .contains("`person_per_day`")
        );
        let fee = value_schema(&field("entry_fee_adult").value_type);
        assert!(
            fee["properties"]["min"]["description"]
                .as_str()
                .unwrap()
                .contains("CHF")
        );
        let dates = value_schema(&field("exact_dates").value_type);
        assert_eq!(dates["properties"]["granularity"], json!({"const": "day"}));
        let reference = value_schema(&ValueType::Reference {
            target: ReferenceTarget::Event,
        });
        assert_eq!(reference["properties"]["target"], json!({"const": "event"}));
    }

    #[test]
    fn the_schema_lists_only_active_fields() {
        let mut catalog = core_catalog();
        catalog[0].status = FieldStatus::Deprecated;
        let schema = EventSchema::new(&event(), &catalog);
        assert_eq!(schema.fields.len(), catalog.len() - 1);
        assert!(
            schema
                .fields
                .iter()
                .all(|field| field.key != catalog[0].key.as_str())
        );
        assert_eq!(schema.catalog_version, CORE_CATALOG_VERSION);
        assert!(
            schema.fields.iter().all(|field| field.label.is_none()),
            "shipped fields"
        );
    }

    fn event() -> Event {
        Event {
            id: EventId::from_uuid(Uuid::from_u128(20)),
            organization_id: OrganizationId::from_uuid(Uuid::from_u128(10)),
            key: EventKey::parse("OPEN30").unwrap(),
            name: EventName::parse("Open Day Testwil").unwrap(),
            time_zone: EventTimeZone::default_zone(),
            version: RecordVersion::FIRST,
            created_at: Timestamp::UNIX_EPOCH,
            status: EventStatus::Active,
            closed_at: None,
            redacted_at: None,
        }
    }

    fn entry(key: &str, state: FactState<Valued>) -> ProfileEntry {
        ProfileEntry {
            field: field(key),
            fact_id: FactId::from_uuid(Uuid::now_v7()),
            version: RecordVersion::FIRST,
            state: state.map(Redactable::Present),
            evidence: vec![DatedEvidence {
                evidence: Evidence {
                    source_version_id: SourceVersionId::from_uuid(Uuid::from_u128(30)),
                    passage: Passage {
                        start: 4,
                        end: 12,
                        quote: Redactable::Present("Flugfeld".to_owned()),
                        page: None,
                    },
                },
                captured_at: Timestamp::UNIX_EPOCH,
            }],
            accepted_by: MemberCaller::new(
                UserId::from_uuid(Uuid::from_u128(40)),
                OrganizationId::from_uuid(Uuid::from_u128(41)),
                OrganizationRole::Member,
            )
            .actor(),
            accepted_at: Timestamp::UNIX_EPOCH,
        }
    }

    fn valued(value: FactValue) -> Valued {
        Valued {
            value,
            approximate: false,
        }
    }

    #[test]
    fn the_profile_keeps_each_state_and_the_open_proposals_in_their_own_lists() {
        let may = DateWindow::new(date(2030, 5, 1), date(2030, 5, 31), Granularity::Month).unwrap();
        let venue = ShortText::parse("Flugfeld").unwrap();
        let profile = EventProfile {
            fields: vec![
                entry(
                    "date_window",
                    FactState::Accepted(valued(FactValue::DateWindow(may))),
                ),
                entry(
                    "venue",
                    FactState::Assumption(valued(FactValue::Text(venue.clone()))),
                ),
                entry("exact_dates", FactState::Unknown),
            ],
            proposals: vec![OpenProposalRef {
                proposal_id: ProposalId::from_uuid(Uuid::from_u128(50)),
                changeset_id: ChangesetId::from_uuid(Uuid::from_u128(51)),
                field_id: field("venue").id,
                state: FactState::Accepted(valued(FactValue::Text(venue))),
                expected_version: Some(RecordVersion::FIRST),
                created_at: Timestamp::UNIX_EPOCH,
            }],
            open_questions: vec![OpenQuestionRef {
                id: OpenQuestionId::from_uuid(Uuid::from_u128(60)),
                local_number: 1,
                text: Redactable::Present(QuestionText::parse("Welcher Samstag?").unwrap()),
                owner: UserId::from_uuid(Uuid::from_u128(1)),
                version: RecordVersion::FIRST,
            }],
            dispute_notes: Default::default(),
        };
        let view = ProfileView::new(&event(), &profile, &core_catalog());
        let json = serde_json::to_value(&view).unwrap();

        assert_eq!(json["accepted"].as_array().unwrap().len(), 1);
        assert_eq!(json["accepted"][0]["field_key"], "date_window");
        assert_eq!(json["accepted"][0]["value"]["granularity"], "month");
        assert_eq!(json["accepted"][0]["evidence"][0]["quote"], "Flugfeld");
        assert_eq!(
            json["accepted"][0]["evidence"][0]["captured_at"],
            "1970-01-01T00:00:00Z"
        );
        let accepted_by = json!({"kind": "member", "id": Uuid::from_u128(40), "channel": "web"});
        assert_eq!(json["accepted"][0]["accepted_by"], accepted_by);
        assert_eq!(json["accepted"][0]["accepted_at"], "1970-01-01T00:00:00Z");
        assert_eq!(json["unknowns"][0]["accepted_by"], accepted_by);
        assert_eq!(
            json["unknowns"][0]["evidence"][0]["captured_at"],
            "1970-01-01T00:00:00Z"
        );
        assert_eq!(json["assumptions"][0]["field_key"], "venue");
        assert_eq!(json["unknowns"][0]["field_key"], "exact_dates");
        assert!(
            json["unknowns"][0].get("value").is_none(),
            "an unknown has no value"
        );
        let proposal = &json["open_proposals"][0];
        assert_eq!(proposal["field_key"], "venue");
        assert_eq!(proposal["proposed_state"], "accepted");
        assert_eq!(proposal["expected_version"], 1);
        assert_eq!(json["open_questions"][0]["readable_id"], "QST-001");
    }

    #[test]
    fn debug_hides_quotes_snippets_and_question_texts() {
        let view = ProfileView::new(
            &event(),
            &EventProfile {
                fields: vec![entry("exact_dates", FactState::Unknown)],
                proposals: Vec::new(),
                open_questions: Vec::new(),
                dispute_notes: Default::default(),
            },
            &[],
        );
        assert!(!format!("{view:?}").contains("Flugfeld"));
        let hit = SearchHitView {
            source_version_id: Uuid::from_u128(30),
            captured_at: Timestamp::UNIX_EPOCH,
            snippet: "Flugfeld".to_owned(),
            start: 4,
            end: 12,
        };
        assert!(!format!("{hit:?}").contains("Flugfeld"));
    }
}
