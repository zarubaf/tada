//! The proposal tool of ADR 0040. A proposal never changes accepted state: a member reviews it in the Review Inbox.
//! No tool accepts, rejects or deletes.

use axum::http::request::Parts;
use rmcp::handler::server::tool::Extension;
use rmcp::handler::server::wrapper::{Json, Parameters};
use rmcp::{tool, tool_router};
use schemars::JsonSchema;
use serde::Serialize;
use tada_app::proposals::{self, Created, NewChangeset, ProposeStores};
use uuid::Uuid;

use crate::errors::{ToolError, caller};
use crate::tools::Tools;

/// The changeset that the member reviews.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct ProposedChangeset {
    pub(crate) changeset_id: Uuid,
    /// The path of the changeset in the Review Inbox of the web client, for the member.
    pub(crate) link: String,
    /// True if a changeset with this `id` and the same content existed, so nothing changed.
    pub(crate) existing: bool,
}

#[tool_router(router = propose_tools, vis = "pub(crate)")]
impl Tools {
    #[tool(
        name = "propose_changeset",
        description = "Propose changes for the member to review. Nothing changes until a reviewer accepts a proposal in the Review Inbox of tada; no tool accepts, rejects or deletes. \
Only a `propose` token of a member who can propose in the event can propose. \
One changeset holds one intake: source_text is the member's own words of this intake, and the proposals are the changes that these words support. \
event_id comes from list_events; leave it out only for a changeset that creates new events. \
The input schema names the kinds of operations and the rules of each field, for example the links of a document draft. \
Use an existing field of get_event_schema first. Never guess a value: propose an unknown instead. \
Take the expected_version of a fact and the fact versions that a draft cites from get_event_profile, and the expected_version of a document from list_documents. \
To change a draft, read it with get_document_version and propose a new version. \
Each proposal cites at least one passage as evidence: of source_text, or of another source version that the member can read in the event, for example a hit of search_sources. \
Always send a new UUIDv7 as the id of the changeset and of each proposal. To retry, send the same changeset with the same ids again. \
If a check fails, tada stores nothing: the result has isError, the problem code and, for validation-failed, each invalid value as a JSON pointer and a code.",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn propose_changeset(
        &self,
        Extension(parts): Extension<Parts>,
        Parameters(input): Parameters<NewChangeset>,
    ) -> Result<Json<ProposedChangeset>, ToolError> {
        let caller = caller(&parts)?;
        let stores = ProposeStores {
            identity: &*self.identity,
            facts: &*self.facts,
            proposals: &*self.proposals,
            sources: &*self.sources,
            documents: &*self.documents,
        };
        let created = proposals::create_changeset(caller, input, stores, &*self.clock).await?;
        let (changeset, existing) = match created {
            Created::New(changeset) => (changeset, false),
            Created::Existing(changeset) => (changeset, true),
        };
        let changeset_id = changeset.id.as_uuid();
        Ok(Json(ProposedChangeset {
            changeset_id,
            link: format!("/inbox/{changeset_id}"),
            existing,
        }))
    }
}
