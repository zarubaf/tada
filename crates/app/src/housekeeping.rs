//! The housekeeping of technical rows (ADR 0071).
//!
//! Sign-in links, sessions, Telegram claims, invitations, mail intents and failed jobs hold personal data
//! (an email address, a user agent, a Telegram name) that has no use after its end. A daily job deletes them
//! after a period. The same job deletes the digests of the AI PM with their items, and the notices of the
//! reminders, and it anonymizes an account that no organization membership holds any more.

use async_trait::async_trait;
use jiff::{SignedDuration, Timestamp};

use crate::caller::{JobRunner, ServiceCaller};
use crate::store::StoreError;

/// Sign-in links, sessions and Telegram claims go this long after their end.
pub const SHORT_KEEP: SignedDuration = SignedDuration::from_hours(30 * 24);
/// Finished invitations, sent or failed mail intents, finished digests, reminder notices, failed jobs and accounts
/// without membership go this long after their end.
pub const LONG_KEEP: SignedDuration = SignedDuration::from_hours(90 * 24);

/// The display name of an anonymized account. The interface shows it as it shows any redacted text.
pub const TOMBSTONE_NAME: &str = "entfernt";

/// What one run deleted: one count for each rule.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct HousekeepingReport {
    pub magic_links: u64,
    pub sessions: u64,
    pub telegram_claims: u64,
    pub invitations: u64,
    pub outbound_intents: u64,
    /// Digests of the AI PM in a final state, with their items.
    pub digests: u64,
    /// Notices of a reminder in `reminder_state`.
    pub reminder_states: u64,
    pub failed_jobs: u64,
    pub anonymized_accounts: u64,
}

#[async_trait]
pub trait HousekeepingStore: Send + Sync {
    /// Applies each rule once, as of `now`. A rule deletes the rows whose end is more than its period before `now`.
    /// The job records one audit event for each anonymized account.
    /// Infrastructure query (ADR 0039): the job covers all organizations and all accounts.
    async fn run(
        &self,
        caller: &ServiceCaller<JobRunner>,
        now: Timestamp,
    ) -> Result<HousekeepingReport, StoreError>;
}
