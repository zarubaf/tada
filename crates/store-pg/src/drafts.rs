//! The stored form of the provenance manifest and the lint warnings of a draft proposal (ADR 0051):
//! the columns `proposal.manifest` and `proposal.lint_warnings`.
//!
//! The apply of a draft reads `proposal.manifest` with SQL (`review::insert_draft`), so the field names of
//! `ManifestRecord` are part of the format. A change of the format needs a new version of the proposal format.

use serde::{Deserialize, Serialize};
use sqlx::types::Uuid;
use tada_app::domain::RecordVersion;
use tada_app::domain::ids::{FactId, SourceVersionId};
use tada_app::domain::sources::Passage;
use tada_app::drafts::{CitedFact, CitedPassage, LintKind, LintWarning, ProvenanceManifest};

use crate::error::InvalidRow;

const MANIFEST: &str = "proposal.manifest";
const LINT_WARNINGS: &str = "proposal.lint_warnings";

#[derive(Serialize, Deserialize)]
struct ManifestRecord {
    facts: Vec<FactRecord>,
    sources: Vec<SourceRecord>,
}

#[derive(Serialize, Deserialize)]
struct FactRecord {
    fact_id: Uuid,
    version: i64,
}

#[derive(Serialize, Deserialize)]
struct SourceRecord {
    source_version_id: Uuid,
    start: u32,
    end: u32,
    quote: String,
}

#[derive(Serialize, Deserialize)]
struct LintRecord {
    line: u32,
    kind: String,
}

pub(crate) fn manifest_to_json(manifest: &ProvenanceManifest) -> serde_json::Value {
    let record = ManifestRecord {
        facts: manifest
            .facts
            .iter()
            .map(|fact| FactRecord {
                fact_id: fact.fact_id.as_uuid(),
                version: fact.version.get(),
            })
            .collect(),
        sources: manifest
            .sources
            .iter()
            .map(|source| SourceRecord {
                source_version_id: source.source_version_id.as_uuid(),
                start: source.passage.start,
                end: source.passage.end,
                quote: source.passage.quote.clone(),
            })
            .collect(),
    };
    serde_json::to_value(record).expect("a manifest record is valid JSON")
}

pub(crate) fn manifest_from_json(
    json: &serde_json::Value,
) -> Result<ProvenanceManifest, InvalidRow> {
    let record = ManifestRecord::deserialize(json).map_err(|_| InvalidRow(MANIFEST))?;
    Ok(ProvenanceManifest {
        facts: record
            .facts
            .into_iter()
            .map(|fact| {
                Ok(CitedFact {
                    fact_id: FactId::from_uuid(fact.fact_id),
                    version: RecordVersion::new(fact.version).ok_or(InvalidRow(MANIFEST))?,
                })
            })
            .collect::<Result<_, InvalidRow>>()?,
        sources: record
            .sources
            .into_iter()
            .map(|source| CitedPassage {
                source_version_id: SourceVersionId::from_uuid(source.source_version_id),
                passage: Passage {
                    start: source.start,
                    end: source.end,
                    quote: source.quote,
                    page: None,
                },
            })
            .collect(),
    })
}

fn kind_name(kind: LintKind) -> &'static str {
    match kind {
        LintKind::Number => "number",
        LintKind::Date => "date",
        LintKind::Money => "money",
        LintKind::RawHtml => "raw-html",
    }
}

const LINT_KINDS: [LintKind; 4] = [
    LintKind::Number,
    LintKind::Date,
    LintKind::Money,
    LintKind::RawHtml,
];

pub(crate) fn lint_to_json(warnings: &[LintWarning]) -> serde_json::Value {
    let records: Vec<LintRecord> = warnings
        .iter()
        .map(|warning| LintRecord {
            line: warning.line,
            kind: kind_name(warning.kind).to_owned(),
        })
        .collect();
    serde_json::to_value(records).expect("lint records are valid JSON")
}

pub(crate) fn lint_from_json(json: &serde_json::Value) -> Result<Vec<LintWarning>, InvalidRow> {
    let records = Vec::<LintRecord>::deserialize(json).map_err(|_| InvalidRow(LINT_WARNINGS))?;
    records
        .into_iter()
        .map(|record| {
            let kind = LINT_KINDS
                .into_iter()
                .find(|kind| kind_name(*kind) == record.kind)
                .ok_or(InvalidRow(LINT_WARNINGS))?;
            Ok(LintWarning {
                line: record.line,
                kind,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_manifest_and_the_lint_warnings_keep_their_values() {
        let manifest = ProvenanceManifest {
            facts: vec![CitedFact {
                fact_id: FactId::from_uuid(Uuid::from_u128(1)),
                version: RecordVersion::new(3).unwrap(),
            }],
            sources: vec![CitedPassage {
                source_version_id: SourceVersionId::from_uuid(Uuid::from_u128(2)),
                passage: Passage {
                    start: 4,
                    end: 12,
                    quote: "Flugfeld".to_owned(),
                    page: None,
                },
            }],
        };
        let json = manifest_to_json(&manifest);
        assert_eq!(json["facts"][0]["version"], 3);
        assert_eq!(manifest_from_json(&json).unwrap(), manifest);

        let warnings: Vec<LintWarning> = LINT_KINDS
            .into_iter()
            .enumerate()
            .map(|(line, kind)| LintWarning {
                line: line as u32 + 1,
                kind,
            })
            .collect();
        let json = lint_to_json(&warnings);
        assert_eq!(json[3]["kind"], "raw-html");
        assert_eq!(lint_from_json(&json).unwrap(), warnings);
    }
}
