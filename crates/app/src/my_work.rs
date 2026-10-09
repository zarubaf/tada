//! My Work (spec 2a, section 4): the open work of the caller across its events.

use std::collections::HashMap;
use std::collections::hash_map::Entry;

use tada_domain::ids::EventId;

use crate::access::{self, Principal};
use crate::records::{Shown, shown};
use crate::review::{InboxPorts, ReviewStore, review_count};
use crate::work::{
    ActionView, ChangeRights, CommitmentView, InEvent, WorkError, WorkPorts, WorkRecord,
};

/// What "My Work" shows (spec 2a, section 4).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MyWorkView {
    /// The actions with the status `open`, `in-progress` or `blocked`.
    /// Due date first, a record without a due date last, then event key and number.
    pub actions: Vec<InEvent<Shown<ActionView>>>,
    /// The commitments with the status `conditional` or `firm`, in the same order.
    pub commitments: Vec<InEvent<Shown<CommitmentView>>>,
    /// The open proposals in the Review Inbox of the caller (ADR 0067).
    pub review_count: u32,
}

/// The open records of the caller in the events that the caller can read, and the number of proposals in the Review
/// Inbox of the caller (spec 2a, section 4).
pub async fn my_work(
    caller: &impl Principal,
    ports: WorkPorts<'_>,
    review: &dyn ReviewStore,
) -> Result<MyWorkView, WorkError> {
    let events = access::readable_events(caller, ports.identity).await?;
    let work = ports
        .work
        .my_open_work(caller.scope(), caller.user_id(), &events)
        .await?;
    let mut rights: HashMap<EventId, ChangeRights> = HashMap::new();
    let record_events = work
        .actions
        .iter()
        .map(|action| action.record.event_id)
        .chain(work.commitments.iter().map(|c| c.record.event_id));
    for event in record_events {
        if let Entry::Vacant(entry) = rights.entry(event) {
            let access = access::event_access(caller, event, ports.identity).await?;
            entry.insert(ChangeRights::of_event(caller, access, event, ports.workstreams).await?);
        }
    }
    let can_change = |record: &dyn WorkRecord, event: EventId| {
        rights
            .get(&event)
            .is_some_and(|rights| rights.allow(record.owner(), record.workstream()))
    };
    let actions = shown(
        caller,
        work.actions,
        |action| action.record.record_ref(),
        |action| can_change(&action.record, action.record.event_id),
        ports.identity,
        ports.work,
    )
    .await?;
    let commitments = shown(
        caller,
        work.commitments,
        |commitment| commitment.record.record_ref(),
        |commitment| can_change(&commitment.record, commitment.record.event_id),
        ports.identity,
        ports.work,
    )
    .await?;
    let inbox = InboxPorts {
        identity: ports.identity,
        review,
        work: ports.work,
        workstreams: ports.workstreams,
    };
    Ok(MyWorkView {
        actions: in_events(actions),
        commitments: in_events(commitments),
        review_count: review_count(caller, inbox, ports.clock.now()).await?,
    })
}

/// Moves the event key out of the shown record.
fn in_events<T>(items: Vec<Shown<InEvent<T>>>) -> Vec<InEvent<Shown<T>>> {
    items
        .into_iter()
        .map(|shown| InEvent {
            event_key: shown.record.event_key,
            record: Shown {
                record: shown.record.record,
                evidence: shown.evidence,
                can_change: shown.can_change,
            },
        })
        .collect()
}
