//! The `MilestoneStore` adapter (ADR 0080). A read calculates the warnings of each milestone in its query:
//! the late actions of the milestone, and "today" in the time zone of its event.

use async_trait::async_trait;
use jiff::Timestamp;
use jiff_sqlx::ToSqlx;
use sqlx::PgConnection;
use sqlx::types::Uuid;
use tada_app::audit::AuditEvent;
use tada_app::caller::OrgScope;
use tada_app::domain::RecordVersion;
use tada_app::domain::events::EventTimeZone;
use tada_app::domain::ids::{ActionId, EventId, LocalIdKind, MilestoneId, UserId, WorkstreamId};
use tada_app::domain::records::{MilestoneStatus, MilestoneTitle};
use tada_app::domain::redaction::Redactable;
use tada_app::domain::work::RecordDescription;
use tada_app::records::{Changed, Created};
use tada_app::store::StoreError;
use tada_app::work::{
    MilestoneFields, MilestoneStore, MilestoneView, MilestoneWarning, NewMilestoneRecord,
    WorkFilter,
};

use super::Select;
use crate::Database;
use crate::audit;
use crate::error::{InvalidRow, store_error, violates};
use crate::local_ids::next_local_number;

struct MilestoneRow {
    id: Uuid,
    event_id: Uuid,
    local_number: i64,
    /// NULL only after a legal redaction (ADR 0072).
    title: Option<String>,
    description: Option<String>,
    date: jiff_sqlx::Date,
    owner_user_id: Uuid,
    workstream_id: Option<Uuid>,
    status: String,
    version: i64,
    /// The time zone of the event, for "today".
    time_zone: String,
    /// The open, in-progress or blocked actions of the milestone that are due after its date.
    late_actions: Vec<Uuid>,
}

impl MilestoneRow {
    fn into_view(self, now: Timestamp) -> Result<MilestoneView, InvalidRow> {
        let status = MilestoneStatus::parse(&self.status).ok_or(InvalidRow("milestone.status"))?;
        let date = self.date.to_jiff();
        let today = EventTimeZone::parse(&self.time_zone)
            .map_err(|_| InvalidRow("event.time_zone"))?
            .today(now);
        let overdue = status
            .is_overdue(date, today)
            .then_some(MilestoneWarning::Overdue);
        let late = self
            .late_actions
            .into_iter()
            .map(|id| MilestoneWarning::LateAction(ActionId::from_uuid(id)));
        Ok(MilestoneView {
            id: MilestoneId::from_uuid(self.id),
            local_number: u64::try_from(self.local_number)
                .map_err(|_| InvalidRow("milestone.local_number"))?,
            event_id: EventId::from_uuid(self.event_id),
            fields: MilestoneFields {
                title: match self.title {
                    Some(title) => Redactable::Present(
                        MilestoneTitle::parse(&title).map_err(|_| InvalidRow("milestone.title"))?,
                    ),
                    None => Redactable::Redacted,
                },
                description: self
                    .description
                    .map(|text| RecordDescription::parse(&text))
                    .transpose()
                    .map_err(|_| InvalidRow("milestone.description"))?,
                date,
                owner: UserId::from_uuid(self.owner_user_id),
                workstream_id: self.workstream_id.map(WorkstreamId::from_uuid),
                status,
            },
            version: RecordVersion::new(self.version).ok_or(InvalidRow("milestone.version"))?,
            warnings: overdue.into_iter().chain(late).collect(),
        })
    }
}

/// The milestones of `select` with their warnings at `now`.
async fn select_milestones(
    conn: &mut PgConnection,
    scope: OrgScope,
    select: &Select,
    now: Timestamp,
) -> Result<Vec<MilestoneView>, sqlx::Error> {
    let rows = sqlx::query_as!(
        MilestoneRow,
        r#"SELECT m.id, m.event_id, m.local_number, m.title, m.description,
                  m.date AS "date: jiff_sqlx::Date", m.owner_user_id, m.workstream_id, m.status, m.version,
                  e.time_zone,
                  ARRAY(SELECT a.id FROM action a
                        WHERE a.organization_id = m.organization_id AND a.event_id = m.event_id
                          AND a.milestone_id = m.id AND a.status IN ('open', 'in-progress', 'blocked')
                          AND a.due_date > m.date
                        ORDER BY a.local_number) AS "late_actions!"
           FROM milestone m
           JOIN event e ON e.organization_id = m.organization_id AND e.id = m.event_id
           WHERE m.organization_id = $1
             AND ($2::uuid IS NULL OR m.event_id = $2)
             AND ($3::uuid IS NULL OR m.id = $3)
             AND ($4::uuid IS NULL OR m.owner_user_id = $4)
             AND ($5::text IS NULL OR m.status = $5)
             AND ($6::uuid IS NULL OR m.workstream_id = $6)
             AND m.local_number > $7
           ORDER BY m.event_id, m.local_number
           LIMIT $8"#,
        scope.organization_id().as_uuid(),
        select.event,
        select.id,
        select.owner,
        select.status,
        select.workstream,
        select.after,
        select.limit,
    )
    .fetch_all(&mut *conn)
    .await?;
    Ok(rows
        .into_iter()
        .map(|row| row.into_view(now))
        .collect::<Result<_, _>>()?)
}

/// The milestone `id` of the event with its warnings at `now`, read inside the transaction of the caller.
pub(crate) async fn milestone_in(
    conn: &mut PgConnection,
    scope: OrgScope,
    event: EventId,
    id: MilestoneId,
    now: Timestamp,
) -> Result<Option<MilestoneView>, sqlx::Error> {
    Ok(
        select_milestones(conn, scope, &Select::one(event, id.as_uuid()), now)
            .await?
            .pop(),
    )
}

/// Inserts a milestone with the version 1 and the next number of its event, and returns the number.
/// The direct command and the apply of a proposal both write a new milestone here.
pub(crate) async fn insert_milestone(
    conn: &mut PgConnection,
    scope: OrgScope,
    milestone: &NewMilestoneRecord,
    at: Timestamp,
) -> Result<i64, sqlx::Error> {
    let number = next_local_number(
        conn,
        scope,
        milestone.event_id.as_uuid(),
        LocalIdKind::Milestone.prefix(),
    )
    .await?;
    let fields = &milestone.fields;
    sqlx::query!(
        "INSERT INTO milestone
             (id, organization_id, event_id, local_number, title, description, date, owner_user_id,
              workstream_id, status, version, created_at, updated_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, 1, $11, $11)",
        milestone.id.as_uuid(),
        scope.organization_id().as_uuid(),
        milestone.event_id.as_uuid(),
        number,
        fields.title.as_present().map(MilestoneTitle::as_str),
        fields.description.as_ref().map(RecordDescription::as_str),
        fields.date.to_sqlx() as _,
        fields.owner.as_uuid(),
        fields.workstream_id.map(WorkstreamId::as_uuid),
        fields.status.as_str(),
        at.to_sqlx() as _,
    )
    .execute(&mut *conn)
    .await?;
    Ok(number)
}

/// Replaces the values of a milestone if its version is `expected`, and returns the new version.
/// `None` means that the event has no such milestone with this version (`update_action`).
pub(crate) async fn update_milestone(
    conn: &mut PgConnection,
    scope: OrgScope,
    event: EventId,
    id: MilestoneId,
    fields: &MilestoneFields,
    expected: RecordVersion,
    at: Timestamp,
) -> Result<Option<i64>, sqlx::Error> {
    sqlx::query_scalar!(
        "UPDATE milestone
         SET title = $5, description = $6, date = $7, owner_user_id = $8, workstream_id = $9,
             status = $10, version = version + 1, updated_at = $11
         WHERE organization_id = $1 AND event_id = $2 AND id = $3 AND version = $4
         RETURNING version",
        scope.organization_id().as_uuid(),
        event.as_uuid(),
        id.as_uuid(),
        expected.get(),
        fields.title.as_present().map(MilestoneTitle::as_str),
        fields.description.as_ref().map(RecordDescription::as_str),
        fields.date.to_sqlx() as _,
        fields.owner.as_uuid(),
        fields.workstream_id.map(WorkstreamId::as_uuid),
        fields.status.as_str(),
        at.to_sqlx() as _,
    )
    .fetch_optional(&mut *conn)
    .await
}

#[async_trait]
impl MilestoneStore for Database {
    async fn create_milestone(
        &self,
        scope: OrgScope,
        milestone: &NewMilestoneRecord,
        at: Timestamp,
        audit: &AuditEvent,
    ) -> Result<Created<MilestoneView>, StoreError> {
        let mut tx = self.pool.begin().await.map_err(store_error)?;
        match insert_milestone(&mut tx, scope, milestone, at).await {
            Ok(_) => {}
            Err(error) if violates(&error, "milestone_pkey") => return Ok(Created::IdTaken),
            Err(error) => return Err(store_error(error)),
        }
        audit::record(&mut tx, audit).await.map_err(store_error)?;
        let view = milestone_in(&mut tx, scope, milestone.event_id, milestone.id, at)
            .await
            .map_err(store_error)?
            .ok_or(InvalidRow("milestone"))?;
        tx.commit().await.map_err(store_error)?;
        Ok(Created::Created(view))
    }

    async fn change_milestone(
        &self,
        scope: OrgScope,
        event: EventId,
        id: MilestoneId,
        fields: &MilestoneFields,
        expected: RecordVersion,
        at: Timestamp,
        audit: &AuditEvent,
    ) -> Result<Changed<MilestoneView>, StoreError> {
        let mut tx = self.pool.begin().await.map_err(store_error)?;
        let updated = update_milestone(&mut tx, scope, event, id, fields, expected, at)
            .await
            .map_err(store_error)?;
        let current = milestone_in(&mut tx, scope, event, id, at)
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

    async fn milestone(
        &self,
        scope: OrgScope,
        event: EventId,
        id: MilestoneId,
        now: Timestamp,
    ) -> Result<Option<MilestoneView>, StoreError> {
        let mut conn = self.pool.acquire().await.map_err(store_error)?;
        milestone_in(&mut conn, scope, event, id, now)
            .await
            .map_err(store_error)
    }

    async fn milestones(
        &self,
        scope: OrgScope,
        event: EventId,
        filter: &WorkFilter<MilestoneStatus>,
        now: Timestamp,
    ) -> Result<Vec<MilestoneView>, StoreError> {
        let mut conn = self.pool.acquire().await.map_err(store_error)?;
        let status = filter.status.map(MilestoneStatus::as_str);
        select_milestones(
            &mut conn,
            scope,
            &Select::filter(event, filter, status),
            now,
        )
        .await
        .map_err(store_error)
    }
}
