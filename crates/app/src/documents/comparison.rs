//! The difference between two draft versions of a document (ADR 0051):
//! a text difference by line, and a fact difference from the two provenance manifests.

use std::collections::BTreeMap;
use std::fmt;

use similar::{ChangeTag, TextDiff};
use tada_domain::RecordVersion;
use tada_domain::ids::{DocumentId, DocumentVersionId, FactId};

use super::{DocumentReads, ReadDocumentError, StoredDraft, get_document};
use crate::access::Principal;
use crate::drafts::{CitedFact, ProvenanceManifest};

/// The difference from an older draft version to a newer one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionDiff {
    /// Each line of the two versions in the order of the newer version, with the removed lines at their places.
    /// A draft has one sentence per line, so this is a difference by sentence.
    pub lines: Vec<LineChange>,
    pub facts: FactDiff,
}

/// One line of a text difference.
#[derive(Clone, PartialEq, Eq)]
pub struct LineChange {
    pub kind: LineKind,
    /// The number of the line in the older version, from 1. `None` for an added line.
    pub old_line: Option<u32>,
    /// The number of the line in the newer version, from 1. `None` for a removed line.
    pub new_line: Option<u32>,
    /// The text of the line without its line end.
    pub text: String,
}

/// The text is document content, so `Debug` leaves it out (ADR 0035).
impl fmt::Debug for LineChange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LineChange")
            .field("kind", &self.kind)
            .field("old_line", &self.old_line)
            .field("new_line", &self.new_line)
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineKind {
    Unchanged,
    Removed,
    Added,
}

/// The facts that the two manifests cite.
/// If a manifest cites more than one version of a fact, the newest of them counts.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FactDiff {
    /// The facts that both versions cite, each in another version.
    pub changed: Vec<FactChange>,
    /// The facts that only the newer version cites.
    pub added: Vec<CitedFact>,
    /// The facts that only the older version cites.
    pub removed: Vec<CitedFact>,
}

/// A fact that the older version cites in the version `from` and the newer version in `to`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FactChange {
    pub fact_id: FactId,
    pub from: RecordVersion,
    pub to: RecordVersion,
}

/// The difference from the draft version `from` to the draft version `to` of the document `document_id`.
/// A version that is not a draft version of this document is not found.
pub async fn diff_versions(
    caller: &impl Principal,
    document_id: DocumentId,
    from: DocumentVersionId,
    to: DocumentVersionId,
    stores: DocumentReads<'_>,
) -> Result<VersionDiff, ReadDocumentError> {
    get_document(caller, document_id, stores).await?;
    let draft = |id| async move {
        stores
            .documents
            .draft(caller.scope(), id)
            .await?
            .filter(|draft| draft.version.document_id == document_id)
            .ok_or(ReadDocumentError::NotFound)
    };
    let (older, newer) = (draft(from).await?, draft(to).await?);
    Ok(compare(&older, &newer))
}

/// The difference between two drafts.
pub(crate) fn compare(older: &StoredDraft, newer: &StoredDraft) -> VersionDiff {
    VersionDiff {
        lines: line_changes(older.markdown.as_str(), newer.markdown.as_str()),
        facts: fact_diff(&older.manifest, &newer.manifest),
    }
}

fn line_changes(older: &str, newer: &str) -> Vec<LineChange> {
    let line_number = |index: Option<usize>| index.and_then(|i| u32::try_from(i + 1).ok());
    TextDiff::from_lines(older, newer)
        .iter_all_changes()
        .map(|change| LineChange {
            kind: match change.tag() {
                ChangeTag::Equal => LineKind::Unchanged,
                ChangeTag::Delete => LineKind::Removed,
                ChangeTag::Insert => LineKind::Added,
            },
            old_line: line_number(change.old_index()),
            new_line: line_number(change.new_index()),
            text: change.value().trim_end_matches('\n').to_owned(),
        })
        .collect()
}

/// The newest cited version of each fact, in the order of the fact IDs.
fn newest_versions(manifest: &ProvenanceManifest) -> BTreeMap<FactId, RecordVersion> {
    let mut versions = BTreeMap::new();
    for cited in &manifest.facts {
        versions
            .entry(cited.fact_id)
            .and_modify(|version: &mut RecordVersion| *version = (*version).max(cited.version))
            .or_insert(cited.version);
    }
    versions
}

fn fact_diff(older: &ProvenanceManifest, newer: &ProvenanceManifest) -> FactDiff {
    let (older, newer) = (newest_versions(older), newest_versions(newer));
    let mut diff = FactDiff::default();
    for (&fact_id, &to) in &newer {
        match older.get(&fact_id) {
            Some(&from) if from != to => diff.changed.push(FactChange { fact_id, from, to }),
            Some(_) => {}
            None => diff.added.push(CitedFact {
                fact_id,
                version: to,
            }),
        }
    }
    diff.removed = older
        .iter()
        .filter(|(fact_id, _)| !newer.contains_key(fact_id))
        .map(|(&fact_id, &version)| CitedFact { fact_id, version })
        .collect();
    diff
}

#[cfg(test)]
mod tests {
    use jiff::Timestamp;
    use tada_domain::documents::DraftMarkdown;
    use tada_domain::ids::{EventId, UserId};
    use uuid::Uuid;

    use super::*;
    use crate::documents::{DraftStatus, VersionContent, VersionView};

    fn fact(n: u128) -> FactId {
        FactId::from_uuid(Uuid::from_u128(n))
    }

    fn cited(n: u128, version: i64) -> CitedFact {
        CitedFact {
            fact_id: fact(n),
            version: RecordVersion::new(version).unwrap(),
        }
    }

    fn draft(markdown: &str, facts: Vec<CitedFact>) -> StoredDraft {
        StoredDraft {
            event_id: EventId::from_uuid(Uuid::from_u128(1)),
            version: VersionView {
                id: DocumentVersionId::from_uuid(Uuid::now_v7()),
                document_id: DocumentId::from_uuid(Uuid::from_u128(2)),
                number: 1,
                sha256: [0; 32],
                uploaded_by: UserId::from_uuid(Uuid::from_u128(3)),
                created_at: Timestamp::UNIX_EPOCH,
                content: VersionContent::Draft {
                    status: DraftStatus::Draft,
                },
            },
            markdown: DraftMarkdown::parse(markdown).unwrap(),
            manifest: ProvenanceManifest {
                facts,
                sources: Vec::new(),
            },
        }
    }

    #[test]
    fn lists_the_changed_lines_by_sentence() {
        let older = draft("# Konzept\nDas Fest ist im Mai.\nEs gibt Kaffee.\n", vec![]);
        let newer = draft(
            "# Konzept\nDas Fest ist im Juni.\nEs gibt Kaffee.\n",
            vec![],
        );

        let lines: Vec<_> = compare(&older, &newer)
            .lines
            .into_iter()
            .map(|line| (line.kind, line.old_line, line.new_line, line.text))
            .collect();

        assert_eq!(
            lines,
            [
                (
                    LineKind::Unchanged,
                    Some(1),
                    Some(1),
                    "# Konzept".to_owned()
                ),
                (
                    LineKind::Removed,
                    Some(2),
                    None,
                    "Das Fest ist im Mai.".to_owned()
                ),
                (
                    LineKind::Added,
                    None,
                    Some(2),
                    "Das Fest ist im Juni.".to_owned()
                ),
                (
                    LineKind::Unchanged,
                    Some(3),
                    Some(3),
                    "Es gibt Kaffee.".to_owned()
                ),
            ]
        );
    }

    #[test]
    fn lists_the_changed_added_and_removed_facts() {
        let older = draft("a\n", vec![cited(1, 1), cited(2, 1), cited(3, 2)]);
        let newer = draft("a\n", vec![cited(1, 2), cited(3, 2), cited(4, 1)]);

        let diff = compare(&older, &newer).facts;

        assert_eq!(
            diff.changed,
            [FactChange {
                fact_id: fact(1),
                from: RecordVersion::new(1).unwrap(),
                to: RecordVersion::new(2).unwrap(),
            }]
        );
        assert_eq!(diff.added, [cited(4, 1)]);
        assert_eq!(diff.removed, [cited(2, 1)]);
    }

    #[test]
    fn the_newest_cited_version_of_a_fact_counts() {
        let older = draft("a\n", vec![cited(1, 1), cited(1, 2)]);
        let newer = draft("a\n", vec![cited(1, 2)]);

        assert_eq!(compare(&older, &newer).facts, FactDiff::default());
    }

    #[test]
    fn debug_shows_no_text() {
        let older = draft("Geheim\n", vec![]);
        let shown = format!("{:?}", compare(&older, &older));
        assert!(!shown.contains("Geheim"), "{shown}");
    }
}
