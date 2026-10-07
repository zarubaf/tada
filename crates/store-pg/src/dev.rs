//! The development authenticator (ADR 0053). Only debug builds contain this module.

use async_trait::async_trait;
use sqlx::types::Uuid;
use tada_app::auth::{AuthenticationError, Authenticator, Credential};
use tada_app::caller::{MemberCaller, OrganizationRole};
use tada_app::domain::ids::{OrganizationId, UserId};
use tada_app::store::StoreError;

use crate::Database;
use crate::error::store_error;

/// The development organization, slug `dev`.
pub const DEV_ORGANIZATION_ID: OrganizationId =
    OrganizationId::from_uuid(Uuid::from_u128(0x0199_b8e0_0000_7000_8000_0000_0000_0001));
/// The development member, an owner of the development organization.
pub const DEV_USER_ID: UserId =
    UserId::from_uuid(Uuid::from_u128(0x0199_b8e0_0000_7000_8000_0000_0000_0002));

/// Gives each request the development member.
#[derive(Debug, Clone, Copy)]
pub struct DevAuthenticator;

#[async_trait]
impl Authenticator for DevAuthenticator {
    async fn authenticate(
        &self,
        _credential: Option<Credential<'_>>,
    ) -> Result<MemberCaller, AuthenticationError> {
        Ok(MemberCaller::new(
            DEV_USER_ID,
            DEV_ORGANIZATION_ID,
            OrganizationRole::Owner,
        ))
    }
}

impl Database {
    /// Creates the development organization, the development member and its owner membership
    /// if they do not exist. The Telegram tables refer to the user.
    pub async fn ensure_dev_organization(&self) -> Result<(), StoreError> {
        let mut tx = self.pool.begin().await.map_err(store_error)?;
        sqlx::query!(
            "INSERT INTO organization (id, slug, name, created_at)
             VALUES ($1, 'dev', 'Development', now())
             ON CONFLICT DO NOTHING",
            DEV_ORGANIZATION_ID.as_uuid(),
        )
        .execute(&mut *tx)
        .await
        .map_err(store_error)?;
        sqlx::query!(
            "INSERT INTO app_user (id, display_name, created_at)
             VALUES ($1, 'Development', now())
             ON CONFLICT DO NOTHING",
            DEV_USER_ID.as_uuid(),
        )
        .execute(&mut *tx)
        .await
        .map_err(store_error)?;
        sqlx::query!(
            "INSERT INTO organization_membership (organization_id, user_id, role, created_at)
             VALUES ($1, $2, 'owner', now())
             ON CONFLICT DO NOTHING",
            DEV_ORGANIZATION_ID.as_uuid(),
            DEV_USER_ID.as_uuid(),
        )
        .execute(&mut *tx)
        .await
        .map_err(store_error)?;
        tx.commit().await.map_err(store_error)
    }
}

#[cfg(test)]
mod tests {
    use tada_app::identity::IdentityStore;

    use super::*;
    use crate::testing::TestDatabase;

    #[tokio::test]
    async fn the_development_member_owns_the_development_organization() {
        let test = TestDatabase::start().await;
        test.database.ensure_dev_organization().await.unwrap();
        test.database.ensure_dev_organization().await.unwrap();

        let memberships: Vec<_> = test
            .database
            .memberships_of(DEV_USER_ID)
            .await
            .unwrap()
            .into_iter()
            .map(|membership| (membership.organization_id, membership.role))
            .collect();
        assert_eq!(
            memberships,
            vec![(DEV_ORGANIZATION_ID, OrganizationRole::Owner)]
        );
    }
}
