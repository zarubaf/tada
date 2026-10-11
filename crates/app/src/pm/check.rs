//! The daily check of the AI PM (ADR 0076). It plans one digest for each eligible recipient of an organization,
//! with the due, overdue and blocked work of the recipient in all events, and queues its send.
//!
//! The check reads the state again in each run. A new worker process continues where the last one stopped:
//! the cooldown and the notices come from the database, not from memory.

use std::collections::HashMap;
use std::fmt::{self, Debug};
use std::sync::Arc;

use async_trait::async_trait;
use jiff::Timestamp;
use jiff::civil::Date;
use tada_domain::events::EventTimeZone;
use tada_domain::identity::OrganizationRole;
use tada_domain::ids::UserId;
use tada_domain::reminders::{Candidate, Memory, Reason, RecordKind, Rules, in_cooldown, select};
use uuid::Uuid;

use crate::access::{EventReach, readable_events_of};
use crate::caller::{JobRunner, OrgScope, ServiceCaller};
use crate::clock::Clock;
use crate::identity::IdentityStore;
use crate::jobs::{Job, JobFailed, JobHandler, JobWarning};
use crate::pm::PM_CHECK_JOB;
use crate::pm::policy::PmPolicyStore;
use crate::store::StoreError;

/// An eligible recipient of a digest: a current member who opted in, with a confirmed Telegram link that is
/// not marked blocked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Recipient {
    pub user_id: UserId,
    /// The organization role, for the events that the recipient can read.
    pub role: OrganizationRole,
    /// The zone of the quiet hours and of the date of today.
    pub zone: EventTimeZone,
    /// The planning time of the last digest in this organization that is sent, may be sent, or waits for its
    /// send. The send time would move with the quiet hours and push the next daily digest out.
    pub last_digest_at: Option<Timestamp>,
}

/// The repository port of the check.
#[async_trait]
pub trait PmStore: Debug + Send + Sync {
    /// The eligible recipients of the organization.
    async fn recipients(&self, scope: OrgScope) -> Result<Vec<Recipient>, StoreError>;

    /// The open actions and commitments of `user` in the active events of `events` that are due on or before
    /// `until`, and the blocked actions. The order is the order of the digest: due date first, none last.
    async fn candidates(
        &self,
        scope: OrgScope,
        user: UserId,
        events: &EventReach,
        until: Date,
    ) -> Result<Vec<Candidate>, StoreError>;

    /// What the earlier digests to `user` said about these records. A record without a notice is missing.
    async fn memory(
        &self,
        scope: OrgScope,
        user: UserId,
        records: &[(RecordKind, Uuid)],
    ) -> Result<Vec<((RecordKind, Uuid), Memory)>, StoreError>;

    /// Stores a pending digest to `user` with `items` in their order, created at `now`, and its send job that
    /// starts at `send_at`, in one transaction. Returns the ID of the intent.
    async fn plan_digest(
        &self,
        scope: OrgScope,
        user: UserId,
        items: &[(Candidate, Reason)],
        now: Timestamp,
        send_at: Timestamp,
    ) -> Result<Uuid, StoreError>;
}

/// The handler of the daily check of one organization. A paused policy plans nothing.
pub struct PmCheck {
    policies: Arc<dyn PmPolicyStore>,
    identity: Arc<dyn IdentityStore>,
    store: Arc<dyn PmStore>,
    clock: Arc<dyn Clock>,
    caller: ServiceCaller<JobRunner>,
}

impl Debug for PmCheck {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PmCheck").finish_non_exhaustive()
    }
}

impl PmCheck {
    pub fn new(
        policies: Arc<dyn PmPolicyStore>,
        identity: Arc<dyn IdentityStore>,
        store: Arc<dyn PmStore>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            policies,
            identity,
            store,
            clock,
            caller: ServiceCaller::new(),
        }
    }

    /// The items of the digest to `recipient`, in the order of the candidates. A record owner can lose the
    /// access to its event (ADR 0063), so only the events that the recipient can read now count (ADR 0052).
    async fn items(
        &self,
        scope: OrgScope,
        recipient: &Recipient,
        rules: &Rules,
    ) -> Result<Vec<(Candidate, Reason)>, StoreError> {
        let user = recipient.user_id;
        let events =
            readable_events_of(scope, user, recipient.role, self.identity.as_ref()).await?;
        let candidates = self
            .store
            .candidates(scope, user, &events, rules.until())
            .await?;
        let keys: Vec<_> = candidates.iter().map(|c| (c.kind, c.id)).collect();
        let memory: HashMap<_, _> = self
            .store
            .memory(scope, user, &keys)
            .await?
            .into_iter()
            .collect();
        Ok(candidates
            .into_iter()
            .filter_map(|candidate| {
                let notice = memory.get(&(candidate.kind, candidate.id));
                select(&candidate, notice, rules).map(|reason| (candidate, reason))
            })
            .collect())
    }
}

#[async_trait]
impl JobHandler for PmCheck {
    fn kind(&self) -> &'static str {
        PM_CHECK_JOB
    }

    async fn run(&self, job: &Job) -> Result<Option<JobWarning>, JobFailed> {
        if job.version != 1 {
            return Err(JobFailed(format!(
                "unknown payload version {}",
                job.version
            )));
        }
        let organization_id = job
            .organization_id
            .ok_or_else(|| JobFailed("the job has no organization".into()))?;
        // The job names its organization, so the check reads and plans in it only (ADR 0039).
        let scope = self.caller.scope(organization_id);
        let (policy, _) = self.policies.get(scope).await.map_err(store_failed)?;
        if policy.paused {
            return Ok(None);
        }
        let now = self.clock.now();
        // A retry after a failure skips the recipients that got their digest in the first attempt:
        // a pending digest is inside the cooldown too.
        for recipient in self.store.recipients(scope).await.map_err(store_failed)? {
            if in_cooldown(recipient.last_digest_at, now, policy.cooldown_hours) {
                continue;
            }
            let rules = Rules::new(&policy, now, recipient.zone.clone());
            let items = self
                .items(scope, &recipient, &rules)
                .await
                .map_err(store_failed)?;
            if items.is_empty() {
                continue;
            }
            let send_at = policy.quiet.send_at(now, &recipient.zone);
            self.store
                .plan_digest(scope, recipient.user_id, &items, now, send_at)
                .await
                .map_err(store_failed)?;
        }
        Ok(None)
    }
}

fn store_failed(error: StoreError) -> JobFailed {
    JobFailed(format!("the store failed: {error}"))
}
