//! A PostgreSQL container with the tada schema, for tests (ADR 0003).

use jiff::Timestamp;
use secrecy::{ExposeSecret, SecretString};
use sqlx::AssertSqlSafe;
use sqlx::types::Uuid;
use tada_app::domain::facts::{CORE_CATALOG_VERSION, core_catalog};
use tada_app::domain::identity::{DisplayName, Email, OrganizationRole};
use tada_app::domain::ids::{EventId, InvitationId, OrganizationId, UserId};
use tada_app::outbound::Purpose;
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
    url: String,
    _container: ContainerAsync<Postgres>,
}

impl TestDatabase {
    /// Starts a new container, applies all migrations and writes the shipped core catalog, as `tada migrate` does.
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
        database
            .sync_catalog(&core_catalog(), CORE_CATALOG_VERSION)
            .await
            .unwrap();
        Self {
            database,
            url,
            _container: container,
        }
    }

    /// The URL of the database for a process under test. The password of the user is `postgres`.
    #[must_use]
    pub fn url(&self) -> &str {
        &self.url
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

    /// Creates an event with the key `key` in the organization.
    ///
    /// # Panics
    ///
    /// If the insert fails, for example because the key is taken.
    #[allow(clippy::unwrap_used)]
    pub async fn create_event(&self, organization: OrganizationId, key: &str) -> EventId {
        let id = Uuid::now_v7();
        sqlx::query!(
            "INSERT INTO event (id, organization_id, key, name, time_zone, version, created_at)
             VALUES ($1, $2, $3, 'Open Day Testwil', 'Europe/Zurich', 1, now())",
            id,
            organization.as_uuid(),
            key,
        )
        .execute(&self.database.pool)
        .await
        .unwrap();
        EventId::from_uuid(id)
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

    /// Starts a session of a user at the time `now` and returns the value of its session cookie.
    /// A test with a moved clock passes the time of that clock, so the session is not idle.
    ///
    /// # Panics
    ///
    /// If the insert fails, for example because the user does not exist.
    #[allow(clippy::unwrap_used)]
    pub async fn sign_in(
        &self,
        user: UserId,
        organization: Option<OrganizationId>,
        now: Timestamp,
    ) -> String {
        let mut conn = self.database.pool.acquire().await.unwrap();
        let token = crate::session::insert_session(&mut conn, user, organization, None, now)
            .await
            .unwrap();
        token.expose_secret().to_owned()
    }

    /// Creates a new member of the organization `slug` with a session in that organization.
    /// The session starts at the wall-clock time; it suits tests with the system clock.
    /// It creates the organization if the slug is free. The user has an invented name and email address.
    /// Returns the organization, the user and the value of the session cookie.
    ///
    /// # Panics
    ///
    /// If an insert fails.
    #[allow(clippy::unwrap_used)]
    pub async fn member(
        &self,
        slug: &str,
        role: OrganizationRole,
    ) -> (OrganizationId, UserId, String) {
        sqlx::query!(
            "INSERT INTO organization (id, slug, name, created_at) VALUES ($1, $2, $2, now())
             ON CONFLICT (slug) DO NOTHING",
            Uuid::now_v7(),
            slug,
        )
        .execute(&self.database.pool)
        .await
        .unwrap();
        let organization = sqlx::query_scalar!("SELECT id FROM organization WHERE slug = $1", slug)
            .fetch_one(&self.database.pool)
            .await
            .unwrap();
        let organization = OrganizationId::from_uuid(organization);
        let number = Uuid::now_v7().simple();
        let user = self
            .create_user(
                &DisplayName::parse(&format!("Member {number}")).unwrap(),
                &Email::parse(&format!("member-{number}@example.org")).unwrap(),
            )
            .await;
        self.add_membership(organization, user, role).await;
        let cookie = self
            .sign_in(user, Some(organization), Timestamp::now())
            .await;
        (organization, user, cookie)
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

    /// The value of a query that returns one row with one column, for the assertions of tests
    /// outside this crate.
    ///
    /// # Panics
    ///
    /// If the query fails or returns no row.
    #[allow(clippy::unwrap_used)]
    pub async fn scalar<T>(&self, sql: &str) -> T
    where
        T: for<'r> sqlx::Decode<'r, sqlx::Postgres> + sqlx::Type<sqlx::Postgres> + Send + Unpin,
    {
        // A test writes the query; it holds no input from outside.
        sqlx::query_scalar(AssertSqlSafe(sql.to_owned()))
            .fetch_one(&self.database.pool)
            .await
            .unwrap()
    }

    /// Queues an outbound intent in a transaction that commits if `commit` is true and rolls back
    /// otherwise.
    ///
    /// # Panics
    ///
    /// If the insert fails.
    #[allow(clippy::unwrap_used)]
    pub async fn queue_outbound(&self, purpose: &Purpose, commit: bool) -> Uuid {
        let mut tx = self.database.pool.begin().await.unwrap();
        let id = crate::outbound::queue_outbound(&mut tx, purpose, Some(Uuid::now_v7()))
            .await
            .unwrap();
        if commit {
            tx.commit().await.unwrap();
        } else {
            tx.rollback().await.unwrap();
        }
        id
    }

    /// Creates a pending invitation and queues its mail, as an invitation command does.
    ///
    /// # Panics
    ///
    /// If an insert fails.
    #[allow(clippy::unwrap_used)]
    pub async fn queue_invitation(
        &self,
        organization: OrganizationId,
        email: &Email,
        display_name: &DisplayName,
        role: OrganizationRole,
    ) -> InvitationId {
        let id = InvitationId::from_uuid(Uuid::now_v7());
        let mut tx = self.database.pool.begin().await.unwrap();
        sqlx::query!(
            "INSERT INTO invitation (id, organization_id, email, display_name, role, created_at)
             VALUES ($1, $2, $3, $4, $5, now())",
            id.as_uuid(),
            organization.as_uuid(),
            email.as_str(),
            display_name.as_str(),
            role.as_str(),
        )
        .execute(&mut *tx)
        .await
        .unwrap();
        crate::outbound::queue_outbound(
            &mut tx,
            &Purpose::Invitation {
                organization_id: organization,
                invitation_id: id,
            },
            None,
        )
        .await
        .unwrap();
        tx.commit().await.unwrap();
        id
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

/// The SQLSTATE of a database error, for example `23514` for a violated CHECK.
/// Empty if the error does not come from the database.
pub fn sqlstate(error: &sqlx::Error) -> String {
    error
        .as_database_error()
        .and_then(|error| error.code())
        .unwrap_or_default()
        .into_owned()
}
