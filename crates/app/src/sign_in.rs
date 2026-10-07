//! Sign-in with a magic link (ADR 0008, ADR 0056).
//!
//! 1. A person asks for a magic link with an email address. Each address gets the same answer.
//! 2. Only a user with a membership gets a mail. The worker creates the token and sends it.
//! 3. The person sends the token from the link. The token works once and starts a new session.

use std::fmt::Debug;

use async_trait::async_trait;
use jiff::Timestamp;
use secrecy::SecretString;
use tada_domain::identity::Email;
use tada_domain::ids::OrganizationId;
use uuid::Uuid;

use crate::clock::Clock;
use crate::problem::ProblemCode;
use crate::store::StoreError;

/// The repository port of sign-in. Each method takes a token as the holder sends it.
/// Only the store hashes it; no table holds the token (ADR 0008).
#[async_trait]
pub trait SignInStore: Debug + Send + Sync {
    /// Queues a magic-link intent for the user of `email` if that user has a membership.
    /// Otherwise it changes nothing. Both cases run the same query, so that the time of the request
    /// does not show if the address is known (ADR 0008).
    async fn queue_magic_link(
        &self,
        email: &Email,
        request_id: Option<Uuid>,
    ) -> Result<(), StoreError>;

    /// Deletes the magic link of `token`. If the link existed and was valid at `now`, the same
    /// transaction starts a session for its user and returns the session token.
    /// The organization of the session is `initial_organization` of the user's memberships.
    async fn redeem_magic_link(
        &self,
        token: &str,
        user_agent: Option<&str>,
        now: Timestamp,
    ) -> Result<Option<SecretString>, StoreError>;
}

/// The organization of a new session: the only organization of a user with one membership.
/// A user with more memberships chooses one later (ADR 0056).
pub fn initial_organization(memberships: &[OrganizationId]) -> Option<OrganizationId> {
    match memberships {
        [organization] => Some(*organization),
        _ => None,
    }
}

/// Asks for a magic link. An invalid or unknown address, or an address without a membership,
/// gets no mail, and the caller gets no information about it (ADR 0056).
pub async fn request_magic_link(
    email: &str,
    request_id: Option<Uuid>,
    store: &dyn SignInStore,
) -> Result<(), StoreError> {
    let Ok(email) = Email::parse(email) else {
        return Ok(());
    };
    store.queue_magic_link(&email, request_id).await
}

/// An error of `redeem_magic_link`.
#[derive(Debug, thiserror::Error)]
pub enum SignInError {
    /// The token is unknown, used or expired. The cases look the same to the caller.
    #[error("the token is not valid")]
    Unauthenticated,
    #[error(transparent)]
    Store(#[from] StoreError),
}

impl SignInError {
    /// All codes that this command can return, for the API contract (ADR 0037).
    pub const CODES: &[ProblemCode] = &[
        ProblemCode::Unauthenticated,
        ProblemCode::Unavailable,
        ProblemCode::Internal,
    ];

    pub fn code(&self) -> ProblemCode {
        match self {
            Self::Unauthenticated => ProblemCode::Unauthenticated,
            Self::Store(StoreError::Unavailable(_)) => ProblemCode::Unavailable,
            Self::Store(StoreError::Internal(_)) => ProblemCode::Internal,
        }
    }
}

/// Uses the token of a magic link once and returns the token of the new session.
pub async fn redeem_magic_link(
    token: &str,
    user_agent: Option<&str>,
    store: &dyn SignInStore,
    clock: &dyn Clock,
) -> Result<SecretString, SignInError> {
    store
        .redeem_magic_link(token, user_agent, clock.now())
        .await?
        .ok_or(SignInError::Unauthenticated)
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use secrecy::ExposeSecret;

    use super::*;

    const NOW: Timestamp = Timestamp::constant(1_900_000_000, 0);

    #[derive(Debug)]
    struct FixedClock;

    impl Clock for FixedClock {
        fn now(&self) -> Timestamp {
            NOW
        }
    }

    /// Records the calls. A redeem succeeds for the token `valid` only.
    #[derive(Debug, Default)]
    struct MemoryStore {
        queued: Mutex<Vec<(Email, Option<Uuid>)>>,
        redeemed_at: Mutex<Vec<Timestamp>>,
    }

    #[async_trait]
    impl SignInStore for MemoryStore {
        async fn queue_magic_link(
            &self,
            email: &Email,
            request_id: Option<Uuid>,
        ) -> Result<(), StoreError> {
            self.queued
                .lock()
                .unwrap()
                .push((email.clone(), request_id));
            Ok(())
        }

        async fn redeem_magic_link(
            &self,
            token: &str,
            _: Option<&str>,
            now: Timestamp,
        ) -> Result<Option<SecretString>, StoreError> {
            self.redeemed_at.lock().unwrap().push(now);
            Ok((token == "valid").then(|| SecretString::from("session")))
        }
    }

    fn organization(n: u128) -> OrganizationId {
        OrganizationId::from_uuid(Uuid::from_u128(n))
    }

    #[test]
    fn only_a_single_membership_gives_the_organization_at_once() {
        assert_eq!(initial_organization(&[]), None);
        assert_eq!(
            initial_organization(&[organization(1)]),
            Some(organization(1))
        );
        assert_eq!(
            initial_organization(&[organization(1), organization(2)]),
            None
        );
    }

    #[tokio::test]
    async fn a_request_queues_the_normalized_address() {
        let store = MemoryStore::default();
        let request = Uuid::from_u128(7);
        request_magic_link(" Anna@Example.org", Some(request), &store)
            .await
            .unwrap();
        assert_eq!(
            *store.queued.lock().unwrap(),
            [(Email::parse("anna@example.org").unwrap(), Some(request))]
        );
    }

    #[tokio::test]
    async fn a_request_with_an_invalid_address_does_nothing_and_succeeds() {
        let store = MemoryStore::default();
        request_magic_link("no address", None, &store)
            .await
            .unwrap();
        assert!(store.queued.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn a_redeem_uses_the_time_of_the_clock() {
        let store = MemoryStore::default();
        let session = redeem_magic_link("valid", None, &store, &FixedClock)
            .await
            .unwrap();
        assert_eq!(session.expose_secret(), "session");
        assert_eq!(*store.redeemed_at.lock().unwrap(), [NOW]);
    }

    #[tokio::test]
    async fn an_invalid_token_is_unauthenticated() {
        let store = MemoryStore::default();
        let result = redeem_magic_link("used", None, &store, &FixedClock).await;
        assert!(matches!(result, Err(SignInError::Unauthenticated)));
    }

    #[test]
    fn each_error_has_a_code_in_its_list() {
        for error in [
            SignInError::Unauthenticated,
            SignInError::Store(StoreError::Internal("test".into())),
        ] {
            assert!(SignInError::CODES.contains(&error.code()), "{error:?}");
        }
    }
}
