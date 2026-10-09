//! My Work (spec 2a, section 4): the open work of the caller across its events.

use crate::access::{self, Principal};
use crate::records::{RecordRef, Shown, shown};
use crate::work::{ActionView, CommitmentView, InEvent, WorkError, WorkPorts};

/// What "My Work" shows (spec 2a, section 4).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MyWorkView {
    /// The actions with the status `open`, `in-progress` or `blocked`.
    /// Due date first, a record without a due date last, then event key and number.
    pub actions: Vec<InEvent<Shown<ActionView>>>,
    /// The commitments with the status `conditional` or `firm`, in the same order.
    pub commitments: Vec<InEvent<Shown<CommitmentView>>>,
    /// The proposals that the caller reviews. Zero until the review routing feeds it.
    pub review_count: u32,
}

/// The open records of the caller in the events that the caller can read (spec 2a, section 4).
pub async fn my_work(
    caller: &impl Principal,
    ports: WorkPorts<'_>,
) -> Result<MyWorkView, WorkError> {
    let events = access::readable_events(caller, ports.identity).await?;
    let work = ports
        .work
        .my_open_work(caller.scope(), caller.user_id(), &events)
        .await?;
    let actions = in_events(
        shown(
            caller,
            work.actions,
            |action| RecordRef::Action(action.record.id),
            ports.identity,
            ports.work,
        )
        .await?,
    );
    let commitments = in_events(
        shown(
            caller,
            work.commitments,
            |commitment| RecordRef::Commitment(commitment.record.id),
            ports.identity,
            ports.work,
        )
        .await?,
    );
    Ok(MyWorkView {
        actions,
        commitments,
        review_count: 0,
    })
}

/// Moves the event key out of the shown record.
fn in_events<T>(items: Vec<Shown<InEvent<T>>>) -> Vec<InEvent<Shown<T>>> {
    items
        .into_iter()
        .map(|Shown { record, evidence }| InEvent {
            event_key: record.event_key,
            record: Shown {
                record: record.record,
                evidence,
            },
        })
        .collect()
}
