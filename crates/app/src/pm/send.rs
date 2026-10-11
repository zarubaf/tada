//! The send of one digest of the AI PM (ADR 0076).
//!
//! The daily check planned the digest some time ago, so the send checks it again: it drops each item whose record
//! changed, closed, went to another owner or left the reach of the recipient, and it cancels the digest if the
//! recipient is no longer eligible or nothing stays. Then it words the digest, with the AI if the organization
//! allows it and with the template if not, and sends it through the `Messenger`.
//!
//! The send claims the intent before the Bot API call: the intent is then `unknown`. A worker that stops after the
//! call and before it records the outcome leaves it `unknown`, and no later attempt sends it again (ADR 0042).
//! An `unknown` send counts as a notice for the rules, as a sent one does: Telegram can have delivered it.

use std::collections::HashMap;
use std::fmt::{self, Debug};
use std::sync::Arc;

use async_trait::async_trait;
use jiff::Timestamp;
use jiff::civil::Date;
use tada_domain::RecordVersion;
use tada_domain::ids::{DigestItemId, EventId, UserId};
use tada_domain::reminders::{Answer, CallbackData, PmPolicy, Reason, RecordKind, Rules};
use tada_domain::work::ActionStatus;
use uuid::Uuid;

use crate::access::readable_events_of;
use crate::agent::AgentPorts;
use crate::ai_cost::{AiCostStore, AiGate, AiSetup, Month, ai_gate, cap_of, configured_for};
use crate::ai_tools::ToolCatalog;
use crate::caller::{JobRunner, OrgScope, ServiceCaller};
use crate::clock::Clock;
use crate::identity::IdentityStore;
use crate::jobs::{Job, JobFailed, JobHandler, JobWarning};
use crate::messenger::{Button, Messenger, SendOutcome};
use crate::model::{AiLimits, Model};
use crate::outbound::{Outcome, intent_id};
use crate::pm::SEND_DIGEST_JOB;
use crate::pm::check::{PmStore, Recipient};
use crate::pm::digest_wording::{
    DIGEST_TASK, DigestItemInput, DigestRequest, WordedLine, inert, word_digest,
};
use crate::pm::policy::PmPolicyStore;
use crate::public_url::PublicUrl;
use crate::store::StoreError;
use crate::telegram::TelegramUserId;

/// A digest lists at most this many items. The rest is "and N more" with a link to My Work (ADR 0076).
pub const MAX_ITEMS: usize = 10;
/// The most characters of a title in the template. Telegram takes at most 4096 characters in one message.
const TITLE_MAX_CHARS: usize = 120;
/// The most characters of an event name in the template.
const EVENT_MAX_CHARS: usize = 60;

/// One planned item of a digest.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DigestItemRef {
    /// The opaque reference that a button carries.
    pub id: DigestItemId,
    pub kind: RecordKind,
    pub record_id: Uuid,
    /// The version of the record at the plan.
    pub version: RecordVersion,
    pub reason: Reason,
}

/// A digest that waits for its send.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingDigest {
    pub user_id: UserId,
    /// The Telegram account of the user, if a link exists.
    pub telegram_user: Option<TelegramUserId>,
    /// The language of the user, for example `de-CH`.
    pub locale: String,
    /// The items in the order of the plan.
    pub items: Vec<DigestItemRef>,
}

/// A record of a digest item as it is now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemNow {
    pub kind: RecordKind,
    pub record_id: Uuid,
    pub event_id: EventId,
    pub version: RecordVersion,
    pub owner: UserId,
    /// True if the record is open and its event is active: an action in `open`, `in-progress` or `blocked`, a
    /// commitment in `conditional` or `firm`.
    pub open: bool,
    /// The title of an action or the text of a commitment.
    pub title: String,
    pub event_name: String,
    pub due_date: Option<Date>,
    /// The status code, for example `open` or `firm`.
    pub status: String,
}

/// The repository port of the send.
#[async_trait]
pub trait DigestStore: Debug + Send + Sync {
    /// The digest `intent` of the organization, if it is `pending`.
    async fn load(
        &self,
        scope: OrgScope,
        intent: Uuid,
    ) -> Result<Option<PendingDigest>, StoreError>;

    /// The records of `items` as they are now. A record that does not exist any more is missing.
    async fn current(
        &self,
        scope: OrgScope,
        items: &[DigestItemRef],
    ) -> Result<Vec<ItemNow>, StoreError>;

    /// Moves a pending digest to `unknown` at `at`, before its send. Returns false if it is not pending.
    async fn claim(&self, scope: OrgScope, intent: Uuid, at: Timestamp)
    -> Result<bool, StoreError>;

    /// Queues a new send job of the pending digest `intent` that starts at `run_at`.
    async fn defer(
        &self,
        scope: OrgScope,
        intent: Uuid,
        run_at: Timestamp,
    ) -> Result<(), StoreError>;

    /// Returns a claimed digest to `pending`, after a send that certainly delivered nothing.
    async fn release(&self, scope: OrgScope, intent: Uuid) -> Result<(), StoreError>;

    /// Records the outcome of a claimed digest at `at` and deletes the items that are not in `kept`, in one
    /// transaction. `Sent` and `Unknown` also write the notice of each kept item to `reminder_state`; `Blocked`
    /// also marks the Telegram link of the recipient as blocked.
    async fn finish(
        &self,
        scope: OrgScope,
        intent: Uuid,
        outcome: DigestOutcome,
        kept: &[DigestItemId],
        at: Timestamp,
    ) -> Result<(), StoreError>;

    /// Moves a pending digest to `canceled` at `at`. Nothing was sent.
    async fn cancel(&self, scope: OrgScope, intent: Uuid, at: Timestamp) -> Result<(), StoreError>;
}

/// The final outcome of a claimed digest.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DigestOutcome {
    Sent,
    /// Telegram can have delivered it. Nobody sends it again (ADR 0042).
    Unknown,
    /// Nothing went out, and a retry would not help, or the retries are over.
    Failed,
    /// The recipient blocked the bot. Nothing went out.
    Blocked,
}

impl DigestOutcome {
    /// The status of the intent.
    pub fn status(self) -> Outcome {
        match self {
            Self::Sent => Outcome::Sent,
            Self::Unknown => Outcome::Unknown,
            Self::Failed | Self::Blocked => Outcome::Failed,
        }
    }

    /// True if Telegram can have shown the digest, so its items count as told.
    pub fn notifies(self) -> bool {
        matches!(self, Self::Sent | Self::Unknown)
    }
}

/// One item of the template digest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TemplateItem {
    /// The number of the item in the message, from 1. Its buttons show it.
    pub number: usize,
    pub kind: RecordKind,
    /// Inert plain text on one line.
    pub title: String,
    /// Inert plain text on one line.
    pub event_name: String,
    pub due_date: Option<Date>,
    pub reason: Reason,
}

/// The text of the items of a digest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DigestBody {
    /// The template: a fixed intro and one line for each item from its values.
    Template(Vec<TemplateItem>),
    /// The AI wording, after its check: the intro and one line for each item, in the order of the items.
    Worded {
        intro: String,
        lines: Vec<WordedItem>,
    },
}

/// One line of the AI wording, bound to its item by the number of the item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WordedItem {
    /// The number of the item in the message, from 1. Its buttons show the same number.
    pub number: usize,
    /// The checked text of the model.
    pub text: String,
}

/// The message of a digest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DigestMessage {
    pub body: DigestBody,
    /// The number of items that the message leaves out.
    pub more: usize,
}

/// What a button says.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonLabel {
    /// The answer to the item with this number.
    Answer(usize, Answer),
    /// The link to the record of the item with this number.
    Open(usize),
    /// The link to My Work.
    MyWork,
}

/// The texts of a digest in the language of the recipient.
pub trait DigestTexts: Debug + Send + Sync {
    /// The plain text of the message.
    fn render(&self, locale: &str, message: &DigestMessage) -> String;

    fn button(&self, locale: &str, label: ButtonLabel) -> String;
}

/// The AI of the digest: the model, the usage and the cap. The worker has it only if the operator set up the
/// model provider (ADR 0045).
pub struct DigestAi {
    pub model: Arc<dyn Model>,
    pub cost: Arc<dyn AiCostStore>,
    pub setup: AiSetup,
    pub limits: AiLimits,
}

impl Debug for DigestAi {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DigestAi")
            .field("setup", &self.setup)
            .field("limits", &self.limits)
            .finish_non_exhaustive()
    }
}

/// The ports of `SendDigest`.
pub struct SendDigestPorts {
    pub store: Arc<dyn DigestStore>,
    pub recipients: Arc<dyn PmStore>,
    pub policies: Arc<dyn PmPolicyStore>,
    pub identity: Arc<dyn IdentityStore>,
    pub messenger: Arc<dyn Messenger>,
    pub texts: Arc<dyn DigestTexts>,
    pub clock: Arc<dyn Clock>,
}

impl Debug for SendDigestPorts {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SendDigestPorts").finish_non_exhaustive()
    }
}

/// The handler of the send job of one digest.
pub struct SendDigest {
    ports: SendDigestPorts,
    ai: Option<DigestAi>,
    public_url: PublicUrl,
    caller: ServiceCaller<JobRunner>,
}

impl Debug for SendDigest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SendDigest")
            .field("ports", &self.ports)
            .field("ai", &self.ai)
            .field("public_url", &self.public_url)
            .finish_non_exhaustive()
    }
}

/// An item that stays in the digest, with its number in the message.
struct Kept<'a> {
    number: usize,
    item: &'a DigestItemRef,
    now: &'a ItemNow,
}

impl SendDigest {
    /// `ai` is `None` if the operator did not set up the model provider: then each digest uses the template.
    pub fn new(ports: SendDigestPorts, ai: Option<DigestAi>, public_url: PublicUrl) -> Self {
        Self {
            ports,
            ai,
            public_url,
            caller: ServiceCaller::new(),
        }
    }

    /// The recipient of the digest, if the user is still eligible and the policy is not paused.
    async fn recipient(
        &self,
        scope: OrgScope,
        policy: &PmPolicy,
        user: UserId,
    ) -> Result<Option<Recipient>, StoreError> {
        if policy.paused {
            return Ok(None);
        }
        // The check and the send use one rule of eligibility.
        Ok(self
            .ports
            .recipients
            .recipients(scope)
            .await?
            .into_iter()
            .find(|recipient| recipient.user_id == user))
    }

    /// The items that stay: the record has the version of the plan, is open, has the recipient as owner, and its
    /// event is in the reach of the recipient now (ADR 0052).
    async fn kept<'a>(
        &self,
        scope: OrgScope,
        recipient: &Recipient,
        items: &'a [DigestItemRef],
        now: &'a HashMap<(RecordKind, Uuid), ItemNow>,
    ) -> Result<Vec<Kept<'a>>, StoreError> {
        let reach = readable_events_of(
            scope,
            recipient.user_id,
            recipient.role,
            self.ports.identity.as_ref(),
        )
        .await?;
        Ok(items
            .iter()
            .filter_map(|item| {
                let now = now.get(&(item.kind, item.record_id))?;
                (now.version == item.version
                    && now.open
                    && now.owner == recipient.user_id
                    && reach.includes(now.event_id))
                .then_some((item, now))
            })
            .enumerate()
            .map(|(index, (item, now))| Kept {
                number: index + 1,
                item,
                now,
            })
            .collect())
    }

    /// The AI wording of `items`, or `None` for the template: AI is not allowed, or the run failed or its answer
    /// failed the checks. The model gets the items only, without a name from the person records, an ID or a
    /// description. A title or an event name is free text and can still name a person.
    async fn wording(
        &self,
        scope: OrgScope,
        policy: &PmPolicy,
        recipient: &Recipient,
        locale: &str,
        items: &[Kept<'_>],
    ) -> Option<DigestBody> {
        let ai = self.ai.as_ref()?;
        let now = self.ports.clock.now();
        match self.allowed(scope, policy, ai, now).await {
            Ok(true) => {}
            Ok(false) => return None,
            Err(error) => {
                tracing::warn!(%error, "cannot read the AI usage; the digest uses the template");
                return None;
            }
        }
        let request = DigestRequest {
            locale: locale.to_owned(),
            today: Rules::new(policy, now, recipient.zone.clone()).today,
            items: items
                .iter()
                .map(|kept| DigestItemInput {
                    reference: reference(kept.number),
                    kind: kept.item.kind,
                    title: kept.now.title.clone(),
                    event_name: kept.now.event_name.clone(),
                    due_date: kept.now.due_date,
                    status: kept.now.status.clone(),
                    reason: kept.item.reason,
                })
                .collect(),
        };
        // Prompt version 1 uses no tool, so the run gets an empty catalog and no data port.
        let tools = ToolCatalog::<()>::new(Vec::new());
        let ports = AgentPorts {
            model: ai.model.as_ref(),
            tools: &tools,
            tool_ports: &(),
            cost: ai.cost.as_ref(),
            clock: self.ports.clock.as_ref(),
            limits: ai.limits,
        };
        let principal =
            self.caller
                .member(recipient.user_id, scope.organization_id(), recipient.role);
        // The runner checks its time limit only between the calls, so the send stops a run that hangs.
        let run = tokio::time::timeout(
            ai.limits.run_timeout.unsigned_abs(),
            word_digest(&ports, principal, &request),
        )
        .await;
        let wording = match run {
            Ok(run) => match run.result {
                Ok(wording) => wording,
                Err(error) => {
                    tracing::warn!(%error, "the AI wording failed; the digest uses the template");
                    return None;
                }
            },
            Err(_) => {
                tracing::warn!("the AI wording took too long; the digest uses the template");
                return None;
            }
        };
        let numbers: Vec<usize> = items.iter().map(|kept| kept.number).collect();
        Some(DigestBody::Worded {
            intro: wording.intro,
            lines: bind_lines(&numbers, wording.lines)?,
        })
    }

    /// True if AI can word a digest of the organization now (ADR 0077).
    async fn allowed(
        &self,
        scope: OrgScope,
        policy: &PmPolicy,
        ai: &DigestAi,
        now: Timestamp,
    ) -> Result<bool, StoreError> {
        let (cap, _) = cap_of(ai.cost.as_ref(), scope, ai.setup).await?;
        let used = ai.cost.month_usage(scope, Month::containing(now)).await?;
        let configured =
            configured_for(ai.cost.as_ref(), scope, ai.setup, DIGEST_TASK.name, now).await?;
        Ok(ai_gate(policy, configured, used, cap) == AiGate::Allowed)
    }

    /// The template body of `items`.
    fn template(items: &[Kept<'_>]) -> DigestBody {
        DigestBody::Template(
            items
                .iter()
                .map(|kept| TemplateItem {
                    number: kept.number,
                    kind: kept.item.kind,
                    title: shorten(&inert(&kept.now.title), TITLE_MAX_CHARS),
                    event_name: shorten(&inert(&kept.now.event_name), EVENT_MAX_CHARS),
                    due_date: kept.now.due_date,
                    reason: kept.item.reason,
                })
                .collect(),
        )
    }

    /// The buttons: one row for each item, from the rules, and a link to My Work if the message leaves items out.
    fn rows(&self, locale: &str, items: &[Kept<'_>], more: usize) -> Vec<Vec<Button>> {
        let texts = self.ports.texts.as_ref();
        let mut rows: Vec<Vec<Button>> = items
            .iter()
            .map(|kept| {
                let answers: &[Answer] = match kept.item.kind {
                    // A blocked action cannot change to blocked again, so it gets no such button.
                    RecordKind::Action if kept.now.status == ActionStatus::Blocked.as_str() => {
                        &[Answer::OnTrack, Answer::Done]
                    }
                    RecordKind::Action => &[Answer::OnTrack, Answer::Blocked, Answer::Done],
                    RecordKind::Commitment => &[Answer::OnTrack, Answer::Fulfilled],
                };
                let mut row: Vec<Button> = answers
                    .iter()
                    .map(|&answer| Button::Callback {
                        label: texts.button(locale, ButtonLabel::Answer(kept.number, answer)),
                        data: CallbackData {
                            answer,
                            item: kept.item.id,
                        }
                        .encode(),
                    })
                    .collect();
                row.push(Button::Url {
                    label: texts.button(locale, ButtonLabel::Open(kept.number)),
                    url: self.public_url.work_page(kept.now.event_id, kept.item.kind),
                });
                row
            })
            .collect();
        if more > 0 {
            rows.push(vec![Button::Url {
                label: texts.button(locale, ButtonLabel::MyWork),
                url: self.public_url.my_work(),
            }]);
        }
        rows
    }

    /// Records `outcome` with the items `kept`.
    async fn finish(
        &self,
        scope: OrgScope,
        intent: Uuid,
        outcome: DigestOutcome,
        kept: &[DigestItemId],
    ) -> Result<(), JobFailed> {
        self.ports
            .store
            .finish(scope, intent, outcome, kept, self.ports.clock.now())
            .await
            .map_err(store_failed)
    }
}

/// `text` with at most `max` characters; a longer text ends with `…`.
fn shorten(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_owned();
    }
    let mut short: String = text.chars().take(max.saturating_sub(1)).collect();
    short.push('…');
    short
}

fn store_failed(error: StoreError) -> JobFailed {
    JobFailed(format!("the store failed: {error}"))
}

#[async_trait]
impl JobHandler for SendDigest {
    fn kind(&self) -> &'static str {
        SEND_DIGEST_JOB
    }

    async fn run(&self, job: &Job) -> Result<Option<JobWarning>, JobFailed> {
        let intent = intent_id(job)?;
        let organization_id = job
            .organization_id
            .ok_or_else(|| JobFailed("the job has no organization".into()))?;
        // The job names its organization, so the send reads and writes in it only (ADR 0039).
        let scope = self.caller.scope(organization_id);
        let store = self.ports.store.as_ref();
        let Some(digest) = store.load(scope, intent).await.map_err(store_failed)? else {
            // An earlier attempt claimed, canceled or finished the digest already.
            return Ok(None);
        };
        let (policy, _) = self.ports.policies.get(scope).await.map_err(store_failed)?;
        let recipient = self
            .recipient(scope, &policy, digest.user_id)
            .await
            .map_err(store_failed)?;
        let (Some(recipient), Some(to)) = (recipient, digest.telegram_user) else {
            store
                .cancel(scope, intent, self.ports.clock.now())
                .await
                .map_err(store_failed)?;
            return Ok(None);
        };
        // The quiet hours hold at the send too: a retry or a worker that comes back in the night waits for their
        // end, before the claim and before the AI run. A new job waits, so the wait costs no attempt.
        let send_at = policy
            .quiet
            .send_at(self.ports.clock.now(), &recipient.zone);
        if send_at > self.ports.clock.now() {
            store
                .defer(scope, intent, send_at)
                .await
                .map_err(store_failed)?;
            return Ok(None);
        }
        let now: HashMap<_, _> = store
            .current(scope, &digest.items)
            .await
            .map_err(store_failed)?
            .into_iter()
            .map(|item| ((item.kind, item.record_id), item))
            .collect();
        let mut kept = self
            .kept(scope, &recipient, &digest.items, &now)
            .await
            .map_err(store_failed)?;
        if kept.is_empty() {
            store
                .cancel(scope, intent, self.ports.clock.now())
                .await
                .map_err(store_failed)?;
            return Ok(None);
        }
        let more = kept.len().saturating_sub(MAX_ITEMS);
        kept.truncate(MAX_ITEMS);
        let locale = digest.locale.as_str();
        let body = match self
            .wording(scope, &policy, &recipient, locale, &kept)
            .await
        {
            Some(body) => body,
            None => Self::template(&kept),
        };
        let text = self
            .ports
            .texts
            .render(locale, &DigestMessage { body, more });
        let rows = self.rows(locale, &kept, more);
        let kept: Vec<DigestItemId> = kept.iter().map(|kept| kept.item.id).collect();

        if !store
            .claim(scope, intent, self.ports.clock.now())
            .await
            .map_err(store_failed)?
        {
            return Ok(None);
        }
        match self.ports.messenger.send(to, &text, &rows).await {
            SendOutcome::Sent => {
                self.finish(scope, intent, DigestOutcome::Sent, &kept)
                    .await?;
                Ok(None)
            }
            SendOutcome::Unknown => {
                // Telegram can have delivered it, so nobody sends it again (ADR 0042).
                self.finish(scope, intent, DigestOutcome::Unknown, &kept)
                    .await?;
                Ok(Some(JobWarning(format!(
                    "the outcome of the digest {intent} is unknown"
                ))))
            }
            SendOutcome::Blocked => {
                self.finish(scope, intent, DigestOutcome::Blocked, &kept)
                    .await?;
                Ok(Some(JobWarning(format!(
                    "the recipient of the digest {intent} blocked the bot"
                ))))
            }
            SendOutcome::Failed => {
                // Nothing went out, and a retry would get the same answer. The next daily check plans a new
                // digest: a failed one starts no cooldown.
                self.finish(scope, intent, DigestOutcome::Failed, &kept)
                    .await?;
                Ok(Some(JobWarning(format!(
                    "Telegram refused the digest {intent}"
                ))))
            }
            SendOutcome::Temporary => {
                if job.is_last_attempt() {
                    self.finish(scope, intent, DigestOutcome::Failed, &kept)
                        .await?;
                } else {
                    // Nothing went out, so the queue can retry with a backoff, as for a mail (ADR 0042).
                    store.release(scope, intent).await.map_err(store_failed)?;
                }
                Err(JobFailed(format!(
                    "Telegram did not take the digest {intent} for now"
                )))
            }
        }
    }
}

/// The reference of the item with `number` in the input of the model.
fn reference(number: usize) -> String {
    number.to_string()
}

/// The lines of the model in the order of the items with `numbers`, each bound to the number of its item, whatever
/// order the model used. `None` if an item has no line.
fn bind_lines(numbers: &[usize], lines: Vec<WordedLine>) -> Option<Vec<WordedItem>> {
    let mut texts: HashMap<String, String> = lines
        .into_iter()
        .map(|line| (line.reference, line.text))
        .collect();
    numbers
        .iter()
        .map(|&number| {
            texts
                .remove(&reference(number))
                .map(|text| WordedItem { number, text })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(reference: &str, text: &str) -> WordedLine {
        WordedLine {
            reference: reference.to_owned(),
            text: text.to_owned(),
        }
    }

    #[test]
    fn each_line_is_bound_to_the_number_of_its_item_in_any_order() {
        let shuffled = vec![
            line("3", "Die Bänke."),
            line("1", "Die Flyer."),
            line("2", "Das Festzelt."),
        ];
        assert_eq!(
            bind_lines(&[1, 2, 3], shuffled),
            Some(vec![
                WordedItem {
                    number: 1,
                    text: "Die Flyer.".to_owned()
                },
                WordedItem {
                    number: 2,
                    text: "Das Festzelt.".to_owned()
                },
                WordedItem {
                    number: 3,
                    text: "Die Bänke.".to_owned()
                },
            ])
        );
        assert_eq!(bind_lines(&[1, 2], vec![line("2", "Das Festzelt.")]), None);
    }

    #[test]
    fn a_long_text_is_shortened_with_an_ellipsis() {
        assert_eq!(shorten("Festzelt", 8), "Festzelt");
        assert_eq!(shorten("Festzelt aufstellen", 9), "Festzelt…");
        assert_eq!(shorten("Fäst", 3), "Fä…");
    }
}
