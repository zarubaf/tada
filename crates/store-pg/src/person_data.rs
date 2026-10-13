//! The `PersonDataStore` adapter: the mentions of a person and the data of its export (ADR 0073).
//!
//! The queries are checked at run time. A term becomes an `ILIKE` pattern in which `%`, `_` and `\` match only
//! themselves.

use async_trait::async_trait;
use jiff::Timestamp;
use jiff_sqlx::Timestamp as SqlxTimestamp;
use sqlx::types::Uuid;
use sqlx::{PgConnection, Row};
use tada_app::audit::AuditEvent;
use tada_app::caller::OrgScope;
use tada_app::domain::ids::{EventId, PersonId, UserId};
use tada_app::domain::redaction::{DisputeTarget, DisputeTargetKind};
use tada_app::person_data::{
    AccountData, ApiTokenData, AuditEventData, EventMembershipData, FoundMention, InvitationData,
    MentionKind, PersonDataStore, PersonRecords, SessionData, TelegramData,
};
use tada_app::store::StoreError;

use crate::Database;
use crate::audit::record;
use crate::dispute_notes::notes_of_targets;
use crate::error::{InvalidRow, store_error};
use crate::redacted;

/// The `ILIKE` pattern that finds `term` inside a text.
fn pattern(term: &str) -> String {
    let mut pattern = String::with_capacity(term.len() + 2);
    pattern.push('%');
    for c in term.chars() {
        if matches!(c, '%' | '_' | '\\') {
            pattern.push('\\');
        }
        pattern.push(c);
    }
    pattern.push('%');
    pattern
}

/// The rows of each table with free text that match a pattern of `$2`, and the commitments and decisions that name
/// the person `$3` by ID: as promisor, or as a party that decided.
/// A matched row has content; a reference by ID can be redacted. The text columns of a row are joined by lines.
/// A document version also matches by the name of its document and by the file name of the upload.
const MENTIONS: &str = "
    SELECT m.kind, m.id, m.event_id, m.text,
           EXISTS (SELECT 1 FROM dispute_note n
                   WHERE n.organization_id = $1 AND n.target_kind = m.kind AND n.target_id = m.id
                     AND n.redacted_at IS NULL) AS disputed
    FROM (
        SELECT 1 AS sort, 'source-item' AS kind, i.id, i.event_id, concat_ws(E'\\n', v.url, v.text) AS text,
               v.captured_at AS at
        FROM source_version v
        JOIN source_item i ON i.organization_id = v.organization_id AND i.id = v.source_item_id
        WHERE v.organization_id = $1 AND (v.text ILIKE ANY ($2) OR v.url ILIKE ANY ($2))
        UNION ALL
        SELECT 2, 'fact-version', v.id, f.event_id, v.value::text, v.created_at
        FROM fact_version v JOIN fact f ON f.organization_id = v.organization_id AND f.id = v.fact_id
        WHERE v.organization_id = $1 AND v.value::text ILIKE ANY ($2)
        UNION ALL
        SELECT 3, 'proposal', p.id, p.event_id, concat_ws(E'\\n', p.reason, p.operation::text), p.created_at
        FROM proposal p
        WHERE p.organization_id = $1 AND (p.reason ILIKE ANY ($2) OR p.operation::text ILIKE ANY ($2))
        UNION ALL
        SELECT 4, 'open-question', q.id, q.event_id, q.text, q.created_at
        FROM open_question q WHERE q.organization_id = $1 AND q.text ILIKE ANY ($2)
        UNION ALL
        SELECT 5, 'action', a.id, a.event_id, concat_ws(E'\\n', a.title, a.description), a.created_at
        FROM action a
        WHERE a.organization_id = $1 AND (a.title ILIKE ANY ($2) OR a.description ILIKE ANY ($2))
        UNION ALL
        SELECT 6, 'commitment', c.id, c.event_id, concat_ws(E'\\n', c.text, c.condition, c.firm_reason),
               c.created_at
        FROM commitment c
        WHERE c.organization_id = $1
          AND (c.text ILIKE ANY ($2) OR c.condition ILIKE ANY ($2) OR c.firm_reason ILIKE ANY ($2))
        UNION ALL
        SELECT 7, 'workstream', w.id, w.event_id, w.name, w.created_at
        FROM workstream w WHERE w.organization_id = $1 AND w.name ILIKE ANY ($2)
        UNION ALL
        SELECT 8, 'document-version', v.id, d.event_id,
               CASE WHEN v.redacted_at IS NULL THEN concat_ws(E'\\n', d.name, v.file_name, v.markdown) END,
               v.created_at
        FROM document_version v JOIN document d ON d.organization_id = v.organization_id AND d.id = v.document_id
        WHERE v.organization_id = $1
          AND (v.markdown ILIKE ANY ($2) OR v.file_name ILIKE ANY ($2) OR d.name ILIKE ANY ($2))
        UNION ALL
        SELECT 9, 'milestone', m.id, m.event_id, concat_ws(E'\\n', m.title, m.description), m.created_at
        FROM milestone m
        WHERE m.organization_id = $1 AND (m.title ILIKE ANY ($2) OR m.description ILIKE ANY ($2))
        UNION ALL
        SELECT 10, 'decision', d.id, d.event_id, concat_ws(E'\\n', d.wording, d.revoked_reason), d.created_at
        FROM decision d
        WHERE d.organization_id = $1 AND (d.wording ILIKE ANY ($2) OR d.revoked_reason ILIKE ANY ($2))
        UNION ALL
        SELECT 11, 'promisor', c.id, c.event_id,
               CASE WHEN c.redacted_at IS NULL THEN concat_ws(E'\\n', c.text, c.condition, c.firm_reason) END,
               c.created_at
        FROM commitment c WHERE c.organization_id = $1 AND c.person_id = $3
        UNION ALL
        SELECT 12, 'decision-party', d.id, d.event_id,
               CASE WHEN d.redacted_at IS NULL THEN concat_ws(E'\\n', d.wording, d.revoked_reason) END,
               d.created_at
        FROM decision d
        WHERE d.organization_id = $1
          AND EXISTS (SELECT 1 FROM decision_party p
                      WHERE p.organization_id = d.organization_id AND p.decision_id = d.id AND p.person_id = $3)
    ) AS m
    ORDER BY m.sort, m.at, m.id";

async fn find(
    conn: &mut PgConnection,
    scope: OrgScope,
    person: PersonId,
    terms: &[String],
) -> Result<Vec<FoundMention>, StoreError> {
    let patterns: Vec<String> = terms.iter().map(|term| pattern(term)).collect();
    let rows = sqlx::query(MENTIONS)
        .bind(scope.organization_id().as_uuid())
        .bind(&patterns)
        .bind(person.as_uuid())
        .fetch_all(conn)
        .await
        .map_err(store_error)?;
    rows.iter()
        .map(|row| {
            let kind: String = row.try_get("kind").map_err(store_error)?;
            let event_id: Option<Uuid> = row.try_get("event_id").map_err(store_error)?;
            Ok(FoundMention {
                kind: kind.parse().map_err(|_| InvalidRow("mention.kind"))?,
                id: row.try_get("id").map_err(store_error)?,
                event_id: event_id.map(EventId::from_uuid),
                text: redacted::content(row.try_get("text").map_err(store_error)?),
                disputed: row.try_get("disputed").map_err(store_error)?,
            })
        })
        .collect()
}

/// The target of a dispute note that a mention names, if a note can be about it.
fn dispute_target(found: &FoundMention) -> Option<DisputeTarget> {
    let kind = match found.kind {
        MentionKind::SourceItem => DisputeTargetKind::SourceItem,
        MentionKind::FactVersion => DisputeTargetKind::FactVersion,
        MentionKind::DocumentVersion => DisputeTargetKind::DocumentVersion,
        MentionKind::Proposal => DisputeTargetKind::Proposal,
        _ => return None,
    };
    Some(DisputeTarget { kind, id: found.id })
}

fn time(value: SqlxTimestamp) -> Timestamp {
    value.to_jiff()
}

/// The account data of `user` in the organization. `None` if the account does not exist.
async fn account(
    conn: &mut PgConnection,
    scope: OrgScope,
    user: UserId,
) -> Result<Option<AccountData>, StoreError> {
    let organization = scope.organization_id().as_uuid();
    let user = user.as_uuid();
    let Some(row) = sqlx::query(
        "SELECT u.display_name, e.email, m.role
         FROM app_user u
         LEFT JOIN email_identity e ON e.user_id = u.id
         LEFT JOIN organization_membership m ON m.user_id = u.id AND m.organization_id = $1
         WHERE u.id = $2",
    )
    .bind(organization)
    .bind(user)
    .fetch_optional(&mut *conn)
    .await
    .map_err(store_error)?
    else {
        return Ok(None);
    };
    let email: Option<String> = row.try_get("email").map_err(store_error)?;

    let event_memberships = sqlx::query(
        "SELECT event_id, event_role, created_at FROM event_membership
         WHERE organization_id = $1 AND user_id = $2 ORDER BY created_at, event_id",
    )
    .bind(organization)
    .bind(user)
    .fetch_all(&mut *conn)
    .await
    .map_err(store_error)?
    .iter()
    .map(|row| {
        Ok(EventMembershipData {
            event_id: row.try_get("event_id")?,
            event_role: row.try_get("event_role")?,
            created_at: time(row.try_get("created_at")?),
        })
    })
    .collect::<Result<Vec<_>, sqlx::Error>>()
    .map_err(store_error)?;

    let telegram =
        sqlx::query("SELECT telegram_user_id, linked_at FROM telegram_identity WHERE user_id = $1")
            .bind(user)
            .fetch_optional(&mut *conn)
            .await
            .map_err(store_error)?
            .map(|row| {
                Ok::<_, sqlx::Error>(TelegramData {
                    telegram_user_id: row.try_get("telegram_user_id")?,
                    linked_at: time(row.try_get("linked_at")?),
                })
            })
            .transpose()
            .map_err(store_error)?;

    // The sessions of this organization and those without one. A session of another organization stays out.
    let sessions = sqlx::query(
        "SELECT created_at, last_used_at, user_agent FROM session
         WHERE user_id = $2 AND (organization_id = $1 OR organization_id IS NULL)
         ORDER BY created_at",
    )
    .bind(organization)
    .bind(user)
    .fetch_all(&mut *conn)
    .await
    .map_err(store_error)?
    .iter()
    .map(|row| {
        Ok(SessionData {
            created_at: time(row.try_get("created_at")?),
            last_used_at: time(row.try_get("last_used_at")?),
            user_agent: row.try_get("user_agent")?,
        })
    })
    .collect::<Result<Vec<_>, sqlx::Error>>()
    .map_err(store_error)?;

    let api_tokens = sqlx::query(
        "SELECT id, name, scope, created_at, expires_at, last_used_at, revoked_at FROM api_token
         WHERE organization_id = $1 AND user_id = $2 ORDER BY created_at, id",
    )
    .bind(organization)
    .bind(user)
    .fetch_all(&mut *conn)
    .await
    .map_err(store_error)?
    .iter()
    .map(|row| {
        Ok(ApiTokenData {
            id: row.try_get("id")?,
            name: row.try_get("name")?,
            scope: row.try_get("scope")?,
            created_at: time(row.try_get("created_at")?),
            expires_at: time(row.try_get("expires_at")?),
            last_used_at: row
                .try_get::<Option<SqlxTimestamp>, _>("last_used_at")?
                .map(time),
            revoked_at: row
                .try_get::<Option<SqlxTimestamp>, _>("revoked_at")?
                .map(time),
        })
    })
    .collect::<Result<Vec<_>, sqlx::Error>>()
    .map_err(store_error)?;

    let invitations = sqlx::query(
        "SELECT id, email, display_name, role, status, created_at, accepted_at, revoked_at FROM invitation
         WHERE organization_id = $1 AND lower(email) = lower($2) ORDER BY created_at, id",
    )
    .bind(organization)
    .bind(email.as_deref())
    .fetch_all(&mut *conn)
    .await
    .map_err(store_error)?
    .iter()
    .map(|row| {
        Ok(InvitationData {
            id: row.try_get("id")?,
            email: row.try_get("email")?,
            display_name: row.try_get("display_name")?,
            role: row.try_get("role")?,
            status: row.try_get("status")?,
            created_at: time(row.try_get("created_at")?),
            accepted_at: row
                .try_get::<Option<SqlxTimestamp>, _>("accepted_at")?
                .map(time),
            revoked_at: row
                .try_get::<Option<SqlxTimestamp>, _>("revoked_at")?
                .map(time),
        })
    })
    .collect::<Result<Vec<_>, sqlx::Error>>()
    .map_err(store_error)?;

    let audit_events = sqlx::query(
        "SELECT id, occurred_at, action, record_kind, record_id FROM audit_event
         WHERE organization_id = $1 AND (actor_id = $2 OR principal_id = $2 OR subject_user_id = $2)
         ORDER BY occurred_at, id",
    )
    .bind(organization)
    .bind(user)
    .fetch_all(&mut *conn)
    .await
    .map_err(store_error)?
    .iter()
    .map(|row| {
        Ok(AuditEventData {
            id: row.try_get("id")?,
            occurred_at: time(row.try_get("occurred_at")?),
            action: row.try_get("action")?,
            record_kind: row.try_get("record_kind")?,
            record_id: row.try_get("record_id")?,
        })
    })
    .collect::<Result<Vec<_>, sqlx::Error>>()
    .map_err(store_error)?;

    Ok(Some(AccountData {
        user_id: user,
        display_name: row.try_get("display_name").map_err(store_error)?,
        email,
        organization_role: row.try_get("role").map_err(store_error)?,
        event_memberships,
        telegram,
        sessions,
        api_tokens,
        invitations,
        audit_events,
    }))
}

#[async_trait]
impl PersonDataStore for Database {
    async fn find_mentions(
        &self,
        scope: OrgScope,
        person: PersonId,
        terms: &[String],
    ) -> Result<Vec<FoundMention>, StoreError> {
        let mut conn = self.pool.acquire().await.map_err(store_error)?;
        find(&mut conn, scope, person, terms).await
    }

    async fn export(
        &self,
        scope: OrgScope,
        person: PersonId,
        user: Option<UserId>,
        terms: &[String],
        audit: &AuditEvent,
    ) -> Result<PersonRecords, StoreError> {
        let mut tx = self.pool.begin().await.map_err(store_error)?;
        // One snapshot for all reads of the export.
        sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ")
            .execute(&mut *tx)
            .await
            .map_err(store_error)?;
        let organization_name: String =
            sqlx::query_scalar("SELECT name FROM organization WHERE id = $1")
                .bind(scope.organization_id().as_uuid())
                .fetch_one(&mut *tx)
                .await
                .map_err(store_error)?;
        let mentions = find(&mut tx, scope, person, terms).await?;
        let targets: Vec<DisputeTarget> = mentions.iter().filter_map(dispute_target).collect();
        let dispute_notes = notes_of_targets(&mut tx, scope, &targets).await?;
        let account = match user {
            Some(user) => account(&mut tx, scope, user).await?,
            None => None,
        };
        record(&mut tx, audit).await.map_err(store_error)?;
        tx.commit().await.map_err(store_error)?;
        Ok(PersonRecords {
            organization_name,
            mentions,
            dispute_notes,
            account,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pattern_escapes_the_wildcards_and_the_escape() {
        assert_eq!(pattern("Beat Muster"), "%Beat Muster%");
        assert_eq!(pattern("100%_x\\y"), "%100\\%\\_x\\\\y%");
    }
}
