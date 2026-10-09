//! My Work (spec 2a, section 4): the open work of the caller across its events.

use crate::access::{self, Principal};
use crate::work::{MyWork, WorkError, WorkStore};

/// What "My Work" shows (spec 2a, section 4).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MyWorkView {
    pub work: MyWork,
    /// The proposals that the caller reviews. Zero until the review routing feeds it.
    pub review_count: u32,
}

/// The open records of the caller in the events where the caller has a role (spec 2a, section 4).
pub async fn my_work(
    caller: &impl Principal,
    work: &dyn WorkStore,
) -> Result<MyWorkView, WorkError> {
    let all_events = access::sees_all_events(caller);
    let work = work
        .my_open_work(caller.scope(), caller.user_id(), all_events)
        .await?;
    Ok(MyWorkView {
        work,
        review_count: 0,
    })
}
