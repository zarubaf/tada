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
        let options = PgConnectOptions::from_str(url)?
            .password(password.expose_secret())
            .application_name("tada");
        let pool = PgPoolOptions::new()
            .acquire_timeout(Duration::from_secs(5))
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
