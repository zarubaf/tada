//! PostgreSQL repositories, migrations, sessions and the job queue.

mod audit;
mod database;
#[cfg(debug_assertions)]
pub mod dev;
mod error;
mod events;
mod heartbeat;
mod identity;
mod jobs;
mod outbound;
mod session;
mod sign_in;
mod telegram;
#[cfg(any(test, feature = "testing"))]
pub mod testing;
mod token;

pub use database::{Database, MigrationFailed};
