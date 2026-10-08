//! The `ProposalStore` adapter (ADR 0050): changesets, proposals, their dependencies and evidence,
//! and the open proposals and open questions of the event profile.
//!
//! The stored form of an operation is the `OperationRecord` of this code module, in the format `OPERATION_VERSION`.
//! A change of the format needs a new version, and this codec must read each older version.

use async_trait::async_trait;
use jiff_sqlx::ToSqlx;
use serde::{Deserialize, Serialize};
use sqlx::types::Uuid;
use sqlx::{PgConnection, PgPool};
use tada_app::audit::AuditEvent;
use tada_app::caller::OrgScope;
use tada_app::domain::RecordVersion;
use tada_app::domain::documents::{DocumentName, DraftMarkdown};
use tada_app::domain::events::{EventKey, EventName, EventTimeZone};
use tada_app::domain::facts::{ChoiceKey, Description, FieldKey, ModuleKey, ShortText};
use tada_app::domain::ids::{
    ChangesetId, DocumentId, EventId, FieldDefinitionId, OpenQuestionId, ProposalId,
    SourceVersionId, UserId,
};
use tada_app::domain::proposals::{DraftDocument, Operation, Proposal, QuestionText, Reason};
use tada_app::domain::sources::{Passage, SourceText};
use tada_app::drafts::DraftProvenance;
use tada_app::facts::{OpenProposalRef, OpenQuestionRef};
use tada_app::proposals::{Changeset, Inserted, ProposalStore};
use tada_app::store::StoreError;

use crate::Database;
use crate::error::{InvalidRow, store_error};
use crate::{actor, audit, drafts, sources, values};

/// The format of `proposal.operation`.
const OPERATION_VERSION: i32 = 1;

const OPERATION: &str = "proposal.operation";

#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum OperationRecord {
    CreateEvent {
        id: Uuid,
        key: String,
        name: String,
        time_zone: String,
    },
    SetFact {
        event_id: Uuid,
        field_id: Uuid,
        state: String,
        value: Option<serde_json::Value>,
        approximate: bool,
        expected_version: Option<i64>,
    },
    AddFieldDefinition {
        id: Uuid,
        event_id: Uuid,
        key: String,
        label: String,
        value_type: serde_json::Value,
        description: String,
        module: String,
    },
    AddChoiceValue {
        event_id: Uuid,
        field_id: Uuid,
        key: String,
        label: String,
    },
    DeprecateField {
        event_id: Uuid,
        field_id: Uuid,
    },
    CreateOpenQuestion {
        id: Uuid,
        event_id: Uuid,
        text: String,
        owner: Uuid,
    },
    CreateDocumentDraft {
        event_id: Uuid,
        document: DocumentRecord,
        markdown: String,
    },
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum DocumentRecord {
    New {
        id: Uuid,
        name: String,
    },
    Existing {
        document_id: Uuid,
        expected_version: i64,
    },
}

pub(crate) fn operation_to_json(operation: &Operation) -> serde_json::Value {
    let record = match operation {
        Operation::CreateEvent {
            id,
            key,
            name,
            time_zone,
        } => OperationRecord::CreateEvent {
            id: id.as_uuid(),
            key: key.as_str().to_owned(),
            name: name.as_str().to_owned(),
            time_zone: time_zone.as_str().to_owned(),
        },
        Operation::SetFact {
            event_id,
            field_id,
            state,
            expected_version,
        } => {
            let (state, value, approximate) = values::fact_state_to_columns(state);
            OperationRecord::SetFact {
                event_id: event_id.as_uuid(),
                field_id: field_id.as_uuid(),
                state: state.to_owned(),
                value,
                approximate,
                expected_version: expected_version.map(RecordVersion::get),
            }
        }
        Operation::AddFieldDefinition {
            id,
            event_id,
            key,
            label,
            value_type,
            description,
            module,
        } => OperationRecord::AddFieldDefinition {
            id: id.as_uuid(),
            event_id: event_id.as_uuid(),
            key: key.as_str().to_owned(),
            label: label.as_str().to_owned(),
            value_type: values::value_type_to_json(value_type),
            description: description.as_str().to_owned(),
            module: module.as_str().to_owned(),
        },
        Operation::AddChoiceValue {
            event_id,
            field_id,
            key,
            label,
        } => OperationRecord::AddChoiceValue {
            event_id: event_id.as_uuid(),
            field_id: field_id.as_uuid(),
            key: key.as_str().to_owned(),
            label: label.as_str().to_owned(),
        },
        Operation::DeprecateField { event_id, field_id } => OperationRecord::DeprecateField {
            event_id: event_id.as_uuid(),
            field_id: field_id.as_uuid(),
        },
        Operation::CreateOpenQuestion {
            id,
            event_id,
            text,
            owner,
        } => OperationRecord::CreateOpenQuestion {
            id: id.as_uuid(),
            event_id: event_id.as_uuid(),
            text: text.as_str().to_owned(),
            owner: owner.as_uuid(),
        },
        Operation::CreateDocumentDraft {
            event_id,
            document,
            markdown,
        } => OperationRecord::CreateDocumentDraft {
            event_id: event_id.as_uuid(),
            document: match document {
                DraftDocument::New { id, name } => DocumentRecord::New {
                    id: id.as_uuid(),
                    name: name.as_str().to_owned(),
                },
                DraftDocument::Existing {
                    document_id,
                    expected_version,
                } => DocumentRecord::Existing {
                    document_id: document_id.as_uuid(),
                    expected_version: expected_version.get(),
                },
            },
            markdown: markdown.as_str().to_owned(),
        },
    };
    serde_json::to_value(record).expect("an operation record is valid JSON")
}

pub(crate) fn operation_from_json(
    version: i32,
    json: &serde_json::Value,
) -> Result<Operation, InvalidRow> {
    if version != OPERATION_VERSION {
        return Err(InvalidRow("proposal.operation_version"));
    }
    let invalid = || InvalidRow(OPERATION);
    let record = OperationRecord::deserialize(json).map_err(|_| invalid())?;
    Ok(match record {
        OperationRecord::CreateEvent {
            id,
            key,
            name,
            time_zone,
        } => Operation::CreateEvent {
            id: EventId::from_uuid(id),
            key: EventKey::parse(&key).map_err(|_| invalid())?,
            name: EventName::parse(&name).map_err(|_| invalid())?,
            time_zone: EventTimeZone::parse(&time_zone).map_err(|_| invalid())?,
        },
        OperationRecord::SetFact {
            event_id,
            field_id,
            state,
            value,
            approximate,
            expected_version,
        } => Operation::SetFact {
            event_id: EventId::from_uuid(event_id),
            field_id: FieldDefinitionId::from_uuid(field_id),
            state: values::fact_state_from_columns(&state, value, approximate)
                .map_err(|_| invalid())?,
            expected_version: expected_version
                .map(|number| RecordVersion::new(number).ok_or_else(invalid))
                .transpose()?,
        },
        OperationRecord::AddFieldDefinition {
            id,
            event_id,
            key,
            label,
            value_type,
            description,
            module,
        } => Operation::AddFieldDefinition {
            id: FieldDefinitionId::from_uuid(id),
            event_id: EventId::from_uuid(event_id),
            key: FieldKey::parse(&key).map_err(|_| invalid())?,
            label: ShortText::parse(&label).map_err(|_| invalid())?,
            value_type: values::value_type_from_json(&value_type).map_err(|_| invalid())?,
            description: Description::parse(&description).map_err(|_| invalid())?,
            module: ModuleKey::parse(&module).map_err(|_| invalid())?,
        },
        OperationRecord::AddChoiceValue {
            event_id,
            field_id,
            key,
            label,
        } => Operation::AddChoiceValue {
            event_id: EventId::from_uuid(event_id),
            field_id: FieldDefinitionId::from_uuid(field_id),
            key: ChoiceKey::parse(&key).map_err(|_| invalid())?,
            label: ShortText::parse(&label).map_err(|_| invalid())?,
        },
        OperationRecord::DeprecateField { event_id, field_id } => Operation::DeprecateField {
            event_id: EventId::from_uuid(event_id),
            field_id: FieldDefinitionId::from_uuid(field_id),
        },
        OperationRecord::CreateOpenQuestion {
            id,
            event_id,
            text,
            owner,
        } => Operation::CreateOpenQuestion {
            id: OpenQuestionId::from_uuid(id),
            event_id: EventId::from_uuid(event_id),
            text: QuestionText::parse(&text).map_err(|_| invalid())?,
            owner: UserId::from_uuid(owner),
        },
        OperationRecord::CreateDocumentDraft {
            event_id,
            document,
            markdown,
        } => Operation::CreateDocumentDraft {
            event_id: EventId::from_uuid(event_id),
            document: match document {
                DocumentRecord::New { id, name } => DraftDocument::New {
                    id: DocumentId::from_uuid(id),
                    name: DocumentName::parse(&name).map_err(|_| invalid())?,
                },
                DocumentRecord::Existing {
                    document_id,
                    expected_version,
                } => DraftDocument::Existing {
                    document_id: DocumentId::from_uuid(document_id),
                    expected_version: RecordVersion::new(expected_version).ok_or_else(invalid)?,
                },
            },
            markdown: DraftMarkdown::parse(&markdown).map_err(|_| invalid())?,
        },
    })
}

/// The columns `target_kind`, `target_id` and `expected_version` of a proposal.
/// A fact has no ID before its first version, so the target of a fact is its field.
fn target(operation: &Operation) -> (&'static str, Uuid, Option<i64>) {
    match operation {
        Operation::CreateEvent { id, .. } => ("event", id.as_uuid(), None),
        Operation::SetFact {
            field_id,
            expected_version,
            ..
        } => (
            "fact",
            field_id.as_uuid(),
            expected_version.map(RecordVersion::get),
        ),
        Operation::AddFieldDefinition { id, .. } => ("field_definition", id.as_uuid(), None),
        Operation::AddChoiceValue { field_id, .. } | Operation::DeprecateField { field_id, .. } => {
            ("field_definition", field_id.as_uuid(), None)
        }
        Operation::CreateOpenQuestion { id, .. } => ("open_question", id.as_uuid(), None),
        Operation::CreateDocumentDraft { document, .. } => match document {
            DraftDocument::New { id, .. } => ("document", id.as_uuid(), None),
            DraftDocument::Existing {
                document_id,
                expected_version,
            } => (
                "document",
                document_id.as_uuid(),
                Some(expected_version.get()),
            ),
        },
    }
}

#[async_trait]
impl ProposalStore for Database {
    async fn taken_ids(&self, ids: &[Uuid]) -> Result<Vec<Uuid>, StoreError> {
        // A cross-organization uniqueness check (ADR 0038): IDs are unique in the whole installation.
        // It returns only the given IDs that exist, and never an organization.
        // The new record of a proposal reserves its ID: else a second changeset could propose the same record,
        // and only one of the two could apply. The target of a fact is its field, which is not a new record.
        // The target of a draft for an existing document is that document, which exists already.
        sqlx::query_scalar!(
            r#"SELECT id AS "id!" FROM changeset WHERE id = ANY($1)
               UNION SELECT id FROM proposal WHERE id = ANY($1)
               UNION SELECT target_id FROM proposal WHERE target_id = ANY($1) AND target_kind <> 'fact'
               UNION SELECT id FROM event WHERE id = ANY($1)
               UNION SELECT id FROM field_definition WHERE id = ANY($1)
               UNION SELECT id FROM open_question WHERE id = ANY($1)
               UNION SELECT id FROM document WHERE id = ANY($1)"#,
            ids,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(store_error)
    }

    async fn insert(
        &self,
        scope: OrgScope,
        changeset: &Changeset,
        source: &SourceText,
        audit: &AuditEvent,
    ) -> Result<Inserted, StoreError> {
        let mut tx = self.pool.begin().await.map_err(store_error)?;
        let item = sources::TextItem {
            kind: sources::TextKind::MemberText,
            event: changeset.event_id,
            version: changeset.source_version_id,
        };
        sources::insert_text(
            &mut tx,
            scope,
            item,
            source,
            &changeset.author,
            changeset.created_at,
        )
        .await
        .map_err(store_error)?;
        match insert_changeset(&mut tx, scope, changeset).await {
            Ok(()) => {}
            Err(sqlx::Error::Database(error))
                if matches!(error.constraint(), Some("changeset_pkey" | "proposal_pkey")) =>
            {
                return Ok(Inserted::IdTaken);
            }
            Err(error) => return Err(store_error(error)),
        }
        audit::record(&mut tx, audit).await.map_err(store_error)?;
        tx.commit().await.map_err(store_error)?;
        Ok(Inserted::Inserted)
    }

    async fn get(
        &self,
        scope: OrgScope,
        id: ChangesetId,
    ) -> Result<Option<(Changeset, SourceText)>, StoreError> {
        let organization = scope.organization_id().as_uuid();
        let Some(changeset) = sqlx::query!(
            r#"SELECT c.event_id, c.author, c.source_version_id,
                      c.created_at AS "created_at: jiff_sqlx::Timestamp", v.text AS "text!"
               FROM changeset c
               JOIN source_version v ON v.organization_id = c.organization_id AND v.id = c.source_version_id
               WHERE c.organization_id = $1 AND c.id = $2"#,
            organization,
            id.as_uuid(),
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(store_error)?
        else {
            return Ok(None);
        };
        let rows = sqlx::query!(
            "SELECT id, operation, operation_version, reason, manifest, lint_warnings
             FROM proposal
             WHERE organization_id = $1 AND changeset_id = $2
             ORDER BY id",
            organization,
            id.as_uuid(),
        )
        .fetch_all(&self.pool)
        .await
        .map_err(store_error)?;
        let dependencies = sqlx::query!(
            "SELECT proposal_id, depends_on
             FROM proposal_dependency
             WHERE organization_id = $1 AND changeset_id = $2
             ORDER BY proposal_id, depends_on",
            organization,
            id.as_uuid(),
        )
        .fetch_all(&self.pool)
        .await
        .map_err(store_error)?;
        // The evidence IDs are UUIDv7 in the order of the insert, so this keeps the order of the passages.
        let evidence = sqlx::query!(
            "SELECT e.proposal_id, e.start_offset, e.end_offset, e.quote, e.page
             FROM proposal_evidence e
             JOIN proposal p ON p.organization_id = e.organization_id AND p.id = e.proposal_id
             WHERE e.organization_id = $1 AND p.changeset_id = $2
             ORDER BY e.id",
            organization,
            id.as_uuid(),
        )
        .fetch_all(&self.pool)
        .await
        .map_err(store_error)?;
        let offset = |value: i32| u32::try_from(value).map_err(|_| InvalidRow("proposal_evidence"));
        let mut proposals = Vec::new();
        let mut draft_provenance = Vec::new();
        for row in rows {
            if let (Some(manifest), Some(lint_warnings)) = (&row.manifest, &row.lint_warnings) {
                draft_provenance.push(DraftProvenance {
                    proposal_id: ProposalId::from_uuid(row.id),
                    manifest: drafts::manifest_from_json(manifest)?,
                    lint_warnings: drafts::lint_from_json(lint_warnings)?,
                });
            }
            proposals.push(Proposal {
                id: ProposalId::from_uuid(row.id),
                operation: operation_from_json(row.operation_version, &row.operation)?,
                depends_on: dependencies
                    .iter()
                    .filter(|dependency| dependency.proposal_id == row.id)
                    .map(|dependency| ProposalId::from_uuid(dependency.depends_on))
                    .collect(),
                evidence: evidence
                    .iter()
                    .filter(|passage| passage.proposal_id == row.id)
                    .map(|passage| {
                        Ok(Passage {
                            start: offset(passage.start_offset)?,
                            end: offset(passage.end_offset)?,
                            quote: passage.quote.clone(),
                            page: passage.page.map(offset).transpose()?,
                        })
                    })
                    .collect::<Result<_, InvalidRow>>()?,
                reason: Reason::parse(&row.reason).map_err(|_| InvalidRow("proposal.reason"))?,
            });
        }
        Ok(Some((
            Changeset {
                id,
                event_id: changeset.event_id.map(EventId::from_uuid),
                author: actor::from_json(&changeset.author)?,
                source_version_id: SourceVersionId::from_uuid(changeset.source_version_id),
                created_at: changeset.created_at.to_jiff(),
                proposals,
                drafts: draft_provenance,
            },
            SourceText::normalize(&changeset.text),
        )))
    }
}

async fn insert_changeset(
    conn: &mut PgConnection,
    scope: OrgScope,
    changeset: &Changeset,
) -> Result<(), sqlx::Error> {
    let organization = scope.organization_id().as_uuid();
    let created_at = changeset.created_at.to_sqlx();
    sqlx::query!(
        "INSERT INTO changeset (id, organization_id, event_id, author, source_version_id, created_at)
         VALUES ($1, $2, $3, $4, $5, $6)",
        changeset.id.as_uuid(),
        organization,
        changeset.event_id.map(EventId::as_uuid),
        actor::to_json(&changeset.author),
        changeset.source_version_id.as_uuid(),
        created_at as _,
    )
    .execute(&mut *conn)
    .await?;
    for proposal in &changeset.proposals {
        let (target_kind, target_id, expected_version) = target(&proposal.operation);
        let draft = changeset
            .drafts
            .iter()
            .find(|draft| draft.proposal_id == proposal.id);
        sqlx::query!(
            "INSERT INTO proposal (id, organization_id, changeset_id, event_id, operation, operation_version,
                                   target_kind, target_id, expected_version, reason, created_at,
                                   manifest, lint_warnings)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)",
            proposal.id.as_uuid(),
            organization,
            changeset.id.as_uuid(),
            proposal.operation.event_id().as_uuid(),
            operation_to_json(&proposal.operation),
            OPERATION_VERSION,
            target_kind,
            target_id,
            expected_version,
            proposal.reason.as_str(),
            created_at as _,
            draft.map(|draft| drafts::manifest_to_json(&draft.manifest)),
            draft.map(|draft| drafts::lint_to_json(&draft.lint_warnings)),
        )
        .execute(&mut *conn)
        .await?;
    }
    // The dependencies refer to other proposals of the changeset, so they follow all proposals.
    for proposal in &changeset.proposals {
        for depends_on in &proposal.depends_on {
            sqlx::query!(
                "INSERT INTO proposal_dependency (organization_id, changeset_id, proposal_id, depends_on)
                 VALUES ($1, $2, $3, $4)",
                organization,
                changeset.id.as_uuid(),
                proposal.id.as_uuid(),
                depends_on.as_uuid(),
            )
            .execute(&mut *conn)
            .await?;
        }
        for passage in &proposal.evidence {
            let offset = |value: u32| {
                i32::try_from(value).map_err(|error| sqlx::Error::Encode(Box::new(error)))
            };
            sqlx::query!(
                "INSERT INTO proposal_evidence
                     (id, organization_id, proposal_id, source_version_id, start_offset, end_offset, quote, page)
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
                Uuid::now_v7(),
                organization,
                proposal.id.as_uuid(),
                changeset.source_version_id.as_uuid(),
                offset(passage.start)?,
                offset(passage.end)?,
                passage.quote,
                passage.page.map(offset).transpose()?,
            )
            .execute(&mut *conn)
            .await?;
        }
    }
    Ok(())
}

/// The open fact proposals of an event, oldest first: the proposals without a review result.
pub(crate) async fn open_fact_proposals(
    pool: &PgPool,
    scope: OrgScope,
    event: EventId,
) -> Result<Vec<OpenProposalRef>, StoreError> {
    let rows = sqlx::query!(
        r#"SELECT p.id, p.changeset_id, p.operation, p.operation_version,
                  p.created_at AS "created_at: jiff_sqlx::Timestamp"
           FROM proposal p
           WHERE p.organization_id = $1 AND p.event_id = $2 AND p.target_kind = 'fact'
             AND NOT EXISTS (
                 SELECT 1 FROM review_result r
                 WHERE r.organization_id = p.organization_id AND r.proposal_id = p.id
             )
           ORDER BY p.created_at, p.id"#,
        scope.organization_id().as_uuid(),
        event.as_uuid(),
    )
    .fetch_all(pool)
    .await
    .map_err(store_error)?;
    rows.into_iter()
        .map(|row| {
            let Operation::SetFact {
                field_id,
                state,
                expected_version,
                ..
            } = operation_from_json(row.operation_version, &row.operation)?
            else {
                return Err(InvalidRow(OPERATION).into());
            };
            Ok(OpenProposalRef {
                proposal_id: ProposalId::from_uuid(row.id),
                changeset_id: ChangesetId::from_uuid(row.changeset_id),
                field_id,
                state,
                expected_version,
                created_at: row.created_at.to_jiff(),
            })
        })
        .collect()
}

/// The open questions of an event, in the order of their event-local numbers.
pub(crate) async fn open_questions(
    pool: &PgPool,
    scope: OrgScope,
    event: EventId,
) -> Result<Vec<OpenQuestionRef>, StoreError> {
    let rows = sqlx::query!(
        "SELECT id, local_number, text, owner_user_id, version
         FROM open_question
         WHERE organization_id = $1 AND event_id = $2 AND status = 'open'
         ORDER BY local_number",
        scope.organization_id().as_uuid(),
        event.as_uuid(),
    )
    .fetch_all(pool)
    .await
    .map_err(store_error)?;
    rows.into_iter()
        .map(|row| {
            Ok(OpenQuestionRef {
                id: OpenQuestionId::from_uuid(row.id),
                local_number: u64::try_from(row.local_number)
                    .map_err(|_| InvalidRow("open_question.local_number"))?,
                text: QuestionText::parse(&row.text)
                    .map_err(|_| InvalidRow("open_question.text"))?,
                owner: UserId::from_uuid(row.owner_user_id),
                version: RecordVersion::new(row.version)
                    .ok_or(InvalidRow("open_question.version"))?,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use jiff::Timestamp;
    use serde_json::json;
    use tada_app::caller::{MemberCaller, OrganizationRole};
    use tada_app::clock::Clock;
    use tada_app::domain::facts::{
        DateWindow, Decimal, FactState, FactValue, Granularity, Label, Range, ValueType, Valued,
        core_catalog,
    };
    use tada_app::domain::identity::{DisplayName, Email, EventRole};
    use tada_app::domain::ids::OrganizationId;
    use tada_app::domain::proposals::Proposal;
    use tada_app::facts::FactStore;
    use tada_app::proposals::{
        Created, NewChangeset, ProposeError, ProposeStores, create_changeset,
    };

    use super::*;
    use crate::testing::{TestDatabase, sqlstate};

    const SOURCE: &str = "Das Open Day findet im Mai 2030 statt. Wir rechnen mit 20000 Besuchern.";

    #[derive(Debug)]
    struct FixedClock;

    impl Clock for FixedClock {
        fn now(&self) -> Timestamp {
            "2030-05-18T08:00:00.123456Z".parse().unwrap()
        }
    }

    fn core_field(key: &str) -> Uuid {
        core_catalog()
            .into_iter()
            .find(|field| field.key.as_str() == key)
            .unwrap()
            .id
            .as_uuid()
    }

    fn passage(quote: &str) -> serde_json::Value {
        let byte = SOURCE.find(quote).unwrap();
        let start = SOURCE[..byte].chars().count();
        json!({"start": start, "end": start + quote.chars().count(), "quote": quote})
    }

    fn stores(test: &TestDatabase) -> ProposeStores<'_> {
        ProposeStores {
            identity: &test.database,
            facts: &test.database,
            proposals: &test.database,
            sources: &test.database,
            documents: &test.database,
        }
    }

    /// An organization with one event and a contributor of the event.
    async fn open_day(test: &TestDatabase) -> (OrganizationId, EventId, MemberCaller) {
        let organization = test.create_organization("testwil").await;
        let event = test.create_event(organization, "OPEN30").await;
        let anna = test
            .create_user(
                &DisplayName::parse("Anna Muster").unwrap(),
                &Email::parse("anna@example.org").unwrap(),
            )
            .await;
        test.add_membership(organization, anna, OrganizationRole::Member)
            .await;
        add_event_role(test, organization, event, anna, EventRole::EventContributor).await;
        let caller = MemberCaller::new(anna, organization, OrganizationRole::Member);
        (organization, event, caller)
    }

    /// A changeset with a new field and two facts: one on the new field and one on a shipped field.
    fn intake(event: EventId) -> (NewChangeset, [Uuid; 3]) {
        let ids = [Uuid::now_v7(), Uuid::now_v7(), Uuid::now_v7()];
        let field = Uuid::now_v7();
        let input = serde_json::from_value(json!({
            "event_id": event.as_uuid(),
            "source_text": SOURCE,
            "proposals": [
                {
                    "id": ids[0],
                    "operation": {
                        "kind": "add_field_definition", "id": field, "event_id": event.as_uuid(),
                        "key": "visitors_total", "label": "Besucher total",
                        "value_type": {"type": "quantity", "unit": "person"},
                        "description": "The expected number of visitors of the whole event.",
                        "module": "open_day",
                    },
                    "evidence": [passage("Besuchern")],
                    "reason": "The member names a visitor number.",
                },
                {
                    "id": ids[1],
                    "operation": {
                        "kind": "set_fact", "event_id": event.as_uuid(), "field_id": field,
                        "state": {"state": "assumption", "approximate": true,
                                  "value": {"type": "quantity", "min": "20000", "max": "20000"}},
                    },
                    "depends_on": [ids[0]],
                    "evidence": [passage("20000 Besuchern")],
                    "reason": "The member expects 20000 visitors.",
                },
                {
                    "id": ids[2],
                    "operation": {
                        "kind": "set_fact", "event_id": event.as_uuid(),
                        "field_id": core_field("date_window"),
                        "state": {"state": "accepted",
                                  "value": {"type": "date_window", "start": "2030-05-01",
                                            "end": "2030-05-31", "granularity": "month"}},
                    },
                    "evidence": [passage("im Mai 2030")],
                    "reason": "The member names the month.",
                },
            ],
        }))
        .unwrap();
        (input, ids)
    }

    /// The changeset of a create that stored a new one.
    fn new(created: Result<Created, ProposeError>) -> Changeset {
        match created.unwrap() {
            Created::New(changeset) => changeset,
            Created::Existing(_) => panic!("not new"),
        }
    }

    async fn count(test: &TestDatabase, table: &str) -> i64 {
        test.scalar(&format!("SELECT count(*) FROM {table}")).await
    }

    async fn add_event_role(
        test: &TestDatabase,
        organization: OrganizationId,
        event: EventId,
        user: UserId,
        role: EventRole,
    ) {
        sqlx::query(
            "INSERT INTO event_membership (organization_id, event_id, user_id, event_role, version, created_at)
             VALUES ($1, $2, $3, $4, 1, now())",
        )
        .bind(organization.as_uuid())
        .bind(event.as_uuid())
        .bind(user.as_uuid())
        .bind(role.as_str())
        .execute(&test.database.pool)
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn stores_a_changeset_with_its_source_proposals_dependencies_and_evidence() {
        let test = TestDatabase::start().await;
        let (_, event, anna) = open_day(&test).await;
        let (input, ids) = intake(event);
        let changeset = new(create_changeset(&anna, input, stores(&test), &FixedClock).await);

        let (text, kind, channel): (String, String, String) = sqlx::query_as(
            "SELECT v.text, v.kind, v.channel FROM source_version v
             JOIN source_item i ON i.id = v.source_item_id
             WHERE v.id = $1 AND i.event_id = $2",
        )
        .bind(changeset.source_version_id.as_uuid())
        .bind(event.as_uuid())
        .fetch_one(&test.database.pool)
        .await
        .unwrap();
        assert_eq!(
            (text.as_str(), kind.as_str(), channel.as_str()),
            (SOURCE, "member-text", "web")
        );

        let author: serde_json::Value =
            sqlx::query_scalar("SELECT author FROM changeset WHERE id = $1")
                .bind(changeset.id.as_uuid())
                .fetch_one(&test.database.pool)
                .await
                .unwrap();
        assert_eq!(actor::from_json(&author).unwrap(), anna.actor());

        let rows: Vec<(Uuid, serde_json::Value, i32, String, Uuid, Option<i64>)> = sqlx::query_as(
            "SELECT id, operation, operation_version, target_kind, target_id, expected_version
             FROM proposal WHERE changeset_id = $1 ORDER BY id",
        )
        .bind(changeset.id.as_uuid())
        .fetch_all(&test.database.pool)
        .await
        .unwrap();
        assert_eq!(rows.len(), 3);
        for (row, proposal) in rows.iter().zip(&changeset.proposals) {
            assert_eq!(row.0, proposal.id.as_uuid());
            assert_eq!(
                operation_from_json(row.2, &row.1).unwrap(),
                proposal.operation
            );
            assert_eq!(
                row.3,
                if row.0 == ids[0] {
                    "field_definition"
                } else {
                    "fact"
                }
            );
            assert_eq!(row.5, None);
        }
        assert_eq!(rows[2].4, core_field("date_window"));

        let dependencies: Vec<(Uuid, Uuid)> =
            sqlx::query_as("SELECT proposal_id, depends_on FROM proposal_dependency")
                .fetch_all(&test.database.pool)
                .await
                .unwrap();
        assert_eq!(dependencies, [(ids[1], ids[0])]);
        let evidence: Vec<(Uuid, Uuid, String)> = sqlx::query_as(
            "SELECT proposal_id, source_version_id, quote FROM proposal_evidence ORDER BY proposal_id",
        )
        .fetch_all(&test.database.pool)
        .await
        .unwrap();
        let expected: Vec<_> = [
            (ids[0], "Besuchern"),
            (ids[1], "20000 Besuchern"),
            (ids[2], "im Mai 2030"),
        ]
        .into_iter()
        .map(|(id, quote)| (id, changeset.source_version_id.as_uuid(), quote.to_owned()))
        .collect();
        assert_eq!(evidence, expected);
        assert_eq!(
            count(&test, "audit_event WHERE action = 'changeset.create'").await,
            1
        );
    }

    #[tokio::test]
    async fn a_cycle_stores_nothing() {
        let test = TestDatabase::start().await;
        let (_, event, anna) = open_day(&test).await;
        let (mut input, ids) = intake(event);
        input.proposals[0].depends_on = vec![ids[1]];
        let result = create_changeset(&anna, input, stores(&test), &FixedClock).await;
        assert!(
            matches!(result, Err(ProposeError::Invalid(_))),
            "{result:?}"
        );
        for table in ["changeset", "proposal", "source_version", "audit_event"] {
            assert_eq!(count(&test, table).await, 0, "{table}");
        }
    }

    #[tokio::test]
    async fn a_viewer_cannot_propose_and_a_contributor_can() {
        let test = TestDatabase::start().await;
        let (organization, event, anna) = open_day(&test).await;
        let bruno = test
            .create_user(
                &DisplayName::parse("Bruno Beispiel").unwrap(),
                &Email::parse("bruno@example.org").unwrap(),
            )
            .await;
        test.add_membership(organization, bruno, OrganizationRole::Member)
            .await;
        add_event_role(&test, organization, event, bruno, EventRole::EventViewer).await;
        let viewer = MemberCaller::new(bruno, organization, OrganizationRole::Member);
        let result = create_changeset(&viewer, intake(event).0, stores(&test), &FixedClock).await;
        assert!(matches!(result, Err(ProposeError::Forbidden)), "{result:?}");
        assert!(
            create_changeset(&anna, intake(event).0, stores(&test), &FixedClock)
                .await
                .is_ok()
        );
    }

    #[tokio::test]
    async fn proposals_never_change() {
        let test = TestDatabase::start().await;
        let (_, event, anna) = open_day(&test).await;
        new(create_changeset(&anna, intake(event).0, stores(&test), &FixedClock).await);
        sqlx::query(
            "INSERT INTO review_result (id, organization_id, proposal_id, result, reviewer, created_at)
             SELECT $1, organization_id, id, 'rejected', '{}'::jsonb, now() FROM proposal LIMIT 1",
        )
        .bind(Uuid::now_v7())
        .execute(&test.database.pool)
        .await
        .unwrap();
        for statement in [
            "UPDATE changeset SET created_at = now()",
            "DELETE FROM changeset",
            "TRUNCATE changeset CASCADE",
            "UPDATE proposal SET reason = 'changed'",
            "DELETE FROM proposal",
            "TRUNCATE proposal CASCADE",
            "UPDATE proposal_dependency SET depends_on = proposal_id",
            "DELETE FROM proposal_dependency",
            "TRUNCATE proposal_dependency",
            "UPDATE proposal_evidence SET quote = 'changed'",
            "DELETE FROM proposal_evidence",
            "TRUNCATE proposal_evidence",
            "UPDATE review_result SET result = 'accepted'",
            "DELETE FROM review_result",
            "TRUNCATE review_result",
        ] {
            let error = sqlx::query(sqlx::AssertSqlSafe(statement))
                .execute(&test.database.pool)
                .await
                .unwrap_err();
            assert_eq!(sqlstate(&error), "23001", "{statement}");
        }
        for (table, rows) in [
            ("changeset", 1),
            ("proposal", 3),
            ("proposal_dependency", 1),
            ("proposal_evidence", 3),
            ("review_result", 1),
        ] {
            assert_eq!(count(&test, table).await, rows, "{table}");
        }
    }

    #[tokio::test]
    async fn the_taken_ids_include_records_of_each_organization() {
        let test = TestDatabase::start().await;
        let (_, event, anna) = open_day(&test).await;
        let other = test.create_organization("musterhausen").await;
        let other_event = test.create_event(other, "FLY31").await;
        let changeset =
            new(create_changeset(&anna, intake(event).0, stores(&test), &FixedClock).await);
        let free = Uuid::now_v7();
        let wanted = [
            free,
            other_event.as_uuid(),
            changeset.id.as_uuid(),
            changeset.proposals[0].id.as_uuid(),
            core_field("audience"),
        ];
        let mut taken = test.database.taken_ids(&wanted).await.unwrap();
        taken.sort();
        let mut expected = wanted[1..].to_vec();
        expected.sort();
        assert_eq!(taken, expected);
    }

    #[tokio::test]
    async fn the_profile_shows_open_fact_proposals_apart_from_the_facts() {
        let test = TestDatabase::start().await;
        let (organization, event, anna) = open_day(&test).await;
        let changeset =
            new(create_changeset(&anna, intake(event).0, stores(&test), &FixedClock).await);
        let other = test.create_organization("musterhausen").await;
        let profile = test.database.profile(anna.scope(), event).await.unwrap();
        assert!(profile.fields.is_empty(), "a proposal is not a fact");
        assert!(profile.open_questions.is_empty());
        let facts: Vec<&Proposal> = changeset.proposals[1..].iter().collect();
        assert_eq!(profile.proposals.len(), 2);
        for (open, proposal) in profile.proposals.iter().zip(facts) {
            let Operation::SetFact {
                field_id, state, ..
            } = &proposal.operation
            else {
                panic!("not a fact");
            };
            assert_eq!(open.proposal_id, proposal.id);
            assert_eq!(open.changeset_id, changeset.id);
            assert_eq!(&open.field_id, field_id);
            assert_eq!(&open.state, state);
            assert_eq!(open.expected_version, None);
            assert_eq!(open.created_at, FixedClock.now());
        }
        let window = DateWindow::new(
            jiff::civil::date(2030, 5, 1),
            jiff::civil::date(2030, 5, 31),
            Granularity::Month,
        )
        .unwrap();
        assert_eq!(
            profile.proposals[1].state,
            FactState::Accepted(Valued {
                value: FactValue::DateWindow(window),
                approximate: false
            })
        );

        // A reviewed proposal is not open. Task 27 writes review results; this test writes one directly.
        sqlx::query(
            "INSERT INTO review_result (id, organization_id, proposal_id, result, reviewer, created_at)
             VALUES ($1, $2, $3, 'rejected', $4, now())",
        )
        .bind(Uuid::now_v7())
        .bind(organization.as_uuid())
        .bind(changeset.proposals[1].id.as_uuid())
        .bind(actor::to_json(&anna.actor()))
        .execute(&test.database.pool)
        .await
        .unwrap();
        let profile = test.database.profile(anna.scope(), event).await.unwrap();
        let open: Vec<_> = profile
            .proposals
            .iter()
            .map(|open| open.proposal_id)
            .collect();
        assert_eq!(open, [changeset.proposals[2].id]);

        let elsewhere = MemberCaller::new(anna.user_id(), other, OrganizationRole::Owner);
        let profile = test
            .database
            .profile(elsewhere.scope(), event)
            .await
            .unwrap();
        assert!(profile.proposals.is_empty());
    }

    #[tokio::test]
    async fn the_profile_lists_open_questions_in_their_order() {
        let test = TestDatabase::start().await;
        let (organization, event, anna) = open_day(&test).await;
        for (number, text, status) in [
            (2, "Wer macht Catering?", "open"),
            (1, "Welcher Samstag?", "open"),
            (3, "Erledigt?", "closed"),
        ] {
            sqlx::query(
                "INSERT INTO open_question
                     (id, organization_id, event_id, local_number, text, owner_user_id, status, version, created_at)
                 VALUES ($1, $2, $3, $4, $5, $6, $7, 1, now())",
            )
            .bind(Uuid::now_v7())
            .bind(organization.as_uuid())
            .bind(event.as_uuid())
            .bind(number)
            .bind(text)
            .bind(anna.user_id().as_uuid())
            .bind(status)
            .execute(&test.database.pool)
            .await
            .unwrap();
        }
        let profile = test.database.profile(anna.scope(), event).await.unwrap();
        let questions: Vec<_> = profile
            .open_questions
            .iter()
            .map(|question| {
                (
                    question.local_number,
                    question.text.as_str(),
                    question.owner,
                )
            })
            .collect();
        assert_eq!(
            questions,
            [
                (1, "Welcher Samstag?", anna.user_id()),
                (2, "Wer macht Catering?", anna.user_id())
            ]
        );
    }

    /// An organization changeset of an owner: a new event and a fact of the new event.
    fn new_event_intake(event: Uuid) -> (NewChangeset, [Uuid; 2]) {
        let ids = [Uuid::now_v7(), Uuid::now_v7()];
        let input = serde_json::from_value(json!({
            "source_text": SOURCE,
            "proposals": [
                {
                    "id": ids[0],
                    "operation": {"kind": "create_event", "id": event, "key": "OPEN31", "name": "Open Day Testwil"},
                    "evidence": [passage("Das Open Day")],
                    "reason": "The member names a new event.",
                },
                {
                    "id": ids[1],
                    "operation": {
                        "kind": "set_fact", "event_id": event, "field_id": core_field("date_window"),
                        "state": {"state": "assumption",
                                  "value": {"type": "date_window", "start": "2030-05-01",
                                            "end": "2030-05-31", "granularity": "month"}},
                    },
                    "depends_on": [ids[0]],
                    "evidence": [passage("im Mai 2030")],
                    "reason": "The member names the month.",
                },
            ],
        }))
        .unwrap();
        (input, ids)
    }

    #[tokio::test]
    async fn the_fact_proposals_of_a_new_event_reach_its_profile_after_the_event_applies() {
        let test = TestDatabase::start().await;
        let (organization, _, _) = open_day(&test).await;
        let owner = MemberCaller::new(
            UserId::from_uuid(Uuid::now_v7()),
            organization,
            OrganizationRole::Owner,
        )
        .with_request(tada_app::caller::Channel::Telegram, None);
        let event = Uuid::now_v7();
        let (input, ids) = new_event_intake(event);
        let changeset = new(create_changeset(&owner, input, stores(&test), &FixedClock).await);
        assert_eq!(changeset.event_id, None);

        let (changeset_event, item_event, channel): (Option<Uuid>, Option<Uuid>, String) =
            sqlx::query_as(
                "SELECT c.event_id, i.event_id, v.channel FROM changeset c
                 JOIN source_version v ON v.id = c.source_version_id
                 JOIN source_item i ON i.id = v.source_item_id
                 WHERE c.id = $1",
            )
            .bind(changeset.id.as_uuid())
            .fetch_one(&test.database.pool)
            .await
            .unwrap();
        assert_eq!(
            (changeset_event, item_event, channel.as_str()),
            (None, None, "telegram")
        );
        let events: Vec<Uuid> = sqlx::query_scalar("SELECT event_id FROM proposal ORDER BY id")
            .fetch_all(&test.database.pool)
            .await
            .unwrap();
        assert_eq!(events, [event, event]);

        // Task 27 applies the event alone and records the result; this test writes both directly.
        sqlx::query(
            "INSERT INTO event (id, organization_id, key, name, time_zone, version, created_at)
             VALUES ($1, $2, 'OPEN31', 'Open Day Testwil', 'Europe/Zurich', 1, now())",
        )
        .bind(event)
        .bind(organization.as_uuid())
        .execute(&test.database.pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO review_result (id, organization_id, proposal_id, result, reviewer, created_at)
             VALUES ($1, $2, $3, 'accepted', $4, now())",
        )
        .bind(Uuid::now_v7())
        .bind(organization.as_uuid())
        .bind(ids[0])
        .bind(actor::to_json(&owner.actor()))
        .execute(&test.database.pool)
        .await
        .unwrap();
        let profile = test
            .database
            .profile(owner.scope(), EventId::from_uuid(event))
            .await
            .unwrap();
        let open: Vec<_> = profile
            .proposals
            .iter()
            .map(|open| open.proposal_id.as_uuid())
            .collect();
        assert_eq!(open, [ids[1]]);
    }

    #[tokio::test]
    async fn a_retry_returns_the_stored_changeset_and_stores_nothing_new() {
        let test = TestDatabase::start().await;
        let (_, event, anna) = open_day(&test).await;
        let (mut input, _) = intake(event);
        input.id = Some(Uuid::now_v7());
        let first = new(create_changeset(&anna, input.clone(), stores(&test), &FixedClock).await);
        let retry = create_changeset(&anna, input.clone(), stores(&test), &FixedClock)
            .await
            .unwrap();
        let Created::Existing(stored) = retry else {
            panic!("not existing");
        };
        let mut expected = first.clone();
        expected.proposals.sort_by_key(|proposal| proposal.id);
        assert_eq!(stored, expected);
        for table in ["changeset", "source_version", "audit_event"] {
            assert_eq!(count(&test, table).await, 1, "{table}");
        }

        let mut changed = input;
        changed.source_text.push_str(" Und Bier.");
        let result = create_changeset(&anna, changed, stores(&test), &FixedClock).await;
        let Err(ProposeError::Invalid(errors)) = result else {
            panic!("not invalid: {result:?}");
        };
        assert_eq!((errors[0].field.as_ref(), errors[0].code), ("id", "taken"));

        // A concurrent request with the same IDs loses the race at the primary key and changes nothing.
        let audit = AuditEvent::new(
            anna.actor(),
            tada_app::audit::AuditAction::ChangesetCreate,
            Some(first.id.as_uuid()),
            Some(anna.scope()),
        );
        let mut again = first;
        again.source_version_id = SourceVersionId::from_uuid(Uuid::now_v7());
        let inserted = test
            .database
            .insert(anna.scope(), &again, &SourceText::normalize(SOURCE), &audit)
            .await
            .unwrap();
        assert_eq!(inserted, Inserted::IdTaken);
        for table in ["changeset", "source_version", "audit_event"] {
            assert_eq!(count(&test, table).await, 1, "{table}");
        }
    }

    #[test]
    fn restores_each_operation() {
        let event = EventId::from_uuid(Uuid::now_v7());
        let field = FieldDefinitionId::from_uuid(Uuid::now_v7());
        let operations = [
            Operation::CreateEvent {
                id: event,
                key: EventKey::parse("OPEN30").unwrap(),
                name: EventName::parse("Open Day Testwil").unwrap(),
                time_zone: EventTimeZone::default_zone(),
            },
            Operation::SetFact {
                event_id: event,
                field_id: field,
                state: FactState::Assumption(Valued {
                    value: FactValue::Quantity(Range::exact(Decimal::new(15, 1).unwrap())),
                    approximate: true,
                }),
                expected_version: RecordVersion::new(3),
            },
            Operation::SetFact {
                event_id: event,
                field_id: field,
                state: FactState::Unknown,
                expected_version: None,
            },
            Operation::AddFieldDefinition {
                id: field,
                event_id: event,
                key: FieldKey::parse("runway_surface").unwrap(),
                label: ShortText::parse("Pistenbelag").unwrap(),
                value_type: ValueType::Choice {
                    values: vec![tada_app::domain::facts::ChoiceValue {
                        key: ChoiceKey::parse("grass").unwrap(),
                        label: Label::Text(ShortText::parse("Gras").unwrap()),
                    }],
                    multiple: false,
                },
                description: Description::parse("The surface of the runway.").unwrap(),
                module: ModuleKey::parse("aviation").unwrap(),
            },
            Operation::AddChoiceValue {
                event_id: event,
                field_id: field,
                key: ChoiceKey::parse("asphalt").unwrap(),
                label: ShortText::parse("Asphalt").unwrap(),
            },
            Operation::DeprecateField {
                event_id: event,
                field_id: field,
            },
            Operation::CreateOpenQuestion {
                id: OpenQuestionId::from_uuid(Uuid::now_v7()),
                event_id: event,
                text: QuestionText::parse("Welcher Samstag?").unwrap(),
                owner: UserId::from_uuid(Uuid::now_v7()),
            },
        ];
        for operation in operations {
            let json = operation_to_json(&operation);
            assert_eq!(
                operation_from_json(OPERATION_VERSION, &json).unwrap(),
                operation,
                "{json}"
            );
        }
        let json = operation_to_json(&Operation::DeprecateField {
            event_id: event,
            field_id: field,
        });
        assert!(operation_from_json(OPERATION_VERSION + 1, &json).is_err());
        assert!(operation_from_json(OPERATION_VERSION, &json!({"kind": "set_fact"})).is_err());
    }
}
