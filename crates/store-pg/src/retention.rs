//! The `RetentionStore` adapter: the retention periods of an organization and the targets that are due (ADR 0071).
//!
//! "Older than the period" counts calendar months in SQL, so a period of one month ends on the same day
//! of the next month, whatever the length of the months.

use async_trait::async_trait;
use jiff::Timestamp;
use jiff_sqlx::ToSqlx;
use sqlx::types::Uuid;
use tada_app::audit::AuditEvent;
use tada_app::caller::{JobRunner, OrgScope, ServiceCaller};
use tada_app::domain::RecordVersion;
use tada_app::domain::ids::OrganizationId;
use tada_app::domain::redaction::{RedactionTarget, RedactionTargetKind};
use tada_app::retention::{Months, Retention, RetentionPeriods, RetentionStore};
use tada_app::store::StoreError;

use crate::Database;
use crate::audit;
use crate::error::{InvalidRow, store_error};
use crate::redaction::cited_by_active_event;

/// A closed event whose period ended. Its source items follow the event, so they are not due on their own.
const DUE_EVENTS: &str = "SELECT e.organization_id, e.id
     FROM event e JOIN organization o ON o.id = e.organization_id
     WHERE e.status = 'closed' AND e.redacted_at IS NULL
       AND o.retention_closed_event_months IS NOT NULL
       AND e.closed_at + make_interval(months => o.retention_closed_event_months::int) <= $1
     ORDER BY e.closed_at, e.id";

/// A source item of no event whose period ended. A source of an active event is never due: neither one that
/// belongs to it nor one that its evidence cites. A cited source becomes due when the event closes.
const DUE_SOURCES: &str = concat!(
    "SELECT s.organization_id, s.id
     FROM source_item s JOIN organization o ON o.id = s.organization_id
     WHERE s.event_id IS NULL AND s.redacted_at IS NULL
       AND o.retention_source_months IS NOT NULL
       AND s.created_at + make_interval(months => o.retention_source_months::int) <= $1
       AND NOT ",
    cited_by_active_event!(),
    "
     ORDER BY s.created_at, s.id"
);

fn months(value: Option<i16>, column: &'static str) -> Result<Option<Months>, InvalidRow> {
    value
        .map(|value| Months::new(i64::from(value)).ok_or(InvalidRow(column)))
        .transpose()
}

fn retention(row: (Option<i16>, Option<i16>, i64)) -> Result<Retention, InvalidRow> {
    Ok(Retention {
        periods: RetentionPeriods {
            closed_event: months(row.0, "organization.retention_closed_event_months")?,
            source: months(row.1, "organization.retention_source_months")?,
        },
        version: RecordVersion::new(row.2).ok_or(InvalidRow("organization.retention_version"))?,
    })
}

#[async_trait]
impl RetentionStore for Database {
    async fn get(&self, scope: OrgScope) -> Result<Retention, StoreError> {
        let row: (Option<i16>, Option<i16>, i64) = sqlx::query_as(
            "SELECT retention_closed_event_months, retention_source_months, retention_version
             FROM organization WHERE id = $1",
        )
        .bind(scope.organization_id().as_uuid())
        .fetch_one(&self.pool)
        .await
        .map_err(store_error)?;
        Ok(retention(row)?)
    }

    async fn set(
        &self,
        scope: OrgScope,
        periods: RetentionPeriods,
        expected_version: RecordVersion,
        event: &AuditEvent,
    ) -> Result<Option<Retention>, StoreError> {
        let mut tx = self.pool.begin().await.map_err(store_error)?;
        let row: Option<(Option<i16>, Option<i16>, i64)> = sqlx::query_as(
            "UPDATE organization
             SET retention_closed_event_months = $2, retention_source_months = $3,
                 retention_version = retention_version + 1
             WHERE id = $1 AND retention_version = $4
             RETURNING retention_closed_event_months, retention_source_months, retention_version",
        )
        .bind(scope.organization_id().as_uuid())
        .bind(periods.closed_event.map(|months| i16::from(months.get())))
        .bind(periods.source.map(|months| i16::from(months.get())))
        .bind(expected_version.get())
        .fetch_optional(&mut *tx)
        .await
        .map_err(store_error)?;
        let Some(row) = row else {
            return Ok(None);
        };
        audit::record(&mut tx, event).await.map_err(store_error)?;
        tx.commit().await.map_err(store_error)?;
        Ok(Some(retention(row)?))
    }

    async fn due_for_retention(
        &self,
        _: &ServiceCaller<JobRunner>,
        now: Timestamp,
    ) -> Result<Vec<(OrganizationId, RedactionTarget)>, StoreError> {
        let mut due = Vec::new();
        for (sql, kind) in [
            (DUE_EVENTS, RedactionTargetKind::Event),
            (DUE_SOURCES, RedactionTargetKind::SourceItem),
        ] {
            let rows: Vec<(Uuid, Uuid)> = sqlx::query_as(sql)
                .bind(now.to_sqlx())
                .fetch_all(&self.pool)
                .await
                .map_err(store_error)?;
            due.extend(rows.into_iter().map(|(organization, id)| {
                (
                    OrganizationId::from_uuid(organization),
                    RedactionTarget { kind, id },
                )
            }));
        }
        Ok(due)
    }
}
