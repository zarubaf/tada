//! The answer of a member to a digest item, from a button in Telegram (ADR 0076).
//!
//! A press changes nothing unless each check passes: the Telegram account has a confirmed link, its user is a
//! current member of the organization of the digest and its recipient, the digest went out less than
//! `BUTTON_LIFE` ago, the answer fits the record, the member can still read the event and change the record, and
//! the record has the version of the item. Blocked, Done and Fulfilled run the existing work command with the
//! capability of the member (ADR 0068). A failed check is `Outdated`, so the member opens the record in tada.
//!
//! The response follows the change and writes only while the record has the version that the answer made. So a
//! response never stands without its change. A change without its response, after a failure between the two
//! writes, leaves the record changed: Done and Fulfilled close it, and a new version of a blocked action counts as
//! new for the rules anyway.

use std::fmt::Debug;

use async_trait::async_trait;
use jiff::{SignedDuration, Timestamp};
use tada_domain::RecordVersion;
use tada_domain::ids::{ActionId, CommitmentId, DigestItemId, EventId, UserId};
use tada_domain::reminders::{Answer, CallbackData, RecordKind};
use tada_domain::work::{ActionStatus, CommitmentStatus};
use uuid::Uuid;

use crate::access::readable_events;
use crate::caller::{MemberCaller, OrgScope, ServiceCaller, TelegramGateway};
use crate::store::StoreError;
use crate::telegram::{TelegramActError, TelegramLinks, TelegramUserId, linked_members};
use crate::work::{
    ActionChange, CommitmentChange, WorkError, WorkPorts, change_action, change_commitment,
    get_action, get_commitment,
};

/// A button works for 7 days after the send of its digest (ADR 0076).
pub const BUTTON_LIFE: SignedDuration = SignedDuration::from_hours(7 * 24);

/// A digest item as a press finds it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PressedItem {
    pub kind: RecordKind,
    pub record_id: Uuid,
    pub event_id: EventId,
    /// The version of the record in the digest.
    pub version: RecordVersion,
    /// The time of the send, or of the claim if the outcome is unknown.
    pub sent_at: Timestamp,
}

/// The repository port of the answers.
#[async_trait]
pub trait DigestAnswers: Debug + Send + Sync {
    /// The item `item` of a digest of the organization to `user` that went out: its outcome is `sent` or `unknown`.
    /// `None` for any other item, also for an item whose record does not exist any more.
    async fn pressed(
        &self,
        scope: OrgScope,
        item: DigestItemId,
        user: UserId,
    ) -> Result<Option<PressedItem>, StoreError>;

    /// Writes `answer` of `user` at `at` as the response to the record of `item` for `version`, and counts
    /// `version` as told to the user. It writes only if the record has the version `version` and the owner `user`
    /// now. Returns false if it wrote nothing.
    async fn respond(
        &self,
        scope: OrgScope,
        item: DigestItemId,
        user: UserId,
        answer: Answer,
        version: RecordVersion,
        at: Timestamp,
    ) -> Result<bool, StoreError>;
}

/// The ports of `answer`.
#[derive(Debug, Clone, Copy)]
pub struct AnswerPorts<'a> {
    pub links: &'a dyn TelegramLinks,
    pub answers: &'a dyn DigestAnswers,
    pub work: WorkPorts<'a>,
}

/// What a press did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnswerOutcome {
    /// On track: the response is written, the record stays as it is.
    Recorded,
    /// The command changed the record, and the response is written.
    Changed,
    /// A check failed: nothing changed.
    Outdated,
}

/// Handles the press of the answer button `data` by the Telegram account `account` at `now`.
/// Only a failure of the store is an error.
pub async fn answer(
    gateway: &ServiceCaller<TelegramGateway>,
    account: TelegramUserId,
    data: CallbackData,
    now: Timestamp,
    ports: AnswerPorts<'_>,
) -> Result<AnswerOutcome, TelegramActError> {
    let callers = match linked_members(gateway, account, ports.links, ports.work.identity).await {
        Ok(callers) => callers,
        Err(TelegramActError::NotLinked) => return Ok(AnswerOutcome::Outdated),
        Err(error) => return Err(error),
    };
    // The reference is random and unique, so at most one organization of the user has the item.
    for caller in callers {
        if let Some(item) = ports
            .answers
            .pressed(caller.scope(), data.item, caller.user_id())
            .await?
        {
            return Ok(press(&caller, data, item, now, ports).await?);
        }
    }
    Ok(AnswerOutcome::Outdated)
}

/// The checks and the change of a press of `caller`, the recipient of `item`.
async fn press(
    caller: &MemberCaller,
    data: CallbackData,
    item: PressedItem,
    now: Timestamp,
    ports: AnswerPorts<'_>,
) -> Result<AnswerOutcome, StoreError> {
    if now >= item.sent_at + BUTTON_LIFE || !fits(data.answer, item.kind) {
        return Ok(AnswerOutcome::Outdated);
    }
    let reach = readable_events(caller, ports.work.identity).await?;
    if !reach.includes(item.event_id) {
        return Ok(AnswerOutcome::Outdated);
    }
    let version = match data.answer {
        Answer::OnTrack => still_changeable(caller, item, ports.work)
            .await?
            .then_some(item.version),
        Answer::Blocked => {
            change_action_status(caller, item, ActionStatus::Blocked, ports.work).await?
        }
        Answer::Done => change_action_status(caller, item, ActionStatus::Done, ports.work).await?,
        Answer::Fulfilled => fulfil(caller, item, ports.work).await?,
    };
    let Some(version) = version else {
        return Ok(AnswerOutcome::Outdated);
    };
    let responded = ports
        .answers
        .respond(
            caller.scope(),
            data.item,
            caller.user_id(),
            data.answer,
            version,
            now,
        )
        .await?;
    Ok(match (data.answer, responded) {
        (Answer::OnTrack, true) => AnswerOutcome::Recorded,
        // The record changed between the check and the write.
        (Answer::OnTrack, false) => AnswerOutcome::Outdated,
        (_, true) => AnswerOutcome::Changed,
        (_, false) => {
            tracing::info!(
                "the record changed again after the answer; the response is not written"
            );
            AnswerOutcome::Changed
        }
    })
}

/// True if the button of `answer` belongs to a record of `kind` (C6: a commitment has no Blocked).
fn fits(answer: Answer, kind: RecordKind) -> bool {
    matches!(
        (answer, kind),
        (Answer::OnTrack, _)
            | (Answer::Blocked | Answer::Done, RecordKind::Action)
            | (Answer::Fulfilled, RecordKind::Commitment)
    )
}

/// True if the record has the version of the item and the member can still change it.
async fn still_changeable(
    caller: &MemberCaller,
    item: PressedItem,
    work: WorkPorts<'_>,
) -> Result<bool, StoreError> {
    let shown = match item.kind {
        RecordKind::Action => get_action(
            caller,
            item.event_id,
            ActionId::from_uuid(item.record_id),
            work,
        )
        .await
        .map(|shown| (shown.record.version, shown.can_change)),
        RecordKind::Commitment => get_commitment(
            caller,
            item.event_id,
            CommitmentId::from_uuid(item.record_id),
            work,
        )
        .await
        .map(|shown| (shown.record.version, shown.can_change)),
    };
    match refused(shown)? {
        Some((version, can_change)) => Ok(version == item.version && can_change),
        None => Ok(false),
    }
}

/// Sets the status of the action of the item. Returns the new version, or `None` if the command refused.
async fn change_action_status(
    caller: &MemberCaller,
    item: PressedItem,
    status: ActionStatus,
    work: WorkPorts<'_>,
) -> Result<Option<RecordVersion>, StoreError> {
    let change = ActionChange {
        title: None,
        description: None,
        owner: None,
        workstream: None,
        due_date: None,
        status: Some(status),
        expected_version: item.version,
    };
    let result = change_action(
        caller,
        item.event_id,
        ActionId::from_uuid(item.record_id),
        change,
        work,
    )
    .await
    .map(|shown| shown.record.version);
    refused(result)
}

/// Marks the commitment of the item as fulfilled. Returns the new version, or `None` if the command refused.
async fn fulfil(
    caller: &MemberCaller,
    item: PressedItem,
    work: WorkPorts<'_>,
) -> Result<Option<RecordVersion>, StoreError> {
    let change = CommitmentChange {
        text: None,
        owner: None,
        workstream: None,
        due_date: None,
        status: Some(CommitmentStatus::Fulfilled),
        expected_version: item.version,
    };
    let result = change_commitment(
        caller,
        item.event_id,
        CommitmentId::from_uuid(item.record_id),
        change,
        work,
    )
    .await
    .map(|shown| shown.record.version);
    refused(result)
}

/// The value of a work command, `None` for a refusal (a failed check), or the failure of the store.
fn refused<T>(result: Result<T, WorkError>) -> Result<Option<T>, StoreError> {
    match result {
        Ok(value) => Ok(Some(value)),
        Err(WorkError::Store(error)) => Err(error),
        Err(_) => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_commitment_has_no_blocked_or_done_and_an_action_no_fulfilled() {
        use Answer::{Blocked, Done, Fulfilled, OnTrack};
        use RecordKind::{Action, Commitment};
        for answer in [OnTrack, Blocked, Done] {
            assert!(fits(answer, Action), "{answer:?}");
        }
        assert!(!fits(Fulfilled, Action));
        for answer in [OnTrack, Fulfilled] {
            assert!(fits(answer, Commitment), "{answer:?}");
        }
        assert!(!fits(Blocked, Commitment));
        assert!(!fits(Done, Commitment));
    }
}
