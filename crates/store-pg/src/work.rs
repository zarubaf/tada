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
    ActionFields, ActionView, CommitmentFields, CommitmentView, MyWork, NewActionRecord,
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

/// Sorts open records by due date; records without a due date come last.
fn by_due_date<T>(items: &mut [T], key: impl Fn(&T) -> (Option<jiff::civil::Date>, EventId, u64)) {
    items.sort_by_key(|item| {
        let (due, event, number) = key(item);
        (due.is_none(), due, event.as_uuid(), number)
    });
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

    async fn my_open_work(&self, scope: OrgScope, user: UserId) -> Result<MyWork, StoreError> {
        let mut conn = self.pool.acquire().await.map_err(store_error)?;
        let mut actions = select_actions(&mut conn, scope, &Select::open_of(user)).await?;
        let mut commitments = select_commitments(&mut conn, scope, &Select::open_of(user)).await?;
        by_due_date(&mut actions, |a| {
            (a.fields.due_date, a.event_id, a.local_number)
        });
        by_due_date(&mut commitments, |c| {
            (c.fields.due_date, c.event_id, c.local_number)
        });
        Ok(MyWork {
            actions,
            commitments,
        })
    }
}

#[cfg(test)]
mod tests {
    use jiff::civil::date;
    use tada_app::audit::AuditAction;
    use tada_app::caller::{MemberCaller, OrganizationRole};
    use tada_app::domain::identity::{DisplayName, Email};
    use tada_app::domain::parties::InstitutionKind;
    use tada_app::parties::{InstitutionFields, PartyStore};

    use super::*;
    use crate::testing::TestDatabase;

    struct Fixture {
        test: TestDatabase,
        scope: OrgScope,
        caller: MemberCaller,
        event: EventId,
        owner: UserId,
        supplier: InstitutionId,
    }

    const AT: &str = "2030-05-18T08:00:00Z";

    impl Fixture {
        async fn start() -> Self {
            let test = TestDatabase::start().await;
            let testwil = test.create_organization("testwil").await;
            let event = test.create_event(testwil, "TEST30").await;
            let owner = test
                .create_user(
                    &DisplayName::parse("Anna Muster").unwrap(),
                    &Email::parse("anna@example.org").unwrap(),
                )
                .await;
            let caller = MemberCaller::new(owner, testwil, OrganizationRole::Owner);
            let scope = caller.scope();
            let supplier = InstitutionId::from_uuid(Uuid::now_v7());
            let fields = InstitutionFields {
                name: PartyName::parse("Testwil Generatoren AG").unwrap(),
                kind: InstitutionKind::Company,
                email: None,
                phone: None,
            };
            let audit = AuditEvent::new(
                caller.actor(),
                AuditAction::InstitutionCreate,
                Some(supplier.as_uuid()),
                Some(scope),
            );
            test.database
                .create_institution(scope, supplier, &fields, AT.parse().unwrap(), &audit)
                .await
                .unwrap();
            Self {
                test,
                scope,
                caller,
                event,
                owner,
                supplier,
            }
        }

        fn audit(&self, action: AuditAction, id: Uuid) -> AuditEvent {
            AuditEvent::new(self.caller.actor(), action, Some(id), Some(self.scope))
        }

        fn action_fields(&self, title: &str, due: Option<jiff::civil::Date>) -> ActionFields {
            ActionFields {
                title: ActionTitle::parse(title).unwrap(),
                description: None,
                owner: self.owner,
                workstream_id: None,
                due_date: due,
                status: ActionStatus::Open,
            }
        }

        async fn action(&self, title: &str, due: Option<jiff::civil::Date>) -> ActionView {
            let record = NewActionRecord {
                id: ActionId::from_uuid(Uuid::now_v7()),
                event_id: self.event,
                fields: self.action_fields(title, due),
            };
            let audit = self.audit(AuditAction::ActionCreate, record.id.as_uuid());
            match self
                .test
                .database
                .create_action(self.scope, &record, AT.parse().unwrap(), &audit)
                .await
                .unwrap()
            {
                WorkCreated::Created(view) => view,
                WorkCreated::IdTaken => panic!("id taken"),
            }
        }

        async fn commitment(&self, condition: Option<&str>) -> CommitmentView {
            let condition = condition.map(|text| ConditionText::parse(text).unwrap());
            let record = NewCommitmentRecord {
                id: CommitmentId::from_uuid(Uuid::now_v7()),
                event_id: self.event,
                promisor: Party::Institution(self.supplier),
                fields: CommitmentFields {
                    text: CommitmentText::parse("Generator delivery Friday 15:00").unwrap(),
                    owner: self.owner,
                    workstream_id: None,
                    due_date: None,
                    status: CommitmentStatus::initial(condition.as_ref()),
                    firm_reason: None,
                },
                condition,
            };
            let audit = self.audit(AuditAction::CommitmentCreate, record.id.as_uuid());
            match self
                .test
                .database
                .create_commitment(self.scope, &record, AT.parse().unwrap(), &audit)
                .await
                .unwrap()
            {
                WorkCreated::Created(view) => view,
                WorkCreated::IdTaken => panic!("id taken"),
            }
        }

        async fn change_action(
            &self,
            scope: OrgScope,
            event: EventId,
            id: ActionId,
            fields: &ActionFields,
            expected: RecordVersion,
        ) -> WorkChanged<ActionView> {
            self.test
                .database
                .change_action(
                    scope,
                    event,
                    id,
                    fields,
                    expected,
                    &self.audit(AuditAction::ActionChange, id.as_uuid()),
                )
                .await
                .unwrap()
        }

        async fn audit_actions(&self) -> Vec<String> {
            sqlx::query_scalar("SELECT action FROM audit_event ORDER BY occurred_at, id")
                .fetch_all(&self.test.database.pool)
                .await
                .unwrap()
        }
    }

    #[tokio::test]
    async fn creates_and_changes_an_action_with_numbers_and_audit_events() {
        let f = Fixture::start().await;
        let first = f
            .action("Generator bestellen", Some(date(2030, 6, 1)))
            .await;
        let second = f.action("Zaun stellen", None).await;
        assert_eq!(
            (first.local_id(), second.local_id()),
            ("ACT-001".into(), "ACT-002".into())
        );
        assert_eq!(first.fields.due_date, Some(date(2030, 6, 1)));
        assert_eq!(
            f.test
                .database
                .action(f.scope, f.event, first.id)
                .await
                .unwrap(),
            Some(first.clone())
        );

        let fields = ActionFields {
            description: Some(ActionDescription::parse("Mit Diesel").unwrap()),
            due_date: None,
            status: ActionStatus::Done,
            ..first.fields.clone()
        };
        let WorkChanged::Changed(changed) = f
            .change_action(f.scope, f.event, first.id, &fields, RecordVersion::FIRST)
            .await
        else {
            panic!("not changed");
        };
        assert_eq!(changed.fields, fields);
        assert_eq!(changed.version.get(), 2);
        assert_eq!(
            f.change_action(f.scope, f.event, first.id, &fields, RecordVersion::FIRST)
                .await,
            WorkChanged::VersionConflict
        );

        let filter = WorkFilter {
            owner: Some(f.owner),
            status: Some(ActionStatus::Open),
            workstream: None,
            after: None,
            limit: 10,
        };
        let open = f
            .test
            .database
            .actions(f.scope, f.event, &filter)
            .await
            .unwrap();
        assert_eq!(open, [second]);
        assert_eq!(
            f.audit_actions().await,
            [
                "institution.create",
                "action.create",
                "action.create",
                "action.change"
            ]
        );
    }

    /// Another event and another organization neither see nor change the record (ADR 0006).
    #[tokio::test]
    async fn keeps_work_records_inside_their_event_and_organization() {
        let f = Fixture::start().await;
        let db = &f.test.database;
        let action = f.action("Generator bestellen", None).await;
        let commitment = f.commitment(None).await;
        let other_event = f
            .test
            .create_event(f.scope.organization_id(), "TEST31")
            .await;
        assert_eq!(
            db.action(f.scope, other_event, action.id).await.unwrap(),
            None
        );
        assert_eq!(
            db.commitment(f.scope, other_event, commitment.id)
                .await
                .unwrap(),
            None
        );
        // The counter of readable IDs counts in each event.
        let other = NewActionRecord {
            id: ActionId::from_uuid(Uuid::now_v7()),
            event_id: other_event,
            fields: f.action_fields("Bar", None),
        };
        let WorkCreated::Created(other) = db
            .create_action(
                f.scope,
                &other,
                AT.parse().unwrap(),
                &f.audit(AuditAction::ActionCreate, other.id.as_uuid()),
            )
            .await
            .unwrap()
        else {
            panic!("not created");
        };
        assert_eq!(other.local_id(), "ACT-001");

        let musterhausen = f.test.create_organization("musterhausen").await;
        let stranger = MemberCaller::new(f.owner, musterhausen, OrganizationRole::Owner).scope();
        assert_eq!(db.action(stranger, f.event, action.id).await.unwrap(), None);
        assert_eq!(
            db.commitment(stranger, f.event, commitment.id)
                .await
                .unwrap(),
            None
        );
        assert_eq!(
            f.change_action(
                stranger,
                f.event,
                action.id,
                &action.fields,
                RecordVersion::FIRST
            )
            .await,
            WorkChanged::NotFound
        );
        assert!(
            db.my_open_work(stranger, f.owner)
                .await
                .unwrap()
                .actions
                .is_empty()
        );
    }

    /// Two changes with the same expected version: exactly one wins.
    #[tokio::test]
    async fn two_changes_with_one_version_leave_one_winner() {
        let f = Fixture::start().await;
        let action = f.action("Generator bestellen", None).await;
        let alpha = f.action_fields("Alpha", None);
        let beta = f.action_fields("Beta", None);
        let (a, b) = tokio::join!(
            f.change_action(f.scope, f.event, action.id, &alpha, RecordVersion::FIRST),
            f.change_action(f.scope, f.event, action.id, &beta, RecordVersion::FIRST)
        );
        let winners = [&a, &b]
            .iter()
            .filter(|changed| matches!(changed, WorkChanged::Changed(_)))
            .count();
        assert_eq!(winners, 1, "{a:?} {b:?}");
        assert!([&a, &b].contains(&&WorkChanged::VersionConflict));
    }

    #[tokio::test]
    async fn a_commitment_keeps_its_condition_and_its_firm_reason() {
        let f = Fixture::start().await;
        let db = &f.test.database;
        let commitment = f.commitment(Some("subject to signed order")).await;
        assert_eq!(commitment.local_id(), "COM-001");
        assert_eq!(commitment.fields.status, CommitmentStatus::Conditional);
        assert_eq!(commitment.promisor.local_id, "INS-001");
        assert_eq!(commitment.promisor.party, Party::Institution(f.supplier));
        assert!(commitment.evidence.is_empty());

        // The schema refuses a conditional commitment with a firm reason.
        let refused = sqlx::query("UPDATE commitment SET firm_reason = 'x' WHERE id = $1")
            .bind(commitment.id.as_uuid())
            .execute(&db.pool)
            .await;
        assert!(refused.is_err());

        let fields = CommitmentFields {
            status: CommitmentStatus::Firm,
            firm_reason: Some(FirmReason::parse("The order is signed.").unwrap()),
            ..commitment.fields.clone()
        };
        let WorkChanged::Changed(firm) = db
            .change_commitment(
                f.scope,
                f.event,
                commitment.id,
                &fields,
                RecordVersion::FIRST,
                &f.audit(AuditAction::CommitmentFirm, commitment.id.as_uuid()),
            )
            .await
            .unwrap()
        else {
            panic!("not changed");
        };
        assert_eq!(firm.fields, fields);
        assert_eq!(firm.condition, commitment.condition);
        assert_eq!(
            f.audit_actions().await,
            ["institution.create", "commitment.create", "commitment.firm"]
        );
    }

    /// Task 8 writes the evidence of accepted proposals; the read shows it with the capture time.
    #[tokio::test]
    async fn a_commitment_shows_its_evidence() {
        let f = Fixture::start().await;
        let db = &f.test.database;
        let commitment = f.commitment(None).await;
        let organization = f.scope.organization_id().as_uuid();
        let (item, version, changeset, proposal) = (
            Uuid::now_v7(),
            Uuid::now_v7(),
            Uuid::now_v7(),
            Uuid::now_v7(),
        );
        let author = serde_json::json!({
            "kind": "member", "id": f.owner.as_uuid(), "principal": null,
            "channel": "web", "request_id": null,
        });
        let text = "Lieferung Freitag 15:00, Bestellung unterschrieben.";
        sqlx::query(
            "INSERT INTO source_item (id, organization_id, event_id, kind, created_at)
             VALUES ($1, $2, $3, 'member-text', '2030-05-02T08:00:00Z')",
        )
        .bind(item)
        .bind(organization)
        .bind(f.event.as_uuid())
        .execute(&db.pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO source_version
                 (id, organization_id, source_item_id, kind, channel, author_actor, text, sha256, captured_at)
             VALUES ($1, $2, $3, 'member-text', 'web', $4, $5, sha256(convert_to($5, 'UTF8')),
                     '2030-05-02T08:00:00Z')",
        )
        .bind(version)
        .bind(organization)
        .bind(item)
        .bind(&author)
        .bind(text)
        .execute(&db.pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO changeset (id, organization_id, event_id, author, source_version_id, created_at)
             VALUES ($1, $2, $3, $4, $5, '2030-05-02T08:00:00Z')",
        )
        .bind(changeset)
        .bind(organization)
        .bind(f.event.as_uuid())
        .bind(&author)
        .bind(version)
        .execute(&db.pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO proposal
                 (id, organization_id, changeset_id, event_id, operation, operation_version,
                  target_kind, target_id, reason, created_at)
             VALUES ($1, $2, $3, $4, '{\"kind\": \"create_commitment\"}', 1, 'commitment', $5,
                     'Die Quelle nennt die Lieferung.', '2030-05-02T08:00:00Z')",
        )
        .bind(proposal)
        .bind(organization)
        .bind(changeset)
        .bind(f.event.as_uuid())
        .bind(commitment.id.as_uuid())
        .execute(&db.pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO record_evidence
                 (id, organization_id, commitment_id, record_version, proposal_id, source_version_id,
                  start_offset, end_offset, quote, page)
             VALUES ($1, $2, $3, 1, $4, $5, 0, 23, 'Lieferung Freitag 15:00', NULL)",
        )
        .bind(Uuid::now_v7())
        .bind(organization)
        .bind(commitment.id.as_uuid())
        .bind(proposal)
        .bind(version)
        .execute(&db.pool)
        .await
        .unwrap();

        let read = db
            .commitment(f.scope, f.event, commitment.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            read.evidence,
            [RecordEvidenceView {
                record_version: RecordVersion::FIRST,
                proposal_id: ProposalId::from_uuid(proposal),
                source_version_id: SourceVersionId::from_uuid(version),
                captured_at: "2030-05-02T08:00:00Z".parse().unwrap(),
                start_offset: 0,
                end_offset: 23,
                quote: "Lieferung Freitag 15:00".to_owned(),
                page: None,
            }]
        );
    }

    #[tokio::test]
    async fn my_open_work_orders_by_due_date_and_leaves_out_closed_records() {
        let f = Fixture::start().await;
        let late = f.action("Spät", Some(date(2030, 6, 9))).await;
        let undated = f.action("Ohne Datum", None).await;
        let early = f.action("Früh", Some(date(2030, 6, 1))).await;
        let done = f.action("Erledigt", None).await;
        let fields = ActionFields {
            status: ActionStatus::Done,
            ..done.fields.clone()
        };
        f.change_action(f.scope, f.event, done.id, &fields, RecordVersion::FIRST)
            .await;
        let commitment = f.commitment(Some("subject to signed order")).await;

        let work = f
            .test
            .database
            .my_open_work(f.scope, f.owner)
            .await
            .unwrap();
        let ids: Vec<_> = work.actions.iter().map(|action| action.id).collect();
        assert_eq!(ids, [early.id, late.id, undated.id]);
        assert_eq!(work.commitments, [commitment]);
    }
}
