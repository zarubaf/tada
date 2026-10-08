//! Sign-in with a magic link (ADR 0008, ADR 0056).
//!
//! 1. A person asks for a magic link with an email address. Each address gets the same answer.
//! 2. Only a user with a membership gets a mail. The worker creates the token and sends it.
//! 3. The person sends the token from the link. The token works once and starts a new session.
//!
//! An invitation works the same way (ADR 0008, ADR 0056): the worker mails a token, the invitee
//! sees what the invitation is for, and the acceptance uses the token once and starts a session.

use std::fmt::Debug;
use std::net::IpAddr;

use async_trait::async_trait;
use jiff::{SignedDuration, Timestamp};
use secrecy::SecretString;
use tada_domain::identity::{Email, OrganizationRole};
use tada_domain::ids::OrganizationId;
use uuid::Uuid;

use crate::clock::Clock;
use crate::problem::{CommandError, ProblemCode};
use crate::rate_limit::{RateDecision, RateLimit, sign_in_limits};
use crate::store::StoreError;

/// The repository port of sign-in. Each method takes a token as the holder sends it.
/// Only the store hashes it; no table holds the token (ADR 0008).
#[async_trait]
pub trait SignInStore: Debug + Send + Sync {
    /// Deletes the magic link of `token`. If the link existed and was valid at `now`, the same
    /// transaction starts a session for its user and returns the session token.
    /// The organization of the session is `initial_organization` of the user's memberships.
    /// A user without a membership gets no session, and the link is used up all the same.
    /// Infrastructure query (ADR 0039): sign-in has no organization yet, and the token names the user.
    async fn redeem_magic_link(
        &self,
        token: &str,
        user_agent: Option<&str>,
        now: Timestamp,
    ) -> Result<Option<SecretString>, StoreError>;

    /// What the pending invitation of `token` is for, if the token is valid at `now`.
    /// It does not use the token.
    /// Infrastructure query (ADR 0039): the token names the invitation, and the invitation names its organization.
    async fn preview_invitation(
        &self,
        token: &str,
        now: Timestamp,
    ) -> Result<Option<InvitationPreview>, StoreError>;

    /// Accepts the pending invitation of `token` if the token is valid at `now`, in one transaction:
    /// it finds or creates the user of the address, gives the membership the role of
    /// `accepted_role`, marks the invitation accepted, deletes all its tokens, records the audit
    /// event and starts a session in the organization of the invitation.
    /// It returns the session token, or `None` for an unknown, used or expired token.
    /// Infrastructure query (ADR 0039): the token names the invitation, and the invitation names its organization.
    async fn accept_invitation(
        &self,
        token: &str,
        user_agent: Option<&str>,
        request_id: Option<Uuid>,
        now: Timestamp,
    ) -> Result<Option<SecretString>, StoreError>;
}

/// What an invitation is for, before the invitee accepts it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvitationPreview {
    pub organization_name: String,
    pub role: OrganizationRole,
}

/// The role of a membership after the acceptance of an invitation with the role `invited`:
/// the higher of the two roles. An invitation never lowers a role (ADR 0056).
pub fn accepted_role(
    existing: Option<OrganizationRole>,
    invited: OrganizationRole,
) -> OrganizationRole {
    existing.map_or(invited, |existing| existing.max(invited))
}

/// The repository port of sign-in requests (ADR 0056).
#[async_trait]
pub trait SignInRequestStore: Debug + Send + Sync {
    /// Counts the request against each of `limits` in the window of `now`. If each limit allows
    /// it, the same transaction queues a magic-link intent for the user of `email` if that user has
    /// a membership. A known and an unknown address both write the counters in one commit, so that
    /// the time of the request does not show if the address is known (ADR 0008).
    /// Infrastructure query (ADR 0039): sign-in has no organization yet, and the address names the user.
    async fn queue_magic_link(
        &self,
        email: &Email,
        limits: &[RateLimit<'_>],
        request_id: Option<Uuid>,
        now: Timestamp,
    ) -> Result<RateDecision, StoreError>;
}

/// The organization of a new session: the only organization of a user with one membership.
/// A user with more memberships chooses one later (ADR 0056).
pub fn initial_organization(memberships: &[OrganizationId]) -> Option<OrganizationId> {
    match memberships {
        [organization] => Some(*organization),
        _ => None,
    }
}

/// An error of `request_magic_link`.
#[derive(Debug, thiserror::Error)]
pub enum RequestSignInError {
    /// The address or the client sent too many requests in the current window (ADR 0056).
    #[error("too many sign-in requests")]
    RateLimited { retry_after: SignedDuration },
    #[error(transparent)]
    Store(#[from] StoreError),
}

impl RequestSignInError {
    /// All codes that this command can return, for the API contract (ADR 0037).
    pub const CODES: &[ProblemCode] = &[
        ProblemCode::RateLimited,
        ProblemCode::Unavailable,
        ProblemCode::Internal,
    ];
}

impl CommandError for RequestSignInError {
    fn code(&self) -> ProblemCode {
        match self {
            Self::RateLimited { .. } => ProblemCode::RateLimited,
            Self::Store(error) => error.code(),
        }
    }

    fn store_error(&self) -> Option<&StoreError> {
        match self {
            Self::Store(error) => Some(error),
            _ => None,
        }
    }

    fn retry_after(&self) -> Option<SignedDuration> {
        match self {
            Self::RateLimited { retry_after } => Some(*retry_after),
            Self::Store(_) => None,
        }
    }
}

/// Asks for a magic link from the client `client_ip`. An invalid or unknown address, or an address
/// without a membership, gets no mail, and the caller gets no information about it (ADR 0056).
/// Each valid address counts against the limits of the address and of the client.
pub async fn request_magic_link(
    email: &str,
    client_ip: IpAddr,
    request_id: Option<Uuid>,
    store: &dyn SignInRequestStore,
    clock: &dyn Clock,
) -> Result<(), RequestSignInError> {
    let Ok(email) = Email::parse(email) else {
        return Ok(());
    };
    let limits = sign_in_limits(&email, client_ip);
    match store
        .queue_magic_link(&email, &limits, request_id, clock.now())
        .await?
    {
        RateDecision::Allowed => Ok(()),
        RateDecision::Limited { retry_after } => {
            Err(RequestSignInError::RateLimited { retry_after })
        }
    }
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
}

impl CommandError for SignInError {
    fn code(&self) -> ProblemCode {
        match self {
            Self::Unauthenticated => ProblemCode::Unauthenticated,
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

/// Shows what the invitation of `token` is for. The token stays valid.
pub async fn preview_invitation(
    token: &str,
    store: &dyn SignInStore,
    clock: &dyn Clock,
) -> Result<InvitationPreview, SignInError> {
    store
        .preview_invitation(token, clock.now())
        .await?
        .ok_or(SignInError::Unauthenticated)
}

/// Accepts the invitation of `token` once and returns the token of the new session.
pub async fn accept_invitation(
    token: &str,
    user_agent: Option<&str>,
    request_id: Option<Uuid>,
    store: &dyn SignInStore,
    clock: &dyn Clock,
) -> Result<SecretString, SignInError> {
    store
        .accept_invitation(token, user_agent, request_id, clock.now())
        .await?
        .ok_or(SignInError::Unauthenticated)
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use secrecy::ExposeSecret;

    use super::*;
    use crate::rate_limit::{RateSubject, SIGN_IN_PER_EMAIL, SIGN_IN_PER_IP};

    const NOW: Timestamp = Timestamp::constant(1_900_000_000, 0);

    #[derive(Debug)]
    struct FixedClock;

    impl Clock for FixedClock {
        fn now(&self) -> Timestamp {
            NOW
        }
    }

    const CLIENT: IpAddr = IpAddr::V4(std::net::Ipv4Addr::new(203, 0, 113, 7));

    /// A queued request: the address, the subjects and limits, the request ID and the time.
    type Queued = (Email, Vec<(String, u32)>, Option<Uuid>, Timestamp);

    /// Records the calls. A redeem, a preview and an acceptance succeed for the token `valid` only.
    /// A request is limited if `limited` is set.
    #[derive(Debug, Default)]
    struct MemoryStore {
        queued: Mutex<Vec<Queued>>,
        redeemed_at: Mutex<Vec<Timestamp>>,
        accepted: Mutex<Vec<(Option<Uuid>, Timestamp)>>,
        limited: Option<SignedDuration>,
    }

    #[async_trait]
    impl SignInRequestStore for MemoryStore {
        async fn queue_magic_link(
            &self,
            email: &Email,
            limits: &[RateLimit<'_>],
            request_id: Option<Uuid>,
            now: Timestamp,
        ) -> Result<RateDecision, StoreError> {
            let limits = limits
                .iter()
                .map(|limit| {
                    let subject = match limit.subject {
                        RateSubject::Email(email) => email.as_str().to_owned(),
                        RateSubject::Ip(address) => address.to_string(),
                    };
                    (subject, limit.limit)
                })
                .collect();
            self.queued
                .lock()
                .unwrap()
                .push((email.clone(), limits, request_id, now));
            Ok(match self.limited {
                None => RateDecision::Allowed,
                Some(retry_after) => RateDecision::Limited { retry_after },
            })
        }
    }

    #[async_trait]
    impl SignInStore for MemoryStore {
        async fn redeem_magic_link(
            &self,
            token: &str,
            _: Option<&str>,
            now: Timestamp,
        ) -> Result<Option<SecretString>, StoreError> {
            self.redeemed_at.lock().unwrap().push(now);
            Ok((token == "valid").then(|| SecretString::from("session")))
        }

        async fn preview_invitation(
            &self,
            token: &str,
            now: Timestamp,
        ) -> Result<Option<InvitationPreview>, StoreError> {
            assert_eq!(now, NOW);
            Ok((token == "valid").then(|| InvitationPreview {
                organization_name: "Open Day Testwil".into(),
                role: OrganizationRole::Admin,
            }))
        }

        async fn accept_invitation(
            &self,
            token: &str,
            _: Option<&str>,
            request_id: Option<Uuid>,
            now: Timestamp,
        ) -> Result<Option<SecretString>, StoreError> {
            self.accepted.lock().unwrap().push((request_id, now));
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
    async fn a_request_queues_the_normalized_address_with_both_limits() {
        let store = MemoryStore::default();
        let request = Uuid::from_u128(7);
        request_magic_link(
            " Anna@Example.org",
            CLIENT,
            Some(request),
            &store,
            &FixedClock,
        )
        .await
        .unwrap();
        assert_eq!(
            *store.queued.lock().unwrap(),
            [(
                Email::parse("anna@example.org").unwrap(),
                vec![
                    ("anna@example.org".to_owned(), SIGN_IN_PER_EMAIL),
                    ("203.0.113.7".to_owned(), SIGN_IN_PER_IP),
                ],
                Some(request),
                NOW
            )]
        );
    }

    #[tokio::test]
    async fn a_request_with_an_invalid_address_does_nothing_and_succeeds() {
        let store = MemoryStore::default();
        request_magic_link("no address", CLIENT, None, &store, &FixedClock)
            .await
            .unwrap();
        assert!(store.queued.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn a_limited_request_gives_the_wait() {
        let store = MemoryStore {
            limited: Some(SignedDuration::from_mins(3)),
            ..MemoryStore::default()
        };
        let result =
            request_magic_link("anna@example.org", CLIENT, None, &store, &FixedClock).await;
        assert!(matches!(
            result,
            Err(RequestSignInError::RateLimited { retry_after }) if retry_after == SignedDuration::from_mins(3)
        ));
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
    fn an_invitation_keeps_the_higher_role() {
        use OrganizationRole::{Admin, Member, Owner};
        for (existing, invited, expected) in [
            (None, Member, Member),
            (None, Owner, Owner),
            (Some(Admin), Member, Admin),
            (Some(Owner), Admin, Owner),
            (Some(Member), Admin, Admin),
            (Some(Admin), Owner, Owner),
            (Some(Admin), Admin, Admin),
        ] {
            assert_eq!(
                accepted_role(existing, invited),
                expected,
                "{existing:?} + {invited:?}"
            );
        }
    }

    #[tokio::test]
    async fn a_preview_of_a_valid_token_names_the_organization_and_the_role() {
        let store = MemoryStore::default();
        let preview = preview_invitation("valid", &store, &FixedClock)
            .await
            .unwrap();
        assert_eq!(preview.organization_name, "Open Day Testwil");
        assert_eq!(preview.role, OrganizationRole::Admin);
        let result = preview_invitation("used", &store, &FixedClock).await;
        assert!(matches!(result, Err(SignInError::Unauthenticated)));
    }

    #[tokio::test]
    async fn an_acceptance_uses_the_time_of_the_clock_and_the_request() {
        let store = MemoryStore::default();
        let request = Uuid::from_u128(7);
        let session = accept_invitation("valid", None, Some(request), &store, &FixedClock)
            .await
            .unwrap();
        assert_eq!(session.expose_secret(), "session");
        assert_eq!(*store.accepted.lock().unwrap(), [(Some(request), NOW)]);
        let result = accept_invitation("used", None, None, &store, &FixedClock).await;
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
        for error in [
            RequestSignInError::RateLimited {
                retry_after: SignedDuration::from_mins(1),
            },
            RequestSignInError::Store(StoreError::Unavailable("test".into())),
        ] {
            assert!(
                RequestSignInError::CODES.contains(&error.code()),
                "{error:?}"
            );
        }
    }
}
