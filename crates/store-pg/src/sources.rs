//! The `SourceStore` adapter (ADR 0050): source items, source versions and their full-text search.

use async_trait::async_trait;
use jiff::Timestamp;
use jiff_sqlx::ToSqlx;
use sha2::{Digest, Sha256};
use sqlx::types::Uuid;
use sqlx::{PgConnection, PgPool};
use tada_app::access::SourceReach;
use tada_app::caller::{Actor, OrgScope};
use tada_app::domain::ids::{EventId, SourceItemId, SourceVersionId};
use tada_app::domain::sources::SourceText;
use tada_app::sources::{SourceHit, SourceStore, SourceVersionRef, SourceVersionText};
use tada_app::store::StoreError;

use crate::Database;
use crate::actor;
use crate::error::{InvalidRow, store_error};

/// The kind of a source item and of its source version that holds a text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TextKind {
    /// The text of a member.
    MemberText,
    /// A value that a reviewer edited (ADR 0050).
    Review,
}

impl TextKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::MemberText => "member-text",
            Self::Review => "review",
        }
    }
}

/// The new source item of a text with its one source version `version`.
/// An item without an event belongs to the organization.
#[derive(Debug, Clone, Copy)]
pub(crate) struct TextItem {
    pub kind: TextKind,
    pub event: Option<EventId>,
    pub version: SourceVersionId,
}

/// The number of characters of a snippet before and after the first word that matches.
const SNIPPET_CONTEXT: usize = 80;

#[async_trait]
impl SourceStore for Database {
    async fn add_member_text(
        &self,
        scope: OrgScope,
        event: EventId,
        text: &SourceText,
        actor: &Actor,
        now: Timestamp,
    ) -> Result<SourceVersionRef, StoreError> {
        let mut tx = self.pool.begin().await.map_err(store_error)?;
        let version = SourceVersionId::from_uuid(Uuid::now_v7());
        let item = TextItem {
            kind: TextKind::MemberText,
            event: Some(event),
            version,
        };
        let stored = insert_text(&mut tx, scope, item, text, actor, now)
            .await
            .map_err(store_error)?;
        tx.commit().await.map_err(store_error)?;
        Ok(stored)
    }

    async fn search(
        &self,
        scope: OrgScope,
        reach: &SourceReach,
        query: &str,
        limit: u32,
    ) -> Result<Vec<SourceHit>, StoreError> {
        let rows = readable_versions(&self.pool, scope, reach, None, Some(query), limit).await?;
        // A source version without text never matches a query; the filter only drops the `None`.
        rows.into_iter()
            .filter_map(|row| Some((row.id, row.captured_at, row.text?)))
            .map(|(id, captured_at, text)| {
                let (start, end) = snippet_range(&text, query);
                let offset = |chars: usize| {
                    u32::try_from(chars)
                        .map_err(|_| StoreError::from(InvalidRow("source_version.text")))
                };
                Ok(SourceHit {
                    source_version_id: SourceVersionId::from_uuid(id),
                    captured_at: captured_at.to_jiff(),
                    snippet: text.chars().skip(start).take(end - start).collect(),
                    start: offset(start)?,
                    end: offset(end)?,
                })
            })
            .collect()
    }

    async fn readable_text(
        &self,
        scope: OrgScope,
        reach: &SourceReach,
        id: SourceVersionId,
    ) -> Result<Option<String>, StoreError> {
        let rows =
            readable_versions(&self.pool, scope, reach, Some(&[id.as_uuid()]), None, 1).await?;
        Ok(rows.into_iter().find_map(|row| row.text))
    }

    async fn texts(
        &self,
        scope: OrgScope,
        reach: &SourceReach,
        ids: &[SourceVersionId],
    ) -> Result<Vec<SourceVersionText>, StoreError> {
        let ids: Vec<Uuid> = ids.iter().map(|id| id.as_uuid()).collect();
        let limit = u32::try_from(ids.len()).unwrap_or(u32::MAX);
        let rows = readable_versions(&self.pool, scope, reach, Some(&ids), None, limit).await?;
        Ok(rows
            .into_iter()
            .map(|row| SourceVersionText {
                id: SourceVersionId::from_uuid(row.id),
                // The stored text is normalized already; normalizing it again keeps it unchanged.
                text: row.text.as_deref().map(SourceText::normalize),
            })
            .collect())
    }
}

/// A source version that a caller can read. A file without text, for example a PDF, has no text.
struct ReadableVersion {
    id: Uuid,
    captured_at: jiff_sqlx::Timestamp,
    text: Option<String>,
}

/// The source versions inside `reach`: the one query that applies the rule of `SourceReach`.
/// `ids` limits the result to these source versions, and `query` to the matches of a web-search query, the best first.
/// A source version without text never matches a query.
async fn readable_versions(
    pool: &PgPool,
    scope: OrgScope,
    reach: &SourceReach,
    ids: Option<&[Uuid]>,
    query: Option<&str>,
    limit: u32,
) -> Result<Vec<ReadableVersion>, StoreError> {
    let (organization, events) = match reach {
        SourceReach::Organization => (true, Vec::new()),
        SourceReach::Events(events) => {
            (false, events.iter().map(|event| event.as_uuid()).collect())
        }
    };
    if !organization && events.is_empty() {
        return Ok(Vec::new());
    }
    sqlx::query_as!(
        ReadableVersion,
        r#"SELECT v.id, v.captured_at AS "captured_at: jiff_sqlx::Timestamp", v.text
           FROM source_version v
           JOIN source_item i ON i.organization_id = v.organization_id AND i.id = v.source_item_id
           WHERE v.organization_id = $1
             AND ($4::uuid[] IS NULL OR v.id = ANY($4))
             AND ($5::text IS NULL OR v.search @@ websearch_to_tsquery('simple', $5))
             AND ($2 OR i.event_id = ANY($3)
                  OR EXISTS (SELECT 1 FROM evidence_link e
                             JOIN fact_version fv ON fv.organization_id = e.organization_id AND fv.id = e.fact_version_id
                             JOIN fact f ON f.organization_id = fv.organization_id AND f.id = fv.fact_id
                             WHERE e.organization_id = v.organization_id AND e.source_version_id = v.id
                               AND f.event_id = ANY($3))
                  OR EXISTS (SELECT 1 FROM proposal_evidence pe
                             JOIN proposal p ON p.organization_id = pe.organization_id AND p.id = pe.proposal_id
                             WHERE pe.organization_id = v.organization_id AND pe.source_version_id = v.id
                               AND p.event_id = ANY($3)))
           ORDER BY CASE WHEN $5::text IS NULL THEN 0 ELSE ts_rank(v.search, websearch_to_tsquery('simple', $5)) END DESC, v.captured_at DESC, v.id
           LIMIT $6"#,
        scope.organization_id().as_uuid(),
        organization,
        &events,
        ids,
        query,
        i64::from(limit),
    )
    .fetch_all(pool)
    .await
    .map_err(store_error)
}

/// Stores a text as the new source item `item`, inside the transaction of the caller.
pub(crate) async fn insert_text(
    conn: &mut PgConnection,
    scope: OrgScope,
    item: TextItem,
    text: &SourceText,
    actor: &Actor,
    now: Timestamp,
) -> Result<SourceVersionRef, sqlx::Error> {
    let organization = scope.organization_id().as_uuid();
    let TextItem {
        kind,
        event,
        version,
    } = item;
    let item = Uuid::now_v7();
    let sha256: [u8; 32] = Sha256::digest(text.as_str().as_bytes()).into();
    sqlx::query!(
        "INSERT INTO source_item (id, organization_id, event_id, kind, created_at)
         VALUES ($1, $2, $3, $4, $5)",
        item,
        organization,
        event.map(EventId::as_uuid),
        kind.as_str(),
        now.to_sqlx() as _,
    )
    .execute(&mut *conn)
    .await?;
    sqlx::query!(
        "INSERT INTO source_version
             (id, organization_id, source_item_id, kind, channel, author_actor, text, sha256, captured_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
        version.as_uuid(),
        organization,
        item,
        kind.as_str(),
        actor.channel().as_str(),
        actor::to_json(actor),
        text.as_str(),
        &sha256[..],
        now.to_sqlx() as _,
    )
    .execute(&mut *conn)
    .await?;
    Ok(SourceVersionRef {
        id: version,
        source_item_id: SourceItemId::from_uuid(item),
        sha256,
        captured_at: now,
    })
}

/// The character range of the snippet of `text`: the first word of `query` in `text`, with up to
/// `SNIPPET_CONTEXT` characters of its line before and after it.
/// The search configuration `simple` compares lowercase words, so this comparison does the same.
/// Without such a word, the snippet is the start of the text.
fn snippet_range(text: &str, query: &str) -> (usize, usize) {
    let terms: Vec<String> = words(query).map(|(_, word)| word.to_lowercase()).collect();
    let chars: Vec<char> = text.chars().collect();
    let Some((start, end)) = words(text)
        .find(|(_, word)| terms.contains(&word.to_lowercase()))
        .map(|(start, word)| (start, start + word.chars().count()))
    else {
        return (0, chars.len().min(2 * SNIPPET_CONTEXT));
    };
    let mut from = start.saturating_sub(SNIPPET_CONTEXT);
    if let Some(newline) = chars[from..start].iter().rposition(|&c| c == '\n') {
        from += newline + 1;
    }
    let mut to = (end + SNIPPET_CONTEXT).min(chars.len());
    if let Some(newline) = chars[end..to].iter().position(|&c| c == '\n') {
        to = end + newline;
    }
    (from, to)
}

/// The words of `text`: runs of letters and digits, with the character offset of their first character.
fn words(text: &str) -> impl Iterator<Item = (usize, String)> + '_ {
    let mut chars = text.chars().enumerate().peekable();
    std::iter::from_fn(move || {
        while chars.next_if(|(_, c)| !c.is_alphanumeric()).is_some() {}
        let (start, first) = chars.next()?;
        let mut word = String::from(first);
        while let Some((_, c)) = chars.next_if(|(_, c)| c.is_alphanumeric()) {
            word.push(c);
        }
        Some((start, word))
    })
}

#[cfg(test)]
mod tests {
    use sha2::{Digest, Sha256};
    use tada_app::caller::{MemberCaller, OrganizationRole};
    use tada_app::domain::ids::{OrganizationId, UserId};
    use tada_app::domain::sources::Passage;

    use super::*;
    use crate::testing::TestDatabase;

    fn member(organization: OrganizationId) -> MemberCaller {
        MemberCaller::new(
            UserId::from_uuid(Uuid::now_v7()),
            organization,
            OrganizationRole::Member,
        )
    }

    async fn add(
        test: &TestDatabase,
        caller: &MemberCaller,
        event: EventId,
        text: &str,
    ) -> SourceVersionRef {
        test.database
            .add_member_text(
                caller.scope(),
                event,
                &SourceText::normalize(text),
                &caller.actor(),
                "2030-05-18T08:00:00.123456Z".parse().unwrap(),
            )
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn the_hash_of_a_text_is_stable() {
        let test = TestDatabase::start().await;
        let testwil = test.create_organization("testwil").await;
        let event = test.create_event(testwil, "TEST30").await;
        let anna = member(testwil);

        let first = add(
            &test,
            &anna,
            event,
            "Das Flugfeld o\u{308}ffnet.\r\nEintritt gratis.",
        )
        .await;
        let second = add(
            &test,
            &anna,
            event,
            "Das Flugfeld öffnet.\nEintritt gratis.",
        )
        .await;
        assert_ne!(first.id, second.id);
        assert_ne!(first.source_item_id, second.source_item_id);
        assert_eq!(first.sha256, second.sha256);
        let expected: [u8; 32] =
            Sha256::digest("Das Flugfeld öffnet.\nEintritt gratis.".as_bytes()).into();
        assert_eq!(first.sha256, expected);

        let (text, channel, kind, author): (String, String, String, serde_json::Value) =
            sqlx::query_as(
                "SELECT text, channel, kind, author_actor FROM source_version WHERE id = $1",
            )
            .bind(first.id.as_uuid())
            .fetch_one(&test.database.pool)
            .await
            .unwrap();
        assert_eq!(text, "Das Flugfeld öffnet.\nEintritt gratis.");
        assert_eq!((channel.as_str(), kind.as_str()), ("web", "member-text"));
        assert_eq!(crate::actor::from_json(&author).unwrap(), anna.actor());
    }

    #[test]
    fn a_snippet_keeps_the_context_of_the_first_word_inside_its_line() {
        let long = format!("{} Flugfeld {}", "a".repeat(100), "b".repeat(100));
        let start = 101 - SNIPPET_CONTEXT;
        assert_eq!(
            snippet_range(&long, "flugfeld"),
            (start, 109 + SNIPPET_CONTEXT)
        );
        let lines = "Termin offen.\nDas Flugfeld ist frei.\nKein Treffer.";
        assert_eq!(snippet_range(lines, "FLUGFELD frei"), (14, 36));
        assert_eq!(snippet_range("Kein Treffer.", "flugfeld"), (0, 13));
    }

    fn events(ids: &[EventId]) -> SourceReach {
        SourceReach::Events(ids.to_vec())
    }

    /// Stores `text` as a source version of the organization of `caller` without an event,
    /// as an organization changeset stores its intake text.
    async fn add_to_organization(
        test: &TestDatabase,
        caller: &MemberCaller,
        text: &str,
    ) -> SourceVersionId {
        let mut conn = test.database.pool.acquire().await.unwrap();
        let item = TextItem {
            kind: TextKind::MemberText,
            event: None,
            version: SourceVersionId::from_uuid(Uuid::now_v7()),
        };
        let now = "2030-05-18T08:00:00Z".parse().unwrap();
        insert_text(
            &mut conn,
            caller.scope(),
            item,
            &SourceText::normalize(text),
            &caller.actor(),
            now,
        )
        .await
        .unwrap()
        .id
    }

    /// Cites `source` as evidence of an unknown fact of the field `date_window` in `event`.
    async fn cite_in_fact(
        test: &TestDatabase,
        caller: &MemberCaller,
        event: EventId,
        source: SourceVersionId,
    ) {
        let field = tada_app::domain::facts::core_catalog()[0].id.as_uuid();
        let (organization, fact, version) = (
            caller.scope().organization_id().as_uuid(),
            Uuid::now_v7(),
            Uuid::now_v7(),
        );
        let pool = &test.database.pool;
        sqlx::query("INSERT INTO fact (id, organization_id, event_id, field_id, version) VALUES ($1, $2, $3, $4, 1)")
            .bind(fact).bind(organization).bind(event.as_uuid()).bind(field)
            .execute(pool).await.unwrap();
        sqlx::query(
            "INSERT INTO fact_version (id, organization_id, fact_id, number, state, approximate, created_at, accepted_by)
             VALUES ($1, $2, $3, 1, 'unknown', false, now(), $4)",
        )
        .bind(version).bind(organization).bind(fact).bind(crate::actor::to_json(&caller.actor()))
        .execute(pool).await.unwrap();
        sqlx::query(
            "INSERT INTO evidence_link (id, organization_id, fact_version_id, source_version_id, start_offset, end_offset, quote)
             VALUES ($1, $2, $3, $4, 0, 3, 'Das')",
        )
        .bind(Uuid::now_v7()).bind(organization).bind(version).bind(source.as_uuid())
        .execute(pool).await.unwrap();
    }

    /// Cites `source` as evidence of an open proposal in `event`.
    async fn cite_in_proposal(
        test: &TestDatabase,
        caller: &MemberCaller,
        event: EventId,
        source: SourceVersionId,
    ) {
        let (organization, changeset, proposal) = (
            caller.scope().organization_id().as_uuid(),
            Uuid::now_v7(),
            Uuid::now_v7(),
        );
        let pool = &test.database.pool;
        sqlx::query(
            "INSERT INTO changeset (id, organization_id, event_id, author, source_version_id, created_at)
             VALUES ($1, $2, NULL, $3, $4, now())",
        )
        .bind(changeset).bind(organization).bind(crate::actor::to_json(&caller.actor())).bind(source.as_uuid())
        .execute(pool).await.unwrap();
        sqlx::query(
            "INSERT INTO proposal (id, organization_id, changeset_id, event_id, operation, operation_version, target_kind, target_id, reason, created_at)
             VALUES ($1, $2, $3, $4, '{}', 1, 'event', $4, 'test', now())",
        )
        .bind(proposal).bind(organization).bind(changeset).bind(event.as_uuid())
        .execute(pool).await.unwrap();
        sqlx::query(
            "INSERT INTO proposal_evidence (id, organization_id, proposal_id, source_version_id, start_offset, end_offset, quote)
             VALUES ($1, $2, $3, $4, 0, 3, 'Das')",
        )
        .bind(Uuid::now_v7()).bind(organization).bind(proposal).bind(source.as_uuid())
        .execute(pool).await.unwrap();
    }

    /// A member reads a source version without an event when a fact or a proposal of its event cites it (ADR 0052).
    #[tokio::test]
    async fn an_event_reaches_the_organization_sources_that_its_evidence_cites() {
        let test = TestDatabase::start().await;
        let db = &test.database;
        let testwil = test.create_organization("testwil").await;
        let open_day = test.create_event(testwil, "TEST30").await;
        let workshop = test.create_event(testwil, "TEST31").await;
        let owner = MemberCaller::new(
            UserId::from_uuid(Uuid::now_v7()),
            testwil,
            OrganizationRole::Owner,
        );
        let by_fact = add_to_organization(&test, &owner, "Das Hangarfest im Juni.").await;
        let by_proposal = add_to_organization(&test, &owner, "Das Hangarfest im Juli.").await;
        let uncited = add_to_organization(&test, &owner, "Das Hangarfest im August.").await;
        cite_in_fact(&test, &owner, open_day, by_fact).await;
        cite_in_proposal(&test, &owner, open_day, by_proposal).await;

        let scope = owner.scope();
        let found = |reach: SourceReach| async move {
            let mut ids: Vec<_> = db
                .search(scope, &reach, "Hangarfest", 10)
                .await
                .unwrap()
                .into_iter()
                .map(|hit| hit.source_version_id)
                .collect();
            ids.sort();
            ids
        };
        let mut cited = vec![by_fact, by_proposal];
        cited.sort();
        assert_eq!(found(events(&[open_day])).await, cited);
        assert!(found(events(&[workshop])).await.is_empty());
        assert_eq!(found(SourceReach::Organization).await.len(), 3);

        let text = |reach: SourceReach, id| async move {
            db.readable_text(scope, &reach, id).await.unwrap()
        };
        assert_eq!(
            text(events(&[open_day]), by_fact).await.as_deref(),
            Some("Das Hangarfest im Juni.")
        );
        assert_eq!(
            text(events(&[open_day]), by_proposal).await.as_deref(),
            Some("Das Hangarfest im Juli.")
        );
        assert_eq!(text(events(&[open_day]), uncited).await, None);
        assert_eq!(text(events(&[workshop]), by_fact).await, None);
        assert_eq!(
            text(SourceReach::Organization, uncited).await.as_deref(),
            Some("Das Hangarfest im August.")
        );
    }

    /// The text of a source version stays inside its organization and its reach (ADR 0006).
    #[tokio::test]
    async fn reads_the_text_of_a_source_inside_the_reach_only() {
        let test = TestDatabase::start().await;
        let db = &test.database;
        let testwil = test.create_organization("testwil").await;
        let musterhausen = test.create_organization("musterhausen").await;
        let open_day = test.create_event(testwil, "TEST30").await;
        let workshop = test.create_event(testwil, "TEST31").await;
        let (anna, bruno) = (member(testwil), member(musterhausen));
        let source = add(&test, &anna, open_day, "Das Flugfeld.").await.id;

        let text = db
            .readable_text(anna.scope(), &events(&[open_day]), source)
            .await
            .unwrap();
        assert_eq!(text.as_deref(), Some("Das Flugfeld."));
        for (scope, reach) in [
            (anna.scope(), events(&[workshop])),
            (anna.scope(), events(&[])),
            (bruno.scope(), SourceReach::Organization),
            (bruno.scope(), events(&[open_day])),
        ] {
            assert_eq!(
                db.readable_text(scope, &reach, source).await.unwrap(),
                None,
                "{reach:?}"
            );
        }
        let unknown = SourceVersionId::from_uuid(Uuid::now_v7());
        assert_eq!(
            db.readable_text(anna.scope(), &SourceReach::Organization, unknown)
                .await
                .unwrap(),
            None
        );
    }

    /// A search finds only source versions of its organization and of the given events (ADR 0006).
    #[tokio::test]
    async fn searches_only_the_given_events_of_the_organization() {
        let test = TestDatabase::start().await;
        let db = &test.database;
        let testwil = test.create_organization("testwil").await;
        let musterhausen = test.create_organization("musterhausen").await;
        let open_day = test.create_event(testwil, "TEST30").await;
        let workshop = test.create_event(testwil, "TEST31").await;
        let fly_in = test.create_event(musterhausen, "FLY31").await;
        let (anna, bruno) = (member(testwil), member(musterhausen));
        let text = "Der Termin ist offen.\nDas Flugfeld Testwil ist im Mai frei.";
        let open_day_source = add(&test, &anna, open_day, text).await;
        let workshop_source = add(&test, &anna, workshop, "Das FLUGFELD ist nass.").await;
        add(
            &test,
            &bruno,
            fly_in,
            "Das Flugfeld Musterhausen ist im Juni frei.",
        )
        .await;
        add(&test, &anna, open_day, "Kein Treffer hier.").await;

        let hits = db
            .search(anna.scope(), &events(&[open_day]), "flugfeld", 10)
            .await
            .unwrap();
        assert_eq!(hits.len(), 1);
        let hit = &hits[0];
        assert_eq!(hit.source_version_id, open_day_source.id);
        assert_eq!(hit.captured_at, open_day_source.captured_at);
        assert!(hit.snippet.contains("Flugfeld"));
        let passage = Passage {
            start: hit.start,
            end: hit.end,
            quote: hit.snippet.clone(),
            page: None,
        };
        assert_eq!(passage.check(text), Ok(()));

        let mut both: Vec<_> = db
            .search(anna.scope(), &events(&[open_day, workshop]), "Flugfeld", 10)
            .await
            .unwrap()
            .into_iter()
            .map(|hit| hit.source_version_id)
            .collect();
        both.sort();
        let mut expected = vec![open_day_source.id, workshop_source.id];
        expected.sort();
        assert_eq!(both, expected);
        let limited = db
            .search(anna.scope(), &events(&[open_day, workshop]), "Flugfeld", 1)
            .await
            .unwrap();
        assert_eq!(limited.len(), 1);

        let searches = [
            (bruno.scope(), vec![open_day], "Flugfeld"),
            (anna.scope(), vec![fly_in], "Flugfeld"),
            (anna.scope(), vec![], "Flugfeld"),
            (anna.scope(), vec![open_day], "Juni"),
        ];
        for (scope, ids, query) in searches {
            let hits = db.search(scope, &events(&ids), query, 10).await.unwrap();
            assert!(hits.is_empty(), "{ids:?} {query}");
        }
    }
}
