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
//! 5. the local ID counters, in the order of their scopes;
//! 6. the document row.
//!
//! A transaction can skip a kind, but it never takes an earlier kind after a later one.
//! A new command that needs the organization lock takes it first, before the lock of a changeset.
//! The apply of a changeset (`review::lock_targets`) takes kinds 2 to 5 before its checks and writes.
//! The upload of a document version (`documents::fits_quota`) takes kind 1, then kind 5 or 6.

mod actor;
mod audit;
mod bootstrap;
mod database;
mod documents;
mod error;
mod event_members;
mod events;
mod facts;
mod heartbeat;
mod identity;
mod jobs;
mod members;
mod outbound;
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
