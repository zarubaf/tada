//! The search of source versions (ADR 0050) for the read tools of AI clients (ADR 0040).
//!
//! The search reads only the source versions that the caller can read (ADR 0006, ADR 0052).
//! `access::source_reach` chooses them before the store runs the full-text search.

use tada_domain::events::EventKey;

use crate::access::{self, AccessError, Principal};
use crate::events::{self, EventStore};
use crate::identity::IdentityStore;
use crate::problem::{CommandError, FieldError, ProblemCode};
use crate::sources::{SourceHit, SourceStore};
use crate::store::StoreError;

/// The number of hits of a search without a limit.
pub const DEFAULT_LIMIT: u32 = 10;
/// The largest number of hits of one search.
pub const MAX_LIMIT: u32 = 50;
/// The longest query, in characters.
pub const MAX_QUERY_CHARS: usize = 200;

/// A search as the caller gives it.
#[derive(Clone)]
pub struct SearchRequest {
    /// The key of one event. Without it, the search reads all source versions that the caller can read.
    pub event_key: Option<String>,
    /// The words to find, in the syntax of a web search: words, `"a phrase"`, `or` and `-word`.
    pub query: String,
    pub limit: Option<u32>,
}

/// The query can contain personal data, so `Debug` shows the event key and the limit only (ADR 0035).
impl std::fmt::Debug for SearchRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SearchRequest")
            .field("event_key", &self.event_key)
            .field("limit", &self.limit)
            .finish_non_exhaustive()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SearchError {
    /// The event does not exist, or the caller cannot see it.
    #[error("the event does not exist or the caller cannot see it")]
    NotFound,
    #[error("invalid values")]
    Invalid(Vec<FieldError>),
    #[error(transparent)]
    Store(#[from] StoreError),
}

impl SearchError {
    /// All codes of the search, for the contract of a driving adapter (ADR 0037).
    pub const CODES: &[ProblemCode] = &[
        ProblemCode::NotFound,
        ProblemCode::ValidationFailed,
        ProblemCode::Unavailable,
        ProblemCode::Internal,
    ];
}

impl From<AccessError> for SearchError {
    fn from(error: AccessError) -> Self {
        match error {
            AccessError::NotFound => Self::NotFound,
            AccessError::Store(error) => Self::Store(error),
        }
    }
}

impl CommandError for SearchError {
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

/// The checked values of a request.
struct Checked {
    event_key: Option<EventKey>,
    query: String,
    limit: u32,
}

fn check(request: SearchRequest) -> Result<Checked, SearchError> {
    let mut errors = Vec::new();
    let event_key = match request.event_key.as_deref().map(EventKey::parse) {
        None => None,
        Some(Ok(key)) => Some(key),
        Some(Err(error)) => {
            errors.push(FieldError::new("event_key", events::key_error_code(error)));
            None
        }
    };
    let query = request.query.trim().to_owned();
    if query.is_empty() {
        errors.push(FieldError::new("query", "empty"));
    } else if query.chars().count() > MAX_QUERY_CHARS {
        errors.push(FieldError::new("query", "too-long"));
    }
    let limit = request.limit.unwrap_or(DEFAULT_LIMIT);
    if !(1..=MAX_LIMIT).contains(&limit) {
        errors.push(FieldError::new("limit", "out-of-range"));
    }
    if errors.is_empty() {
        Ok(Checked {
            event_key,
            query,
            limit,
        })
    } else {
        Err(SearchError::Invalid(errors))
    }
}

/// The source versions that contain the words of the query, the best matches first.
///
/// The search reads only the source versions that the caller can read (`access::source_reach`).
/// With an event key, it reads only the source versions of that event and those that the event cites as evidence.
/// An event that the caller cannot see is not found.
pub async fn search_sources(
    caller: &impl Principal,
    request: SearchRequest,
    events: &dyn EventStore,
    identity: &dyn IdentityStore,
    sources: &dyn SourceStore,
) -> Result<Vec<SourceHit>, SearchError> {
    let checked = check(request)?;
    let reach = match &checked.event_key {
        Some(key) => {
            let event = events::find_event(caller, key, events, identity).await?;
            access::event_source_reach(caller, event.id, identity).await?
        }
        None => access::source_reach(caller, identity).await?,
    };
    Ok(sources
        .search(caller.scope(), &reach, &checked.query, checked.limit)
        .await?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(event_key: Option<&str>, query: &str, limit: Option<u32>) -> SearchRequest {
        SearchRequest {
            event_key: event_key.map(str::to_owned),
            query: query.to_owned(),
            limit,
        }
    }

    fn errors(request: SearchRequest) -> Vec<(String, &'static str)> {
        match check(request) {
            Err(SearchError::Invalid(errors)) => errors
                .into_iter()
                .map(|error| (error.field.into_owned(), error.code))
                .collect(),
            _ => panic!("valid"),
        }
    }

    #[test]
    fn a_search_has_a_default_limit_and_a_trimmed_query() {
        let checked = check(request(Some("OPEN30"), "  Flugfeld ", None)).unwrap();
        assert_eq!(checked.query, "Flugfeld");
        assert_eq!(checked.limit, DEFAULT_LIMIT);
        assert_eq!(checked.event_key.unwrap().as_str(), "OPEN30");
        assert!(check(request(None, "Flugfeld", Some(MAX_LIMIT))).is_ok());
    }

    #[test]
    fn names_each_invalid_value() {
        let long = "a".repeat(MAX_QUERY_CHARS + 1);
        assert_eq!(
            errors(request(Some("open30"), " ", Some(0))),
            [
                ("event_key".to_owned(), "characters"),
                ("query".to_owned(), "empty"),
                ("limit".to_owned(), "out-of-range"),
            ]
        );
        assert_eq!(
            errors(request(None, &long, Some(MAX_LIMIT + 1))),
            [
                ("query".to_owned(), "too-long"),
                ("limit".to_owned(), "out-of-range"),
            ]
        );
    }

    #[test]
    fn debug_hides_the_query() {
        let request = request(None, "Flugfeld", None);
        assert!(!format!("{request:?}").contains("Flugfeld"));
    }

    #[test]
    fn each_error_gives_a_code_of_its_list() {
        for error in [
            SearchError::NotFound,
            SearchError::Invalid(Vec::new()),
            SearchError::Store(StoreError::Unavailable("test".into())),
            SearchError::Store(StoreError::Internal("test".into())),
        ] {
            assert!(SearchError::CODES.contains(&error.code()), "{error:?}");
        }
    }
}
