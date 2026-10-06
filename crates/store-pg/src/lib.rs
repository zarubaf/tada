//! PostgreSQL repositories, migrations, sessions and the job queue.

mod database;
#[cfg(debug_assertions)]
pub mod dev;
mod error;
mod events;
mod heartbeat;
mod jobs;
#[cfg(any(test, feature = "testing"))]
pub mod testing;

pub use database::{Database, MigrationFailed};
