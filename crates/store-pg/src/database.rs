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
    /// image still starts after an expand migration (ADR 0006).
    pub async fn migrate(&self) -> Result<(), MigrationFailed> {
        let mut migrator = sqlx::migrate!();
        migrator.set_ignore_missing(true);
        migrator.run(&self.pool).await.map_err(MigrationFailed)
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
    /// The versions of the committed migrations, in order.
    /// The numbers 16 and 18 stay unused: an existing database would apply a migration with such a number
    /// after the newer migrations, and a new database before them (see `migrations/README.md`).
    const VERSIONS: [i64; 22] = [
        1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 17, 19, 20, 21, 22, 23, 24,
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
