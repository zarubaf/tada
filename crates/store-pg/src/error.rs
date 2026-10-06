//! The mapping from `sqlx` errors to the store errors of the `app` crate.

use tada_app::store::StoreError;

pub(crate) fn store_error(error: sqlx::Error) -> StoreError {
    match error {
        sqlx::Error::PoolTimedOut
        | sqlx::Error::PoolClosed
        | sqlx::Error::Io(_)
        | sqlx::Error::Tls(_) => StoreError::Unavailable(Box::new(error)),
        error => StoreError::Internal(Box::new(error)),
    }
}

/// A row that breaks a rule of the domain types. The constraints should make this impossible.
#[derive(Debug, thiserror::Error)]
#[error("the column {0} holds an invalid value")]
pub(crate) struct InvalidRow(pub &'static str);

impl From<InvalidRow> for StoreError {
    fn from(error: InvalidRow) -> Self {
        StoreError::Internal(Box::new(error))
    }
}
