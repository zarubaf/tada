//! The `PrivacyStore` adapter: the privacy notice of an organization (ADR 0045).

use async_trait::async_trait;
use tada_app::audit::AuditEvent;
use tada_app::caller::OrgScope;
use tada_app::domain::RecordVersion;
use tada_app::privacy::{PrivacyNotice, PrivacyStore};
use tada_app::store::StoreError;

use crate::Database;
use crate::audit;
use crate::error::{InvalidRow, store_error};

#[async_trait]
impl PrivacyStore for Database {
    async fn get(&self, scope: OrgScope) -> Result<PrivacyNotice, StoreError> {
        let row = sqlx::query!(
            "SELECT privacy_notice, privacy_notice_version FROM organization WHERE id = $1",
            scope.organization_id().as_uuid(),
        )
        .fetch_one(&self.pool)
        .await
        .map_err(store_error)?;
        Ok(PrivacyNotice {
            markdown: row.privacy_notice,
            version: RecordVersion::new(row.privacy_notice_version)
                .ok_or(InvalidRow("organization.privacy_notice_version"))?,
        })
    }

    async fn set(
        &self,
        scope: OrgScope,
        markdown: Option<&str>,
        expected_version: RecordVersion,
        audit: &AuditEvent,
    ) -> Result<Option<PrivacyNotice>, StoreError> {
        let mut tx = self.pool.begin().await.map_err(store_error)?;
        let version = sqlx::query_scalar!(
            "UPDATE organization
             SET privacy_notice = $2, privacy_notice_version = privacy_notice_version + 1
             WHERE id = $1 AND privacy_notice_version = $3
             RETURNING privacy_notice_version",
            scope.organization_id().as_uuid(),
            markdown,
            expected_version.get(),
        )
        .fetch_optional(&mut *tx)
        .await
        .map_err(store_error)?;
        let Some(version) = version else {
            return Ok(None);
        };
        audit::record(&mut tx, audit).await.map_err(store_error)?;
        tx.commit().await.map_err(store_error)?;
        Ok(Some(PrivacyNotice {
            markdown: markdown.map(str::to_owned),
            version: RecordVersion::new(version)
                .ok_or(InvalidRow("organization.privacy_notice_version"))?,
        }))
    }
}
