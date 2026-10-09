//! The `WorkStore` adapter: actions and commitments (ADR 0068).

use std::collections::HashMap;

use async_trait::async_trait;
use jiff::Timestamp;
use jiff_sqlx::ToSqlx;
use sqlx::PgConnection;
use sqlx::types::Uuid;
use tada_app::access::EventReach;
use tada_app::audit::AuditEvent;
use tada_app::caller::OrgScope;
use tada_app::domain::RecordVersion;
use tada_app::domain::events::EventKey;
use tada_app::domain::ids::{
    ActionId, CommitmentId, EventId, InstitutionId, LocalIdKind, PersonId, UserId, WorkstreamId,
};
use tada_app::domain::parties::{Party, PartyName};
use tada_app::domain::work::{
    ActionDescription, ActionStatus, ActionTitle, CommitmentStatus, CommitmentText, ConditionText,
    FirmReason,
};
use tada_app::parties::PartyRef;
use tada_app::records::{Changed, Created, NumberCursor};
use tada_app::store::StoreError;
use tada_app::work::{
    ActionFields, ActionView, CommitmentFields, CommitmentView, InEvent, MyWork, NewActionRecord,
    NewCommitmentRecord, WorkFilter, WorkStore,
};

use crate::Database;
use crate::audit;
use crate::error::{InvalidRow, store_error, violates};
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

    fn into_view(self) -> Result<CommitmentView, InvalidRow> {
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
    /// Only the records of these events. `None` reads each event of the organization.
    events: Option<Vec<Uuid>>,
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

    fn open_of(user: UserId, events: Option<Vec<Uuid>>) -> Self {
        Self {
            owner: Some(user.as_uuid()),
            open_only: true,
            events,
            ..Self::default()
        }
    }
}

/// The first number that a page starts after. A cursor beyond the largest number gives an empty page.
fn after_number(after: Option<NumberCursor>) -> i64 {
    after.map_or(0, |cursor| i64::try_from(cursor.0).unwrap_or(i64::MAX))
}

async fn select_actions(
    conn: &mut PgConnection,
    scope: OrgScope,
    select: &Select,
) -> Result<Vec<ActionView>, sqlx::Error> {
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
             AND ($10::uuid[] IS NULL OR event_id = ANY($10))
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
        select.events.as_deref(),
    )
    .fetch_all(&mut *conn)
    .await?;
    Ok(rows
        .into_iter()
        .map(ActionView::try_from)
        .collect::<Result<_, _>>()?)
}

/// The commitments of `select` with their promisors.
async fn select_commitments(
    conn: &mut PgConnection,
    scope: OrgScope,
    select: &Select,
) -> Result<Vec<CommitmentView>, sqlx::Error> {
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
             AND ($10::uuid[] IS NULL OR c.event_id = ANY($10))
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
        select.events.as_deref(),
    )
    .fetch_all(&mut *conn)
    .await?;
    Ok(rows
        .into_iter()
        .map(|row| row.into_view())
        .collect::<Result<_, _>>()?)
}

/// The action `id` of the event, read inside the transaction of the caller.
pub(crate) async fn action_in(
    conn: &mut PgConnection,
    scope: OrgScope,
    event: EventId,
    id: ActionId,
) -> Result<Option<ActionView>, sqlx::Error> {
    Ok(
        select_actions(conn, scope, &Select::one(event, id.as_uuid()))
            .await?
            .pop(),
    )
}

/// The commitment `id` of the event, read inside the transaction of the caller.
pub(crate) async fn commitment_in(
    conn: &mut PgConnection,
    scope: OrgScope,
    event: EventId,
    id: CommitmentId,
) -> Result<Option<CommitmentView>, sqlx::Error> {
    Ok(
        select_commitments(conn, scope, &Select::one(event, id.as_uuid()))
            .await?
            .pop(),
    )
}

/// Inserts an action with the version 1 and the next number of its event, and returns the number.
/// The direct command and the apply of a proposal both write a new action here (ADR 0068).
pub(crate) async fn insert_action(
    conn: &mut PgConnection,
    scope: OrgScope,
    action: &NewActionRecord,
    at: Timestamp,
) -> Result<i64, sqlx::Error> {
    let number = next_local_number(
        conn,
        scope,
        action.event_id.as_uuid(),
        LocalIdKind::Action.prefix(),
    )
    .await?;
    let fields = &action.fields;
    sqlx::query!(
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
    .execute(&mut *conn)
    .await?;
    Ok(number)
}

/// Replaces the values of an action if its version is `expected`, and returns the new version.
/// `None` means that the event has no such action with this version. The row lock of the update
/// serializes two changes: the second one sees the new version and changes nothing.
/// The direct command and the apply of a proposal both change an action here (ADR 0068).
pub(crate) async fn update_action(
    conn: &mut PgConnection,
    scope: OrgScope,
    event: EventId,
    id: ActionId,
    fields: &ActionFields,
    expected: RecordVersion,
    at: Timestamp,
) -> Result<Option<i64>, sqlx::Error> {
    sqlx::query_scalar!(
        "UPDATE action
         SET title = $5, description = $6, owner_user_id = $7, workstream_id = $8,
             due_date = $9, status = $10, version = version + 1, updated_at = $11
         WHERE organization_id = $1 AND event_id = $2 AND id = $3 AND version = $4
         RETURNING version",
        scope.organization_id().as_uuid(),
        event.as_uuid(),
        id.as_uuid(),
        expected.get(),
        fields.title.as_str(),
        fields.description.as_ref().map(ActionDescription::as_str),
        fields.owner.as_uuid(),
        fields.workstream_id.map(WorkstreamId::as_uuid),
        fields.due_date.map(|date| date.to_sqlx()) as _,
        fields.status.as_str(),
        at.to_sqlx() as _,
    )
    .fetch_optional(&mut *conn)
    .await
}

/// Inserts a commitment with the version 1 and the next number of its event, and returns the number.
/// The direct command and the apply of a proposal both write a new commitment here (ADR 0068).
pub(crate) async fn insert_commitment(
    conn: &mut PgConnection,
    scope: OrgScope,
    commitment: &NewCommitmentRecord,
    at: Timestamp,
) -> Result<i64, sqlx::Error> {
    let number = next_local_number(
        conn,
        scope,
        commitment.event_id.as_uuid(),
        LocalIdKind::Commitment.prefix(),
    )
    .await?;
    let (person, institution) = match commitment.promisor {
        Party::Person(id) => (Some(id.as_uuid()), None),
        Party::Institution(id) => (None, Some(id.as_uuid())),
    };
    let fields = &commitment.fields;
    sqlx::query!(
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
    .execute(&mut *conn)
    .await?;
    Ok(number)
}

/// Like `update_action`, for a commitment. The condition and the promisor never change.
pub(crate) async fn update_commitment(
    conn: &mut PgConnection,
    scope: OrgScope,
    event: EventId,
    id: CommitmentId,
    fields: &CommitmentFields,
    expected: RecordVersion,
    at: Timestamp,
) -> Result<Option<i64>, sqlx::Error> {
    sqlx::query_scalar!(
        "UPDATE commitment
         SET text = $5, owner_user_id = $6, workstream_id = $7, due_date = $8, status = $9,
             firm_reason = $10, version = version + 1, updated_at = $11
         WHERE organization_id = $1 AND event_id = $2 AND id = $3 AND version = $4
         RETURNING version",
        scope.organization_id().as_uuid(),
        event.as_uuid(),
        id.as_uuid(),
        expected.get(),
        fields.text.as_str(),
        fields.owner.as_uuid(),
        fields.workstream_id.map(WorkstreamId::as_uuid),
        fields.due_date.map(|date| date.to_sqlx()) as _,
        fields.status.as_str(),
        fields.firm_reason.as_ref().map(FirmReason::as_str),
        at.to_sqlx() as _,
    )
    .fetch_optional(&mut *conn)
    .await
}

/// The keys of the events in `events`, or of each event of the organization for `None`.
async fn event_keys_of(
    conn: &mut PgConnection,
    scope: OrgScope,
    events: Option<&[Uuid]>,
) -> Result<HashMap<Uuid, EventKey>, StoreError> {
    let rows = sqlx::query!(
        "SELECT id, key FROM event
         WHERE organization_id = $1 AND ($2::uuid[] IS NULL OR id = ANY($2))",
        scope.organization_id().as_uuid(),
        events,
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

/// Adds the event key to each record and sorts them: due date first, none last, then event key and number.
/// The select read only the records of the events in `keys`.
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
    ) -> Result<Created<ActionView>, StoreError> {
        let mut tx = self.pool.begin().await.map_err(store_error)?;
        match insert_action(&mut tx, scope, action, at).await {
            Ok(_) => {}
            Err(error) if violates(&error, "action_pkey") => return Ok(Created::IdTaken),
            Err(error) => return Err(store_error(error)),
        }
        audit::record(&mut tx, audit).await.map_err(store_error)?;
        let view = action_in(&mut tx, scope, action.event_id, action.id)
            .await
            .map_err(store_error)?
            .ok_or(InvalidRow("action"))?;
        tx.commit().await.map_err(store_error)?;
        Ok(Created::Created(view))
    }

    async fn change_action(
        &self,
        scope: OrgScope,
        event: EventId,
        id: ActionId,
        fields: &ActionFields,
        expected: RecordVersion,
        at: Timestamp,
        audit: &AuditEvent,
    ) -> Result<Changed<ActionView>, StoreError> {
        let mut tx = self.pool.begin().await.map_err(store_error)?;
        let updated = update_action(&mut tx, scope, event, id, fields, expected, at)
            .await
            .map_err(store_error)?;
        let current = action_in(&mut tx, scope, event, id)
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

    async fn action(
        &self,
        scope: OrgScope,
        event: EventId,
        id: ActionId,
    ) -> Result<Option<ActionView>, StoreError> {
        let mut conn = self.pool.acquire().await.map_err(store_error)?;
        action_in(&mut conn, scope, event, id)
            .await
            .map_err(store_error)
    }

    async fn actions(
        &self,
        scope: OrgScope,
        event: EventId,
        filter: &WorkFilter<ActionStatus>,
    ) -> Result<Vec<ActionView>, StoreError> {
        let mut conn = self.pool.acquire().await.map_err(store_error)?;
        let status = filter.status.map(ActionStatus::as_str);
        select_actions(&mut conn, scope, &Select::filter(event, filter, status))
            .await
            .map_err(store_error)
    }

    async fn create_commitment(
        &self,
        scope: OrgScope,
        commitment: &NewCommitmentRecord,
        at: Timestamp,
        audit: &AuditEvent,
    ) -> Result<Created<CommitmentView>, StoreError> {
        let mut tx = self.pool.begin().await.map_err(store_error)?;
        match insert_commitment(&mut tx, scope, commitment, at).await {
            Ok(_) => {}
            Err(error) if violates(&error, "commitment_pkey") => return Ok(Created::IdTaken),
            Err(error) => return Err(store_error(error)),
        }
        audit::record(&mut tx, audit).await.map_err(store_error)?;
        let view = commitment_in(&mut tx, scope, commitment.event_id, commitment.id)
            .await
            .map_err(store_error)?
            .ok_or(InvalidRow("commitment"))?;
        tx.commit().await.map_err(store_error)?;
        Ok(Created::Created(view))
    }

    async fn change_commitment(
        &self,
        scope: OrgScope,
        event: EventId,
        id: CommitmentId,
        fields: &CommitmentFields,
        expected: RecordVersion,
        at: Timestamp,
        audit: &AuditEvent,
    ) -> Result<Changed<CommitmentView>, StoreError> {
        let mut tx = self.pool.begin().await.map_err(store_error)?;
        let updated = update_commitment(&mut tx, scope, event, id, fields, expected, at)
            .await
            .map_err(store_error)?;
        let current = commitment_in(&mut tx, scope, event, id)
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

    async fn commitment(
        &self,
        scope: OrgScope,
        event: EventId,
        id: CommitmentId,
    ) -> Result<Option<CommitmentView>, StoreError> {
        let mut conn = self.pool.acquire().await.map_err(store_error)?;
        commitment_in(&mut conn, scope, event, id)
            .await
            .map_err(store_error)
    }

    async fn commitments(
        &self,
        scope: OrgScope,
        event: EventId,
        filter: &WorkFilter<CommitmentStatus>,
    ) -> Result<Vec<CommitmentView>, StoreError> {
        let mut conn = self.pool.acquire().await.map_err(store_error)?;
        let status = filter.status.map(CommitmentStatus::as_str);
        select_commitments(&mut conn, scope, &Select::filter(event, filter, status))
            .await
            .map_err(store_error)
    }

    async fn my_open_work(
        &self,
        scope: OrgScope,
        user: UserId,
        events: &EventReach,
    ) -> Result<MyWork, StoreError> {
        let events = match events {
            EventReach::Organization => None,
            EventReach::Events(events) => Some(events.iter().map(|id| id.as_uuid()).collect()),
        };
        let mut conn = self.pool.acquire().await.map_err(store_error)?;
        let keys = event_keys_of(&mut conn, scope, events.as_deref()).await?;
        let select = Select::open_of(user, events);
        let actions = select_actions(&mut conn, scope, &select)
            .await
            .map_err(store_error)?;
        let commitments = select_commitments(&mut conn, scope, &select)
            .await
            .map_err(store_error)?;
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
