//! The queries and the proposal command that the AI tools call (ADR 0010).
//!
//! The ports of the tools are opaque: other crates create `AiToolPorts` from the stores, but cannot
//! take a store out of it again. So code outside this crate that has only `AiToolPorts` and an `AiCaller`,
//! for example a tool of the crate ai-tools, reaches stored data only through the functions of this module.
//! These functions read, or propose; none changes accepted state.
//!
//! ```compile_fail,E0616
//! use tada_app::ai_queries::{AiToolPorts, AiToolStores};
//!
//! fn stores(ports: &AiToolPorts) -> &AiToolStores {
//!     &ports.0
//! }
//! ```

use std::fmt;
use std::sync::Arc;

use tada_domain::events::{Event, EventKey};
use tada_domain::facts::FieldDefinition;
use tada_domain::ids::{DocumentVersionId, EventId};
use tada_domain::work::{ActionStatus, CommitmentStatus};

use crate::access::AccessError;
use crate::caller::AiCaller;
use crate::clock::Clock;
use crate::documents::{
    self, DocumentCursor, DocumentReads, DocumentStore, DocumentView, ReadDocumentError,
    StoredDraft,
};
use crate::event_members::{self, EventMember, EventMemberStore, ListEventMembersError};
use crate::events::{self, EventCursor, EventStore, ListEventsError};
use crate::facts::{self, EventProfile, FactStore};
use crate::identity::IdentityStore;
use crate::paging::{Page, PageLimit};
use crate::parties::{self, InstitutionView, PartyReadError, PartyStore, PersonView};
use crate::proposals::{self, Created, NewChangeset, ProposalStore, ProposeError, ProposeStores};
use crate::records::{NumberCursor, Shown};
use crate::search::{self, SearchError, SearchRequest};
use crate::sources::{self, PassageError, PassageRequest, SourceHit, SourcePassage, SourceStore};
use crate::work::{self, ActionView, CommitmentView, WorkError, WorkPorts, WorkQuery, WorkStore};
use crate::workstreams::{self, Workstream, WorkstreamError, WorkstreamStore};

/// The stores that the AI tools read and propose through, as the wiring gives them.
#[derive(Debug, Clone)]
pub struct AiToolStores {
    pub events: Arc<dyn EventStore>,
    /// The event memberships, for the members of an event.
    pub members: Arc<dyn EventMemberStore>,
    pub identity: Arc<dyn IdentityStore>,
    pub facts: Arc<dyn FactStore>,
    pub sources: Arc<dyn SourceStore>,
    pub proposals: Arc<dyn ProposalStore>,
    /// The documents that the document tools read and that draft proposals target.
    pub documents: Arc<dyn DocumentStore>,
    pub clock: Arc<dyn Clock>,
    /// The workstreams, promisors and work records that proposals name.
    pub workstreams: Arc<dyn WorkstreamStore>,
    pub parties: Arc<dyn PartyStore>,
    pub work: Arc<dyn WorkStore>,
}

/// The ports of the AI tools. The stores stay inside: only the functions of this module use them.
#[derive(Clone)]
pub struct AiToolPorts(AiToolStores);

impl AiToolPorts {
    pub fn new(stores: AiToolStores) -> Self {
        Self(stores)
    }

    fn document_reads(&self) -> DocumentReads<'_> {
        DocumentReads {
            identity: &*self.0.identity,
            documents: &*self.0.documents,
            facts: &*self.0.facts,
            sources: &*self.0.sources,
        }
    }

    fn work_ports(&self) -> WorkPorts<'_> {
        WorkPorts {
            identity: &*self.0.identity,
            work: &*self.0.work,
            workstreams: &*self.0.workstreams,
            parties: &*self.0.parties,
            clock: &*self.0.clock,
        }
    }
}

/// The stores do not show in a log line.
impl fmt::Debug for AiToolPorts {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AiToolPorts")
            .finish_non_exhaustive()
    }
}

/// `events::list_events`.
pub async fn list_events(
    caller: &AiCaller,
    after: Option<EventCursor>,
    limit: PageLimit,
    ports: &AiToolPorts,
) -> Result<Page<Event, EventCursor>, ListEventsError> {
    events::list_events(caller, after, limit, &*ports.0.events).await
}

/// `event_members::list_event_members`.
pub async fn list_event_members(
    caller: &AiCaller,
    event: EventId,
    ports: &AiToolPorts,
) -> Result<Vec<EventMember>, ListEventMembersError> {
    event_members::list_event_members(caller, event, &*ports.0.identity, &*ports.0.members).await
}

/// `events::find_event`.
pub async fn find_event(
    caller: &AiCaller,
    key: &EventKey,
    ports: &AiToolPorts,
) -> Result<Event, AccessError> {
    events::find_event(caller, key, &*ports.0.events, &*ports.0.identity).await
}

/// `facts::get_field_catalog`.
pub async fn get_field_catalog(
    caller: &AiCaller,
    event: EventId,
    ports: &AiToolPorts,
) -> Result<Vec<FieldDefinition>, AccessError> {
    facts::get_field_catalog(caller, event, &*ports.0.identity, &*ports.0.facts).await
}

/// `facts::get_event_profile`.
pub async fn get_event_profile(
    caller: &AiCaller,
    event: EventId,
    ports: &AiToolPorts,
) -> Result<EventProfile, AccessError> {
    facts::get_event_profile(caller, event, &*ports.0.identity, &*ports.0.facts).await
}

/// `search::search_sources`.
pub async fn search_sources(
    caller: &AiCaller,
    request: SearchRequest,
    ports: &AiToolPorts,
) -> Result<Vec<SourceHit>, SearchError> {
    let (events, identity, sources) = (&*ports.0.events, &*ports.0.identity, &*ports.0.sources);
    search::search_sources(caller, request, events, identity, sources).await
}

/// `sources::get_source_passage`.
pub async fn get_source_passage(
    caller: &AiCaller,
    request: PassageRequest,
    ports: &AiToolPorts,
) -> Result<SourcePassage, PassageError> {
    sources::get_source_passage(caller, request, &*ports.0.identity, &*ports.0.sources).await
}

/// `documents::list_documents`.
pub async fn list_documents(
    caller: &AiCaller,
    event: EventId,
    name: Option<&str>,
    after: Option<DocumentCursor>,
    limit: PageLimit,
    ports: &AiToolPorts,
) -> Result<Page<DocumentView, DocumentCursor>, ReadDocumentError> {
    documents::list_documents(caller, event, name, after, limit, ports.document_reads()).await
}

/// `documents::get_draft`.
pub async fn get_draft(
    caller: &AiCaller,
    id: DocumentVersionId,
    ports: &AiToolPorts,
) -> Result<StoredDraft, ReadDocumentError> {
    documents::get_draft(caller, id, ports.document_reads()).await
}

/// `workstreams::list_workstreams`.
pub async fn list_workstreams(
    caller: &AiCaller,
    event: EventId,
    ports: &AiToolPorts,
) -> Result<Vec<Workstream>, WorkstreamError> {
    workstreams::list_workstreams(caller, event, &*ports.0.identity, &*ports.0.workstreams).await
}

/// `work::list_actions`.
pub async fn list_actions(
    caller: &AiCaller,
    event: EventId,
    query: WorkQuery<ActionStatus>,
    ports: &AiToolPorts,
) -> Result<Page<Shown<ActionView>, NumberCursor>, WorkError> {
    work::list_actions(caller, event, query, ports.work_ports()).await
}

/// `work::list_commitments`.
pub async fn list_commitments(
    caller: &AiCaller,
    event: EventId,
    query: WorkQuery<CommitmentStatus>,
    ports: &AiToolPorts,
) -> Result<Page<Shown<CommitmentView>, NumberCursor>, WorkError> {
    work::list_commitments(caller, event, query, ports.work_ports()).await
}

/// `parties::list_persons`.
pub async fn list_persons(
    caller: &AiCaller,
    query: Option<&str>,
    after: Option<NumberCursor>,
    limit: PageLimit,
    ports: &AiToolPorts,
) -> Result<Page<Shown<PersonView>, NumberCursor>, PartyReadError> {
    let (identity, store) = (&*ports.0.identity, &*ports.0.parties);
    parties::list_persons(caller, query, after, limit, identity, store).await
}

/// `parties::list_institutions`.
pub async fn list_institutions(
    caller: &AiCaller,
    query: Option<&str>,
    after: Option<NumberCursor>,
    limit: PageLimit,
    ports: &AiToolPorts,
) -> Result<Page<Shown<InstitutionView>, NumberCursor>, PartyReadError> {
    let (identity, store) = (&*ports.0.identity, &*ports.0.parties);
    parties::list_institutions(caller, query, after, limit, identity, store).await
}

/// `proposals::create_changeset`: the one write of the AI tools, a changeset for review (ADR 0010).
pub async fn create_changeset(
    caller: &AiCaller,
    input: NewChangeset,
    ports: &AiToolPorts,
) -> Result<Created, ProposeError> {
    let stores = ProposeStores {
        identity: &*ports.0.identity,
        facts: &*ports.0.facts,
        proposals: &*ports.0.proposals,
        sources: &*ports.0.sources,
        documents: &*ports.0.documents,
        workstreams: &*ports.0.workstreams,
        parties: &*ports.0.parties,
        work: &*ports.0.work,
    };
    proposals::create_changeset(caller, input, stores, &*ports.0.clock).await
}
