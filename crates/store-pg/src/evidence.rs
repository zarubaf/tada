//! The evidence of records (ADR 0068, ADR 0069): the passages that support each accepted version of an action,
//! a commitment, a person or an institution. The apply writes them; each record read reads them here.

use async_trait::async_trait;
use sqlx::PgConnection;
use sqlx::types::Uuid;
use tada_app::access::SourceReach;
use tada_app::caller::OrgScope;
use tada_app::domain::RecordVersion;
use tada_app::domain::ids::{
    ActionId, CommitmentId, InstitutionId, PersonId, ProposalId, SourceVersionId,
};
use tada_app::domain::sources::Evidence;
use tada_app::records::{EvidenceStore, RecordEvidenceView, RecordRef};
use tada_app::store::StoreError;

use crate::Database;
use crate::error::{InvalidRow, store_error};
use crate::sources::readable_source_ids;

struct EvidenceRow {
    action_id: Option<Uuid>,
    commitment_id: Option<Uuid>,
    person_id: Option<Uuid>,
    institution_id: Option<Uuid>,
    record_version: i64,
    proposal_id: Uuid,
    source_version_id: Uuid,
    captured_at: jiff_sqlx::Timestamp,
    start_offset: i32,
    end_offset: i32,
    quote: String,
    page: Option<i32>,
}

impl EvidenceRow {
    fn record(&self) -> Result<RecordRef, InvalidRow> {
        match (
            self.action_id,
            self.commitment_id,
            self.person_id,
            self.institution_id,
        ) {
            (Some(id), None, None, None) => Ok(RecordRef::Action(ActionId::from_uuid(id))),
            (None, Some(id), None, None) => Ok(RecordRef::Commitment(CommitmentId::from_uuid(id))),
            (None, None, Some(id), None) => Ok(RecordRef::Person(PersonId::from_uuid(id))),
            (None, None, None, Some(id)) => {
                Ok(RecordRef::Institution(InstitutionId::from_uuid(id)))
            }
            _ => Err(InvalidRow("record_evidence.record")),
        }
    }
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

#[async_trait]
impl EvidenceStore for Database {
    async fn evidence_of(
        &self,
        scope: OrgScope,
        records: &[RecordRef],
        reach: &SourceReach,
    ) -> Result<Vec<(RecordRef, RecordEvidenceView)>, StoreError> {
        let ids = |kind: fn(&RecordRef) -> Option<Uuid>| -> Vec<Uuid> {
            records.iter().filter_map(kind).collect()
        };
        let actions = ids(|record| match record {
            RecordRef::Action(id) => Some(id.as_uuid()),
            _ => None,
        });
        let commitments = ids(|record| match record {
            RecordRef::Commitment(id) => Some(id.as_uuid()),
            _ => None,
        });
        let persons = ids(|record| match record {
            RecordRef::Person(id) => Some(id.as_uuid()),
            _ => None,
        });
        let institutions = ids(|record| match record {
            RecordRef::Institution(id) => Some(id.as_uuid()),
            _ => None,
        });
        let organization = scope.organization_id().as_uuid();
        // A person or an institution belongs to the organization, but the source of its evidence belongs to an event:
        // the caller sees only the passages of the sources that it can read (ADR 0050). The filter comes before the
        // read of the quotes: first the cited source versions, then the readable ones of them, then the passages.
        let cited = sqlx::query_scalar!(
            "SELECT DISTINCT source_version_id FROM record_evidence
             WHERE organization_id = $1
               AND (action_id = ANY($2) OR commitment_id = ANY($3)
                    OR person_id = ANY($4) OR institution_id = ANY($5))",
            organization,
            &actions,
            &commitments,
            &persons,
            &institutions,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(store_error)?;
        if cited.is_empty() {
            return Ok(Vec::new());
        }
        let readable: Vec<Uuid> = readable_source_ids(&self.pool, scope, reach, &cited)
            .await?
            .into_iter()
            .collect();
        let rows = sqlx::query_as!(
            EvidenceRow,
            r#"SELECT e.action_id, e.commitment_id, e.person_id, e.institution_id, e.record_version,
                      e.proposal_id, e.source_version_id,
                      v.captured_at AS "captured_at: jiff_sqlx::Timestamp",
                      e.start_offset, e.end_offset, e.quote, e.page
               FROM record_evidence e
               JOIN source_version v
                   ON v.organization_id = e.organization_id AND v.id = e.source_version_id
               WHERE e.organization_id = $1
                 AND (e.action_id = ANY($2) OR e.commitment_id = ANY($3)
                      OR e.person_id = ANY($4) OR e.institution_id = ANY($5))
                 AND e.source_version_id = ANY($6)
               ORDER BY e.record_version, e.start_offset, e.id"#,
            organization,
            &actions,
            &commitments,
            &persons,
            &institutions,
            &readable,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(store_error)?;
        rows.into_iter()
            .map(|row| Ok((row.record()?, RecordEvidenceView::try_from(row)?)))
            .collect()
    }
}

/// One row of `record_evidence` for each passage, with the record version that the step produced.
pub(crate) async fn insert_evidence(
    conn: &mut PgConnection,
    scope: OrgScope,
    record: RecordRef,
    version: i64,
    proposal: ProposalId,
    evidence: &[Evidence],
) -> Result<(), sqlx::Error> {
    let (action, commitment, person, institution) = match record {
        RecordRef::Action(id) => (Some(id.as_uuid()), None, None, None),
        RecordRef::Commitment(id) => (None, Some(id.as_uuid()), None, None),
        RecordRef::Person(id) => (None, None, Some(id.as_uuid()), None),
        RecordRef::Institution(id) => (None, None, None, Some(id.as_uuid())),
    };
    let offset =
        |value: u32| i32::try_from(value).map_err(|error| sqlx::Error::Encode(Box::new(error)));
    for Evidence {
        source_version_id,
        passage,
    } in evidence
    {
        sqlx::query!(
            "INSERT INTO record_evidence
                 (id, organization_id, action_id, commitment_id, person_id, institution_id, record_version,
                  proposal_id, source_version_id, start_offset, end_offset, quote, page)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)",
            Uuid::now_v7(),
            scope.organization_id().as_uuid(),
            action,
            commitment,
            person,
            institution,
            version,
            proposal.as_uuid(),
            source_version_id.as_uuid(),
            offset(passage.start)?,
            offset(passage.end)?,
            passage.quote,
            passage.page.map(offset).transpose()?,
        )
        .execute(&mut *conn)
        .await?;
    }
    Ok(())
}
