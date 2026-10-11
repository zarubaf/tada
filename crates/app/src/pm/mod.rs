//! The AI-PM: the reminder policy of an organization and its daily check (ADR 0076).

pub mod policy;

/// The job kind of the daily check of an organization. Its schedule follows the policy.
pub const PM_CHECK_JOB: &str = "pm-check";
