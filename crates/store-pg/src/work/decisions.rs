//! The `DecisionStore` adapter (ADR 0080). A decision keeps its parties in `decision_party`, in their order.
//! A supersede inserts the successor and ends the old decision in one transaction; the end repeats the expected
//! version and the status `in-force`, so of two concurrent ends of one decision only one changes it.

use std::collections::HashMap;

use async_trait::async_trait;
use jiff::Timestamp;
use jiff_sqlx::ToSqlx;
use sqlx::PgConnection;
use sqlx::types::Uuid;
use tada_app::audit::AuditEvent;
use tada_app::caller::OrgScope;
use tada_app::domain::RecordVersion;
use tada_app::domain::ids::{DecisionId, EventId, LocalIdKind, UserId, WorkstreamId};
use tada_app::domain::parties::Party;
use tada_app::domain::records::{DecisionChange, DecisionStatus, DecisionWording, RecordReason};
use tada_app::domain::redaction::Redactable;
use tada_app::parties::PartyRef;
use tada_app::records::{Changed, Created};
use tada_app::store::StoreError;
use tada_app::work::{
    DecisionFields, DecisionState, DecisionStore, DecisionView, NewDecisionRecord, Superseded,
    WorkFilter,
};

use super::{PartyColumns, Select, party_ref};
use crate::Database;
use crate::audit;
use crate::error::{InvalidRow, store_error, violates};
use crate::local_ids::next_local_number;

struct DecisionRow {
    id: Uuid,
    event_id: Uuid,
    local_number: i64,
    /// NULL only after a legal redaction (ADR 0072).
    wording: Option<String>,
    decided_on: jiff_sqlx::Date,
    owner_user_id: Uuid,
    workstream_id: Option<Uuid>,
    status: String,
    superseded_by: Option<Uuid>,
    /// NULL for a revoked decision only after a legal redaction.
    revoked_reason: Option<String>,
    version: i64,
}

impl DecisionRow {
    fn state(&self) -> Result<DecisionState, InvalidRow> {
        let status = DecisionStatus::parse(&self.status).ok_or(InvalidRow("decision.status"))?;
        Ok(match status {
            DecisionStatus::InForce => DecisionState::InForce,
            DecisionStatus::Superseded => DecisionState::Superseded {
                by: DecisionId::from_uuid(
                    self.superseded_by
                        .ok_or(InvalidRow("decision.superseded_by"))?,
                ),
            },
            DecisionStatus::Revoked => DecisionState::Revoked {
                reason: match self.revoked_reason.as_deref() {
                    Some(reason) => Redactable::Present(
                        RecordReason::parse(reason)
                            .map_err(|_| InvalidRow("decision.revoked_reason"))?,
                    ),
                    None => Redactable::Redacted,
                },
            },
        })
    }

    fn into_view(self, parties: Vec<PartyRef>) -> Result<DecisionView, InvalidRow> {
        if parties.is_empty() {
            return Err(InvalidRow("decision_party"));
        }
        Ok(DecisionView {
            id: DecisionId::from_uuid(self.id),
            local_number: u64::try_from(self.local_number)
                .map_err(|_| InvalidRow("decision.local_number"))?,
            event_id: EventId::from_uuid(self.event_id),
            wording: match self.wording.as_deref() {
                Some(wording) => Redactable::Present(
                    DecisionWording::parse(wording).map_err(|_| InvalidRow("decision.wording"))?,
                ),
                None => Redactable::Redacted,
            },
            decided_on: self.decided_on.to_jiff(),
            parties,
            fields: DecisionFields {
                owner: UserId::from_uuid(self.owner_user_id),
                workstream_id: self.workstream_id.map(WorkstreamId::from_uuid),
            },
            state: self.state()?,
            version: RecordVersion::new(self.version).ok_or(InvalidRow("decision.version"))?,
        })
    }
}

/// The decisions of `select` with their parties: one query for the decisions, one for all their parties.
async fn select_decisions(
    conn: &mut PgConnection,
    scope: OrgScope,
    select: &Select,
) -> Result<Vec<DecisionView>, sqlx::Error> {
    let organization = scope.organization_id().as_uuid();
    let rows = sqlx::query_as!(
        DecisionRow,
        r#"SELECT id, event_id, local_number, wording, decided_on AS "decided_on: jiff_sqlx::Date",
                  owner_user_id, workstream_id, status, superseded_by, revoked_reason, version
           FROM decision
           WHERE organization_id = $1
             AND ($2::uuid IS NULL OR event_id = $2)
             AND ($3::uuid IS NULL OR id = $3)
             AND ($4::uuid IS NULL OR owner_user_id = $4)
             AND ($5::text IS NULL OR status = $5)
             AND ($6::uuid IS NULL OR workstream_id = $6)
             AND local_number > $7
           ORDER BY event_id, local_number
           LIMIT $8"#,
        organization,
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
    let ids: Vec<Uuid> = rows.iter().map(|row| row.id).collect();
    let party_rows = sqlx::query!(
        r#"SELECT d.decision_id, d.person_id, d.institution_id,
                  COALESCE(p.local_number, i.local_number) AS "number?",
                  COALESCE(p.name, i.name) AS "name?"
           FROM decision_party d
           LEFT JOIN person p ON p.organization_id = d.organization_id AND p.id = d.person_id
           LEFT JOIN institution i ON i.organization_id = d.organization_id AND i.id = d.institution_id
           WHERE d.organization_id = $1 AND d.decision_id = ANY($2)
           ORDER BY d.decision_id, d.position"#,
        organization,
        &ids,
    )
    .fetch_all(&mut *conn)
    .await?;
    let mut parties: HashMap<Uuid, Vec<PartyRef>> = HashMap::new();
    for row in party_rows {
        let columns = PartyColumns {
            person_id: row.person_id,
            institution_id: row.institution_id,
            number: row.number,
            name: row.name.as_deref(),
        };
        parties
            .entry(row.decision_id)
            .or_default()
            .push(party_ref(columns, "decision_party")?);
    }
    Ok(rows
        .into_iter()
        .map(|row| {
            let parties = parties.remove(&row.id).unwrap_or_default();
            row.into_view(parties)
        })
        .collect::<Result<_, _>>()?)
}

/// The decision `id` of the event, read inside the transaction of the caller.
pub(crate) async fn decision_in(
    conn: &mut PgConnection,
    scope: OrgScope,
    event: EventId,
    id: DecisionId,
) -> Result<Option<DecisionView>, sqlx::Error> {
    Ok(
        select_decisions(conn, scope, &Select::one(event, id.as_uuid()))
            .await?
            .pop(),
    )
}

/// Inserts a decision in force with the version 1, the next number of its event and its parties, and returns the
/// number. The direct commands and the apply of a proposal all write a new decision here.
pub(crate) async fn insert_decision(
    conn: &mut PgConnection,
    scope: OrgScope,
    decision: &NewDecisionRecord,
    at: Timestamp,
) -> Result<i64, sqlx::Error> {
    let organization = scope.organization_id().as_uuid();
    let number = next_local_number(
        conn,
        scope,
        decision.event_id.as_uuid(),
        LocalIdKind::Decision.prefix(),
    )
    .await?;
    sqlx::query!(
        "INSERT INTO decision
             (id, organization_id, event_id, local_number, wording, decided_on, owner_user_id, workstream_id,
              status, version, created_at, updated_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, 'in-force', 1, $9, $9)",
        decision.id.as_uuid(),
        organization,
        decision.event_id.as_uuid(),
        number,
        decision.wording.as_str(),
        decision.decided_on.to_sqlx() as _,
        decision.fields.owner.as_uuid(),
        decision.fields.workstream_id.map(WorkstreamId::as_uuid),
        at.to_sqlx() as _,
    )
    .execute(&mut *conn)
    .await?;
    let (mut persons, mut institutions) = (Vec::new(), Vec::new());
    for party in decision.parties.as_slice() {
        let (person, institution) = match *party {
            Party::Person(id) => (Some(id.as_uuid()), None),
            Party::Institution(id) => (None, Some(id.as_uuid())),
        };
        persons.push(person);
        institutions.push(institution);
    }
    // The position of each party is its index in the list (`decision_party.position`, from 0).
    sqlx::query!(
        "INSERT INTO decision_party (organization_id, decision_id, position, person_id, institution_id)
         SELECT $1, $2, (t.ordinality - 1)::smallint, t.person_id, t.institution_id
         FROM unnest($3::uuid[], $4::uuid[]) WITH ORDINALITY AS t (person_id, institution_id, ordinality)",
        organization,
        decision.id.as_uuid(),
        &persons as &[Option<Uuid>],
        &institutions as &[Option<Uuid>],
    )
    .execute(&mut *conn)
    .await?;
    Ok(number)
}

/// Replaces the owner and the workstream of a decision if its version is `expected`, and returns the new version.
/// `None` means that the event has no such decision with this version.
pub(crate) async fn update_decision(
    conn: &mut PgConnection,
    scope: OrgScope,
    event: EventId,
    id: DecisionId,
    fields: &DecisionFields,
    expected: RecordVersion,
    at: Timestamp,
) -> Result<Option<i64>, sqlx::Error> {
    sqlx::query_scalar!(
        "UPDATE decision
         SET owner_user_id = $5, workstream_id = $6, version = version + 1, updated_at = $7
         WHERE organization_id = $1 AND event_id = $2 AND id = $3 AND version = $4
         RETURNING version",
        scope.organization_id().as_uuid(),
        event.as_uuid(),
        id.as_uuid(),
        expected.get(),
        fields.owner.as_uuid(),
        fields.workstream_id.map(WorkstreamId::as_uuid),
        at.to_sqlx() as _,
    )
    .fetch_optional(&mut *conn)
    .await
}

/// Ends a decision in force with `change` if its version is `expected`, and returns the new version.
/// `None` means that the event has no such decision in force with this version. A successor of another event breaks
/// the foreign key, and the decision itself breaks `decision_not_superseded_by_itself`.
pub(crate) async fn end_decision(
    conn: &mut PgConnection,
    scope: OrgScope,
    event: EventId,
    id: DecisionId,
    change: &DecisionChange,
    expected: RecordVersion,
    at: Timestamp,
) -> Result<Option<i64>, sqlx::Error> {
    let (successor, reason) = match change {
        DecisionChange::Supersede { successor } => (Some(successor.as_uuid()), None),
        DecisionChange::Revoke { reason } => (None, Some(reason.as_str())),
    };
    sqlx::query_scalar!(
        "UPDATE decision
         SET status = $5, superseded_by = $6, revoked_reason = $7, version = version + 1, updated_at = $8
         WHERE organization_id = $1 AND event_id = $2 AND id = $3 AND version = $4 AND status = 'in-force'
         RETURNING version",
        scope.organization_id().as_uuid(),
        event.as_uuid(),
        id.as_uuid(),
        expected.get(),
        change.status().as_str(),
        successor,
        reason,
        at.to_sqlx() as _,
    )
    .fetch_optional(&mut *conn)
    .await
}

/// The result of a change that did not update a row: an unknown decision, or another version or status.
async fn not_changed<T>(
    conn: &mut PgConnection,
    scope: OrgScope,
    event: EventId,
    id: DecisionId,
) -> Result<Changed<T>, StoreError> {
    let exists = decision_in(conn, scope, event, id)
        .await
        .map_err(store_error)?
        .is_some();
    Ok(if exists {
        Changed::VersionConflict
    } else {
        Changed::NotFound
    })
}

#[async_trait]
impl DecisionStore for Database {
    async fn create_decision(
        &self,
        scope: OrgScope,
        decision: &NewDecisionRecord,
        at: Timestamp,
        audit: &AuditEvent,
    ) -> Result<Created<DecisionView>, StoreError> {
        let mut tx = self.pool.begin().await.map_err(store_error)?;
        match insert_decision(&mut tx, scope, decision, at).await {
            Ok(_) => {}
            Err(error) if violates(&error, "decision_pkey") => return Ok(Created::IdTaken),
            Err(error) => return Err(store_error(error)),
        }
        audit::record(&mut tx, audit).await.map_err(store_error)?;
        let view = decision_in(&mut tx, scope, decision.event_id, decision.id)
            .await
            .map_err(store_error)?
            .ok_or(InvalidRow("decision"))?;
        tx.commit().await.map_err(store_error)?;
        Ok(Created::Created(view))
    }

    async fn change_decision(
        &self,
        scope: OrgScope,
        event: EventId,
        id: DecisionId,
        fields: &DecisionFields,
        expected: RecordVersion,
        at: Timestamp,
        audit: &AuditEvent,
    ) -> Result<Changed<DecisionView>, StoreError> {
        let mut tx = self.pool.begin().await.map_err(store_error)?;
        let updated = update_decision(&mut tx, scope, event, id, fields, expected, at)
            .await
            .map_err(store_error)?;
        if updated.is_none() {
            return not_changed(&mut tx, scope, event, id).await;
        }
        audit::record(&mut tx, audit).await.map_err(store_error)?;
        let view = decision_in(&mut tx, scope, event, id)
            .await
            .map_err(store_error)?
            .ok_or(InvalidRow("decision"))?;
        tx.commit().await.map_err(store_error)?;
        Ok(Changed::Changed(view))
    }

    async fn supersede_decision(
        &self,
        scope: OrgScope,
        event: EventId,
        old: DecisionId,
        successor: &NewDecisionRecord,
        expected: RecordVersion,
        at: Timestamp,
        audit: &[AuditEvent],
    ) -> Result<Created<Changed<Superseded>>, StoreError> {
        let mut tx = self.pool.begin().await.map_err(store_error)?;
        // The successor comes first: the end of the old decision refers to it.
        match insert_decision(&mut tx, scope, successor, at).await {
            Ok(_) => {}
            Err(error) if violates(&error, "decision_pkey") => return Ok(Created::IdTaken),
            Err(error) => return Err(store_error(error)),
        }
        let change = DecisionChange::Supersede {
            successor: successor.id,
        };
        let ended = end_decision(&mut tx, scope, event, old, &change, expected, at)
            .await
            .map_err(store_error)?;
        if ended.is_none() {
            // The rollback at the drop of the transaction removes the successor too.
            return Ok(Created::Created(
                not_changed(&mut tx, scope, event, old).await?,
            ));
        }
        for entry in audit {
            audit::record(&mut tx, entry).await.map_err(store_error)?;
        }
        let old = decision_in(&mut tx, scope, event, old)
            .await
            .map_err(store_error)?
            .ok_or(InvalidRow("decision"))?;
        let successor = decision_in(&mut tx, scope, event, successor.id)
            .await
            .map_err(store_error)?
            .ok_or(InvalidRow("decision"))?;
        tx.commit().await.map_err(store_error)?;
        Ok(Created::Created(Changed::Changed(Superseded {
            old,
            successor,
        })))
    }

    async fn revoke_decision(
        &self,
        scope: OrgScope,
        event: EventId,
        id: DecisionId,
        reason: &RecordReason,
        expected: RecordVersion,
        at: Timestamp,
        audit: &AuditEvent,
    ) -> Result<Changed<DecisionView>, StoreError> {
        let mut tx = self.pool.begin().await.map_err(store_error)?;
        let change = DecisionChange::Revoke {
            reason: reason.clone(),
        };
        let ended = end_decision(&mut tx, scope, event, id, &change, expected, at)
            .await
            .map_err(store_error)?;
        if ended.is_none() {
            return not_changed(&mut tx, scope, event, id).await;
        }
        audit::record(&mut tx, audit).await.map_err(store_error)?;
        let view = decision_in(&mut tx, scope, event, id)
            .await
            .map_err(store_error)?
            .ok_or(InvalidRow("decision"))?;
        tx.commit().await.map_err(store_error)?;
        Ok(Changed::Changed(view))
    }

    async fn decision(
        &self,
        scope: OrgScope,
        event: EventId,
        id: DecisionId,
    ) -> Result<Option<DecisionView>, StoreError> {
        let mut conn = self.pool.acquire().await.map_err(store_error)?;
        decision_in(&mut conn, scope, event, id)
            .await
            .map_err(store_error)
    }

    async fn decisions(
        &self,
        scope: OrgScope,
        event: EventId,
        filter: &WorkFilter<DecisionStatus>,
    ) -> Result<Vec<DecisionView>, StoreError> {
        let mut conn = self.pool.acquire().await.map_err(store_error)?;
        let status = filter.status.map(DecisionStatus::as_str);
        select_decisions(&mut conn, scope, &Select::filter(event, filter, status))
            .await
            .map_err(store_error)
    }
}
