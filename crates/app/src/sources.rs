//! Sources (ADR 0050): the immutable texts that evidence points to, and their full-text search.

use std::fmt::Debug;

use async_trait::async_trait;
use jiff::Timestamp;
use schemars::JsonSchema;
use serde::Deserialize;
use tada_domain::ids::{EventId, SourceItemId, SourceVersionId};
use tada_domain::sources::{self as passages, SourceText};
use uuid::Uuid;

use crate::access::{self, Principal, SourceReach};
use crate::caller::{Actor, OrgScope};
use crate::identity::IdentityStore;
use crate::problem::{CommandError, FieldError, ProblemCode};
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

/// A stored source version with its normalized text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceVersionText {
    pub id: SourceVersionId,
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

    /// The source versions inside `reach` that contain the words of `query`, the best matches first.
    async fn search(
        &self,
        scope: OrgScope,
        reach: &SourceReach,
        query: &str,
        limit: u32,
    ) -> Result<Vec<SourceHit>, StoreError>;

    /// The source versions of `ids` inside `reach` (`access::source_reach`).
    /// An ID outside the reach, or of another organization, gives nothing.
    async fn texts(
        &self,
        scope: OrgScope,
        reach: &SourceReach,
        ids: &[SourceVersionId],
    ) -> Result<Vec<SourceVersionText>, StoreError>;
    /// The normalized text of the source version `id`, if it is inside `reach` and has a text.
    async fn readable_text(
        &self,
        scope: OrgScope,
        reach: &SourceReach,
        id: SourceVersionId,
    ) -> Result<Option<String>, StoreError>;
}

/// The longest passage that `get_source_passage` returns, in characters.
pub const MAX_PASSAGE_CHARS: u32 = 10_000;

/// A range of a source version as the caller gives it, for `get_source_passage`.
/// The JSON Schema of the MCP tool comes from this type (ADR 0040).
#[derive(Debug, Clone, Copy, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PassageRequest {
    pub source_version_id: Uuid,
    /// The offset of the first character, in characters of the normalized text.
    pub start: u32,
    #[schemars(description = end_description())]
    pub end: u32,
}

fn end_description() -> String {
    format!(
        "The offset after the last character. A passage has at most {MAX_PASSAGE_CHARS} characters."
    )
}

/// The exact text of a range of a source version, for a citation (ADR 0050).
#[derive(Clone, PartialEq, Eq)]
pub struct SourcePassage {
    pub source_version_id: SourceVersionId,
    pub start: u32,
    pub end: u32,
    pub quote: String,
}

/// The quote can contain personal data, so `Debug` shows the range only (ADR 0035).
impl Debug for SourcePassage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SourcePassage")
            .field("source_version_id", &self.source_version_id)
            .field("start", &self.start)
            .field("end", &self.end)
            .finish_non_exhaustive()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum PassageError {
    /// The source version does not exist, has no text, or the caller cannot read it.
    #[error("the source version does not exist or the caller cannot read it")]
    NotFound,
    #[error("invalid values")]
    Invalid(Vec<FieldError>),
    #[error(transparent)]
    Store(#[from] StoreError),
}

impl PassageError {
    /// All codes of `get_source_passage`, for the contract of a driving adapter (ADR 0037).
    pub const CODES: &[ProblemCode] = &[
        ProblemCode::NotFound,
        ProblemCode::ValidationFailed,
        ProblemCode::Unavailable,
        ProblemCode::Internal,
    ];
}

impl CommandError for PassageError {
    fn code(&self) -> ProblemCode {
        match self {
            Self::NotFound => ProblemCode::NotFound,
            Self::Invalid(_) => ProblemCode::ValidationFailed,
            Self::Store(error) => error.code(),
        }
    }

    fn store_error(&self) -> Option<&StoreError> {
        match self {
            Self::Store(error) => Some(error),
            _ => None,
        }
    }

    fn field_errors(&self) -> &[FieldError] {
        match self {
            Self::Invalid(errors) => errors,
            _ => &[],
        }
    }
}

/// The text from `start` to `end` of a source version that the caller can read (`access::source_reach`).
/// The offsets count characters of the normalized text, as in a passage of evidence.
pub async fn get_source_passage(
    caller: &impl Principal,
    request: PassageRequest,
    identity: &dyn IdentityStore,
    sources: &dyn SourceStore,
) -> Result<SourcePassage, PassageError> {
    let PassageRequest {
        source_version_id,
        start,
        end,
    } = request;
    let id = SourceVersionId::from_uuid(source_version_id);
    if end.saturating_sub(start) > MAX_PASSAGE_CHARS {
        return Err(PassageError::Invalid(vec![FieldError::new(
            "end", "too-long",
        )]));
    }
    let reach = access::source_reach(caller, identity).await?;
    let text = sources
        .readable_text(caller.scope(), &reach, id)
        .await?
        .ok_or(PassageError::NotFound)?;
    let quote = passages::quote(&text, start, end).map_err(|error| {
        let code = match error {
            passages::PassageError::Empty => "empty",
            _ => "out-of-range",
        };
        PassageError::Invalid(vec![FieldError::new("end", code)])
    })?;
    Ok(SourcePassage {
        source_version_id: id,
        start,
        end,
        quote: quote.to_owned(),
    })
}

#[cfg(test)]
mod tests {
    use tada_domain::identity::{EventRole, OrganizationRole};
    use tada_domain::ids::{OrganizationId, UserId};
    use uuid::Uuid;

    use super::*;
    use crate::caller::MemberCaller;
    use crate::identity::{Membership, UserRef};

    const TEXT: &str = "Das Flugfeld öffnet im Mai.";

    /// One source version of the event `open_day` of Testwil; Anna is a viewer of the event if `viewer` is true.
    #[derive(Debug)]
    struct Memory {
        viewer: bool,
    }

    fn testwil() -> OrganizationId {
        OrganizationId::from_uuid(Uuid::from_u128(10))
    }

    fn open_day() -> EventId {
        EventId::from_uuid(Uuid::from_u128(20))
    }

    fn source() -> SourceVersionId {
        SourceVersionId::from_uuid(Uuid::from_u128(30))
    }

    fn anna() -> MemberCaller {
        MemberCaller::new(
            UserId::from_uuid(Uuid::from_u128(1)),
            testwil(),
            OrganizationRole::Member,
        )
    }

    #[async_trait]
    impl SourceStore for Memory {
        async fn add_member_text(
            &self,
            _: OrgScope,
            _: EventId,
            _: &SourceText,
            _: &Actor,
            _: Timestamp,
        ) -> Result<SourceVersionRef, StoreError> {
            unreachable!()
        }

        async fn search(
            &self,
            _: OrgScope,
            _: &SourceReach,
            _: &str,
            _: u32,
        ) -> Result<Vec<SourceHit>, StoreError> {
            unreachable!()
        }

        async fn readable_text(
            &self,
            scope: OrgScope,
            reach: &SourceReach,
            id: SourceVersionId,
        ) -> Result<Option<String>, StoreError> {
            let inside = match reach {
                SourceReach::Organization => true,
                SourceReach::Events(events) => events.contains(&open_day()),
            };
            let found = inside && scope.organization_id() == testwil() && id == source();
            Ok(found.then(|| TEXT.to_owned()))
        }

        async fn texts(
            &self,
            _: OrgScope,
            _: &SourceReach,
            _: &[SourceVersionId],
        ) -> Result<Vec<SourceVersionText>, StoreError> {
            unreachable!()
        }
    }

    #[async_trait]
    impl IdentityStore for Memory {
        async fn user(&self, _: UserId) -> Result<Option<UserRef>, StoreError> {
            unreachable!()
        }

        async fn memberships_of(&self, _: UserId) -> Result<Vec<Membership>, StoreError> {
            unreachable!()
        }

        async fn membership(
            &self,
            _: OrgScope,
            _: UserId,
        ) -> Result<Option<OrganizationRole>, StoreError> {
            unreachable!()
        }

        async fn event_exists(&self, _: OrgScope, _: EventId) -> Result<bool, StoreError> {
            unreachable!()
        }

        async fn event_role(
            &self,
            _: OrgScope,
            _: EventId,
            _: UserId,
        ) -> Result<Option<EventRole>, StoreError> {
            unreachable!()
        }

        async fn event_roles_of(
            &self,
            _: OrgScope,
            _: UserId,
        ) -> Result<Vec<(EventId, EventRole)>, StoreError> {
            Ok(if self.viewer {
                vec![(open_day(), EventRole::EventViewer)]
            } else {
                Vec::new()
            })
        }
    }

    async fn passage(memory: &Memory, start: u32, end: u32) -> Result<SourcePassage, PassageError> {
        let request = PassageRequest {
            source_version_id: source().as_uuid(),
            start,
            end,
        };
        get_source_passage(&anna(), request, memory, memory).await
    }

    fn codes(result: Result<SourcePassage, PassageError>) -> Vec<(String, &'static str)> {
        match result {
            Err(PassageError::Invalid(errors)) => errors
                .into_iter()
                .map(|error| (error.field.into_owned(), error.code))
                .collect(),
            other => panic!("not invalid: {other:?}"),
        }
    }

    #[tokio::test]
    async fn a_reader_of_the_event_gets_the_exact_quote() {
        let memory = Memory { viewer: true };
        let passage = passage(&memory, 4, 12).await.unwrap();
        assert_eq!(passage.quote, "Flugfeld");
        assert_eq!(
            (passage.source_version_id, passage.start, passage.end),
            (source(), 4, 12)
        );
    }

    #[tokio::test]
    async fn a_member_outside_the_reach_does_not_find_the_source() {
        let memory = Memory { viewer: false };
        assert!(matches!(
            passage(&memory, 4, 12).await,
            Err(PassageError::NotFound)
        ));
    }

    #[tokio::test]
    async fn rejects_an_empty_a_too_long_and_an_outside_range() {
        let memory = Memory { viewer: true };
        assert_eq!(
            codes(passage(&memory, 4, 4).await),
            [("end".to_owned(), "empty")]
        );
        assert_eq!(
            codes(passage(&memory, 20, 99).await),
            [("end".to_owned(), "out-of-range")]
        );
        let too_long = passage(&memory, 0, MAX_PASSAGE_CHARS + 1).await;
        assert_eq!(codes(too_long), [("end".to_owned(), "too-long")]);
    }

    #[test]
    fn debug_hides_the_quote() {
        let passage = SourcePassage {
            source_version_id: source(),
            start: 4,
            end: 12,
            quote: "Flugfeld".to_owned(),
        };
        assert!(!format!("{passage:?}").contains("Flugfeld"));
    }

    #[test]
    fn each_error_gives_a_code_of_its_list() {
        for error in [
            PassageError::NotFound,
            PassageError::Invalid(Vec::new()),
            PassageError::Store(StoreError::Unavailable("test".into())),
            PassageError::Store(StoreError::Internal("test".into())),
        ] {
            assert!(PassageError::CODES.contains(&error.code()), "{error:?}");
        }
    }
}
