//! The port that identifies the member behind a request (ADR 0008, ADR 0053).

use std::fmt::Debug;

use async_trait::async_trait;

use crate::caller::MemberCaller;
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

#[async_trait]
pub trait Authenticator: Debug + Send + Sync {
    /// Returns the member of `credential`, or `Unauthenticated` if it is missing or invalid.
    async fn authenticate(
        &self,
        credential: Option<Credential<'_>>,
    ) -> Result<MemberCaller, AuthenticationError>;
}

#[derive(Debug, thiserror::Error)]
pub enum AuthenticationError {
    #[error("no valid session")]
    Unauthenticated,
    #[error(transparent)]
    Store(#[from] StoreError),
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
