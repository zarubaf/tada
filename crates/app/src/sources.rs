//! Sources (ADR 0050): the immutable texts that evidence points to, and their full-text search.

use std::fmt::Debug;

use async_trait::async_trait;
use jiff::Timestamp;
use tada_domain::ids::{EventId, SourceItemId, SourceVersionId};
use tada_domain::sources::SourceText;

use crate::caller::{Actor, OrgScope};
use crate::store::StoreError;

/// A stored source version.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceVersionRef {
    pub id: SourceVersionId,
    pub source_item_id: SourceItemId,
    /// The SHA-256 hash of the normalized text.
    pub sha256: [u8; 32],
    pub captured_at: Timestamp,
}

/// A source version that contains the words of a search.
/// The snippet is the text from `start` to `end`, in characters of the normalized text, so it can serve as a passage.
#[derive(Clone, PartialEq, Eq)]
pub struct SourceHit {
    pub source_version_id: SourceVersionId,
    pub captured_at: Timestamp,
    pub snippet: String,
    pub start: u32,
    pub end: u32,
}

/// The snippet can contain personal data, so `Debug` shows the range only (ADR 0035).
impl Debug for SourceHit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SourceHit")
            .field("source_version_id", &self.source_version_id)
            .field("captured_at", &self.captured_at)
            .field("start", &self.start)
            .field("end", &self.end)
            .finish_non_exhaustive()
    }
}

/// A stored source version with the event of its source item and its normalized text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceVersionText {
    pub id: SourceVersionId,
    /// `None` for a source item of the organization.
    pub event_id: Option<EventId>,
    /// `None` for a file without text, for example a PDF.
    pub text: Option<SourceText>,
}

/// The repository port for source items and source versions. Each method stays inside `scope`.
#[async_trait]
pub trait SourceStore: Debug + Send + Sync {
    /// Stores the text of a member as a new source item with one source version of the kind `member-text`.
    /// The source version records the actor as its author and the channel of the actor.
    async fn add_member_text(
        &self,
        scope: OrgScope,
        event: EventId,
        text: &SourceText,
        actor: &Actor,
        now: Timestamp,
    ) -> Result<SourceVersionRef, StoreError>;

    /// The source versions of the events `events` that contain the words of `query`, the best matches first.
    async fn search(
        &self,
        scope: OrgScope,
        events: &[EventId],
        query: &str,
        limit: u32,
    ) -> Result<Vec<SourceHit>, StoreError>;

    /// The source versions of `ids` in the organization. An ID of another organization gives nothing.
    async fn texts(
        &self,
        scope: OrgScope,
        ids: &[SourceVersionId],
    ) -> Result<Vec<SourceVersionText>, StoreError>;
}
