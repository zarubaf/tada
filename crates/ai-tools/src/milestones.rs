//! The read tool for the milestones of an event (ADR 0080). It follows the form of `list_actions`.

use std::sync::LazyLock;

use async_trait::async_trait;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::work::{EvidenceItem, evidence, list_description, query, text};
use super::{AiToolError, AiToolPorts, TypedTool, find_event};
use tada_app::ai_queries;
use tada_app::caller::AiCaller;
use tada_app::domain::events::EventKey;
use tada_app::domain::ids::WorkstreamId;
use tada_app::domain::records::{MilestoneStatus, MilestoneTitle};
use tada_app::records::Shown;
use tada_app::work::{MilestoneView, MilestoneWarning};

/// The input of `list_milestones`.
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct MilestonesInput {
    /// The key of the event, for example `FLY28`, from `list_events`.
    #[schemars(regex(pattern = EventKey::PATTERN))]
    event_key: String,
    /// Only the milestones with this status.
    status: Option<MilestoneStatusInput>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
enum MilestoneStatusInput {
    Open,
    Reached,
    Canceled,
}

impl From<MilestoneStatusInput> for MilestoneStatus {
    fn from(status: MilestoneStatusInput) -> Self {
        match status {
            MilestoneStatusInput::Open => Self::Open,
            MilestoneStatusInput::Reached => Self::Reached,
            MilestoneStatusInput::Canceled => Self::Canceled,
        }
    }
}

#[derive(Debug, Serialize, JsonSchema)]
pub(super) struct MilestoneList {
    milestones: Vec<MilestoneItem>,
    /// True if the event has more milestones than the list shows.
    more: bool,
}

/// A warning about a milestone at the time of the read.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "kebab-case")]
enum WarningItem {
    /// The milestone is open and its date is before today in the time zone of the event.
    Overdue,
    /// An action of the milestone is open, in progress or blocked, and it is due after the date of the milestone.
    LateAction { action_id: Uuid },
}

impl From<MilestoneWarning> for WarningItem {
    fn from(warning: MilestoneWarning) -> Self {
        match warning {
            MilestoneWarning::Overdue => Self::Overdue,
            MilestoneWarning::LateAction(id) => Self::LateAction {
                action_id: id.as_uuid(),
            },
        }
    }
}

#[derive(Debug, Serialize, JsonSchema)]
pub(super) struct MilestoneItem {
    id: Uuid,
    /// The readable ID, for example `MIL-001`.
    local_id: String,
    /// Empty after a legal redaction (ADR 0072).
    title: String,
    /// True after a legal redaction: the texts are empty (ADR 0072).
    redacted: bool,
    description: Option<String>,
    /// The date as `YYYY-MM-DD`.
    date: String,
    owner_user_id: Uuid,
    workstream_id: Option<Uuid>,
    /// `open`, `reached` or `canceled`.
    status: &'static str,
    warnings: Vec<WarningItem>,
    version: i64,
    evidence: Vec<EvidenceItem>,
}

impl From<&Shown<MilestoneView>> for MilestoneItem {
    fn from(shown: &Shown<MilestoneView>) -> Self {
        let milestone = &shown.record;
        let fields = &milestone.fields;
        Self {
            id: milestone.id.as_uuid(),
            local_id: milestone.local_id(),
            title: text(&fields.title, MilestoneTitle::as_str),
            redacted: fields.title.as_present().is_none(),
            description: fields
                .description
                .as_ref()
                .map(|text| text.as_str().to_owned()),
            date: fields.date.to_string(),
            owner_user_id: fields.owner.as_uuid(),
            workstream_id: fields.workstream_id.map(WorkstreamId::as_uuid),
            status: fields.status.as_str(),
            warnings: milestone
                .warnings
                .iter()
                .copied()
                .map(WarningItem::from)
                .collect(),
            version: milestone.version.get(),
            evidence: evidence(shown),
        }
    }
}

static LIST_MILESTONES: LazyLock<String> = LazyLock::new(|| {
    list_description(
        "List the milestones of an event: the dates that its plan holds, each with its owner, status, warnings and \
the evidence of the accepted proposals that made it. An action can name one milestone.",
        "the event has more milestones than this tool can show",
    )
});

pub(super) struct ListMilestones;

#[async_trait]
impl TypedTool for ListMilestones {
    type Input = MilestonesInput;
    type Output = MilestoneList;
    const NAME: &'static str = "list_milestones";
    const READ_ONLY: bool = true;

    fn description() -> &'static str {
        &LIST_MILESTONES
    }

    async fn run(
        &self,
        caller: &AiCaller,
        input: MilestonesInput,
        ports: &AiToolPorts,
    ) -> Result<MilestoneList, AiToolError> {
        let event = find_event(caller, &input.event_key, ports).await?;
        let query = query(input.status.map(MilestoneStatus::from));
        let page = ai_queries::list_milestones(caller, event.id, query, ports).await?;
        Ok(MilestoneList {
            milestones: page.items.iter().map(MilestoneItem::from).collect(),
            more: page.next.is_some(),
        })
    }
}
