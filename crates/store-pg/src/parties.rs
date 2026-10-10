//! The `PartyStore` adapter: persons and institutions (ADR 0069).

use async_trait::async_trait;
use jiff::Timestamp;
use jiff_sqlx::ToSqlx;
use sqlx::PgConnection;
use sqlx::types::Uuid;
use tada_app::audit::AuditEvent;
use tada_app::caller::OrgScope;
use tada_app::domain::RecordVersion;
use tada_app::domain::identity::Email;
use tada_app::domain::ids::{InstitutionId, LocalIdKind, PersonId, UserId};
use tada_app::domain::parties::{InstitutionKind, Party, PartyName, PhoneNumber, normalized_name};
use tada_app::parties::{
    InstitutionFields, InstitutionView, PartyRef, PartyStore, PersonFields, PersonView, names_match,
};
use tada_app::records::{Changed, Created, NumberCursor};
use tada_app::store::StoreError;

use crate::Database;
use crate::audit;
use crate::error::{InvalidRow, store_error, violates};
use crate::local_ids::next_local_number;

struct PersonRow {
    id: Uuid,
    local_number: i64,
    name: String,
    email: Option<String>,
    phone: Option<String>,
    user_id: Option<Uuid>,
    version: i64,
}

impl TryFrom<PersonRow> for PersonView {
    type Error = InvalidRow;

    fn try_from(row: PersonRow) -> Result<Self, InvalidRow> {
        Ok(Self {
            id: PersonId::from_uuid(row.id),
            local_number: u64::try_from(row.local_number)
                .map_err(|_| InvalidRow("person.local_number"))?,
            name: PartyName::parse(&row.name).map_err(|_| InvalidRow("person.name"))?,
            email: parse_email(row.email, "person.email")?,
            phone: parse_phone(row.phone, "person.phone")?,
            user_id: row.user_id.map(UserId::from_uuid),
            version: RecordVersion::new(row.version).ok_or(InvalidRow("person.version"))?,
        })
    }
}

struct InstitutionRow {
    id: Uuid,
    local_number: i64,
    name: String,
    kind: String,
    email: Option<String>,
    phone: Option<String>,
    version: i64,
}

impl TryFrom<InstitutionRow> for InstitutionView {
    type Error = InvalidRow;

    fn try_from(row: InstitutionRow) -> Result<Self, InvalidRow> {
        Ok(Self {
            id: InstitutionId::from_uuid(row.id),
            local_number: u64::try_from(row.local_number)
                .map_err(|_| InvalidRow("institution.local_number"))?,
            name: PartyName::parse(&row.name).map_err(|_| InvalidRow("institution.name"))?,
            kind: InstitutionKind::parse(&row.kind).ok_or(InvalidRow("institution.kind"))?,
            email: parse_email(row.email, "institution.email")?,
            phone: parse_phone(row.phone, "institution.phone")?,
            version: RecordVersion::new(row.version).ok_or(InvalidRow("institution.version"))?,
        })
    }
}

fn parse_email(text: Option<String>, column: &'static str) -> Result<Option<Email>, InvalidRow> {
    text.map(|text| Email::parse(&text).map_err(|_| InvalidRow(column)))
        .transpose()
}

fn parse_phone(
    text: Option<String>,
    column: &'static str,
) -> Result<Option<PhoneNumber>, InvalidRow> {
    text.map(|text| PhoneNumber::parse(&text).map_err(|_| InvalidRow(column)))
        .transpose()
}

/// The rows of `query` in the order of their numbers, then at most `limit` of the rows whose
/// normalized name contains the normalized `query`. Club scale: the filter runs here, not in SQL,
/// because the normalization (ADR 0069) is a Rust function.
fn filtered<T>(
    items: impl IntoIterator<Item = T>,
    name: impl Fn(&T) -> &str,
    query: Option<&str>,
    limit: u32,
) -> Vec<T> {
    items
        .into_iter()
        .filter(|item| query.is_none_or(|query| normalized_name(name(item)).contains(query)))
        .take(limit as usize)
        .collect()
}

/// The first number that a page starts after. A cursor beyond the largest number gives an empty page.
fn after_number(after: Option<NumberCursor>) -> i64 {
    after.map_or(0, |cursor| i64::try_from(cursor.0).unwrap_or(i64::MAX))
}

/// Inserts a person with the version 1 and the next number of the organization, and returns the number.
/// The direct command and the apply of a proposal both write a new person here (ADR 0069).
pub(crate) async fn insert_person(
    conn: &mut PgConnection,
    scope: OrgScope,
    id: PersonId,
    fields: &PersonFields,
    at: Timestamp,
) -> Result<i64, sqlx::Error> {
    let organization = scope.organization_id().as_uuid();
    let number = next_local_number(conn, scope, organization, LocalIdKind::Person.prefix()).await?;
    sqlx::query!(
        "INSERT INTO person
             (id, organization_id, local_number, name, email, phone, user_id, version,
              created_at, updated_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7, 1, $8, $8)",
        id.as_uuid(),
        organization,
        number,
        fields.name.as_str(),
        fields.email.as_ref().map(Email::as_str),
        fields.phone.as_ref().map(PhoneNumber::as_str),
        fields.user_id.map(UserId::as_uuid),
        at.to_sqlx() as _,
    )
    .execute(&mut *conn)
    .await?;
    Ok(number)
}

/// Inserts an institution with the version 1 and the next number of the organization, and returns the number.
/// The direct command and the apply of a proposal both write a new institution here (ADR 0069).
pub(crate) async fn insert_institution(
    conn: &mut PgConnection,
    scope: OrgScope,
    id: InstitutionId,
    fields: &InstitutionFields,
    at: Timestamp,
) -> Result<i64, sqlx::Error> {
    let organization = scope.organization_id().as_uuid();
    let number =
        next_local_number(conn, scope, organization, LocalIdKind::Institution.prefix()).await?;
    sqlx::query!(
        "INSERT INTO institution
             (id, organization_id, local_number, name, kind, email, phone, version,
              created_at, updated_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7, 1, $8, $8)",
        id.as_uuid(),
        organization,
        number,
        fields.name.as_str(),
        fields.kind.as_str(),
        fields.email.as_ref().map(Email::as_str),
        fields.phone.as_ref().map(PhoneNumber::as_str),
        at.to_sqlx() as _,
    )
    .execute(&mut *conn)
    .await?;
    Ok(number)
}

#[async_trait]
impl PartyStore for Database {
    async fn create_person(
        &self,
        scope: OrgScope,
        id: PersonId,
        fields: &PersonFields,
        at: Timestamp,
        audit: &AuditEvent,
    ) -> Result<Created<PersonView>, StoreError> {
        let mut tx = self.pool.begin().await.map_err(store_error)?;
        let number = match insert_person(&mut tx, scope, id, fields, at).await {
            Ok(number) => number,
            Err(error) if violates(&error, "person_pkey") => return Ok(Created::IdTaken),
            Err(error) => return Err(store_error(error)),
        };
        audit::record(&mut tx, audit).await.map_err(store_error)?;
        tx.commit().await.map_err(store_error)?;
        Ok(Created::Created(PersonView {
            id,
            local_number: u64::try_from(number).map_err(|_| InvalidRow("person.local_number"))?,
            name: fields.name.clone(),
            email: fields.email.clone(),
            phone: fields.phone.clone(),
            user_id: fields.user_id,
            version: RecordVersion::FIRST,
        }))
    }

    async fn change_person(
        &self,
        scope: OrgScope,
        id: PersonId,
        fields: &PersonFields,
        expected: RecordVersion,
        at: Timestamp,
        audit: &AuditEvent,
    ) -> Result<Changed<PersonView>, StoreError> {
        let organization = scope.organization_id().as_uuid();
        let mut tx = self.pool.begin().await.map_err(store_error)?;
        let row = sqlx::query_as!(
            PersonRow,
            "UPDATE person
             SET name = $4, email = $5, phone = $6, user_id = $7, version = version + 1,
                 updated_at = $8
             WHERE organization_id = $1 AND id = $2 AND version = $3
             RETURNING id, local_number, name, email, phone, user_id, version",
            organization,
            id.as_uuid(),
            expected.get(),
            fields.name.as_str(),
            fields.email.as_ref().map(Email::as_str),
            fields.phone.as_ref().map(PhoneNumber::as_str),
            fields.user_id.map(UserId::as_uuid),
            at.to_sqlx() as _,
        )
        .fetch_optional(&mut *tx)
        .await
        .map_err(store_error)?;
        let Some(row) = row else {
            let exists = sqlx::query_scalar!(
                r#"SELECT EXISTS (SELECT 1 FROM person WHERE organization_id = $1 AND id = $2) AS "exists!""#,
                organization,
                id.as_uuid(),
            )
            .fetch_one(&mut *tx)
            .await
            .map_err(store_error)?;
            return Ok(if exists {
                Changed::VersionConflict
            } else {
                Changed::NotFound
            });
        };
        audit::record(&mut tx, audit).await.map_err(store_error)?;
        tx.commit().await.map_err(store_error)?;
        Ok(Changed::Changed(row.try_into()?))
    }

    async fn person(
        &self,
        scope: OrgScope,
        id: PersonId,
    ) -> Result<Option<PersonView>, StoreError> {
        let row = sqlx::query_as!(
            PersonRow,
            "SELECT id, local_number, name, email, phone, user_id, version
             FROM person WHERE organization_id = $1 AND id = $2",
            scope.organization_id().as_uuid(),
            id.as_uuid(),
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(store_error)?;
        Ok(row.map(PersonView::try_from).transpose()?)
    }

    async fn persons(
        &self,
        scope: OrgScope,
        query: Option<&str>,
        after: Option<NumberCursor>,
        limit: u32,
    ) -> Result<Vec<PersonView>, StoreError> {
        let rows = sqlx::query_as!(
            PersonRow,
            "SELECT id, local_number, name, email, phone, user_id, version
             FROM person WHERE organization_id = $1 AND local_number > $2
             ORDER BY local_number",
            scope.organization_id().as_uuid(),
            after_number(after),
        )
        .fetch_all(&self.pool)
        .await
        .map_err(store_error)?;
        let rows = filtered(rows, |row| row.name.as_str(), query, limit);
        Ok(rows
            .into_iter()
            .map(PersonView::try_from)
            .collect::<Result<_, _>>()?)
    }

    async fn create_institution(
        &self,
        scope: OrgScope,
        id: InstitutionId,
        fields: &InstitutionFields,
        at: Timestamp,
        audit: &AuditEvent,
    ) -> Result<Created<InstitutionView>, StoreError> {
        let mut tx = self.pool.begin().await.map_err(store_error)?;
        let number = match insert_institution(&mut tx, scope, id, fields, at).await {
            Ok(number) => number,
            Err(error) if violates(&error, "institution_pkey") => return Ok(Created::IdTaken),
            Err(error) => return Err(store_error(error)),
        };
        audit::record(&mut tx, audit).await.map_err(store_error)?;
        tx.commit().await.map_err(store_error)?;
        Ok(Created::Created(InstitutionView {
            id,
            local_number: u64::try_from(number)
                .map_err(|_| InvalidRow("institution.local_number"))?,
            name: fields.name.clone(),
            kind: fields.kind,
            email: fields.email.clone(),
            phone: fields.phone.clone(),
            version: RecordVersion::FIRST,
        }))
    }

    async fn change_institution(
        &self,
        scope: OrgScope,
        id: InstitutionId,
        fields: &InstitutionFields,
        expected: RecordVersion,
        at: Timestamp,
        audit: &AuditEvent,
    ) -> Result<Changed<InstitutionView>, StoreError> {
        let organization = scope.organization_id().as_uuid();
        let mut tx = self.pool.begin().await.map_err(store_error)?;
        let row = sqlx::query_as!(
            InstitutionRow,
            "UPDATE institution
             SET name = $4, kind = $5, email = $6, phone = $7, version = version + 1,
                 updated_at = $8
             WHERE organization_id = $1 AND id = $2 AND version = $3
             RETURNING id, local_number, name, kind, email, phone, version",
            organization,
            id.as_uuid(),
            expected.get(),
            fields.name.as_str(),
            fields.kind.as_str(),
            fields.email.as_ref().map(Email::as_str),
            fields.phone.as_ref().map(PhoneNumber::as_str),
            at.to_sqlx() as _,
        )
        .fetch_optional(&mut *tx)
        .await
        .map_err(store_error)?;
        let Some(row) = row else {
            let exists = sqlx::query_scalar!(
                r#"SELECT EXISTS (SELECT 1 FROM institution WHERE organization_id = $1 AND id = $2) AS "exists!""#,
                organization,
                id.as_uuid(),
            )
            .fetch_one(&mut *tx)
            .await
            .map_err(store_error)?;
            return Ok(if exists {
                Changed::VersionConflict
            } else {
                Changed::NotFound
            });
        };
        audit::record(&mut tx, audit).await.map_err(store_error)?;
        tx.commit().await.map_err(store_error)?;
        Ok(Changed::Changed(row.try_into()?))
    }

    async fn institution(
        &self,
        scope: OrgScope,
        id: InstitutionId,
    ) -> Result<Option<InstitutionView>, StoreError> {
        let row = sqlx::query_as!(
            InstitutionRow,
            "SELECT id, local_number, name, kind, email, phone, version
             FROM institution WHERE organization_id = $1 AND id = $2",
            scope.organization_id().as_uuid(),
            id.as_uuid(),
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(store_error)?;
        Ok(row.map(InstitutionView::try_from).transpose()?)
    }

    async fn institutions(
        &self,
        scope: OrgScope,
        query: Option<&str>,
        after: Option<NumberCursor>,
        limit: u32,
    ) -> Result<Vec<InstitutionView>, StoreError> {
        let rows = sqlx::query_as!(
            InstitutionRow,
            "SELECT id, local_number, name, kind, email, phone, version
             FROM institution WHERE organization_id = $1 AND local_number > $2
             ORDER BY local_number",
            scope.organization_id().as_uuid(),
            after_number(after),
        )
        .fetch_all(&self.pool)
        .await
        .map_err(store_error)?;
        let rows = filtered(rows, |row| row.name.as_str(), query, limit);
        Ok(rows
            .into_iter()
            .map(InstitutionView::try_from)
            .collect::<Result<_, _>>()?)
    }

    async fn named_like(
        &self,
        scope: OrgScope,
        normalized: &str,
    ) -> Result<Vec<PartyRef>, StoreError> {
        let organization = scope.organization_id().as_uuid();
        let persons = sqlx::query!(
            "SELECT id, local_number, name FROM person
             WHERE organization_id = $1 ORDER BY local_number",
            organization,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(store_error)?;
        let institutions = sqlx::query!(
            "SELECT id, local_number, name FROM institution
             WHERE organization_id = $1 ORDER BY local_number",
            organization,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(store_error)?;
        let candidates = persons
            .into_iter()
            .map(|row| {
                (
                    Party::Person(PersonId::from_uuid(row.id)),
                    LocalIdKind::Person,
                    row.local_number,
                    row.name,
                )
            })
            .chain(institutions.into_iter().map(|row| {
                (
                    Party::Institution(InstitutionId::from_uuid(row.id)),
                    LocalIdKind::Institution,
                    row.local_number,
                    row.name,
                )
            }));
        let mut found = Vec::new();
        for (party, kind, number, name) in candidates {
            if !names_match(normalized, &normalized_name(&name)) {
                continue;
            }
            let number = u64::try_from(number).map_err(|_| InvalidRow("local_number"))?;
            found.push(PartyRef {
                party,
                local_id: kind.readable_id(number),
                name: PartyName::parse(&name).map_err(|_| InvalidRow("name"))?,
            });
        }
        Ok(found)
    }
}
