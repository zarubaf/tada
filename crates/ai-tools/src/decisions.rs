//! The read tool for the decisions of an event (ADR 0080). It follows the form of `list_actions`.

use std::sync::LazyLock;

use async_trait::async_trait;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::work::{EvidenceItem, PartyItem, evidence, list_description, query, text};
use super::{AiToolError, AiToolPorts, TypedTool, find_event};
use tada_app::ai_queries;
use tada_app::caller::AiCaller;
use tada_app::domain::events::EventKey;
use tada_app::domain::ids::WorkstreamId;
use tada_app::domain::records::{DecisionStatus, DecisionWording, RecordReason};
use tada_app::proposals::DecisionStatusInput;
use tada_app::records::Shown;
use tada_app::work::{DecisionState, DecisionView};

/// The input of `list_decisions`.
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct DecisionsInput {
    /// The key of the event, for example `FLY28`, from `list_events`.
    #[schemars(regex(pattern = EventKey::PATTERN))]
    event_key: String,
    /// Only the decisions with this status.
    status: Option<DecisionStatusInput>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub(super) struct DecisionList {
    decisions: Vec<DecisionItem>,
    /// True if the event has more decisions than the list shows.
    more: bool,
}

#[derive(Debug, Serialize, JsonSchema)]
pub(super) struct DecisionItem {
    id: Uuid,
    /// The readable ID, for example `DEC-001`.
    local_id: String,
    /// The words of the decision. It can have several lines. Empty after a legal redaction (ADR 0072).
    wording: String,
    /// True after a legal redaction: the texts are empty (ADR 0072).
    redacted: bool,
    /// The date of the decision as `YYYY-MM-DD`.
    decided_on: String,
    /// The persons and institutions that decided, in their order.
    decided_by: Vec<PartyItem>,
    /// The member who follows the decision up.
    owner_user_id: Uuid,
    workstream_id: Option<Uuid>,
    /// `in-force`, `superseded` or `revoked`.
    status: &'static str,
    /// The newer decision that replaced this one. Only a `superseded` decision has it.
    superseded_by: Option<Uuid>,
    /// Why the decision was revoked. Only a `revoked` decision has it, and a legal redaction removes it.
    revoked_reason: Option<String>,
    version: i64,
    evidence: Vec<EvidenceItem>,
}

impl From<&Shown<DecisionView>> for DecisionItem {
    fn from(shown: &Shown<DecisionView>) -> Self {
        let decision = &shown.record;
        let (superseded_by, revoked_reason) = match &decision.state {
            DecisionState::InForce => (None, None),
            DecisionState::Superseded { by } => (Some(by.as_uuid()), None),
            DecisionState::Revoked { reason } => (
                None,
                reason
                    .as_present()
                    .map(|reason| RecordReason::as_str(reason).to_owned()),
            ),
        };
        Self {
            id: decision.id.as_uuid(),
            local_id: decision.local_id(),
            wording: text(&decision.wording, DecisionWording::as_str),
            redacted: decision.wording.as_present().is_none(),
            decided_on: decision.decided_on.to_string(),
            decided_by: decision.parties.iter().map(PartyItem::from).collect(),
            owner_user_id: decision.fields.owner.as_uuid(),
            workstream_id: decision.fields.workstream_id.map(WorkstreamId::as_uuid),
            status: decision.state.status().as_str(),
            superseded_by,
            revoked_reason,
            version: decision.version.get(),
            evidence: evidence(shown),
        }
    }
}

static LIST_DECISIONS: LazyLock<String> = LazyLock::new(|| {
    list_description(
        "List the decisions of an event: what the club decided, when, who decided it, its owner, its status and \
the evidence of the accepted proposals that made or ended it. A decision never changes its wording: a newer decision \
supersedes it. Read the list before you propose a decision, so you supersede the decision in force instead of a duplicate.",
        "the event has more decisions than this tool can show",
    )
});

pub(super) struct ListDecisions;

#[async_trait]
impl TypedTool for ListDecisions {
    type Input = DecisionsInput;
    type Output = DecisionList;
    const NAME: &'static str = "list_decisions";
    const READ_ONLY: bool = true;

    fn description() -> &'static str {
        &LIST_DECISIONS
    }

    async fn run(
        &self,
        caller: &AiCaller,
        input: DecisionsInput,
        ports: &AiToolPorts,
    ) -> Result<DecisionList, AiToolError> {
        let event = find_event(caller, &input.event_key, ports).await?;
        let query = query(input.status.map(DecisionStatus::from));
        let page = ai_queries::list_decisions(caller, event.id, query, ports).await?;
        Ok(DecisionList {
            decisions: page.items.iter().map(DecisionItem::from).collect(),
            more: page.next.is_some(),
        })
    }
}
