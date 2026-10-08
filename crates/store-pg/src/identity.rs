//! The `IdentityStore` adapter (ADR 0008, ADR 0052).

use async_trait::async_trait;
use tada_app::caller::OrgScope;
use tada_app::domain::identity::{DisplayName, EventRole, OrganizationRole};
use tada_app::domain::ids::{EventId, OrganizationId, UserId};
use tada_app::identity::{IdentityStore, Membership, UserRef};
use tada_app::store::StoreError;

use crate::Database;
use crate::error::{InvalidRow, store_error};

pub(crate) fn organization_role(name: &str) -> Result<OrganizationRole, InvalidRow> {
    OrganizationRole::parse(name).ok_or(InvalidRow("role"))
}

#[async_trait]
impl IdentityStore for Database {
    async fn user(&self, id: UserId) -> Result<Option<UserRef>, StoreError> {
        let row = sqlx::query!(
            "SELECT display_name, locale FROM app_user WHERE id = $1",
            id.as_uuid(),
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(store_error)?;
        row.map(|row| {
            Ok(UserRef {
                id,
                display_name: DisplayName::parse(&row.display_name)
                    .map_err(|_| InvalidRow("display_name"))?,
                locale: row.locale,
            })
        })
        .transpose()
    }

    async fn memberships_of(&self, user: UserId) -> Result<Vec<Membership>, StoreError> {
        let rows = sqlx::query!(
            "SELECT m.organization_id, o.name, m.role
             FROM organization_membership m JOIN organization o ON o.id = m.organization_id
             WHERE m.user_id = $1
             ORDER BY o.name, o.id",
            user.as_uuid(),
        )
        .fetch_all(&self.pool)
        .await
        .map_err(store_error)?;
        rows.into_iter()
            .map(|row| {
                Ok(Membership {
                    organization_id: OrganizationId::from_uuid(row.organization_id),
                    organization_name: row.name,
                    role: organization_role(&row.role)?,
                })
            })
            .collect()
    }

    async fn membership(
        &self,
        scope: OrgScope,
        user: UserId,
    ) -> Result<Option<OrganizationRole>, StoreError> {
        let role = sqlx::query_scalar!(
            "SELECT role FROM organization_membership WHERE organization_id = $1 AND user_id = $2",
            scope.organization_id().as_uuid(),
            user.as_uuid(),
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(store_error)?;
        Ok(role.as_deref().map(organization_role).transpose()?)
    }

    async fn event_exists(&self, scope: OrgScope, event: EventId) -> Result<bool, StoreError> {
        sqlx::query_scalar!(
            r#"SELECT EXISTS (SELECT 1 FROM event WHERE organization_id = $1 AND id = $2) AS "exists!""#,
            scope.organization_id().as_uuid(),
            event.as_uuid(),
        )
        .fetch_one(&self.pool)
        .await
        .map_err(store_error)
    }

    async fn event_role(
        &self,
        scope: OrgScope,
        event: EventId,
        user: UserId,
    ) -> Result<Option<EventRole>, StoreError> {
        let role = sqlx::query_scalar!(
            "SELECT event_role FROM event_membership
             WHERE organization_id = $1 AND event_id = $2 AND user_id = $3",
            scope.organization_id().as_uuid(),
            event.as_uuid(),
            user.as_uuid(),
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(store_error)?;
        Ok(role
            .as_deref()
            .map(|name| EventRole::parse(name).ok_or(InvalidRow("event_role")))
            .transpose()?)
    }

    async fn event_roles_of(
        &self,
        scope: OrgScope,
        user: UserId,
    ) -> Result<Vec<(EventId, EventRole)>, StoreError> {
        let rows = sqlx::query!(
            "SELECT event_id, event_role FROM event_membership
             WHERE organization_id = $1 AND user_id = $2
             ORDER BY event_id",
            scope.organization_id().as_uuid(),
            user.as_uuid(),
        )
        .fetch_all(&self.pool)
        .await
        .map_err(store_error)?;
        Ok(rows
            .iter()
            .map(|row| {
                let role = EventRole::parse(&row.event_role).ok_or(InvalidRow("event_role"))?;
                Ok::<_, InvalidRow>((EventId::from_uuid(row.event_id), role))
            })
            .collect::<Result<_, _>>()?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{TestDatabase, sqlstate};
    use sqlx::types::Uuid;
    use tada_app::caller::MemberCaller;
    use tada_app::domain::identity::Email;

    fn name(text: &str) -> DisplayName {
        DisplayName::parse(text).unwrap()
    }

    fn email(text: &str) -> Email {
        Email::parse(text).unwrap()
    }

    fn scope(organization: OrganizationId) -> OrgScope {
        MemberCaller::new(
            UserId::from_uuid(Uuid::now_v7()),
            organization,
            OrganizationRole::Owner,
        )
        .scope()
    }

    async fn create_event(test: &TestDatabase, organization: OrganizationId) -> EventId {
        let id = Uuid::now_v7();
        sqlx::query(
            "INSERT INTO event (id, organization_id, key, name, time_zone, version, created_at)
             VALUES ($1, $2, 'TEST30', 'Open Day Testwil', 'Europe/Zurich', 1, now())",
        )
        .bind(id)
        .bind(organization.as_uuid())
        .execute(&test.database.pool)
        .await
        .unwrap();
        EventId::from_uuid(id)
    }

    async fn add_event_membership(
        test: &TestDatabase,
        organization: OrganizationId,
        event: EventId,
        user: UserId,
        role: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO event_membership (organization_id, event_id, user_id, event_role, version, created_at)
             VALUES ($1, $2, $3, $4, 1, now())",
        )
        .bind(organization.as_uuid())
        .bind(event.as_uuid())
        .bind(user.as_uuid())
        .bind(role)
        .execute(&test.database.pool)
        .await
        .map(|_| ())
    }

    #[tokio::test]
    async fn one_email_address_belongs_to_one_user() {
        let test = TestDatabase::start().await;
        test.create_user(&name("Anna Muster"), &email("anna@example.org"))
            .await;

        let second = Uuid::now_v7();
        sqlx::query(
            "INSERT INTO app_user (id, display_name, created_at) VALUES ($1, 'Ben', now())",
        )
        .bind(second)
        .execute(&test.database.pool)
        .await
        .unwrap();
        let rejected = sqlx::query(
            "INSERT INTO email_identity (user_id, email, created_at) VALUES ($1, 'anna@example.org', now())",
        )
        .bind(second)
        .execute(&test.database.pool)
        .await;
        assert!(
            rejected.is_err(),
            "the unique constraint rejects the address"
        );
    }

    #[tokio::test]
    async fn reads_a_user_by_id() {
        let test = TestDatabase::start().await;
        let anna = test
            .create_user(&name("Anna Muster"), &email("anna@example.org"))
            .await;

        let user = test.database.user(anna).await.unwrap().unwrap();
        assert_eq!(user.id, anna);
        assert_eq!(user.display_name, name("Anna Muster"));
        assert_eq!(user.locale, "de-CH");
        let unknown = UserId::from_uuid(Uuid::now_v7());
        assert_eq!(test.database.user(unknown).await.unwrap(), None);
    }

    #[tokio::test]
    async fn lists_the_memberships_of_a_user_in_two_organizations() {
        let test = TestDatabase::start().await;
        let testwil = test.create_organization("testwil").await;
        let musterhausen = test.create_organization("musterhausen").await;
        let anna = test
            .create_user(&name("Anna Muster"), &email("anna@example.org"))
            .await;
        test.add_membership(testwil, anna, OrganizationRole::Owner)
            .await;
        test.add_membership(musterhausen, anna, OrganizationRole::Member)
            .await;

        let memberships = test.database.memberships_of(anna).await.unwrap();
        assert_eq!(
            memberships,
            vec![
                Membership {
                    organization_id: musterhausen,
                    organization_name: "musterhausen".to_owned(),
                    role: OrganizationRole::Member,
                },
                Membership {
                    organization_id: testwil,
                    organization_name: "testwil".to_owned(),
                    role: OrganizationRole::Owner,
                },
            ],
            "in the order of the organization names"
        );

        assert_eq!(
            test.database
                .membership(scope(musterhausen), anna)
                .await
                .unwrap(),
            Some(OrganizationRole::Member)
        );
        let third = test.create_organization("other").await;
        assert_eq!(
            test.database.membership(scope(third), anna).await.unwrap(),
            None
        );
    }

    #[tokio::test]
    async fn reads_the_event_role_of_a_member() {
        let test = TestDatabase::start().await;
        let testwil = test.create_organization("testwil").await;
        let anna = test
            .create_user(&name("Anna Muster"), &email("anna@example.org"))
            .await;
        test.add_membership(testwil, anna, OrganizationRole::Member)
            .await;
        let event = create_event(&test, testwil).await;
        assert_eq!(
            test.database
                .event_role(scope(testwil), event, anna)
                .await
                .unwrap(),
            None
        );
        add_event_membership(&test, testwil, event, anna, "event-contributor")
            .await
            .unwrap();
        assert_eq!(
            test.database
                .event_role(scope(testwil), event, anna)
                .await
                .unwrap(),
            Some(EventRole::EventContributor)
        );
    }

    #[tokio::test]
    async fn an_event_exists_only_in_its_organization() {
        let test = TestDatabase::start().await;
        let testwil = test.create_organization("testwil").await;
        let musterhausen = test.create_organization("musterhausen").await;
        let event = create_event(&test, testwil).await;
        assert!(
            test.database
                .event_exists(scope(testwil), event)
                .await
                .unwrap()
        );
        assert!(
            !test
                .database
                .event_exists(scope(musterhausen), event)
                .await
                .unwrap()
        );
        let unknown = EventId::from_uuid(Uuid::now_v7());
        assert!(
            !test
                .database
                .event_exists(scope(testwil), unknown)
                .await
                .unwrap()
        );
    }

    #[tokio::test]
    async fn an_event_membership_cannot_cross_organizations() {
        let test = TestDatabase::start().await;
        let testwil = test.create_organization("testwil").await;
        let musterhausen = test.create_organization("musterhausen").await;
        let anna = test
            .create_user(&name("Anna Muster"), &email("anna@example.org"))
            .await;
        test.add_membership(testwil, anna, OrganizationRole::Member)
            .await;
        test.add_membership(musterhausen, anna, OrganizationRole::Member)
            .await;
        let event = create_event(&test, testwil).await;

        let rejected = add_event_membership(&test, musterhausen, event, anna, "event-viewer").await;
        assert!(
            rejected.is_err(),
            "the event belongs to another organization"
        );
    }

    #[tokio::test]
    async fn removing_an_organization_membership_removes_its_event_memberships() {
        let test = TestDatabase::start().await;
        let testwil = test.create_organization("testwil").await;
        let anna = test
            .create_user(&name("Anna Muster"), &email("anna@example.org"))
            .await;
        test.add_membership(testwil, anna, OrganizationRole::Member)
            .await;
        let event = create_event(&test, testwil).await;
        add_event_membership(&test, testwil, event, anna, "event-manager")
            .await
            .unwrap();

        sqlx::query(
            "DELETE FROM organization_membership WHERE organization_id = $1 AND user_id = $2",
        )
        .bind(testwil.as_uuid())
        .bind(anna.as_uuid())
        .execute(&test.database.pool)
        .await
        .unwrap();
        let left: i64 = sqlx::query_scalar("SELECT count(*) FROM event_membership")
            .fetch_one(&test.database.pool)
            .await
            .unwrap();
        assert_eq!(left, 0);
    }

    #[tokio::test]
    async fn a_telegram_identity_needs_a_user() {
        let test = TestDatabase::start().await;
        let rejected = sqlx::query(
            "INSERT INTO telegram_identity (telegram_user_id, user_id, linked_at) VALUES (42, $1, now())",
        )
        .bind(Uuid::now_v7())
        .execute(&test.database.pool)
        .await;
        assert!(rejected.is_err());
    }

    #[tokio::test]
    async fn an_email_identity_holds_a_normalized_address() {
        let test = TestDatabase::start().await;
        let user_id = Uuid::now_v7();
        sqlx::query(
            "INSERT INTO app_user (id, display_name, created_at) VALUES ($1, 'Anna Muster', now())",
        )
        .bind(user_id)
        .execute(&test.database.pool)
        .await
        .unwrap();
        for address in ["Anna@Example.org", " anna@example.org", "anna@example.org "] {
            let rejected = sqlx::query(
                "INSERT INTO email_identity (user_id, email, created_at) VALUES ($1, $2, now())",
            )
            .bind(user_id)
            .bind(address)
            .execute(&test.database.pool)
            .await;
            assert_eq!(sqlstate(&rejected.unwrap_err()), "23514", "{address:?}");
        }
    }
}
