//! A PostgreSQL container with the tada schema, for tests (ADR 0003).

use secrecy::SecretString;
use sqlx::types::Uuid;
use tada_app::domain::ids::OrganizationId;
use testcontainers_modules::postgres::Postgres;
use testcontainers_modules::testcontainers::runners::AsyncRunner;
use testcontainers_modules::testcontainers::{ContainerAsync, ImageExt};

use crate::Database;

/// The PostgreSQL image of `compose.yaml`.
const POSTGRES_TAG: &str = "18.6-trixie";

/// A migrated database. The container stops when this value is dropped.
#[derive(Debug)]
pub struct TestDatabase {
    pub database: Database,
    _container: ContainerAsync<Postgres>,
}

impl TestDatabase {
    /// Starts a new container and applies all migrations.
    ///
    /// # Panics
    ///
    /// If Docker is not available or a migration fails.
    #[allow(clippy::unwrap_used, clippy::expect_used)]
    pub async fn start() -> Self {
        let container = Postgres::default()
            .with_tag(POSTGRES_TAG)
            .start()
            .await
            .expect("cannot start PostgreSQL; is Docker running?");
        let port = container.get_host_port_ipv4(5432).await.unwrap();
        let url = format!("postgres://postgres@127.0.0.1:{port}/postgres");
        let database = Database::connect_lazy(&url, &SecretString::from("postgres")).unwrap();
        database.migrate().await.unwrap();
        Self {
            database,
            _container: container,
        }
    }

    /// Creates an organization with the slug `slug`.
    ///
    /// # Panics
    ///
    /// If the insert fails.
    #[allow(clippy::unwrap_used)]
    pub async fn create_organization(&self, slug: &str) -> OrganizationId {
        let id = Uuid::now_v7();
        sqlx::query!(
            "INSERT INTO organization (id, slug, name, created_at) VALUES ($1, $2, $2, now())",
            id,
            slug,
        )
        .execute(&self.database.pool)
        .await
        .unwrap();
        OrganizationId::from_uuid(id)
    }
}
