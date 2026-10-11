//! The AI PM: the reminder policy of an organization and its daily check (ADR 0076).

pub mod check;
pub mod digest_wording;
pub mod digests;
pub mod policy;
pub mod send;

use tada_domain::reminders::PmPolicy;

use crate::jobs::{ScheduleRunner, ScheduleSpec};
use crate::store::StoreError;

/// The job kind of the daily check of an organization. Its schedule follows the policy.
pub const PM_CHECK_JOB: &str = "pm-check";

/// The schedule row of the daily check. A missed check starts once: a worker that was down sends no burst
/// of checks (ADR 0076).
pub const PM_CHECK_SCHEDULE: ScheduleSpec = ScheduleSpec {
    kind: PM_CHECK_JOB,
    missed_run_limit: 1,
};

/// Adds the schedule of the default policy for each organization without a saved policy, so that it gets
/// its daily check too. Returns the number of new schedules.
pub async fn ensure_default_check_schedules(
    runner: &dyn ScheduleRunner,
) -> Result<u32, StoreError> {
    runner
        .ensure_default_schedules(PM_CHECK_SCHEDULE, &PmPolicy::default().schedule)
        .await
}

/// The job kind of the send of one digest. Payload version 1 is `{"intent_id": "<uuid>"}`.
pub const SEND_DIGEST_JOB: &str = "send-telegram-digest";
