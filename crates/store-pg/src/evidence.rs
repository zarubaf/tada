//! The evidence of records (ADR 0068, ADR 0069, ADR 0081): the passages that support each accepted version of an
//! action, a commitment, a person, an institution, a milestone, a decision or a risk. The apply writes them; each record read reads them
//! here. `record_evidence` has one column for each kind of record, and exactly one of them is set.

use async_trait::async_trait;
use sqlx::PgConnection;
use sqlx::types::Uuid;
use tada_app::access::SourceReach;
use tada_app::caller::OrgScope;
use tada_app::domain::RecordVersion;
use tada_app::domain::ids::{
    ActionId, CommitmentId, DecisionId, InstitutionId, MilestoneId, PersonId, ProposalId, RiskId,
    SourceVersionId,
};
use tada_app::domain::records::RecordKind;
use tada_app::domain::sources::Evidence;
use tada_app::records::{EvidenceStore, RecordEvidenceView, RecordRef};
use tada_app::store::StoreError;

use crate::error::{InvalidRow, store_error};
use crate::sources::readable_source_ids;
use crate::{Database, redacted};

/// The IDs of the records of `kind` in `records`, for the column or the table of that kind.
pub(crate) fn ids_of(records: &[RecordRef], kind: RecordKind) -> Vec<Uuid> {
    records
        .iter()
        .filter(|record| record.kind() == kind)
        .map(|record| record.as_uuid())
        .collect()
}

struct EvidenceRow {
    action_id: Option<Uuid>,
    commitment_id: Option<Uuid>,
    person_id: Option<Uuid>,
    institution_id: Option<Uuid>,
    milestone_id: Option<Uuid>,
    decision_id: Option<Uuid>,
    risk_id: Option<Uuid>,
    record_version: i64,
    proposal_id: Uuid,
    source_version_id: Uuid,
    captured_at: jiff_sqlx::Timestamp,
    start_offset: i32,
    end_offset: i32,
    quote: Option<String>,
    page: Option<i32>,
}

impl EvidenceRow {
    /// The one record of the row. A row with no record or with two breaks the `CHECK` of the table.
    fn record(&self) -> Result<RecordRef, InvalidRow> {
        let set = [
            self.action_id
                .map(|id| RecordRef::Action(ActionId::from_uuid(id))),
            self.commitment_id
                .map(|id| RecordRef::Commitment(CommitmentId::from_uuid(id))),
            self.person_id
                .map(|id| RecordRef::Person(PersonId::from_uuid(id))),
            self.institution_id
                .map(|id| RecordRef::Institution(InstitutionId::from_uuid(id))),
            self.milestone_id
                .map(|id| RecordRef::Milestone(MilestoneId::from_uuid(id))),
            self.decision_id
                .map(|id| RecordRef::Decision(DecisionId::from_uuid(id))),
            self.risk_id
                .map(|id| RecordRef::Risk(RiskId::from_uuid(id))),
        ];
        match set.into_iter().flatten().collect::<Vec<_>>()[..] {
            [record] => Ok(record),
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
            quote: redacted::content(row.quote),
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
        let ids = |kind| ids_of(records, kind);
        let (actions, commitments) = (ids(RecordKind::Action), ids(RecordKind::Commitment));
        let (persons, institutions) = (ids(RecordKind::Person), ids(RecordKind::Institution));
        let (milestones, decisions) = (ids(RecordKind::Milestone), ids(RecordKind::Decision));
        let risks = ids(RecordKind::Risk);
        let organization = scope.organization_id().as_uuid();
        // A person or an institution belongs to the organization, but the source of its evidence belongs to an event:
        // the caller sees only the passages of the sources that it can read (ADR 0050). The filter comes before the
        // read of the quotes: first the cited source versions, then the readable ones of them, then the passages.
        let cited = sqlx::query_scalar!(
            "SELECT DISTINCT source_version_id FROM record_evidence
             WHERE organization_id = $1
               AND (action_id = ANY($2) OR commitment_id = ANY($3)
                    OR person_id = ANY($4) OR institution_id = ANY($5) OR milestone_id = ANY($6)
                    OR decision_id = ANY($7) OR risk_id = ANY($8))",
            organization,
            &actions,
            &commitments,
            &persons,
            &institutions,
            &milestones,
            &decisions,
            &risks,
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
            r#"SELECT e.action_id, e.commitment_id, e.person_id, e.institution_id, e.milestone_id,
                      e.decision_id, e.risk_id, e.record_version,
                      e.proposal_id, e.source_version_id,
                      v.captured_at AS "captured_at: jiff_sqlx::Timestamp",
                      e.start_offset, e.end_offset, e.quote, e.page
               FROM record_evidence e
               JOIN source_version v
                   ON v.organization_id = e.organization_id AND v.id = e.source_version_id
               WHERE e.organization_id = $1
                 AND (e.action_id = ANY($2) OR e.commitment_id = ANY($3)
                      OR e.person_id = ANY($4) OR e.institution_id = ANY($5) OR e.milestone_id = ANY($6)
                      OR e.decision_id = ANY($7) OR e.risk_id = ANY($8))
                 AND e.source_version_id = ANY($9)
               ORDER BY e.record_version, e.start_offset, e.id"#,
            organization,
            &actions,
            &commitments,
            &persons,
            &institutions,
            &milestones,
            &decisions,
            &risks,
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
    let offset =
        |value: u32| i32::try_from(value).map_err(|error| sqlx::Error::Encode(Box::new(error)));
    // The value of one record column: the ID if the record has the kind of the column.
    let column = |kind| (record.kind() == kind).then(|| record.as_uuid());
    for Evidence {
        source_version_id,
        passage,
    } in evidence
    {
        sqlx::query!(
            "INSERT INTO record_evidence
                 (id, organization_id, action_id, commitment_id, person_id, institution_id, milestone_id,
                  decision_id, risk_id, record_version, proposal_id, source_version_id, start_offset, end_offset,
                  quote, page, redacted_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16,
                     CASE WHEN $15::text IS NULL THEN now() END)",
            Uuid::now_v7(),
            scope.organization_id().as_uuid(),
            column(RecordKind::Action),
            column(RecordKind::Commitment),
            column(RecordKind::Person),
            column(RecordKind::Institution),
            column(RecordKind::Milestone),
            column(RecordKind::Decision),
            column(RecordKind::Risk),
            version,
            proposal.as_uuid(),
            source_version_id.as_uuid(),
            offset(passage.start)?,
            offset(passage.end)?,
            // A quote that a legal redaction removed stays a tombstone in its copy (ADR 0072).
            passage.quote.as_present().map(String::as_str),
            passage.page.map(offset).transpose()?,
        )
        .execute(&mut *conn)
        .await?;
    }
    Ok(())
}
