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
//! 8. the organization memberships, in the order of their user IDs;
//! 9. the event memberships, in the order of their event IDs and user IDs;
//! 10. the API token rows.
//!
//! A transaction can skip a kind, but it never takes an earlier kind after a later one.
//! No transaction combines kinds 2 to 6 with kinds 7 to 10 today, except that the apply inserts event memberships,
//! which takes a key share lock on the organization membership of the reviewer.
//! A new command that needs the organization lock takes it first, before the lock of a changeset.
//! The apply of a changeset (`review::lock_targets`) takes kinds 2 to 6 before its checks and writes.
//! The upload of a document version (`documents::fits_quota`) takes kind 1, then kind 5 or 6.
//! The approval of a draft version (`DocumentStore::approve`) takes kind 6 only.
//! An invitation (`members::invite`) takes kind 1, then kind 7. The acceptance of an invitation takes kind 7, then kind 8.
//! The removal of a member (`members::lock_membership`) takes kind 8, then kind 9, and its cascade deletes rows of kind 10.
//! A change of an event role (`event_members::lock_member`) takes kind 9.

mod actor;
mod audit;
mod bootstrap;
mod database;
mod documents;
mod drafts;
mod error;
mod event_members;
mod events;
mod export;
mod facts;
mod heartbeat;
mod identity;
mod jobs;
mod members;
mod outbound;
mod privacy;
mod proposals;
pub mod rate_limit;
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

pub use database::{Database, MigrationFailed};
pub use facts::SyncCatalogError;
pub use sign_in::PgSignInRequestStore;
