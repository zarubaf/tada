//! PostgreSQL repositories, migrations, sessions and the job queue.
//!
//! # Lock order
//!
//! A transaction that locks rows of more than one kind takes them in this global order, so two transactions
//! cannot deadlock on their row locks:
//!
//! 1. the organization row (`FOR NO KEY UPDATE`), for example to check a quota of the organization;
//! 2. the changeset row;
//! 3. the event field definitions, in the order of their IDs;
//! 4. the facts, in the order of their IDs;
//! 5. the local ID counters, in the order of (scope, kind);
//! 6. the document rows, in the order of their IDs;
//! 7. the invitation rows;
//! 8. the account row (`app_user`), `FOR UPDATE` or `FOR NO KEY UPDATE`;
//! 9. the organization memberships, in the order of their user IDs;
//! 10. the event memberships, in the order of their event IDs and user IDs;
//! 11. the API token rows.
//!
//! A transaction can skip a kind, but it never takes an earlier kind after a later one.
//! No transaction combines kinds 2 to 6 with kinds 7 to 11 today, except that the apply inserts event memberships,
//! which takes a key share lock on the organization membership of the reviewer.
//! A new command that needs the organization lock takes it first, before the lock of a changeset.
//! The apply of a changeset (`review::lock_targets`) takes kinds 2 to 6 before its checks and writes.
//! The upload of a document version (`documents::fits_quota`) takes kind 1, then kind 5 or 6.
//! The approval of a draft version (`DocumentStore::approve`) takes kind 6 only.
//! An invitation (`members::invite`) takes kind 1, then kind 7.
//! The acceptance of an invitation takes kind 7, then the account `FOR UPDATE` (kind 8), then kind 9.
//! The removal of a member (`members::remove`) takes the account `FOR NO KEY UPDATE` (kind 8), then kind 9 and
//! kind 10 (`members::lock_membership`), and its cascade deletes rows of kind 11.
//! The lock of the account lets the key share lock of a foreign-key insert pass, so a writer that refers to the
//! account does not wait for a removal.
//! A change of an event role (`event_members::lock_member`) takes kind 10.
//! A change of a workstream (`workstreams::change`) locks its one row and takes no other kind.
//! A change of an action or a commitment (`work::lock_version`) locks its one row and takes no other kind.
//! A legal redaction (`redaction::apply`) locks its target row and the cited source items and versions `FOR UPDATE`
//! before it reads its cascade, and then updates content rows, among them document rows (kind 6).
//! Each writer that copies or cites a source version takes `FOR SHARE` on it first (`redaction::share_sources`):
//! a proposal insert at its start, an apply before kind 2, an upload of a new version on its source item after kind 6.
//! So a writer and a redaction wait for each other, and no copy of redacted content goes in (ADR 0072).

mod actor;
mod audit;
mod bootstrap;
mod database;
mod dispute_notes;
mod documents;
mod drafts;
mod error;
mod event_members;
mod events;
mod evidence;
mod export;
mod facts;
mod heartbeat;
mod housekeeping;
mod identity;
mod jobs;
mod local_ids;
mod members;
mod outbound;
mod parties;
mod person_data;
mod privacy;
mod proposals;
pub mod rate_limit;
mod redacted;
mod redaction;
mod retention;
mod review;
mod session;
mod sign_in;
mod sources;
mod telegram;
#[cfg(any(test, feature = "testing"))]
pub mod testing;
mod token;
mod tokens;
mod values;
mod work;
mod workstreams;

pub use database::{Database, MigrationFailed, SchemaCheckError};
pub use facts::SyncCatalogError;
pub use sign_in::PgSignInRequestStore;
