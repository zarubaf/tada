//! The mapping from `sqlx` errors to the store errors of the `app` crate.

use tada_app::store::StoreError;

/// The SQLSTATE codes of a deadlock and of a serialization failure. A retry of the request can succeed.
const RETRY_CODES: &[&str] = &["40P01", "40001"];

pub(crate) fn store_error(error: sqlx::Error) -> StoreError {
    match error {
        sqlx::Error::PoolTimedOut
        | sqlx::Error::PoolClosed
        | sqlx::Error::Io(_)
        | sqlx::Error::Tls(_) => StoreError::Unavailable(Box::new(error)),
        sqlx::Error::Database(ref database)
            if database
                .code()
                .is_some_and(|code| RETRY_CODES.contains(&code.as_ref())) =>
        {
            StoreError::Unavailable(Box::new(error))
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::TestDatabase;

    async fn error_of(test: &TestDatabase, sql: &str) -> sqlx::Error {
        sqlx::query(sqlx::AssertSqlSafe(sql.to_owned()))
            .execute(&test.database.pool)
            .await
            .unwrap_err()
    }

    /// A deadlock or a serialization failure goes away on a retry, so the client gets `unavailable`, not `internal`.
    #[tokio::test]
    async fn deadlocks_and_serialization_failures_are_unavailable() {
        let test = TestDatabase::start().await;
        for code in ["40P01", "40001"] {
            let sql =
                format!("DO $$ BEGIN RAISE EXCEPTION 'retry' USING ERRCODE = '{code}'; END $$");
            let error = store_error(error_of(&test, &sql).await);
            assert!(
                matches!(error, StoreError::Unavailable(_)),
                "{code}: {error:?}"
            );
        }
        let error = store_error(error_of(&test, "SELECT 1 / 0").await);
        assert!(matches!(error, StoreError::Internal(_)), "{error:?}");
    }
}
