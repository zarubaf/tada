//! The AI-PM: the reminder policy of an organization and its daily check (ADR 0076).

pub mod check;
pub mod digest_wording;
pub mod policy;

/// The job kind of the daily check of an organization. Its schedule follows the policy.
pub const PM_CHECK_JOB: &str = "pm-check";

/// The job kind of the send of one digest. Payload version 1 is `{"intent_id": "<uuid>"}`.
pub const SEND_DIGEST_JOB: &str = "send-telegram-digest";
