//! Durable jobs (ADRs 0007 and 0054): the port of the queue, the port of a job handler, and the step
//! that runs one job.

use std::collections::{BTreeSet, HashMap};
use std::fmt::Debug;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use jiff::Timestamp;
use serde_json::Value;
use tada_domain::ids::OrganizationId;
use tada_domain::schedule::LocalSchedule;
use uuid::Uuid;

use crate::store::StoreError;

/// A job that a command adds in its own transaction.
#[derive(Debug, Clone, PartialEq)]
pub struct NewJob {
    /// The kind selects the handler, for example `send-invitation`.
    pub kind: &'static str,
    /// The version of the payload. A handler accepts all versions that can still be in the queue.
    pub version: i32,
    /// No direct identifiers: names and addresses stay in the records that the payload refers to.
    pub payload: Value,
    pub organization_id: Option<OrganizationId>,
    /// The request that caused the job (ADR 0035).
    pub request_id: Option<Uuid>,
    /// The earliest start. `None` means now.
    pub run_at: Option<Timestamp>,
}

/// A job that a worker claimed.
#[derive(Debug, Clone, PartialEq)]
pub struct Job {
    pub id: Uuid,
    pub kind: String,
    pub version: i32,
    pub payload: Value,
    pub organization_id: Option<OrganizationId>,
    pub request_id: Option<Uuid>,
    /// 1 for the first attempt.
    pub attempt: i32,
    /// After a failed attempt with this number, the job fails for good.
    pub max_attempts: i32,
}

impl Job {
    /// True if a failure of this attempt fails the job for good.
    pub fn is_last_attempt(&self) -> bool {
        self.attempt >= self.max_attempts
    }
}

/// The queue, as the worker sees it. The database clock gives due times and leases (ADR 0038).
#[async_trait]
pub trait JobQueue: Debug + Send + Sync {
    /// Claims the oldest due job for `lease`. Another worker can claim the job again after the lease.
    /// Infrastructure query (ADR 0039): the worker serves the jobs of all organizations, and the job names its own.
    async fn claim(&self, worker_id: Uuid, lease: Duration) -> Result<Option<Job>, StoreError>;

    /// Removes a job that completed. Returns false if the worker no longer holds the job.
    /// Infrastructure query (ADR 0039): the worker holds the job, and the job names its own organization.
    async fn complete(&self, job: &Job, worker_id: Uuid) -> Result<bool, StoreError>;

    /// Records a failed attempt. The job runs again after a backoff, or fails for good after its
    /// last attempt. Returns false if the worker no longer holds the job.
    /// Infrastructure query (ADR 0039): the worker holds the job, and the job names its own organization.
    async fn fail(&self, job: &Job, worker_id: Uuid, reason: &str) -> Result<bool, StoreError>;
}

/// What a schedule row starts: the job kind and the most missed runs that one step starts (ADR 0076).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScheduleSpec {
    pub kind: &'static str,
    pub missed_run_limit: u8,
}

/// The job kinds that a schedule can start. A row of another kind cannot be read.
pub const SCHEDULED_KINDS: &[&str] = &[crate::pm::PM_CHECK_JOB];

/// What one scheduler step did.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ScheduleStep {
    /// The number of jobs that the step added.
    pub started: u32,
    /// The schedules that the step could not read, with their ID and kind. The step moved each one day
    /// forward, so it does not block the others.
    pub skipped: Vec<(Uuid, String)>,
}

/// The step of the worker loop that starts the jobs of due schedules (ADR 0076).
/// The database clock gives the current time (ADR 0054).
#[async_trait]
pub trait ScheduleRunner: Debug + Send + Sync {
    /// Adds a schedule of `spec` at `schedule` for each organization that has no schedule of the kind and
    /// no saved reminder policy. Returns the number of new schedules.
    /// Infrastructure query (ADR 0039): each organization without a schedule gets its own row.
    async fn ensure_default_schedules(
        &self,
        spec: ScheduleSpec,
        schedule: &LocalSchedule,
    ) -> Result<u32, StoreError>;

    /// Starts the jobs of each schedule whose next run has passed, and moves the schedule to its first
    /// run after now. One transaction for each schedule; another runner skips a schedule that this one
    /// holds, so no run starts twice. A schedule starts at most its limit of missed runs.
    /// A schedule of a kind outside `kinds` cannot be read, and it moves one day forward.
    /// Infrastructure query (ADR 0039): starts due schedules of all organizations, and each job names its own.
    async fn start_due(&self, kinds: &[&'static str]) -> Result<ScheduleStep, StoreError>;
}

/// The code that runs one kind of job. It calls `app` commands with a service caller (ADR 0039).
#[async_trait]
pub trait JobHandler: Debug + Send + Sync {
    fn kind(&self) -> &'static str;

    /// Runs the job. `Ok(Some(warning))` completes the job, and the worker logs the warning.
    async fn run(&self, job: &Job) -> Result<Option<JobWarning>, JobFailed>;
}

/// A completed job that the operator must know about, for example a mail that the server rejected.
/// The text goes into the log, so it contains no direct identifiers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobWarning(pub String);

/// A failed attempt. The reason goes into the job row and the log, so it contains no direct identifiers.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{0}")]
pub struct JobFailed(pub String);

/// The handlers of all job kinds.
#[derive(Debug, Clone, Default)]
pub struct Handlers(HashMap<&'static str, Arc<dyn JobHandler>>);

impl Handlers {
    pub fn with(mut self, handler: Arc<dyn JobHandler>) -> Self {
        self.0.insert(handler.kind(), handler);
        self
    }

    /// The job kinds that have a handler.
    pub fn kinds(&self) -> BTreeSet<&'static str> {
        self.0.keys().copied().collect()
    }
}

/// What `run_next` did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ran {
    /// No job was due.
    Idle,
    Completed(Uuid),
    /// The job completed, and its handler reported a warning.
    CompletedWithWarning(Uuid, JobWarning),
    Failed(Uuid),
    /// The lease expired and another worker holds the job now. The result of this attempt is lost.
    LeaseLost(Uuid),
}

/// Claims one due job and runs its handler.
pub async fn run_next(
    queue: &dyn JobQueue,
    handlers: &Handlers,
    worker_id: Uuid,
    lease: Duration,
) -> Result<Ran, StoreError> {
    let Some(job) = queue.claim(worker_id, lease).await? else {
        return Ok(Ran::Idle);
    };
    let result = match handlers.0.get(job.kind.as_str()) {
        Some(handler) => handler.run(&job).await,
        None => Err(JobFailed(format!(
            "no handler for the job kind {}",
            job.kind
        ))),
    };
    let held = match &result {
        Ok(_) => queue.complete(&job, worker_id).await?,
        Err(failure) => queue.fail(&job, worker_id, &failure.0).await?,
    };
    Ok(match (held, result) {
        (false, _) => Ran::LeaseLost(job.id),
        (true, Ok(None)) => Ran::Completed(job.id),
        (true, Ok(Some(warning))) => Ran::CompletedWithWarning(job.id, warning),
        (true, Err(_)) => Ran::Failed(job.id),
    })
}
