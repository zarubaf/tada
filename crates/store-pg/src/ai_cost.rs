//! The `AiCostStore` adapter: the price table, the usage and the monthly cap (ADRs 0047 and 0077).
//!
//! The tables keep amounts as `numeric` francs. The queries convert them to and from whole millirappen,
//! so no float touches an amount.

use async_trait::async_trait;
use jiff::civil::Date;
use jiff_sqlx::ToSqlx;
use sqlx::types::Uuid;
use tada_app::ai_cost::{AiCostStore, Chf, Month, Price, UsageRecord};
use tada_app::audit::AuditEvent;
use tada_app::caller::OrgScope;
use tada_app::domain::RecordVersion;
use tada_app::domain::ids::EventId;
use tada_app::store::StoreError;

use crate::Database;
use crate::audit;
use crate::error::{InvalidRow, store_error};

fn version(value: i64, column: &'static str) -> Result<RecordVersion, InvalidRow> {
    RecordVersion::new(value).ok_or(InvalidRow(column))
}

#[async_trait]
impl AiCostStore for Database {
    async fn price(
        &self,
        _: OrgScope,
        model_id: &str,
        on: Date,
    ) -> Result<Option<Price>, StoreError> {
        // A part of a millirappen rounds up, so that the cost is never too low.
        let row = sqlx::query!(
            r#"SELECT ceil(input_chf_per_mtok * 100000)::bigint AS "input!",
                      ceil(output_chf_per_mtok * 100000)::bigint AS "output!"
               FROM ai_price WHERE model_id = $1 AND valid_from <= $2
               ORDER BY valid_from DESC LIMIT 1"#,
            model_id,
            on.to_sqlx() as _,
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(store_error)?;
        Ok(row.map(|row| Price {
            input_per_mtok: Chf::from_millirappen(row.input),
            output_per_mtok: Chf::from_millirappen(row.output),
        }))
    }

    async fn month_usage(&self, scope: OrgScope, month: Month) -> Result<Chf, StoreError> {
        let used = sqlx::query_scalar!(
            r#"SELECT ceil(coalesce(sum(cost_chf), 0) * 100000)::bigint AS "used!" FROM ai_usage
               WHERE organization_id = $1 AND at >= $2 AND at < $3"#,
            scope.organization_id().as_uuid(),
            month.start.to_sqlx() as _,
            month.end.to_sqlx() as _,
        )
        .fetch_one(&self.pool)
        .await
        .map_err(store_error)?;
        Ok(Chf::from_millirappen(used))
    }

    async fn record(&self, scope: OrgScope, usage: &UsageRecord) -> Result<(), StoreError> {
        let tokens = |count: u32| {
            i32::try_from(count)
                .map_err(|_| StoreError::Internal("a token count exceeds the column".into()))
        };
        sqlx::query!(
            "INSERT INTO ai_usage (id, organization_id, event_id, task, prompt_version, model_id,
                 input_tokens, output_tokens, cost_chf, at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, round($9::bigint::numeric / 100000, 5), $10)",
            Uuid::now_v7(),
            scope.organization_id().as_uuid(),
            usage.event_id.map(EventId::as_uuid),
            usage.task,
            i32::from(usage.prompt_version),
            usage.model_id,
            tokens(usage.usage.input_tokens)?,
            tokens(usage.usage.output_tokens)?,
            usage.cost.millirappen(),
            usage.at.to_sqlx() as _,
        )
        .execute(&self.pool)
        .await
        .map_err(store_error)?;
        Ok(())
    }

    async fn cap(&self, scope: OrgScope) -> Result<Option<(Chf, RecordVersion)>, StoreError> {
        let row = sqlx::query!(
            r#"SELECT floor(monthly_chf * 100000)::bigint AS "monthly!", version
               FROM ai_cap WHERE organization_id = $1"#,
            scope.organization_id().as_uuid(),
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(store_error)?;
        row.map(|row| {
            Ok((
                Chf::from_millirappen(row.monthly),
                version(row.version, "ai_cap.version")?,
            ))
        })
        .transpose()
    }

    async fn set_cap(
        &self,
        scope: OrgScope,
        monthly: Chf,
        expected: RecordVersion,
        event: &AuditEvent,
    ) -> Result<Option<RecordVersion>, StoreError> {
        let organization = scope.organization_id().as_uuid();
        let mut tx = self.pool.begin().await.map_err(store_error)?;
        let mut changed = sqlx::query_scalar!(
            "UPDATE ai_cap SET monthly_chf = round($2::bigint::numeric / 100000, 5), version = version + 1
             WHERE organization_id = $1 AND version = $3
             RETURNING version",
            organization,
            monthly.millirappen(),
            expected.get(),
        )
        .fetch_optional(&mut *tx)
        .await
        .map_err(store_error)?;
        // An organization without a row has the default with version 1. A concurrent first set wins the insert,
        // so this one conflicts.
        if changed.is_none() && expected == RecordVersion::FIRST {
            changed = sqlx::query_scalar!(
                "INSERT INTO ai_cap (organization_id, monthly_chf, version)
                 VALUES ($1, round($2::bigint::numeric / 100000, 5), 2)
                 ON CONFLICT (organization_id) DO NOTHING
                 RETURNING version",
                organization,
                monthly.millirappen(),
            )
            .fetch_optional(&mut *tx)
            .await
            .map_err(store_error)?;
        }
        let Some(changed) = changed else {
            return Ok(None);
        };
        audit::record(&mut tx, event).await.map_err(store_error)?;
        tx.commit().await.map_err(store_error)?;
        Ok(Some(version(changed, "ai_cap.version")?))
    }

    async fn ai_enabled(&self, scope: OrgScope) -> Result<bool, StoreError> {
        let enabled = sqlx::query_scalar!(
            "SELECT ai_enabled FROM pm_policy WHERE organization_id = $1",
            scope.organization_id().as_uuid(),
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(store_error)?;
        Ok(enabled.unwrap_or(false))
    }
}
