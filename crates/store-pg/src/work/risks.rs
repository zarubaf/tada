//! The `RiskStore` adapter (ADR 0081). A list shows the worst risk first: the score (likelihood times impact,
//! low 1, medium 2, high 3), then the impact, then the number. `RiskRating::sort_key` states the same order in Rust.

use async_trait::async_trait;
use jiff::Timestamp;
use jiff_sqlx::ToSqlx;
use sqlx::PgConnection;
use sqlx::types::Uuid;
use tada_app::audit::AuditEvent;
use tada_app::caller::OrgScope;
use tada_app::domain::RecordVersion;
use tada_app::domain::ids::{EventId, LocalIdKind, RiskId, UserId, WorkstreamId};
use tada_app::domain::proposals::Operation;
use tada_app::domain::records::{Level, RiskRating, RiskStatus, RiskTitle};
use tada_app::domain::redaction::Redactable;
use tada_app::domain::work::RecordDescription;
use tada_app::records::{Changed, Created};
use tada_app::store::StoreError;
use tada_app::work::{NewRiskRecord, RiskCursor, RiskFields, RiskStore, RiskView, WorkFilter};

use crate::Database;
use crate::audit;
use crate::error::{InvalidRow, store_error, violates};
use crate::local_ids::next_local_number;

struct RiskRow {
    id: Uuid,
    event_id: Uuid,
    local_number: i64,
    /// NULL only after a legal redaction (ADR 0072).
    title: Option<String>,
    description: Option<String>,
    likelihood: String,
    impact: String,
    mitigation: Option<String>,
    owner_user_id: Uuid,
    workstream_id: Option<Uuid>,
    status: String,
    version: i64,
}

impl TryFrom<RiskRow> for RiskView {
    type Error = InvalidRow;

    fn try_from(row: RiskRow) -> Result<Self, InvalidRow> {
        let text = |text: Option<String>, what: &'static str| {
            text.map(|text| RecordDescription::parse(&text))
                .transpose()
                .map_err(|_| InvalidRow(what))
        };
        Ok(Self {
            id: RiskId::from_uuid(row.id),
            local_number: u64::try_from(row.local_number)
                .map_err(|_| InvalidRow("risk.local_number"))?,
            event_id: EventId::from_uuid(row.event_id),
            fields: RiskFields {
                title: match row.title {
                    Some(title) => Redactable::Present(
                        RiskTitle::parse(&title).map_err(|_| InvalidRow("risk.title"))?,
                    ),
                    None => Redactable::Redacted,
                },
                description: text(row.description, "risk.description")?,
                rating: RiskRating {
                    likelihood: Level::parse(&row.likelihood)
                        .ok_or(InvalidRow("risk.likelihood"))?,
                    impact: Level::parse(&row.impact).ok_or(InvalidRow("risk.impact"))?,
                },
                mitigation: text(row.mitigation, "risk.mitigation")?,
                owner: UserId::from_uuid(row.owner_user_id),
                workstream_id: row.workstream_id.map(WorkstreamId::from_uuid),
                status: RiskStatus::parse(&row.status).ok_or(InvalidRow("risk.status"))?,
            },
            version: RecordVersion::new(row.version).ok_or(InvalidRow("risk.version"))?,
        })
    }
}

/// The risks of the event that match `filter`, the worst first. The score is the weight of the likelihood times the
/// weight of the impact (low 1, medium 2, high 3); the order is the score, then the weight of the impact (both
/// descending), then the local number (`RiskRating::sort_key` states the same order in Rust). A page starts after
/// the key of `filter.after`, the rating and the number the cursor was issued with.
async fn select_risks(
    conn: &mut PgConnection,
    scope: OrgScope,
    event: EventId,
    filter: &WorkFilter<RiskStatus, RiskCursor>,
) -> Result<Vec<RiskView>, sqlx::Error> {
    let after = filter.after;
    let rows = sqlx::query_as!(
        RiskRow,
        r#"SELECT id AS "id!", event_id AS "event_id!", local_number AS "local_number!", title, description,
                  likelihood AS "likelihood!", impact AS "impact!", mitigation, owner_user_id AS "owner_user_id!",
                  workstream_id, status AS "status!", version AS "version!"
           FROM (
               SELECT r.*,
                      (CASE r.likelihood WHEN 'low' THEN 1 WHEN 'medium' THEN 2 ELSE 3 END)
                        * (CASE r.impact WHEN 'low' THEN 1 WHEN 'medium' THEN 2 ELSE 3 END) AS score,
                      (CASE r.impact WHEN 'low' THEN 1 WHEN 'medium' THEN 2 ELSE 3 END) AS impact_weight
               FROM risk r
               WHERE r.organization_id = $1 AND r.event_id = $2
           ) ranked
           WHERE ($3::uuid IS NULL OR owner_user_id = $3)
             AND ($4::text IS NULL OR status = $4)
             AND ($5::uuid IS NULL OR workstream_id = $5)
             AND ($6::int IS NULL OR (-score, -impact_weight, local_number) > (-$6::int, -$7::int, $8::bigint))
           ORDER BY score DESC, impact_weight DESC, local_number
           LIMIT $9"#,
        scope.organization_id().as_uuid(),
        event.as_uuid(),
        filter.owner.map(UserId::as_uuid),
        filter.status.map(RiskStatus::as_str),
        filter.workstream.map(WorkstreamId::as_uuid),
        after.map(|cursor| i32::from(cursor.rating.score())),
        after.map(|cursor| i32::from(cursor.rating.impact.weight())),
        after.map(|cursor| i64::try_from(cursor.number).unwrap_or(i64::MAX)),
        i64::from(filter.limit),
    )
    .fetch_all(&mut *conn)
    .await?;
    Ok(rows
        .into_iter()
        .map(RiskView::try_from)
        .collect::<Result<_, _>>()?)
}

/// The risk `id` of the event, read inside the transaction of the caller.
pub(crate) async fn risk_in(
    conn: &mut PgConnection,
    scope: OrgScope,
    event: EventId,
    id: RiskId,
) -> Result<Option<RiskView>, sqlx::Error> {
    let row = sqlx::query_as!(
        RiskRow,
        r#"SELECT id AS "id!", event_id AS "event_id!", local_number AS "local_number!", title, description,
                  likelihood AS "likelihood!", impact AS "impact!", mitigation, owner_user_id AS "owner_user_id!",
                  workstream_id, status AS "status!", version AS "version!"
           FROM risk
           WHERE organization_id = $1 AND event_id = $2 AND id = $3"#,
        scope.organization_id().as_uuid(),
        event.as_uuid(),
        id.as_uuid(),
    )
    .fetch_optional(&mut *conn)
    .await?;
    Ok(row.map(RiskView::try_from).transpose()?)
}

/// Inserts a risk with the version 1 and the next number of its event, and returns the number.
/// The direct command and the apply of a proposal both write a new risk here.
pub(crate) async fn insert_risk(
    conn: &mut PgConnection,
    scope: OrgScope,
    risk: &NewRiskRecord,
    at: Timestamp,
) -> Result<i64, sqlx::Error> {
    let number = next_local_number(
        conn,
        scope,
        risk.event_id.as_uuid(),
        LocalIdKind::Risk.prefix(),
    )
    .await?;
    let fields = &risk.fields;
    sqlx::query!(
        "INSERT INTO risk
             (id, organization_id, event_id, local_number, title, description, likelihood, impact, mitigation,
              owner_user_id, workstream_id, status, version, created_at, updated_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, 1, $13, $13)",
        risk.id.as_uuid(),
        scope.organization_id().as_uuid(),
        risk.event_id.as_uuid(),
        number,
        fields.title.as_present().map(RiskTitle::as_str),
        fields.description.as_ref().map(RecordDescription::as_str),
        fields.rating.likelihood.as_str(),
        fields.rating.impact.as_str(),
        fields.mitigation.as_ref().map(RecordDescription::as_str),
        fields.owner.as_uuid(),
        fields.workstream_id.map(WorkstreamId::as_uuid),
        fields.status.as_str(),
        at.to_sqlx() as _,
    )
    .execute(&mut *conn)
    .await?;
    Ok(number)
}

/// Replaces the values of a risk if its version is `expected`, and returns the new version.
/// `None` means that the event has no such risk with this version (`update_action`).
pub(crate) async fn update_risk(
    conn: &mut PgConnection,
    scope: OrgScope,
    event: EventId,
    id: RiskId,
    fields: &RiskFields,
    expected: RecordVersion,
    at: Timestamp,
) -> Result<Option<i64>, sqlx::Error> {
    sqlx::query_scalar!(
        "UPDATE risk
         SET title = $5, description = $6, likelihood = $7, impact = $8, mitigation = $9,
             owner_user_id = $10, workstream_id = $11, status = $12, version = version + 1, updated_at = $13
         WHERE organization_id = $1 AND event_id = $2 AND id = $3 AND version = $4
         RETURNING version",
        scope.organization_id().as_uuid(),
        event.as_uuid(),
        id.as_uuid(),
        expected.get(),
        fields.title.as_present().map(RiskTitle::as_str),
        fields.description.as_ref().map(RecordDescription::as_str),
        fields.rating.likelihood.as_str(),
        fields.rating.impact.as_str(),
        fields.mitigation.as_ref().map(RecordDescription::as_str),
        fields.owner.as_uuid(),
        fields.workstream_id.map(WorkstreamId::as_uuid),
        fields.status.as_str(),
        at.to_sqlx() as _,
    )
    .fetch_optional(&mut *conn)
    .await
}

/// The apply of `CreateRisk`: the risk starts open with the version 1. Returns its number.
/// `risk` is the ID and the event, `assignee` the owner and the workstream.
#[expect(
    clippy::too_many_arguments,
    reason = "the values of the operation, one by one, so that the store builds the record and no caller does"
)]
pub(crate) async fn create_risk_in_plan(
    conn: &mut PgConnection,
    scope: OrgScope,
    risk: (RiskId, EventId),
    title: &RiskTitle,
    description: Option<&RecordDescription>,
    rating: RiskRating,
    mitigation: Option<&RecordDescription>,
    assignee: (UserId, Option<WorkstreamId>),
    at: Timestamp,
) -> Result<i64, sqlx::Error> {
    let record = NewRiskRecord {
        id: risk.0,
        event_id: risk.1,
        fields: RiskFields {
            title: Redactable::Present(title.clone()),
            description: description.cloned(),
            rating,
            mitigation: mitigation.cloned(),
            owner: assignee.0,
            workstream_id: assignee.1,
            status: RiskStatus::Open,
        },
    };
    insert_risk(conn, scope, &record, at).await
}

/// What a proposal changes in a risk.
pub(crate) enum RiskEdit {
    Status(RiskStatus),
    Rating(RiskRating),
}

impl RiskEdit {
    /// The event, the risk, the change and the expected version of a change operation of a risk.
    /// `None` for each other operation.
    pub(crate) fn of(operation: &Operation) -> Option<(EventId, RiskId, Self, RecordVersion)> {
        match *operation {
            Operation::ChangeRiskStatus {
                event_id,
                risk_id,
                status,
                expected_version,
            } => Some((event_id, risk_id, Self::Status(status), expected_version)),
            Operation::ChangeRiskRating {
                event_id,
                risk_id,
                rating,
                expected_version,
            } => Some((event_id, risk_id, Self::Rating(rating), expected_version)),
            _ => None,
        }
    }
}

/// The apply of a change of a risk: the status along its transitions, or the rating. The risk keeps its other values.
/// Returns the new version. A missing risk, a refused transition and another version are `RowNotFound`: the apply
/// turns each into a conflict, as for an action.
pub(crate) async fn change_risk_in_plan(
    conn: &mut PgConnection,
    scope: OrgScope,
    event: EventId,
    id: RiskId,
    edit: RiskEdit,
    expected: RecordVersion,
    at: Timestamp,
) -> Result<i64, sqlx::Error> {
    let current = risk_in(conn, scope, event, id)
        .await?
        .ok_or(sqlx::Error::RowNotFound)?;
    let fields = match edit {
        RiskEdit::Status(status) => RiskFields {
            status: current
                .fields
                .status
                .change_to(status)
                .map_err(|_| sqlx::Error::RowNotFound)?,
            ..current.fields
        },
        RiskEdit::Rating(rating) => RiskFields {
            rating,
            ..current.fields
        },
    };
    update_risk(conn, scope, event, id, &fields, expected, at)
        .await?
        .ok_or(sqlx::Error::RowNotFound)
}

#[async_trait]
impl RiskStore for Database {
    async fn create_risk(
        &self,
        scope: OrgScope,
        risk: &NewRiskRecord,
        at: Timestamp,
        audit: &AuditEvent,
    ) -> Result<Created<RiskView>, StoreError> {
        let mut tx = self.pool.begin().await.map_err(store_error)?;
        match insert_risk(&mut tx, scope, risk, at).await {
            Ok(_) => {}
            Err(error) if violates(&error, "risk_pkey") => return Ok(Created::IdTaken),
            Err(error) => return Err(store_error(error)),
        }
        audit::record(&mut tx, audit).await.map_err(store_error)?;
        let view = risk_in(&mut tx, scope, risk.event_id, risk.id)
            .await
            .map_err(store_error)?
            .ok_or(InvalidRow("risk"))?;
        tx.commit().await.map_err(store_error)?;
        Ok(Created::Created(view))
    }

    async fn change_risk(
        &self,
        scope: OrgScope,
        event: EventId,
        id: RiskId,
        fields: &RiskFields,
        expected: RecordVersion,
        at: Timestamp,
        audit: &AuditEvent,
    ) -> Result<Changed<RiskView>, StoreError> {
        let mut tx = self.pool.begin().await.map_err(store_error)?;
        let updated = update_risk(&mut tx, scope, event, id, fields, expected, at)
            .await
            .map_err(store_error)?;
        let current = risk_in(&mut tx, scope, event, id)
            .await
            .map_err(store_error)?;
        let Some(view) = current else {
            return Ok(Changed::NotFound);
        };
        if updated.is_none() {
            return Ok(Changed::VersionConflict);
        }
        audit::record(&mut tx, audit).await.map_err(store_error)?;
        tx.commit().await.map_err(store_error)?;
        Ok(Changed::Changed(view))
    }

    async fn risk(
        &self,
        scope: OrgScope,
        event: EventId,
        id: RiskId,
    ) -> Result<Option<RiskView>, StoreError> {
        let mut conn = self.pool.acquire().await.map_err(store_error)?;
        risk_in(&mut conn, scope, event, id)
            .await
            .map_err(store_error)
    }

    async fn risks(
        &self,
        scope: OrgScope,
        event: EventId,
        filter: &WorkFilter<RiskStatus, RiskCursor>,
    ) -> Result<Vec<RiskView>, StoreError> {
        let mut conn = self.pool.acquire().await.map_err(store_error)?;
        select_risks(&mut conn, scope, event, filter)
            .await
            .map_err(store_error)
    }
}
