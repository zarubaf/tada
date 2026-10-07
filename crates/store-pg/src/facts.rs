//! The `FactStore` adapter (ADR 0049) and the sync of the shipped field catalog.

use std::collections::HashMap;

use async_trait::async_trait;
use sqlx::types::Uuid;
use tada_app::caller::OrgScope;
use tada_app::domain::RecordVersion;
use tada_app::domain::facts::{
    Description, FieldDefinition, FieldKey, FieldScope, FieldStatus, ModuleKey,
};
use tada_app::domain::ids::{EventId, FactId, FactVersionId, FieldDefinitionId, SourceVersionId};
use tada_app::domain::sources::Passage;
use tada_app::facts::{EventProfile, EvidenceRef, FactStore, FactVersionRef, ProfileEntry};
use tada_app::store::StoreError;

use crate::Database;
use crate::error::{InvalidRow, store_error};
use crate::{proposals, values};

/// The sync of a shipped field catalog failed. It changed nothing.
#[derive(Debug, thiserror::Error)]
pub enum SyncCatalogError {
    #[error("the field {0} of the catalog is not a shipped field")]
    NotShipped(String),
    /// The value type of a key never changes (ADR 0049). A different type needs a new field.
    #[error("the value type of the shipped field {0} changed")]
    ValueTypeChanged(String),
    #[error("the key of the shipped field {0} changed")]
    KeyChanged(String),
    /// A newer binary wrote the catalog. An older binary never downgrades it.
    #[error("the stored catalog version {stored} is newer than the catalog version {given}")]
    NewerCatalog { stored: u32, given: u32 },
    #[error("the database failed")]
    Database(#[from] sqlx::Error),
}

impl Database {
    /// Writes the shipped fields of `catalog` (ADR 0049): it inserts the missing fields and updates the labels,
    /// descriptions, modules and statuses of the others. The Rust catalog is the one authority for shipped fields.
    /// It fails and changes nothing if a field has another key or another value type than its stored row,
    /// or if the stored catalog is newer than `catalog_version`.
    pub async fn sync_catalog(
        &self,
        catalog: &[FieldDefinition],
        catalog_version: u32,
    ) -> Result<(), SyncCatalogError> {
        let mut tx = self.pool.begin().await?;
        let stored = sqlx::query_scalar!(
            "SELECT max(catalog_version) FROM field_definition WHERE event_id IS NULL",
        )
        .fetch_one(&mut *tx)
        .await?;
        if let Some(stored) = stored.filter(|stored| *stored > i64::from(catalog_version)) {
            return Err(SyncCatalogError::NewerCatalog {
                stored: u32::try_from(stored).unwrap_or(u32::MAX),
                given: catalog_version,
            });
        }
        for field in catalog {
            let key = field.key.as_str();
            if field.scope != FieldScope::Shipped {
                return Err(SyncCatalogError::NotShipped(key.to_owned()));
            }
            let stored = sqlx::query!(
                "SELECT key, value_type FROM field_definition WHERE id = $1 FOR UPDATE",
                field.id.as_uuid(),
            )
            .fetch_optional(&mut *tx)
            .await?;
            let (label_message, label_text) = values::label_to_columns(&field.label);
            let Some(stored) = stored else {
                sqlx::query!(
                    "INSERT INTO field_definition (id, key, label_text, label_message, value_type, description,
                                                   module, status, catalog_version, created_at)
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, now())",
                    field.id.as_uuid(),
                    key,
                    label_text,
                    label_message,
                    values::value_type_to_json(&field.value_type),
                    field.description.as_str(),
                    field.module.as_str(),
                    status_name(field.status),
                    i64::from(catalog_version),
                )
                .execute(&mut *tx)
                .await?;
                continue;
            };
            if stored.key != key {
                return Err(SyncCatalogError::KeyChanged(key.to_owned()));
            }
            if values::value_type_from_json(&stored.value_type)
                .ok()
                .as_ref()
                != Some(&field.value_type)
            {
                return Err(SyncCatalogError::ValueTypeChanged(key.to_owned()));
            }
            sqlx::query!(
                "UPDATE field_definition
                 SET label_text = $2, label_message = $3, description = $4, module = $5, status = $6,
                     catalog_version = $7
                 WHERE id = $1",
                field.id.as_uuid(),
                label_text,
                label_message,
                field.description.as_str(),
                field.module.as_str(),
                status_name(field.status),
                i64::from(catalog_version),
            )
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(())
    }
}

fn status_name(status: FieldStatus) -> &'static str {
    match status {
        FieldStatus::Active => "active",
        FieldStatus::Deprecated => "deprecated",
    }
}

/// The columns of a field definition.
struct FieldRow {
    id: Uuid,
    event_id: Option<Uuid>,
    key: String,
    label_text: Option<String>,
    label_message: Option<String>,
    value_type: serde_json::Value,
    description: String,
    module: String,
    status: String,
}

impl TryFrom<FieldRow> for FieldDefinition {
    type Error = InvalidRow;

    fn try_from(row: FieldRow) -> Result<Self, InvalidRow> {
        Ok(FieldDefinition {
            id: FieldDefinitionId::from_uuid(row.id),
            key: FieldKey::parse(&row.key).map_err(|_| InvalidRow("field_definition.key"))?,
            label: values::label_from_columns(row.label_message, row.label_text)?,
            value_type: values::value_type_from_json(&row.value_type)?,
            description: Description::parse(&row.description)
                .map_err(|_| InvalidRow("field_definition.description"))?,
            module: ModuleKey::parse(&row.module)
                .map_err(|_| InvalidRow("field_definition.module"))?,
            status: match row.status.as_str() {
                "active" => FieldStatus::Active,
                "deprecated" => FieldStatus::Deprecated,
                _ => return Err(InvalidRow("field_definition.status")),
            },
            scope: match row.event_id {
                None => FieldScope::Shipped,
                Some(event) => FieldScope::Event(EventId::from_uuid(event)),
            },
        })
    }
}

/// The current version of a fact with its field.
struct ProfileRow {
    fact_id: Uuid,
    version_id: Uuid,
    number: i64,
    state: String,
    value: Option<serde_json::Value>,
    approximate: bool,
    id: Uuid,
    event_id: Option<Uuid>,
    key: String,
    label_text: Option<String>,
    label_message: Option<String>,
    value_type: serde_json::Value,
    description: String,
    module: String,
    status: String,
}

struct EvidenceRow {
    fact_version_id: Uuid,
    source_version_id: Uuid,
    start_offset: i32,
    end_offset: i32,
    quote: String,
    page: Option<i32>,
}

impl TryFrom<EvidenceRow> for EvidenceRef {
    type Error = InvalidRow;

    fn try_from(row: EvidenceRow) -> Result<Self, InvalidRow> {
        let offset = |value: i32| u32::try_from(value).map_err(|_| InvalidRow("evidence_link"));
        Ok(EvidenceRef {
            source_version_id: SourceVersionId::from_uuid(row.source_version_id),
            passage: Passage {
                start: offset(row.start_offset)?,
                end: offset(row.end_offset)?,
                quote: row.quote,
                page: row.page.map(offset).transpose()?,
            },
        })
    }
}

fn record_version(number: i64) -> Result<RecordVersion, InvalidRow> {
    RecordVersion::new(number).ok_or(InvalidRow("fact_version.number"))
}

#[async_trait]
impl FactStore for Database {
    async fn catalog(
        &self,
        scope: OrgScope,
        event: EventId,
    ) -> Result<Vec<FieldDefinition>, StoreError> {
        // Shipped fields have no organization (the schema exception of migration 0010).
        let rows = sqlx::query_as!(
            FieldRow,
            r#"SELECT id, event_id, key, label_text, label_message, value_type, description, module, status
               FROM field_definition
               WHERE event_id IS NULL OR (organization_id = $1 AND event_id = $2)
               ORDER BY key COLLATE "C", id"#,
            scope.organization_id().as_uuid(),
            event.as_uuid(),
        )
        .fetch_all(&self.pool)
        .await
        .map_err(store_error)?;
        Ok(rows
            .into_iter()
            .map(FieldDefinition::try_from)
            .collect::<Result<_, _>>()?)
    }

    async fn profile(&self, scope: OrgScope, event: EventId) -> Result<EventProfile, StoreError> {
        let organization = scope.organization_id().as_uuid();
        let rows = sqlx::query_as!(
            ProfileRow,
            r#"SELECT f.id AS fact_id, v.id AS version_id, v.number, v.state, v.value, v.approximate,
                      d.id, d.event_id, d.key, d.label_text, d.label_message, d.value_type, d.description,
                      d.module, d.status
               FROM fact f
               JOIN fact_version v ON v.organization_id = f.organization_id AND v.fact_id = f.id
                                  AND v.number = f.version
               JOIN field_definition d ON d.id = f.field_id
               WHERE f.organization_id = $1 AND f.event_id = $2
               ORDER BY d.key COLLATE "C", d.id"#,
            organization,
            event.as_uuid(),
        )
        .fetch_all(&self.pool)
        .await
        .map_err(store_error)?;
        let version_ids: Vec<Uuid> = rows.iter().map(|row| row.version_id).collect();
        let evidence_rows = sqlx::query_as!(
            EvidenceRow,
            "SELECT fact_version_id, source_version_id, start_offset, end_offset, quote, page
             FROM evidence_link
             WHERE organization_id = $1 AND fact_version_id = ANY($2)
             ORDER BY source_version_id, start_offset, id",
            organization,
            &version_ids,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(store_error)?;
        let mut evidence: HashMap<Uuid, Vec<EvidenceRef>> = HashMap::new();
        for row in evidence_rows {
            let version = row.fact_version_id;
            evidence
                .entry(version)
                .or_default()
                .push(EvidenceRef::try_from(row)?);
        }
        let fields = rows
            .into_iter()
            .map(|row| {
                Ok(ProfileEntry {
                    fact_id: FactId::from_uuid(row.fact_id),
                    version: record_version(row.number)?,
                    state: values::fact_state_from_columns(&row.state, row.value, row.approximate)?,
                    evidence: evidence.remove(&row.version_id).unwrap_or_default(),
                    field: FieldDefinition::try_from(FieldRow {
                        id: row.id,
                        event_id: row.event_id,
                        key: row.key,
                        label_text: row.label_text,
                        label_message: row.label_message,
                        value_type: row.value_type,
                        description: row.description,
                        module: row.module,
                        status: row.status,
                    })?,
                })
            })
            .collect::<Result<_, InvalidRow>>()?;
        Ok(EventProfile {
            fields,
            proposals: proposals::open_fact_proposals(&self.pool, scope, event).await?,
            open_questions: proposals::open_questions(&self.pool, scope, event).await?,
        })
    }

    async fn current_version(
        &self,
        scope: OrgScope,
        event: EventId,
        field: FieldDefinitionId,
    ) -> Result<Option<FactVersionRef>, StoreError> {
        let row = sqlx::query!(
            "SELECT v.id, v.fact_id, v.number, v.state, v.value, v.approximate
             FROM fact f
             JOIN fact_version v ON v.organization_id = f.organization_id AND v.fact_id = f.id
                                AND v.number = f.version
             WHERE f.organization_id = $1 AND f.event_id = $2 AND f.field_id = $3",
            scope.organization_id().as_uuid(),
            event.as_uuid(),
            field.as_uuid(),
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(store_error)?;
        let Some(row) = row else {
            return Ok(None);
        };
        Ok(Some(FactVersionRef {
            id: FactVersionId::from_uuid(row.id),
            fact_id: FactId::from_uuid(row.fact_id),
            number: record_version(row.number)?,
            state: values::fact_state_from_columns(&row.state, row.value, row.approximate)?,
        }))
    }
}

#[cfg(test)]
mod tests {
    use jiff::civil::date;
    use tada_app::caller::{Actor, MemberCaller, OrganizationRole};
    use tada_app::domain::facts::{
        CORE_CATALOG_VERSION, DateWindow, FactState, FactValue, Granularity, Label, ShortText,
        Unit, ValueType, Valued, core_catalog,
    };
    use tada_app::domain::ids::{OrganizationId, UserId};
    use tada_app::domain::sources::{Passage, SourceText};
    use tada_app::sources::SourceStore;

    use super::*;
    use crate::actor;
    use crate::testing::{TestDatabase, sqlstate};

    fn scope_of(organization: OrganizationId) -> OrgScope {
        MemberCaller::new(
            UserId::from_uuid(Uuid::now_v7()),
            organization,
            OrganizationRole::Owner,
        )
        .scope()
    }

    fn core(key: &str) -> FieldDefinition {
        core_catalog()
            .into_iter()
            .find(|field| field.key.as_str() == key)
            .unwrap()
    }

    fn by_key(mut fields: Vec<FieldDefinition>) -> Vec<FieldDefinition> {
        fields.sort_by(|a, b| a.key.cmp(&b.key));
        fields
    }

    fn author() -> Actor {
        MemberCaller::new(
            UserId::from_uuid(Uuid::now_v7()),
            OrganizationId::from_uuid(Uuid::now_v7()),
            OrganizationRole::Member,
        )
        .actor()
    }

    fn venue(name: &str) -> Valued {
        Valued {
            value: FactValue::Text(ShortText::parse(name).unwrap()),
            approximate: false,
        }
    }

    /// Adds the field `key` of the event `event`, with a text label.
    async fn add_event_field(
        test: &TestDatabase,
        organization: OrganizationId,
        event: EventId,
        key: &str,
    ) -> FieldDefinition {
        let field = FieldDefinition {
            id: FieldDefinitionId::from_uuid(Uuid::now_v7()),
            key: FieldKey::parse(key).unwrap(),
            label: Label::Text(ShortText::parse("Anzahl Hangars").unwrap()),
            value_type: ValueType::Quantity {
                unit: Unit::parse("hangar").unwrap(),
            },
            description: Description::parse("The number of hangars.").unwrap(),
            module: ModuleKey::parse("aviation").unwrap(),
            status: FieldStatus::Active,
            scope: FieldScope::Event(event),
        };
        sqlx::query(
            "INSERT INTO field_definition
                 (id, organization_id, event_id, key, label_text, value_type, description, module, status, created_at)
             VALUES ($1, $2, $3, $4, 'Anzahl Hangars', $5, 'The number of hangars.', 'aviation', 'active', now())",
        )
        .bind(field.id.as_uuid())
        .bind(organization.as_uuid())
        .bind(event.as_uuid())
        .bind(key)
        .bind(values::value_type_to_json(&field.value_type))
        .execute(&test.database.pool)
        .await
        .unwrap();
        field
    }

    /// Inserts a fact without versions.
    async fn insert_fact(
        test: &TestDatabase,
        organization: OrganizationId,
        event: EventId,
        field: FieldDefinitionId,
    ) -> Result<FactId, sqlx::Error> {
        let fact = Uuid::now_v7();
        sqlx::query(
            "INSERT INTO fact (id, organization_id, event_id, field_id, version)
             VALUES ($1, $2, $3, $4, 1)",
        )
        .bind(fact)
        .bind(organization.as_uuid())
        .bind(event.as_uuid())
        .bind(field.as_uuid())
        .execute(&test.database.pool)
        .await?;
        Ok(FactId::from_uuid(fact))
    }

    /// Inserts the version `number` of a fact and makes it current, with an evidence link if `evidence` is given.
    async fn insert_version(
        test: &TestDatabase,
        organization: OrganizationId,
        fact: FactId,
        number: i64,
        state: &FactState<Valued>,
        evidence: Option<&EvidenceRef>,
    ) -> FactVersionId {
        let id = Uuid::now_v7();
        let (state, value, approximate) = values::fact_state_to_columns(state);
        sqlx::query(
            "INSERT INTO fact_version
                 (id, organization_id, fact_id, number, state, value, approximate, created_at, accepted_by)
             VALUES ($1, $2, $3, $4, $5, $6, $7, now(), $8)",
        )
        .bind(id)
        .bind(organization.as_uuid())
        .bind(fact.as_uuid())
        .bind(number)
        .bind(state)
        .bind(value)
        .bind(approximate)
        .bind(actor::to_json(&author()))
        .execute(&test.database.pool)
        .await
        .unwrap();
        sqlx::query("UPDATE fact SET version = $2 WHERE id = $1")
            .bind(fact.as_uuid())
            .bind(number)
            .execute(&test.database.pool)
            .await
            .unwrap();
        if let Some(evidence) = evidence {
            let passage = &evidence.passage;
            sqlx::query(
                "INSERT INTO evidence_link
                     (id, organization_id, fact_version_id, source_version_id, start_offset, end_offset, quote, page)
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
            )
            .bind(Uuid::now_v7())
            .bind(organization.as_uuid())
            .bind(id)
            .bind(evidence.source_version_id.as_uuid())
            .bind(i32::try_from(passage.start).unwrap())
            .bind(i32::try_from(passage.end).unwrap())
            .bind(&passage.quote)
            .bind(passage.page.map(|page| i32::try_from(page).unwrap()))
            .execute(&test.database.pool)
            .await
            .unwrap();
        }
        FactVersionId::from_uuid(id)
    }

    /// Stores `text` as a member text of the event and returns the evidence of the first `quote` in it.
    async fn evidence(
        test: &TestDatabase,
        scope: OrgScope,
        event: EventId,
        text: &str,
        quote: &str,
    ) -> EvidenceRef {
        let text = SourceText::normalize(text);
        let source = test
            .database
            .add_member_text(scope, event, &text, &author(), jiff::Timestamp::now())
            .await
            .unwrap();
        let bytes = text.as_str().find(quote).unwrap();
        let start = u32::try_from(text.as_str()[..bytes].chars().count()).unwrap();
        let passage = Passage {
            start,
            end: start + u32::try_from(quote.chars().count()).unwrap(),
            quote: quote.to_owned(),
            page: None,
        };
        passage.check(text.as_str()).unwrap();
        EvidenceRef {
            source_version_id: source.id,
            passage,
        }
    }

    #[tokio::test]
    async fn a_test_database_has_the_core_catalog() {
        let test = TestDatabase::start().await;
        let testwil = test.create_organization("testwil").await;
        let event = test.create_event(testwil, "TEST30").await;
        let catalog = test
            .database
            .catalog(scope_of(testwil), event)
            .await
            .unwrap();
        assert_eq!(catalog, by_key(core_catalog()));
    }

    #[tokio::test]
    async fn syncs_the_catalog_again_without_a_change() {
        let test = TestDatabase::start().await;
        let count = i64::try_from(core_catalog().len()).unwrap();
        let rows = "SELECT count(*) FROM field_definition";
        assert_eq!(test.scalar::<i64>(rows).await, count);
        test.database
            .sync_catalog(&core_catalog(), CORE_CATALOG_VERSION)
            .await
            .unwrap();
        assert_eq!(test.scalar::<i64>(rows).await, count);
    }

    #[tokio::test]
    async fn the_sync_updates_a_description_and_rejects_a_changed_value_type() {
        let test = TestDatabase::start().await;
        let db = &test.database;
        let mut catalog = core_catalog();
        let venue = catalog
            .iter_mut()
            .find(|field| field.key.as_str() == "venue")
            .unwrap();
        venue.description = Description::parse("The airfield of the event.").unwrap();
        db.sync_catalog(&catalog, CORE_CATALOG_VERSION + 1)
            .await
            .unwrap();
        let description: String = test
            .scalar("SELECT description FROM field_definition WHERE key = 'venue'")
            .await;
        assert_eq!(description, "The airfield of the event.");

        let mut changed = core_catalog();
        changed
            .iter_mut()
            .find(|field| field.key.as_str() == "venue")
            .unwrap()
            .value_type = ValueType::Boolean;
        let error = db
            .sync_catalog(&changed, CORE_CATALOG_VERSION + 2)
            .await
            .unwrap_err();
        assert!(matches!(error, SyncCatalogError::ValueTypeChanged(ref key) if key == "venue"));
        let value_type: serde_json::Value = test
            .scalar("SELECT value_type FROM field_definition WHERE key = 'venue'")
            .await;
        assert_eq!(value_type, values::value_type_to_json(&ValueType::Text));
    }

    /// An older binary never downgrades a catalog that a newer binary wrote.
    #[tokio::test]
    async fn the_sync_rejects_an_older_catalog_version_and_changes_nothing() {
        let test = TestDatabase::start().await;
        let db = &test.database;
        db.sync_catalog(&core_catalog(), CORE_CATALOG_VERSION + 1)
            .await
            .unwrap();
        let mut older = core_catalog();
        older
            .iter_mut()
            .find(|field| field.key.as_str() == "venue")
            .unwrap()
            .description = Description::parse("An old description.").unwrap();
        let error = db
            .sync_catalog(&older, CORE_CATALOG_VERSION)
            .await
            .unwrap_err();
        assert!(matches!(
            error,
            SyncCatalogError::NewerCatalog { stored, given }
                if stored == CORE_CATALOG_VERSION + 1 && given == CORE_CATALOG_VERSION
        ));
        let description: String = test
            .scalar("SELECT description FROM field_definition WHERE key = 'venue'")
            .await;
        assert_eq!(description, core("venue").description.as_str());
        let versions: i64 = test
            .scalar("SELECT max(catalog_version) FROM field_definition")
            .await;
        assert_eq!(versions, i64::from(CORE_CATALOG_VERSION + 1));
    }

    #[tokio::test]
    async fn the_sync_rejects_a_field_of_an_event() {
        let test = TestDatabase::start().await;
        let mut field = core("venue");
        field.scope = FieldScope::Event(EventId::from_uuid(Uuid::now_v7()));
        let error = test
            .database
            .sync_catalog(&[field], CORE_CATALOG_VERSION)
            .await
            .unwrap_err();
        assert!(matches!(error, SyncCatalogError::NotShipped(ref key) if key == "venue"));
    }

    #[tokio::test]
    async fn rejects_an_unknown_fact_version_with_a_value() {
        let test = TestDatabase::start().await;
        let testwil = test.create_organization("testwil").await;
        let event = test.create_event(testwil, "TEST30").await;
        let fact = insert_fact(&test, testwil, event, core("venue").id)
            .await
            .unwrap();
        let error = sqlx::query(
            "INSERT INTO fact_version
                 (id, organization_id, fact_id, number, state, value, approximate, created_at, accepted_by)
             VALUES ($1, $2, $3, 1, 'unknown', '{}', false, now(), '{}')",
        )
        .bind(Uuid::now_v7())
        .bind(testwil.as_uuid())
        .bind(fact.as_uuid())
        .execute(&test.database.pool)
        .await
        .unwrap_err();
        assert_eq!(sqlstate(&error), "23514");
    }

    #[tokio::test]
    async fn rejects_a_fact_on_a_field_of_another_event() {
        let test = TestDatabase::start().await;
        let testwil = test.create_organization("testwil").await;
        let open_day = test.create_event(testwil, "TEST30").await;
        let fly_in = test.create_event(testwil, "FLY31").await;
        let hangars = add_event_field(&test, testwil, fly_in, "hangar_count").await;

        let error = insert_fact(&test, testwil, open_day, hangars.id)
            .await
            .unwrap_err();
        assert_eq!(sqlstate(&error), "23514");
        assert!(
            insert_fact(&test, testwil, fly_in, hangars.id)
                .await
                .is_ok()
        );
    }

    #[tokio::test]
    async fn reads_the_profile_and_the_current_version_with_evidence() {
        let test = TestDatabase::start().await;
        let db = &test.database;
        let testwil = test.create_organization("testwil").await;
        let scope = scope_of(testwil);
        let event = test.create_event(testwil, "TEST30").await;
        let hangars = add_event_field(&test, testwil, event, "hangar_count").await;
        let date_window = core("date_window");

        let fact = insert_fact(&test, testwil, event, date_window.id)
            .await
            .unwrap();
        let text = "Wir planen Mai oder Juni 2030.";
        let source = evidence(&test, scope, event, text, "Mai oder Juni 2030").await;
        insert_version(&test, testwil, fact, 1, &FactState::Unknown, None).await;
        let window =
            DateWindow::new(date(2030, 5, 1), date(2030, 6, 30), Granularity::Month).unwrap();
        let assumption = FactState::Assumption(Valued {
            value: FactValue::DateWindow(window),
            approximate: true,
        });
        let version = insert_version(&test, testwil, fact, 2, &assumption, Some(&source)).await;
        let hangar_fact = insert_fact(&test, testwil, event, hangars.id)
            .await
            .unwrap();
        insert_version(&test, testwil, hangar_fact, 1, &FactState::Unknown, None).await;

        let profile = db.profile(scope, event).await.unwrap();
        let expected = [
            ProfileEntry {
                field: date_window.clone(),
                fact_id: fact,
                version: RecordVersion::new(2).unwrap(),
                state: assumption.clone(),
                evidence: vec![source],
            },
            ProfileEntry {
                field: hangars.clone(),
                fact_id: hangar_fact,
                version: RecordVersion::FIRST,
                state: FactState::Unknown,
                evidence: Vec::new(),
            },
        ];
        assert_eq!(profile.fields, expected);

        let current = db.current_version(scope, event, date_window.id).await;
        let expected = FactVersionRef {
            id: version,
            fact_id: fact,
            number: RecordVersion::new(2).unwrap(),
            state: assumption,
        };
        assert_eq!(current.unwrap(), Some(expected));
        let missing = db.current_version(scope, event, core("venue").id).await;
        assert_eq!(missing.unwrap(), None);

        let mut catalog = core_catalog();
        catalog.push(hangars);
        assert_eq!(db.catalog(scope, event).await.unwrap(), by_key(catalog));
    }

    /// Facts, event fields and evidence of another organization stay hidden (ADR 0006).
    #[tokio::test]
    async fn keeps_facts_and_fields_inside_the_organization() {
        let test = TestDatabase::start().await;
        let db = &test.database;
        let testwil = test.create_organization("testwil").await;
        let musterhausen = test.create_organization("musterhausen").await;
        let open_day = test.create_event(testwil, "TEST30").await;
        let fly_in = test.create_event(musterhausen, "FLY31").await;
        let venue_field = core("venue");
        for (organization, event, name) in [
            (testwil, open_day, "Flugfeld Testwil"),
            (musterhausen, fly_in, "Flugplatz Musterhausen"),
        ] {
            let fact = insert_fact(&test, organization, event, venue_field.id)
                .await
                .unwrap();
            let source = evidence(&test, scope_of(organization), event, name, name).await;
            let state = FactState::Accepted(venue(name));
            insert_version(&test, organization, fact, 1, &state, Some(&source)).await;
        }
        add_event_field(&test, musterhausen, fly_in, "hangar_count").await;

        let profile = db.profile(scope_of(testwil), open_day).await.unwrap();
        let states: Vec<_> = profile.fields.iter().map(|entry| &entry.state).collect();
        assert_eq!(states, [&FactState::Accepted(venue("Flugfeld Testwil"))]);
        assert_eq!(
            profile.fields[0].evidence[0].passage.quote,
            "Flugfeld Testwil"
        );

        let other = scope_of(musterhausen);
        assert!(db.profile(other, open_day).await.unwrap().fields.is_empty());
        let current = db.current_version(other, open_day, venue_field.id).await;
        assert_eq!(current.unwrap(), None);
        // The field of the other organization's event appears only in its own scope.
        let foreign_catalog = db.catalog(scope_of(testwil), fly_in).await.unwrap();
        assert_eq!(foreign_catalog, by_key(core_catalog()));
        let own_catalog = db.catalog(other, fly_in).await.unwrap();
        assert_eq!(own_catalog.len(), core_catalog().len() + 1);
    }
}
