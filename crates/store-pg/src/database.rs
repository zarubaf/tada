//! The connection pool, the migrations and the readiness check.

use std::str::FromStr;
use std::time::Duration;

use async_trait::async_trait;
use secrecy::{ExposeSecret, SecretString};
use sqlx::PgPool;
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use tada_app::health::{DependencyCheck, DependencyUnavailable};

/// The tada database. Cloning is cheap: all clones share one pool.
#[derive(Debug, Clone)]
pub struct Database {
    pub(crate) pool: PgPool,
}

/// A migration failed. The source error names the migration.
#[derive(Debug, thiserror::Error)]
#[error("the migration failed")]
pub struct MigrationFailed(#[source] sqlx::migrate::MigrateError);

/// The schema check failed.
#[derive(Debug, thiserror::Error)]
pub enum SchemaCheckError {
    /// A newer release migrated the database. Start the newer image or restore the previous schema.
    #[error("the database schema {applied} is newer than the schema {known} of this binary")]
    TooNew { applied: i64, known: i64 },
    /// The check could not read the schema, for example because the database is down.
    #[error("cannot read the schema version")]
    Unavailable(#[source] sqlx::Error),
}

/// The newest migration that the database applied, or `None` on an empty database.
pub(crate) async fn applied_schema(pool: &PgPool) -> Result<Option<i64>, sqlx::Error> {
    sqlx::query_scalar!(r#"SELECT max(version) AS "version?" FROM _sqlx_migrations WHERE success"#)
        .fetch_one(pool)
        .await
}

/// The number of the last migration that this binary has.
fn known_schema() -> i64 {
    sqlx::migrate!()
        .iter()
        .map(|migration| migration.version)
        .max()
        .unwrap_or(0)
}

impl Database {
    /// Creates the pool. The first query opens the first connection, so a process can start while the
    /// database is down, and `GET /readyz` shows it.
    ///
    /// The URL must not contain the password (ADR 0036).
    pub fn connect_lazy(url: &str, password: &SecretString) -> Result<Self, sqlx::Error> {
        Self::connect_lazy_with_timeout(url, password, Duration::from_secs(5))
    }

    /// Like [`Self::connect_lazy`] with another acquire timeout. Only tests use it.
    pub(crate) fn connect_lazy_with_timeout(
        url: &str,
        password: &SecretString,
        acquire_timeout: Duration,
    ) -> Result<Self, sqlx::Error> {
        let options = PgConnectOptions::from_str(url)?
            .password(password.expose_secret())
            .application_name("tada");
        let pool = PgPoolOptions::new()
            .acquire_timeout(acquire_timeout)
            .connect_lazy_with(options);
        Ok(Self { pool })
    }

    /// Applies the pending migrations.
    ///
    /// Migrations that the database has but this binary does not know stay applied, so the previous
    /// image can still run `migrate`. The other roles of a previous image refuse to start on such a
    /// schema (`check_schema`, ADR 0075): start the newer image, or restore the previous schema.
    pub async fn migrate(&self) -> Result<(), MigrationFailed> {
        let mut migrator = sqlx::migrate!();
        migrator.set_ignore_missing(true);
        migrator.run(&self.pool).await.map_err(MigrationFailed)
    }

    /// Fails if the database has a migration that this binary does not know and that is newer than all
    /// migrations that it knows. The binary must not start on such a schema (ADR 0075).
    pub async fn check_schema(&self) -> Result<(), SchemaCheckError> {
        let applied = applied_schema(&self.pool)
            .await
            .map_err(SchemaCheckError::Unavailable)?;
        let known = known_schema();
        match applied {
            Some(applied) if applied > known => Err(SchemaCheckError::TooNew { applied, known }),
            _ => Ok(()),
        }
    }

    /// Closes all connections. Waits for the connections in use.
    pub async fn close(&self) {
        self.pool.close().await;
    }
}

#[async_trait]
impl DependencyCheck for Database {
    fn name(&self) -> &'static str {
        "database"
    }

    async fn check(&self) -> Result<(), DependencyUnavailable> {
        sqlx::query("SELECT 1")
            .execute(&self.pool)
            .await
            .map(|_| ())
            .map_err(|error| DependencyUnavailable(Box::new(error)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::TestDatabase;

    #[tokio::test]
    async fn role_refuses_newer_schema() {
        let test = TestDatabase::start().await;
        test.database.check_schema().await.unwrap();

        sqlx::query(
            "INSERT INTO _sqlx_migrations (version, description, success, checksum, execution_time)
             VALUES ($1, 'from a newer release', true, '\\x00', 1)",
        )
        .bind(known_schema() + 1)
        .execute(test.pool())
        .await
        .unwrap();
        let error = test.database.check_schema().await.unwrap_err();
        assert!(
            matches!(error, SchemaCheckError::TooNew { applied, known } if applied == known + 1)
        );
    }

    /// The versions of the committed migrations, in order.
    /// The numbers 16 and 18 stay unused: an existing database would apply a migration with such a number
    /// after the newer migrations, and a new database before them (see `migrations/README.md`).
    const VERSIONS: [i64; 26] = [
        1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 17, 19, 20, 21, 22, 23, 24, 25, 26, 27,
        28,
    ];

    #[test]
    fn a_new_migration_takes_the_next_number_after_the_highest() {
        let versions: Vec<i64> = sqlx::migrate!()
            .iter()
            .map(|migration| migration.version)
            .collect();
        assert_eq!(
            versions, VERSIONS,
            "add a new migration with the next number after the highest, then add the number here"
        );
    }
}
