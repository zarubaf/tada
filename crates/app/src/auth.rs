//! The port that identifies the member behind a request (ADR 0008, ADR 0062).

use std::fmt::Debug;

use async_trait::async_trait;

use crate::caller::{AiCaller, MemberCaller};
use crate::problem::{CommandError, ProblemCode};
use crate::store::StoreError;

/// The secret that a request shows to prove who sends it.
#[derive(Clone, Copy)]
pub enum Credential<'a> {
    /// The value of the session cookie (ADR 0008).
    Session(&'a str),
    /// A personal API token (ADR 0039).
    ApiToken(&'a str),
}

impl Debug for Credential<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // The secret never goes to a log (ADR 0035).
        match self {
            Self::Session(_) => f.write_str("Session(redacted)"),
            Self::ApiToken(_) => f.write_str("ApiToken(redacted)"),
        }
    }
}

/// The caller of a valid credential (ADR 0039).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Authenticated {
    /// A member with a session.
    Member(MemberCaller),
    /// An AI client of a member with a personal API token: an AI can be behind any token.
    Ai(AiCaller),
}

#[async_trait]
pub trait Authenticator: Debug + Send + Sync {
    /// Returns the caller of `credential`. It returns `Unauthenticated` if the credential is missing
    /// or invalid, and `OrganizationRequired` if the session has no organization or the membership
    /// in it no longer exists.
    async fn authenticate(
        &self,
        credential: Option<Credential<'_>>,
    ) -> Result<Authenticated, AuthenticationError>;
}

#[derive(Debug, thiserror::Error)]
pub enum AuthenticationError {
    #[error("no valid session")]
    Unauthenticated,
    /// The session has no organization, or the membership in it no longer exists (ADR 0056).
    #[error("the session has no organization")]
    OrganizationRequired,
    #[error(transparent)]
    Store(#[from] StoreError),
}

impl AuthenticationError {
    /// All codes of a failed authentication, for the API contract (ADR 0037).
    pub const CODES: &[ProblemCode] = &[
        ProblemCode::Unauthenticated,
        ProblemCode::OrganizationRequired,
        ProblemCode::Unavailable,
        ProblemCode::Internal,
    ];
}

impl CommandError for AuthenticationError {
    fn code(&self) -> ProblemCode {
        match self {
            Self::Unauthenticated => ProblemCode::Unauthenticated,
            Self::OrganizationRequired => ProblemCode::OrganizationRequired,
            Self::Store(error) => error.code(),
        }
    }

    fn store_error(&self) -> Option<&StoreError> {
        match self {
            Self::Store(error) => Some(error),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_hides_the_secret() {
        for credential in [
            Credential::Session("secret"),
            Credential::ApiToken("secret"),
        ] {
            assert!(!format!("{credential:?}").contains("secret"));
        }
    }
}
