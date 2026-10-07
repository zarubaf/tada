//! A PostgreSQL container with the tada schema, for tests (ADR 0003).

use secrecy::SecretString;
use sqlx::AssertSqlSafe;
use sqlx::types::Uuid;
use tada_app::domain::identity::{DisplayName, Email, OrganizationRole};
use tada_app::domain::ids::{OrganizationId, UserId};
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

    /// Creates a user with an email identity.
    ///
    /// # Panics
    ///
    /// If the insert fails, for example because the email address is taken.
    #[allow(clippy::unwrap_used)]
    pub async fn create_user(&self, display_name: &DisplayName, email: &Email) -> UserId {
        let id = Uuid::now_v7();
        let mut tx = self.database.pool.begin().await.unwrap();
        sqlx::query!(
            "INSERT INTO app_user (id, display_name, created_at) VALUES ($1, $2, now())",
            id,
            display_name.as_str(),
        )
        .execute(&mut *tx)
        .await
        .unwrap();
        sqlx::query!(
            "INSERT INTO email_identity (user_id, email, created_at) VALUES ($1, $2, now())",
            id,
            email.as_str(),
        )
        .execute(&mut *tx)
        .await
        .unwrap();
        tx.commit().await.unwrap();
        UserId::from_uuid(id)
    }

    /// Fails if any text, `jsonb` or `bytea` column of any table contains `secret` (ADR 0008).
    ///
    /// # Panics
    ///
    /// If a table contains the secret, or a query fails.
    #[allow(clippy::unwrap_used)]
    pub async fn assert_no_plaintext(&self, secret: &str) {
        let columns: Vec<(String, String, String)> = sqlx::query_as(
            "SELECT quote_ident(table_name), quote_ident(column_name), data_type::text
             FROM information_schema.columns
             WHERE table_schema = 'public' AND data_type IN ('text', 'character varying', 'jsonb', 'bytea')",
        )
        .fetch_all(&self.database.pool)
        .await
        .unwrap();
        assert!(!columns.is_empty(), "the schema has no columns to search");
        for (table, column, data_type) in columns {
            let value = match data_type.as_str() {
                "bytea" => column.clone(),
                _ => format!("convert_to({column}::text, 'UTF8')"),
            };
            // The names come from the catalog, quoted by `quote_ident`. The secret is a bind parameter.
            let found: bool = sqlx::query_scalar(AssertSqlSafe(format!(
                "SELECT EXISTS (SELECT 1 FROM {table} WHERE position(convert_to($1, 'UTF8') IN {value}) > 0)"
            )))
            .bind(secret)
            .fetch_one(&self.database.pool)
            .await
            .unwrap();
            assert!(
                !found,
                "the column {table}.{column} holds the secret in plain text"
            );
        }
    }

    /// Adds a membership of a user in an organization.
    ///
    /// # Panics
    ///
    /// If the insert fails.
    #[allow(clippy::unwrap_used)]
    pub async fn add_membership(
        &self,
        organization: OrganizationId,
        user: UserId,
        role: OrganizationRole,
    ) {
        sqlx::query!(
            "INSERT INTO organization_membership (organization_id, user_id, role, created_at)
             VALUES ($1, $2, $3, now())",
            organization.as_uuid(),
            user.as_uuid(),
            role.as_str(),
        )
        .execute(&self.database.pool)
        .await
        .unwrap();
    }
}
