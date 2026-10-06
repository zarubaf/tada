//! The port for the dependencies that a process needs to serve requests (ADR 0025).

use std::error::Error;
use std::fmt::Debug;

use async_trait::async_trait;

/// A dependency that `GET /readyz` checks, for example the database or the object storage.
#[async_trait]
pub trait DependencyCheck: Debug + Send + Sync {
    /// A short, stable name for logs, for example `database`.
    fn name(&self) -> &'static str;

    /// Returns an error if the dependency does not respond.
    async fn check(&self) -> Result<(), DependencyUnavailable>;
}

/// The dependency did not respond. The source error is for logs only.
#[derive(Debug, thiserror::Error)]
#[error("the dependency is unavailable")]
pub struct DependencyUnavailable(#[source] pub Box<dyn Error + Send + Sync>);
