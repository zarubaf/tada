//! The read tool for the risks of an event (ADR 0080). It follows the form of `list_actions`.

use std::sync::LazyLock;

use async_trait::async_trait;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::work::{EvidenceItem, evidence, list_description_in_order, query, text};
use super::{AiToolError, AiToolPorts, TypedTool, find_event};
use tada_app::ai_queries;
use tada_app::caller::AiCaller;
use tada_app::domain::events::EventKey;
use tada_app::domain::ids::WorkstreamId;
use tada_app::domain::records::{RiskStatus, RiskTitle};
use tada_app::proposals::RiskStatusInput;
use tada_app::records::Shown;
use tada_app::work::RiskView;

/// The input of `list_risks`.
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct RisksInput {
    /// The key of the event, for example `FLY28`, from `list_events`.
    #[schemars(regex(pattern = EventKey::PATTERN))]
    event_key: String,
    /// Only the risks with this status.
    status: Option<RiskStatusInput>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub(super) struct RiskList {
    risks: Vec<RiskItem>,
    /// True if the event has more risks than the list shows.
    more: bool,
}

#[derive(Debug, Serialize, JsonSchema)]
pub(super) struct RiskItem {
    id: Uuid,
    /// The readable ID, for example `RSK-001`.
    local_id: String,
    /// Empty after a legal redaction (ADR 0072).
    title: String,
    /// True after a legal redaction: the texts are empty (ADR 0072).
    redacted: bool,
    /// It can have several lines.
    description: Option<String>,
    /// `low`, `medium` or `high`.
    likelihood: &'static str,
    /// `low`, `medium` or `high`.
    impact: &'static str,
    /// What the club does about the risk. It can have several lines.
    mitigation: Option<String>,
    owner_user_id: Uuid,
    workstream_id: Option<Uuid>,
    /// `open`, `mitigated`, `occurred` or `closed`.
    status: &'static str,
    version: i64,
    evidence: Vec<EvidenceItem>,
}

impl From<&Shown<RiskView>> for RiskItem {
    fn from(shown: &Shown<RiskView>) -> Self {
        let risk = &shown.record;
        let fields = &risk.fields;
        Self {
            id: risk.id.as_uuid(),
            local_id: risk.local_id(),
            title: text(&fields.title, RiskTitle::as_str),
            redacted: fields.title.as_present().is_none(),
            description: fields
                .description
                .as_ref()
                .map(|text| text.as_str().to_owned()),
            likelihood: fields.rating.likelihood.as_str(),
            impact: fields.rating.impact.as_str(),
            mitigation: fields
                .mitigation
                .as_ref()
                .map(|text| text.as_str().to_owned()),
            owner_user_id: fields.owner.as_uuid(),
            workstream_id: fields.workstream_id.map(WorkstreamId::as_uuid),
            status: fields.status.as_str(),
            version: risk.version.get(),
            evidence: evidence(shown),
        }
    }
}

static LIST_RISKS: LazyLock<String> = LazyLock::new(|| {
    list_description_in_order(
        "List the risks of an event: what can go wrong, each with its likelihood and impact (low, medium or high), \
mitigation, owner, status and the evidence of the accepted proposals that made it. Read the list before you \
propose a risk, so you change the rating or the status of a known risk instead of proposing a duplicate. \
A rating change is its own operation; a status change does not change the rating.",
        "the worst risk first (likelihood times impact, with low 1, medium 2 and high 3, then impact, then number)",
        "the event has more risks than this tool can show",
    )
});

pub(super) struct ListRisks;

#[async_trait]
impl TypedTool for ListRisks {
    type Input = RisksInput;
    type Output = RiskList;
    const NAME: &'static str = "list_risks";
    const READ_ONLY: bool = true;

    fn description() -> &'static str {
        &LIST_RISKS
    }

    async fn run(
        &self,
        caller: &AiCaller,
        input: RisksInput,
        ports: &AiToolPorts,
    ) -> Result<RiskList, AiToolError> {
        let event = find_event(caller, &input.event_key, ports).await?;
        let query = query(input.status.map(RiskStatus::from));
        let page = ai_queries::list_risks(caller, event.id, query, ports).await?;
        Ok(RiskList {
            risks: page.items.iter().map(RiskItem::from).collect(),
            more: page.next.is_some(),
        })
    }
}
