//! The `SourceStore` adapter (ADR 0050): source items, source versions and their full-text search.

use async_trait::async_trait;
use jiff::Timestamp;
use jiff_sqlx::ToSqlx;
use sha2::{Digest, Sha256};
use sqlx::PgConnection;
use sqlx::types::Uuid;
use tada_app::caller::{Actor, OrgScope};
use tada_app::domain::ids::{EventId, SourceItemId, SourceVersionId};
use tada_app::domain::sources::SourceText;
use tada_app::sources::{SourceHit, SourceStore, SourceVersionRef};
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
        events: &[EventId],
        query: &str,
        limit: u32,
    ) -> Result<Vec<SourceHit>, StoreError> {
        if events.is_empty() {
            return Ok(Vec::new());
        }
        let events: Vec<Uuid> = events.iter().map(|event| event.as_uuid()).collect();
        let rows = sqlx::query!(
            r#"SELECT v.id, v.captured_at AS "captured_at: jiff_sqlx::Timestamp", v.text AS "text!"
               FROM source_version v
               JOIN source_item i ON i.organization_id = v.organization_id AND i.id = v.source_item_id,
                    websearch_to_tsquery('simple', $3) q
               WHERE v.organization_id = $1 AND i.event_id = ANY($2) AND v.search @@ q AND v.text IS NOT NULL
               ORDER BY ts_rank(v.search, q) DESC, v.captured_at DESC, v.id
               LIMIT $4"#,
            scope.organization_id().as_uuid(),
            &events,
            query,
            i64::from(limit),
        )
        .fetch_all(&self.pool)
        .await
        .map_err(store_error)?;
        rows.into_iter()
            .map(|row| {
                let (start, end) = snippet_range(&row.text, query);
                let offset = |chars: usize| {
                    u32::try_from(chars)
                        .map_err(|_| StoreError::from(InvalidRow("source_version.text")))
                };
                Ok(SourceHit {
                    source_version_id: SourceVersionId::from_uuid(row.id),
                    captured_at: row.captured_at.to_jiff(),
                    snippet: row.text.chars().skip(start).take(end - start).collect(),
                    start: offset(start)?,
                    end: offset(end)?,
                })
            })
            .collect()
    }
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
            .search(anna.scope(), &[open_day], "flugfeld", 10)
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
            .search(anna.scope(), &[open_day, workshop], "Flugfeld", 10)
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
            .search(anna.scope(), &[open_day, workshop], "Flugfeld", 1)
            .await
            .unwrap();
        assert_eq!(limited.len(), 1);

        let searches = [
            (bruno.scope(), vec![open_day], "Flugfeld"),
            (anna.scope(), vec![fly_in], "Flugfeld"),
            (anna.scope(), vec![], "Flugfeld"),
            (anna.scope(), vec![open_day], "Juni"),
        ];
        for (scope, events, query) in searches {
            let hits = db.search(scope, &events, query, 10).await.unwrap();
            assert!(hits.is_empty(), "{events:?} {query}");
        }
    }
}
