//! The `WorkStore` adapter: actions and commitments (ADR 0068).

use std::collections::HashMap;

use async_trait::async_trait;
use jiff::Timestamp;
use jiff_sqlx::ToSqlx;
use sqlx::PgConnection;
use sqlx::types::Uuid;
use tada_app::audit::AuditEvent;
use tada_app::caller::OrgScope;
use tada_app::domain::RecordVersion;
use tada_app::domain::events::EventKey;
use tada_app::domain::ids::{
    ActionId, CommitmentId, EventId, InstitutionId, LocalIdKind, PersonId, ProposalId,
    SourceVersionId, UserId, WorkstreamId,
};
use tada_app::domain::parties::{Party, PartyName};
use tada_app::domain::work::{
    ActionDescription, ActionStatus, ActionTitle, CommitmentStatus, CommitmentText, ConditionText,
    FirmReason,
};
use tada_app::parties::PartyRef;
use tada_app::store::StoreError;
use tada_app::work::{
    ActionFields, ActionView, CommitmentFields, CommitmentView, InEvent, MyWork, NewActionRecord,
    NewCommitmentRecord, RecordEvidenceView, WorkChanged, WorkCreated, WorkCursor, WorkFilter,
    WorkStore,
};

use crate::Database;
use crate::audit;
use crate::error::{InvalidRow, store_error};
use crate::local_ids::next_local_number;

struct ActionRow {
    id: Uuid,
    event_id: Uuid,
    local_number: i64,
    title: String,
    description: Option<String>,
    owner_user_id: Uuid,
    workstream_id: Option<Uuid>,
    due_date: Option<jiff_sqlx::Date>,
    status: String,
    version: i64,
}

impl TryFrom<ActionRow> for ActionView {
    type Error = InvalidRow;

    fn try_from(row: ActionRow) -> Result<Self, InvalidRow> {
        Ok(Self {
            id: ActionId::from_uuid(row.id),
            local_number: u64::try_from(row.local_number)
                .map_err(|_| InvalidRow("action.local_number"))?,
            event_id: EventId::from_uuid(row.event_id),
            fields: ActionFields {
                title: ActionTitle::parse(&row.title).map_err(|_| InvalidRow("action.title"))?,
                description: row
                    .description
                    .map(|text| ActionDescription::parse(&text))
                    .transpose()
                    .map_err(|_| InvalidRow("action.description"))?,
                owner: UserId::from_uuid(row.owner_user_id),
                workstream_id: row.workstream_id.map(WorkstreamId::from_uuid),
                due_date: row.due_date.map(jiff_sqlx::Date::to_jiff),
                status: ActionStatus::parse(&row.status).ok_or(InvalidRow("action.status"))?,
            },
            version: RecordVersion::new(row.version).ok_or(InvalidRow("action.version"))?,
        })
    }
}

struct CommitmentRow {
    id: Uuid,
    event_id: Uuid,
    local_number: i64,
    text: String,
    condition: Option<String>,
    person_id: Option<Uuid>,
    institution_id: Option<Uuid>,
    promisor_number: Option<i64>,
    promisor_name: Option<String>,
    owner_user_id: Uuid,
    workstream_id: Option<Uuid>,
    due_date: Option<jiff_sqlx::Date>,
    status: String,
    firm_reason: Option<String>,
    version: i64,
}

impl CommitmentRow {
    fn promisor(&self) -> Result<PartyRef, InvalidRow> {
        let (party, kind) = match (self.person_id, self.institution_id) {
            (Some(id), None) => (Party::Person(PersonId::from_uuid(id)), LocalIdKind::Person),
            (None, Some(id)) => (
                Party::Institution(InstitutionId::from_uuid(id)),
                LocalIdKind::Institution,
            ),
            _ => return Err(InvalidRow("commitment.promisor")),
        };
        let number = self
            .promisor_number
            .and_then(|number| u64::try_from(number).ok())
            .ok_or(InvalidRow("commitment.promisor"))?;
        let name = self
            .promisor_name
            .as_deref()
            .and_then(|name| PartyName::parse(name).ok())
            .ok_or(InvalidRow("commitment.promisor"))?;
        Ok(PartyRef {
            party,
            local_id: kind.readable_id(number),
            name,
        })
    }

    fn into_view(self, evidence: Vec<RecordEvidenceView>) -> Result<CommitmentView, InvalidRow> {
        Ok(CommitmentView {
            id: CommitmentId::from_uuid(self.id),
            local_number: u64::try_from(self.local_number)
                .map_err(|_| InvalidRow("commitment.local_number"))?,
            event_id: EventId::from_uuid(self.event_id),
            condition: self
                .condition
                .as_deref()
                .map(ConditionText::parse)
                .transpose()
                .map_err(|_| InvalidRow("commitment.condition"))?,
            promisor: self.promisor()?,
            fields: CommitmentFields {
                text: CommitmentText::parse(&self.text)
                    .map_err(|_| InvalidRow("commitment.text"))?,
                owner: UserId::from_uuid(self.owner_user_id),
                workstream_id: self.workstream_id.map(WorkstreamId::from_uuid),
                due_date: self.due_date.map(jiff_sqlx::Date::to_jiff),
                status: CommitmentStatus::parse(&self.status)
                    .ok_or(InvalidRow("commitment.status"))?,
                firm_reason: self
                    .firm_reason
                    .as_deref()
                    .map(FirmReason::parse)
                    .transpose()
                    .map_err(|_| InvalidRow("commitment.firm_reason"))?,
            },
            version: RecordVersion::new(self.version).ok_or(InvalidRow("commitment.version"))?,
            evidence,
        })
    }
}

struct EvidenceRow {
    commitment_id: Uuid,
    record_version: i64,
    proposal_id: Uuid,
    source_version_id: Uuid,
    captured_at: jiff_sqlx::Timestamp,
    start_offset: i32,
    end_offset: i32,
    quote: String,
    page: Option<i32>,
}

impl TryFrom<EvidenceRow> for RecordEvidenceView {
    type Error = InvalidRow;

    fn try_from(row: EvidenceRow) -> Result<Self, InvalidRow> {
        let offset = |value: i32| u32::try_from(value).map_err(|_| InvalidRow("record_evidence"));
        Ok(Self {
            record_version: RecordVersion::new(row.record_version)
                .ok_or(InvalidRow("record_evidence.record_version"))?,
            proposal_id: ProposalId::from_uuid(row.proposal_id),
            source_version_id: SourceVersionId::from_uuid(row.source_version_id),
            captured_at: row.captured_at.to_jiff(),
            start_offset: offset(row.start_offset)?,
            end_offset: offset(row.end_offset)?,
            quote: row.quote,
            page: row.page.map(offset).transpose()?,
        })
    }
}

/// The records that one select reads. Each field that is `None` does not filter.
#[derive(Debug, Default)]
struct Select {
    event: Option<Uuid>,
    id: Option<Uuid>,
    owner: Option<Uuid>,
    status: Option<&'static str>,
    workstream: Option<Uuid>,
    after: i64,
    /// Only the records that are still open: for "my work".
    open_only: bool,
    limit: Option<i64>,
}

impl Select {
    fn one(event: EventId, id: Uuid) -> Self {
        Self {
            event: Some(event.as_uuid()),
            id: Some(id),
            ..Self::default()
        }
    }

    fn filter<S>(event: EventId, filter: &WorkFilter<S>, status: Option<&'static str>) -> Self {
        Self {
            event: Some(event.as_uuid()),
            owner: filter.owner.map(UserId::as_uuid),
            status,
            workstream: filter.workstream.map(WorkstreamId::as_uuid),
            after: after_number(filter.after),
            limit: Some(i64::from(filter.limit)),
            ..Self::default()
        }
    }

    fn open_of(user: UserId) -> Self {
        Self {
            owner: Some(user.as_uuid()),
            open_only: true,
            ..Self::default()
        }
    }
}

/// The first number that a page starts after. A cursor beyond the largest number gives an empty page.
fn after_number(after: Option<WorkCursor>) -> i64 {
    after.map_or(0, |cursor| i64::try_from(cursor.0).unwrap_or(i64::MAX))
}

fn violates(error: &sqlx::Error, constraint: &str) -> bool {
    matches!(error, sqlx::Error::Database(error) if error.constraint() == Some(constraint))
}

async fn select_actions(
    conn: &mut PgConnection,
    scope: OrgScope,
    select: &Select,
) -> Result<Vec<ActionView>, StoreError> {
    let rows = sqlx::query_as!(
        ActionRow,
        r#"SELECT id, event_id, local_number, title, description, owner_user_id, workstream_id,
                  due_date AS "due_date: jiff_sqlx::Date", status, version
           FROM action
           WHERE organization_id = $1
             AND ($2::uuid IS NULL OR event_id = $2)
             AND ($3::uuid IS NULL OR id = $3)
             AND ($4::uuid IS NULL OR owner_user_id = $4)
             AND ($5::text IS NULL OR status = $5)
             AND ($6::uuid IS NULL OR workstream_id = $6)
             AND local_number > $7
             AND (NOT $8 OR status IN ('open', 'in-progress', 'blocked'))
           ORDER BY event_id, local_number
           LIMIT $9"#,
        scope.organization_id().as_uuid(),
        select.event,
        select.id,
        select.owner,
        select.status,
        select.workstream,
        select.after,
        select.open_only,
        select.limit,
    )
    .fetch_all(&mut *conn)
    .await
    .map_err(store_error)?;
    Ok(rows
        .into_iter()
        .map(ActionView::try_from)
        .collect::<Result<_, _>>()?)
}

/// The commitments of `select` with their promisors and their evidence.
async fn select_commitments(
    conn: &mut PgConnection,
    scope: OrgScope,
    select: &Select,
) -> Result<Vec<CommitmentView>, StoreError> {
    let organization = scope.organization_id().as_uuid();
    let rows = sqlx::query_as!(
        CommitmentRow,
        r#"SELECT c.id, c.event_id, c.local_number, c.text, c.condition, c.person_id, c.institution_id,
                  COALESCE(p.local_number, i.local_number) AS "promisor_number?",
                  COALESCE(p.name, i.name) AS "promisor_name?",
                  c.owner_user_id, c.workstream_id, c.due_date AS "due_date: jiff_sqlx::Date",
                  c.status, c.firm_reason, c.version
           FROM commitment c
           LEFT JOIN person p ON p.organization_id = c.organization_id AND p.id = c.person_id
           LEFT JOIN institution i
               ON i.organization_id = c.organization_id AND i.id = c.institution_id
           WHERE c.organization_id = $1
             AND ($2::uuid IS NULL OR c.event_id = $2)
             AND ($3::uuid IS NULL OR c.id = $3)
             AND ($4::uuid IS NULL OR c.owner_user_id = $4)
             AND ($5::text IS NULL OR c.status = $5)
             AND ($6::uuid IS NULL OR c.workstream_id = $6)
             AND c.local_number > $7
             AND (NOT $8 OR c.status IN ('conditional', 'firm'))
           ORDER BY c.event_id, c.local_number
           LIMIT $9"#,
        organization,
        select.event,
        select.id,
        select.owner,
        select.status,
        select.workstream,
        select.after,
        select.open_only,
        select.limit,
    )
    .fetch_all(&mut *conn)
    .await
    .map_err(store_error)?;
    let ids: Vec<Uuid> = rows.iter().map(|row| row.id).collect();
    let evidence = sqlx::query_as!(
        EvidenceRow,
        r#"SELECT e.commitment_id AS "commitment_id!", e.record_version, e.proposal_id,
                  e.source_version_id, v.captured_at AS "captured_at: jiff_sqlx::Timestamp",
                  e.start_offset, e.end_offset, e.quote, e.page
           FROM record_evidence e
           JOIN source_version v
               ON v.organization_id = e.organization_id AND v.id = e.source_version_id
           WHERE e.organization_id = $1 AND e.commitment_id = ANY($2)
           ORDER BY e.record_version, e.start_offset, e.id"#,
        organization,
        &ids,
    )
    .fetch_all(&mut *conn)
    .await
    .map_err(store_error)?;
    let mut by_commitment: HashMap<Uuid, Vec<RecordEvidenceView>> = HashMap::new();
    for row in evidence {
        let id = row.commitment_id;
        by_commitment
            .entry(id)
            .or_default()
            .push(RecordEvidenceView::try_from(row)?);
    }
    Ok(rows
        .into_iter()
        .map(|row| {
            let evidence = by_commitment.remove(&row.id).unwrap_or_default();
            row.into_view(evidence)
        })
        .collect::<Result<_, _>>()?)
}

/// Locks the row of a record and checks its version. The row lock serializes two changes:
/// the second one sees the new version and conflicts.
async fn lock_version(
    conn: &mut PgConnection,
    scope: OrgScope,
    table: Table,
    event: EventId,
    id: Uuid,
    expected: RecordVersion,
) -> Result<Option<WorkChanged<()>>, StoreError> {
    let (organization, event) = (scope.organization_id().as_uuid(), event.as_uuid());
    let current = match table {
        Table::Action => {
            sqlx::query_scalar!(
                "SELECT version FROM action
             WHERE organization_id = $1 AND event_id = $2 AND id = $3
             FOR UPDATE",
                organization,
                event,
                id,
            )
            .fetch_optional(&mut *conn)
            .await
        }
        Table::Commitment => {
            sqlx::query_scalar!(
                "SELECT version FROM commitment
             WHERE organization_id = $1 AND event_id = $2 AND id = $3
             FOR UPDATE",
                organization,
                event,
                id,
            )
            .fetch_optional(&mut *conn)
            .await
        }
    }
    .map_err(store_error)?;
    Ok(match current {
        None => Some(WorkChanged::NotFound),
        Some(version) if version != expected.get() => Some(WorkChanged::VersionConflict),
        Some(_) => None,
    })
}

#[derive(Debug, Clone, Copy)]
enum Table {
    Action,
    Commitment,
}

/// The keys of the events that `user` can read now: the events with an event role of the user,
/// or all events of the organization if `all_events` is set.
async fn event_keys_of(
    conn: &mut PgConnection,
    scope: OrgScope,
    user: UserId,
    all_events: bool,
) -> Result<HashMap<Uuid, EventKey>, StoreError> {
    let rows = sqlx::query!(
        "SELECT e.id, e.key
         FROM event e
         WHERE e.organization_id = $1
           AND ($3 OR EXISTS (SELECT 1 FROM event_membership m
                              WHERE m.organization_id = e.organization_id
                                AND m.event_id = e.id AND m.user_id = $2))",
        scope.organization_id().as_uuid(),
        user.as_uuid(),
        all_events,
    )
    .fetch_all(&mut *conn)
    .await
    .map_err(store_error)?;
    rows.into_iter()
        .map(|row| {
            let key = EventKey::parse(&row.key).map_err(|_| InvalidRow("event key"))?;
            Ok((row.id, key))
        })
        .collect()
}

/// Keeps the records of the events in `keys` and sorts them: due date first, none last,
/// then event key and number.
fn in_events<T>(
    items: Vec<T>,
    keys: &HashMap<Uuid, EventKey>,
    sort: impl Fn(&T) -> (Option<jiff::civil::Date>, EventId, u64),
) -> Vec<InEvent<T>> {
    let mut found: Vec<_> = items
        .into_iter()
        .filter_map(|record| {
            let (due, event, number) = sort(&record);
            let event_key = keys.get(&event.as_uuid())?.clone();
            Some((
                (due.is_none(), due, event_key.clone(), number),
                event_key,
                record,
            ))
        })
        .collect();
    found.sort_by(|a, b| a.0.cmp(&b.0));
    found
        .into_iter()
        .map(|(_, event_key, record)| InEvent { event_key, record })
        .collect()
}

#[async_trait]
impl WorkStore for Database {
    async fn create_action(
        &self,
        scope: OrgScope,
        action: &NewActionRecord,
        at: Timestamp,
        audit: &AuditEvent,
    ) -> Result<WorkCreated<ActionView>, StoreError> {
        let mut tx = self.pool.begin().await.map_err(store_error)?;
        let number = next_local_number(
            &mut tx,
            scope,
            action.event_id.as_uuid(),
            LocalIdKind::Action.prefix(),
        )
        .await
        .map_err(store_error)?;
        let fields = &action.fields;
        let inserted = sqlx::query!(
            "INSERT INTO action
                 (id, organization_id, event_id, local_number, title, description, owner_user_id,
                  workstream_id, due_date, status, version, created_at, updated_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, 1, $11, $11)",
            action.id.as_uuid(),
            scope.organization_id().as_uuid(),
            action.event_id.as_uuid(),
            number,
            fields.title.as_str(),
            fields.description.as_ref().map(ActionDescription::as_str),
            fields.owner.as_uuid(),
            fields.workstream_id.map(WorkstreamId::as_uuid),
            fields.due_date.map(|date| date.to_sqlx()) as _,
            fields.status.as_str(),
            at.to_sqlx() as _,
        )
        .execute(&mut *tx)
        .await;
        match inserted {
            Ok(_) => {}
            Err(error) if violates(&error, "action_pkey") => return Ok(WorkCreated::IdTaken),
            Err(error) => return Err(store_error(error)),
        }
        audit::record(&mut tx, audit).await.map_err(store_error)?;
        let view = select_actions(
            &mut tx,
            scope,
            &Select::one(action.event_id, action.id.as_uuid()),
        )
        .await?
        .pop()
        .ok_or(InvalidRow("action"))?;
        tx.commit().await.map_err(store_error)?;
        Ok(WorkCreated::Created(view))
    }

    async fn change_action(
        &self,
        scope: OrgScope,
        event: EventId,
        id: ActionId,
        fields: &ActionFields,
        expected: RecordVersion,
        audit: &AuditEvent,
    ) -> Result<WorkChanged<ActionView>, StoreError> {
        let mut tx = self.pool.begin().await.map_err(store_error)?;
        let id = id.as_uuid();
        match lock_version(&mut tx, scope, Table::Action, event, id, expected).await? {
            Some(WorkChanged::NotFound) => return Ok(WorkChanged::NotFound),
            Some(_) => return Ok(WorkChanged::VersionConflict),
            None => {}
        }
        sqlx::query!(
            "UPDATE action
             SET title = $4, description = $5, owner_user_id = $6, workstream_id = $7,
                 due_date = $8, status = $9, version = version + 1, updated_at = now()
             WHERE organization_id = $1 AND event_id = $2 AND id = $3",
            scope.organization_id().as_uuid(),
            event.as_uuid(),
            id,
            fields.title.as_str(),
            fields.description.as_ref().map(ActionDescription::as_str),
            fields.owner.as_uuid(),
            fields.workstream_id.map(WorkstreamId::as_uuid),
            fields.due_date.map(|date| date.to_sqlx()) as _,
            fields.status.as_str(),
        )
        .execute(&mut *tx)
        .await
        .map_err(store_error)?;
        audit::record(&mut tx, audit).await.map_err(store_error)?;
        let view = select_actions(&mut tx, scope, &Select::one(event, id))
            .await?
            .pop()
            .ok_or(InvalidRow("action"))?;
        tx.commit().await.map_err(store_error)?;
        Ok(WorkChanged::Changed(view))
    }

    async fn action(
        &self,
        scope: OrgScope,
        event: EventId,
        id: ActionId,
    ) -> Result<Option<ActionView>, StoreError> {
        let mut conn = self.pool.acquire().await.map_err(store_error)?;
        Ok(
            select_actions(&mut conn, scope, &Select::one(event, id.as_uuid()))
                .await?
                .pop(),
        )
    }

    async fn actions(
        &self,
        scope: OrgScope,
        event: EventId,
        filter: &WorkFilter<ActionStatus>,
    ) -> Result<Vec<ActionView>, StoreError> {
        let mut conn = self.pool.acquire().await.map_err(store_error)?;
        let status = filter.status.map(ActionStatus::as_str);
        select_actions(&mut conn, scope, &Select::filter(event, filter, status)).await
    }

    async fn create_commitment(
        &self,
        scope: OrgScope,
        commitment: &NewCommitmentRecord,
        at: Timestamp,
        audit: &AuditEvent,
    ) -> Result<WorkCreated<CommitmentView>, StoreError> {
        let mut tx = self.pool.begin().await.map_err(store_error)?;
        let number = next_local_number(
            &mut tx,
            scope,
            commitment.event_id.as_uuid(),
            LocalIdKind::Commitment.prefix(),
        )
        .await
        .map_err(store_error)?;
        let (person, institution) = match commitment.promisor {
            Party::Person(id) => (Some(id.as_uuid()), None),
            Party::Institution(id) => (None, Some(id.as_uuid())),
        };
        let fields = &commitment.fields;
        let inserted = sqlx::query!(
            "INSERT INTO commitment
                 (id, organization_id, event_id, local_number, text, condition, person_id,
                  institution_id, owner_user_id, workstream_id, due_date, status, firm_reason,
                  version, created_at, updated_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, 1, $14, $14)",
            commitment.id.as_uuid(),
            scope.organization_id().as_uuid(),
            commitment.event_id.as_uuid(),
            number,
            fields.text.as_str(),
            commitment.condition.as_ref().map(ConditionText::as_str),
            person,
            institution,
            fields.owner.as_uuid(),
            fields.workstream_id.map(WorkstreamId::as_uuid),
            fields.due_date.map(|date| date.to_sqlx()) as _,
            fields.status.as_str(),
            fields.firm_reason.as_ref().map(FirmReason::as_str),
            at.to_sqlx() as _,
        )
        .execute(&mut *tx)
        .await;
        match inserted {
            Ok(_) => {}
            Err(error) if violates(&error, "commitment_pkey") => return Ok(WorkCreated::IdTaken),
            Err(error) => return Err(store_error(error)),
        }
        audit::record(&mut tx, audit).await.map_err(store_error)?;
        let view = select_commitments(
            &mut tx,
            scope,
            &Select::one(commitment.event_id, commitment.id.as_uuid()),
        )
        .await?
        .pop()
        .ok_or(InvalidRow("commitment"))?;
        tx.commit().await.map_err(store_error)?;
        Ok(WorkCreated::Created(view))
    }

    async fn change_commitment(
        &self,
        scope: OrgScope,
        event: EventId,
        id: CommitmentId,
        fields: &CommitmentFields,
        expected: RecordVersion,
        audit: &AuditEvent,
    ) -> Result<WorkChanged<CommitmentView>, StoreError> {
        let mut tx = self.pool.begin().await.map_err(store_error)?;
        let id = id.as_uuid();
        match lock_version(&mut tx, scope, Table::Commitment, event, id, expected).await? {
            Some(WorkChanged::NotFound) => return Ok(WorkChanged::NotFound),
            Some(_) => return Ok(WorkChanged::VersionConflict),
            None => {}
        }
        sqlx::query!(
            "UPDATE commitment
             SET text = $4, owner_user_id = $5, workstream_id = $6, due_date = $7, status = $8,
                 firm_reason = $9, version = version + 1, updated_at = now()
             WHERE organization_id = $1 AND event_id = $2 AND id = $3",
            scope.organization_id().as_uuid(),
            event.as_uuid(),
            id,
            fields.text.as_str(),
            fields.owner.as_uuid(),
            fields.workstream_id.map(WorkstreamId::as_uuid),
            fields.due_date.map(|date| date.to_sqlx()) as _,
            fields.status.as_str(),
            fields.firm_reason.as_ref().map(FirmReason::as_str),
        )
        .execute(&mut *tx)
        .await
        .map_err(store_error)?;
        audit::record(&mut tx, audit).await.map_err(store_error)?;
        let view = select_commitments(&mut tx, scope, &Select::one(event, id))
            .await?
            .pop()
            .ok_or(InvalidRow("commitment"))?;
        tx.commit().await.map_err(store_error)?;
        Ok(WorkChanged::Changed(view))
    }

    async fn commitment(
        &self,
        scope: OrgScope,
        event: EventId,
        id: CommitmentId,
    ) -> Result<Option<CommitmentView>, StoreError> {
        let mut conn = self.pool.acquire().await.map_err(store_error)?;
        Ok(
            select_commitments(&mut conn, scope, &Select::one(event, id.as_uuid()))
                .await?
                .pop(),
        )
    }

    async fn commitments(
        &self,
        scope: OrgScope,
        event: EventId,
        filter: &WorkFilter<CommitmentStatus>,
    ) -> Result<Vec<CommitmentView>, StoreError> {
        let mut conn = self.pool.acquire().await.map_err(store_error)?;
        let status = filter.status.map(CommitmentStatus::as_str);
        select_commitments(&mut conn, scope, &Select::filter(event, filter, status)).await
    }

    async fn my_open_work(
        &self,
        scope: OrgScope,
        user: UserId,
        all_events: bool,
    ) -> Result<MyWork, StoreError> {
        let mut conn = self.pool.acquire().await.map_err(store_error)?;
        let keys = event_keys_of(&mut conn, scope, user, all_events).await?;
        let actions = select_actions(&mut conn, scope, &Select::open_of(user)).await?;
        let commitments = select_commitments(&mut conn, scope, &Select::open_of(user)).await?;
        Ok(MyWork {
            actions: in_events(actions, &keys, |a| {
                (a.fields.due_date, a.event_id, a.local_number)
            }),
            commitments: in_events(commitments, &keys, |c| {
                (c.fields.due_date, c.event_id, c.local_number)
            }),
        })
    }
}

#[cfg(test)]
mod tests;
