//! The development authenticator (ADR 0053). Only debug builds contain this module.

use async_trait::async_trait;
use sqlx::types::Uuid;
use tada_app::auth::{AuthenticationError, Authenticator};
use tada_app::caller::{MemberCaller, OrganizationRole};
use tada_app::domain::ids::{OrganizationId, UserId};
use tada_app::store::StoreError;

use crate::Database;
use crate::error::store_error;

/// The development organization, slug `dev`.
pub const DEV_ORGANIZATION_ID: OrganizationId =
    OrganizationId::from_uuid(Uuid::from_u128(0x0199_b8e0_0000_7000_8000_0000_0000_0001));
/// The development member, an owner of the development organization. It has no database row.
pub const DEV_USER_ID: UserId =
    UserId::from_uuid(Uuid::from_u128(0x0199_b8e0_0000_7000_8000_0000_0000_0002));

/// Gives each request the development member.
#[derive(Debug, Clone, Copy)]
pub struct DevAuthenticator;

#[async_trait]
impl Authenticator for DevAuthenticator {
    async fn authenticate(
        &self,
        _session_token: Option<&str>,
    ) -> Result<MemberCaller, AuthenticationError> {
        Ok(MemberCaller::new(
            DEV_USER_ID,
            DEV_ORGANIZATION_ID,
            OrganizationRole::Owner,
        ))
    }
}

impl Database {
    /// Creates the development organization if it does not exist.
    pub async fn ensure_dev_organization(&self) -> Result<(), StoreError> {
        sqlx::query!(
            "INSERT INTO organization (id, slug, name, created_at)
             VALUES ($1, 'dev', 'Development', now())
             ON CONFLICT DO NOTHING",
            DEV_ORGANIZATION_ID.as_uuid(),
        )
        .execute(&self.pool)
        .await
        .map(|_| ())
        .map_err(store_error)
    }
}
