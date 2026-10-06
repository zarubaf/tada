//! The port that identifies the member behind a request (ADR 0008, ADR 0053).

use std::fmt::Debug;

use async_trait::async_trait;

use crate::caller::MemberCaller;
use crate::store::StoreError;

#[async_trait]
pub trait Authenticator: Debug + Send + Sync {
    /// Returns the member of the session `session_token`, or `Unauthenticated` if it is missing or invalid.
    async fn authenticate(
        &self,
        session_token: Option<&str>,
    ) -> Result<MemberCaller, AuthenticationError>;
}

#[derive(Debug, thiserror::Error)]
pub enum AuthenticationError {
    #[error("no valid session")]
    Unauthenticated,
    #[error(transparent)]
    Store(#[from] StoreError),
}
