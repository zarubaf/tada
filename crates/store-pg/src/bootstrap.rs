//! The `BootstrapStore` adapter: the first owner of an organization (ADR 0036).

use async_trait::async_trait;
use jiff_sqlx::ToSqlx;
use sqlx::types::Uuid;
use tada_app::audit::AuditEvent;
use tada_app::bootstrap::{BootstrapOutcome, BootstrapStore, OwnerInvitation};
use tada_app::caller::{Bootstrap, ServiceCaller};
use tada_app::domain::identity::OrganizationRole;
use tada_app::domain::ids::{InvitationId, OrganizationId};
use tada_app::outbound::Purpose;
use tada_app::store::StoreError;

use crate::Database;
use crate::audit::record;
use crate::error::store_error;
use crate::outbound::queue_outbound;

#[async_trait]
impl BootstrapStore for Database {
    async fn invite_first_owner(
        &self,
        caller: &ServiceCaller<Bootstrap>,
        invitation: &OwnerInvitation,
    ) -> Result<BootstrapOutcome, StoreError> {
        let mut tx = self.pool.begin().await.map_err(store_error)?;
        let now = invitation.now.to_sqlx();

        let created = sqlx::query_scalar!(
            "INSERT INTO organization (id, slug, name, created_at) VALUES ($1, $2, $3, $4)
             ON CONFLICT (slug) DO NOTHING
             RETURNING id",
            Uuid::now_v7(),
            invitation.slug.as_str(),
            invitation.name.as_str(),
            now as _,
        )
        .fetch_optional(&mut *tx)
        .await
        .map_err(store_error)?;
        // An infrastructure query by slug (ADR 0039). The lock serializes two runs for one slug.
        let organization_id = sqlx::query_scalar!(
            "SELECT id FROM organization WHERE slug = $1 FOR UPDATE",
            invitation.slug.as_str(),
        )
        .fetch_one(&mut *tx)
        .await
        .map_err(store_error)?;
        let organization_id = OrganizationId::from_uuid(organization_id);
        let scope = caller.scope(organization_id);

        let has_owner = sqlx::query_scalar!(
            r#"SELECT EXISTS (
                   SELECT 1 FROM organization_membership WHERE organization_id = $1 AND role = $2
               ) AS "exists!""#,
            organization_id.as_uuid(),
            OrganizationRole::Owner.as_str(),
        )
        .fetch_one(&mut *tx)
        .await
        .map_err(store_error)?;
        if has_owner {
            return Ok(BootstrapOutcome::OwnerExists);
        }

        sqlx::query!(
            "UPDATE invitation SET status = 'revoked', revoked_at = $3
             WHERE organization_id = $1 AND role = $2 AND status = 'pending'",
            organization_id.as_uuid(),
            OrganizationRole::Owner.as_str(),
            now as _,
        )
        .execute(&mut *tx)
        .await
        .map_err(store_error)?;
        let invitation_id = InvitationId::from_uuid(Uuid::now_v7());
        sqlx::query!(
            "INSERT INTO invitation (id, organization_id, email, display_name, role, created_at)
             VALUES ($1, $2, $3, $4, $5, $6)",
            invitation_id.as_uuid(),
            organization_id.as_uuid(),
            invitation.email.as_str(),
            invitation.display_name.as_str(),
            OrganizationRole::Owner.as_str(),
            now as _,
        )
        .execute(&mut *tx)
        .await
        .map_err(store_error)?;
        queue_outbound(
            &mut tx,
            &Purpose::Invitation {
                organization_id,
                invitation_id,
            },
            None,
        )
        .await
        .map_err(store_error)?;

        let actor = caller.actor();
        if let Some(id) = created {
            let event = AuditEvent::new(
                actor,
                "organization.create",
                "organization",
                Some(id),
                Some(scope),
            );
            record(&mut tx, &event).await.map_err(store_error)?;
        }
        let event = AuditEvent::new(
            actor,
            "invitation.create",
            "invitation",
            Some(invitation_id.as_uuid()),
            Some(scope),
        );
        record(&mut tx, &event).await.map_err(store_error)?;

        tx.commit().await.map_err(store_error)?;
        Ok(BootstrapOutcome::InvitationQueued {
            invitation_id,
            organization_id,
        })
    }
}
