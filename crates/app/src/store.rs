//! Errors that all repository ports share.

use std::error::Error;

/// A repository call failed for a reason that is not a domain rule.
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    /// The database did not respond. The client can retry.
    #[error("the store is unavailable")]
    Unavailable(#[source] Box<dyn Error + Send + Sync>),
    /// An unexpected error, for example a constraint that the domain types should have prevented.
    #[error("the store failed")]
    Internal(#[source] Box<dyn Error + Send + Sync>),
}
