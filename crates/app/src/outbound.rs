//! Outbound intents and the job that sends them (ADRs 0007, 0008 and 0042).
//!
//! No command sends mail. A command stores an outbound intent and a send job in its own transaction.
//! The worker then creates the token, sends the message and records the outcome.

use std::fmt;
use std::sync::Arc;

use async_trait::async_trait;
use jiff::{SignedDuration, Timestamp};
use secrecy::{ExposeSecret, SecretString};
use serde_json::Value;
use tada_domain::identity::Email;
use tada_domain::ids::{InvitationId, OrganizationId, UserId};
use uuid::Uuid;

use crate::caller::{JobRunner, OrgScope, ServiceCaller};
use crate::clock::Clock;
use crate::jobs::{Job, JobFailed, JobHandler, JobWarning};
use crate::mail::{MailTexts, Mailer, OutgoingMessage, SendError};
use crate::store::StoreError;

/// A magic link expires after 15 minutes (ADR 0008).
pub const MAGIC_LINK_LIFETIME: SignedDuration = SignedDuration::from_mins(15);
/// An invitation link expires after 7 days (ADR 0008).
pub const INVITATION_LIFETIME: SignedDuration = SignedDuration::from_hours(7 * 24);

/// The job kind that sends one outbound intent.
/// Payload version 1 is `{"intent_id": "<uuid>"}`: no token and no address (ADR 0042).
pub const SEND_JOB: &str = "send-outbound";

/// Why tada sends a message. This is the only description of an intent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Purpose {
    /// A sign-in link for a user. It belongs to the user, not to an organization.
    MagicLink { user_id: UserId },
    /// An invitation to an organization.
    Invitation {
        organization_id: OrganizationId,
        invitation_id: InvitationId,
    },
}

impl Purpose {
    /// The organization of the intent. A magic link has none.
    pub fn organization_id(self) -> Option<OrganizationId> {
        match self {
            Self::MagicLink { .. } => None,
            Self::Invitation {
                organization_id, ..
            } => Some(organization_id),
        }
    }
}

/// An intent that waits for its send, with all that the message needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingIntent {
    pub purpose: Purpose,
    pub to: Email,
    pub locale: String,
    /// The name of the organization of an invitation. A magic link has none.
    pub organization_name: Option<String>,
    /// The left part of the `Message-ID` header. It stays the same for each attempt.
    pub message_id: String,
    /// True if the message must not go out any more: its invitation is accepted or revoked.
    pub obsolete: bool,
}

/// The result of a send.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Sent,
    /// The mail server rejected the message for good.
    Failed,
    /// The server can have taken the message. Nobody sends it again blindly (ADR 0042).
    Unknown,
}

impl Outcome {
    /// The name that the database uses.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Sent => "sent",
            Self::Failed => "failed",
            Self::Unknown => "unknown",
        }
    }
}

/// The repository port of the send job.
#[async_trait]
pub trait OutboundStore: Send + Sync {
    /// The intent, if it still waits for its send.
    /// An infrastructure query without a scope (ADR 0039): it returns the organization of the intent
    /// in its purpose.
    async fn load_pending(&self, intent_id: Uuid) -> Result<Option<PendingIntent>, StoreError>;

    /// Moves a pending intent to `unknown` before its send. Returns false if it is not pending.
    /// From then on, no attempt sends it again unless `release` returns it.
    async fn claim(&self, intent_id: Uuid) -> Result<bool, StoreError>;

    /// Stores the hash of a new magic-link token and returns the token.
    async fn issue_magic_link(
        &self,
        user_id: UserId,
        expires_at: Timestamp,
    ) -> Result<SecretString, StoreError>;

    /// Stores the hash of a new invitation token and returns the token.
    async fn issue_invitation_token(
        &self,
        scope: OrgScope,
        invitation_id: InvitationId,
        expires_at: Timestamp,
    ) -> Result<SecretString, StoreError>;

    /// Records the outcome of a claimed intent.
    async fn finish(&self, intent_id: Uuid, outcome: Outcome) -> Result<(), StoreError>;

    /// Returns a claimed intent to pending, after a send that certainly did not deliver the message.
    async fn release(&self, intent_id: Uuid) -> Result<(), StoreError>;
}

/// `TADA_PUBLIC_URL` (ADR 0042). Each link starts with it, and its host is the right part of each
/// `Message-ID`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublicUrl {
    /// The URL without the final slash, for example `https://tada.example.org`.
    origin: String,
    host: String,
}

/// The public URL is not an `http` or `https` URL without a path, a query and a user.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("the public URL must be an http or https URL without a path")]
pub struct InvalidPublicUrl;

impl PublicUrl {
    pub fn parse(url: &str) -> Result<Self, InvalidPublicUrl> {
        let authority = ["https://", "http://"]
            .iter()
            .find_map(|scheme| url.strip_prefix(scheme))
            .ok_or(InvalidPublicUrl)?;
        let authority = authority.strip_suffix('/').unwrap_or(authority);
        if authority.is_empty() || authority.contains(['/', '?', '#', '@']) {
            return Err(InvalidPublicUrl);
        }
        // An IPv6 address keeps its brackets, as a `Message-ID` domain literal needs them.
        let host = match authority.find(']') {
            Some(end) if authority.starts_with('[') => &authority[..=end],
            _ => authority.split(':').next().unwrap_or(authority),
        };
        Ok(Self {
            origin: url.strip_suffix('/').unwrap_or(url).to_owned(),
            host: host.to_owned(),
        })
    }
}

/// The handler of the send job.
///
/// Each attempt creates a new token, because the store keeps only the hash of the old one.
/// The old token expires unused.
///
/// The handler claims the intent before it creates the token: the intent is then `unknown`.
/// It returns the intent to `pending` only after a temporary failure, which certainly delivered
/// nothing. So a mail is never sent twice (ADR 0042).
/// The remaining window: if the worker stops after the claim, or the store cannot record the
/// outcome, the intent stays `unknown`, and a mail that went out unrecorded, or did not go out at
/// all, is not sent again.
pub struct SendOutbound {
    store: Arc<dyn OutboundStore>,
    mailer: Arc<dyn Mailer>,
    texts: Arc<dyn MailTexts>,
    clock: Arc<dyn Clock>,
    public_url: PublicUrl,
    caller: ServiceCaller<JobRunner>,
}

impl fmt::Debug for SendOutbound {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SendOutbound")
            .field("public_url", &self.public_url)
            .finish_non_exhaustive()
    }
}

impl SendOutbound {
    pub fn new(
        store: Arc<dyn OutboundStore>,
        mailer: Arc<dyn Mailer>,
        texts: Arc<dyn MailTexts>,
        clock: Arc<dyn Clock>,
        public_url: PublicUrl,
    ) -> Self {
        Self {
            store,
            mailer,
            texts,
            clock,
            public_url,
            caller: ServiceCaller::new(),
        }
    }

    /// Creates the token and renders the message. The token is only in the link of the message.
    async fn message(&self, intent: PendingIntent) -> Result<OutgoingMessage, JobFailed> {
        let now = self.clock.now();
        let rendered = match intent.purpose {
            Purpose::MagicLink { user_id } => {
                let token = self
                    .store
                    .issue_magic_link(user_id, now + MAGIC_LINK_LIFETIME)
                    .await
                    .map_err(store_failed)?;
                let link = self.link("/sign-in/link", &token);
                self.texts.magic_link(&intent.locale, &link)
            }
            Purpose::Invitation {
                organization_id,
                invitation_id,
            } => {
                // The intent named its organization, so the handler continues in it (ADR 0039).
                let scope = self.caller.scope(organization_id);
                let organization_name = intent
                    .organization_name
                    .as_deref()
                    .ok_or_else(|| JobFailed("the invitation has no organization name".into()))?;
                let token = self
                    .store
                    .issue_invitation_token(scope, invitation_id, now + INVITATION_LIFETIME)
                    .await
                    .map_err(store_failed)?;
                let link = self.link("/invitation", &token);
                self.texts
                    .invitation(&intent.locale, organization_name, &link)
            }
        };
        Ok(OutgoingMessage {
            to: intent.to,
            subject: rendered.subject,
            text: rendered.text,
            html: rendered.html,
            message_id: format!("<{}@{}>", intent.message_id, self.public_url.host),
        })
    }

    /// The token goes into the fragment, so that the browser never sends it in a GET request (ADR 0008).
    fn link(&self, path: &str, token: &SecretString) -> String {
        format!(
            "{}{path}#token={}",
            self.public_url.origin,
            token.expose_secret()
        )
    }

    /// Records the outcome of the claimed intent.
    async fn finish(&self, intent_id: Uuid, outcome: Outcome) -> Result<(), JobFailed> {
        self.store
            .finish(intent_id, outcome)
            .await
            .map_err(store_failed)
    }
}

/// The intent ID of a version 1 payload.
fn intent_id(job: &Job) -> Result<Uuid, JobFailed> {
    if job.version != 1 {
        return Err(JobFailed(format!(
            "the payload version {} is unknown",
            job.version
        )));
    }
    job.payload
        .get("intent_id")
        .and_then(Value::as_str)
        .and_then(|id| Uuid::parse_str(id).ok())
        .ok_or_else(|| JobFailed("the payload has no intent ID".into()))
}

fn store_failed(error: StoreError) -> JobFailed {
    JobFailed(error.to_string())
}

#[async_trait]
impl JobHandler for SendOutbound {
    fn kind(&self) -> &'static str {
        SEND_JOB
    }

    async fn run(&self, job: &Job) -> Result<Option<JobWarning>, JobFailed> {
        let intent_id = intent_id(job)?;
        let Some(intent) = self
            .store
            .load_pending(intent_id)
            .await
            .map_err(store_failed)?
        else {
            // An earlier attempt claimed the intent already.
            return Ok(None);
        };
        if !self.store.claim(intent_id).await.map_err(store_failed)? {
            return Ok(None);
        }
        if intent.obsolete {
            self.finish(intent_id, Outcome::Failed).await?;
            return Ok(Some(JobWarning(format!(
                "the invitation of the intent {intent_id} is no longer pending, so nothing was sent"
            ))));
        }
        let message = match self.message(intent).await {
            Ok(message) => message,
            Err(failure) => {
                // Nothing went out, so a retry is safe.
                self.store.release(intent_id).await.map_err(store_failed)?;
                return Err(failure);
            }
        };
        match self.mailer.send(&message).await {
            Ok(()) => {
                self.finish(intent_id, Outcome::Sent).await?;
                Ok(None)
            }
            Err(error @ SendError::Rejected(_)) => {
                self.finish(intent_id, Outcome::Failed).await?;
                Ok(Some(JobWarning(format!("intent {intent_id}: {error}"))))
            }
            Err(error @ SendError::Unknown) => {
                self.finish(intent_id, Outcome::Unknown).await?;
                Ok(Some(JobWarning(format!("intent {intent_id}: {error}"))))
            }
            Err(error @ SendError::Temporary(_)) => {
                if job.is_last_attempt() {
                    self.finish(intent_id, Outcome::Failed).await?;
                } else {
                    // The queue retries with a backoff.
                    self.store.release(intent_id).await.map_err(store_failed)?;
                }
                Err(JobFailed(error.to_string()))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use serde_json::json;
    use tada_domain::ids::OrganizationId;

    use super::*;
    use crate::jobs::Job;
    use crate::mail::{OutgoingMessage, Rendered, SendError};

    const NOW: Timestamp = Timestamp::constant(1_800_000_000, 0);

    #[derive(Debug)]
    struct FixedClock;

    impl Clock for FixedClock {
        fn now(&self) -> Timestamp {
            NOW
        }
    }

    /// One intent with its status. Each issued token is `token-<n>`.
    struct MemoryStore {
        intent: PendingIntent,
        status: Mutex<&'static str>,
        issued: Mutex<Vec<(String, Timestamp)>>,
        finished: Mutex<Vec<Outcome>>,
        /// Simulates a database failure when the handler records the outcome.
        finish_fails: bool,
    }

    impl MemoryStore {
        fn with(intent: PendingIntent) -> Self {
            Self {
                intent,
                status: Mutex::new("pending"),
                issued: Mutex::default(),
                finished: Mutex::default(),
                finish_fails: false,
            }
        }

        fn status(&self) -> &'static str {
            *self.status.lock().unwrap()
        }

        fn issue(&self, expires_at: Timestamp) -> SecretString {
            let mut issued = self.issued.lock().unwrap();
            let token = format!("token-{}", issued.len() + 1);
            issued.push((token.clone(), expires_at));
            SecretString::from(token)
        }

        fn failure() -> StoreError {
            StoreError::Internal("the database failed".into())
        }
    }

    #[async_trait]
    impl OutboundStore for MemoryStore {
        async fn load_pending(&self, _: Uuid) -> Result<Option<PendingIntent>, StoreError> {
            Ok((self.status() == "pending").then(|| self.intent.clone()))
        }

        async fn claim(&self, _: Uuid) -> Result<bool, StoreError> {
            let mut status = self.status.lock().unwrap();
            let claimed = *status == "pending";
            if claimed {
                *status = "unknown";
            }
            Ok(claimed)
        }

        async fn issue_magic_link(
            &self,
            _: UserId,
            expires_at: Timestamp,
        ) -> Result<SecretString, StoreError> {
            Ok(self.issue(expires_at))
        }

        async fn issue_invitation_token(
            &self,
            _: OrgScope,
            _: InvitationId,
            expires_at: Timestamp,
        ) -> Result<SecretString, StoreError> {
            Ok(self.issue(expires_at))
        }

        async fn finish(&self, _: Uuid, outcome: Outcome) -> Result<(), StoreError> {
            if self.finish_fails {
                return Err(Self::failure());
            }
            let mut status = self.status.lock().unwrap();
            if *status == "unknown" {
                *status = outcome.as_str();
                self.finished.lock().unwrap().push(outcome);
            }
            Ok(())
        }

        async fn release(&self, _: Uuid) -> Result<(), StoreError> {
            let mut status = self.status.lock().unwrap();
            if *status == "unknown" {
                *status = "pending";
            }
            Ok(())
        }
    }

    /// Records each message and answers with `result`.
    struct TestMailer {
        result: Result<(), SendError>,
        sent: Mutex<Vec<OutgoingMessage>>,
    }

    impl TestMailer {
        fn answering(result: Result<(), SendError>) -> Self {
            Self {
                result,
                sent: Mutex::default(),
            }
        }
    }

    #[async_trait]
    impl Mailer for TestMailer {
        async fn send(&self, message: &OutgoingMessage) -> Result<(), SendError> {
            self.sent.lock().unwrap().push(message.clone());
            self.result.clone()
        }
    }

    /// The text part is the link, so that a test can read it.
    struct LinkTexts;

    impl MailTexts for LinkTexts {
        fn magic_link(&self, _: &str, link: &str) -> Rendered {
            Rendered {
                subject: "Anmeldung".into(),
                text: link.into(),
                html: String::new(),
            }
        }

        fn invitation(&self, _: &str, organization_name: &str, link: &str) -> Rendered {
            Rendered {
                subject: organization_name.into(),
                text: link.into(),
                html: String::new(),
            }
        }
    }

    fn magic_link() -> PendingIntent {
        PendingIntent {
            purpose: Purpose::MagicLink {
                user_id: UserId::from_uuid(Uuid::from_u128(1)),
            },
            to: Email::parse("anna@example.org").unwrap(),
            locale: "de-CH".into(),
            organization_name: None,
            message_id: "abc".into(),
            obsolete: false,
        }
    }

    fn invitation() -> PendingIntent {
        PendingIntent {
            purpose: Purpose::Invitation {
                organization_id: OrganizationId::from_uuid(Uuid::from_u128(2)),
                invitation_id: InvitationId::from_uuid(Uuid::from_u128(3)),
            },
            organization_name: Some("Fliegerclub Musterhausen".into()),
            ..magic_link()
        }
    }

    fn job(attempt: i32) -> Job {
        Job {
            id: Uuid::from_u128(9),
            kind: SEND_JOB.into(),
            version: 1,
            payload: json!({"intent_id": Uuid::from_u128(4)}),
            organization_id: None,
            request_id: None,
            attempt,
            max_attempts: 10,
        }
    }

    struct Setup {
        store: Arc<MemoryStore>,
        mailer: Arc<TestMailer>,
        handler: SendOutbound,
    }

    fn setup(intent: PendingIntent, result: Result<(), SendError>) -> Setup {
        setup_with(MemoryStore::with(intent), result)
    }

    fn setup_with(store: MemoryStore, result: Result<(), SendError>) -> Setup {
        let store = Arc::new(store);
        let mailer = Arc::new(TestMailer::answering(result));
        let handler = SendOutbound::new(
            store.clone(),
            mailer.clone(),
            Arc::new(LinkTexts),
            Arc::new(FixedClock),
            PublicUrl::parse("https://tada.example.org/").unwrap(),
        );
        Setup {
            store,
            mailer,
            handler,
        }
    }

    fn sent(setup: &Setup) -> Vec<OutgoingMessage> {
        setup.mailer.sent.lock().unwrap().clone()
    }

    fn finished(setup: &Setup) -> Vec<Outcome> {
        setup.store.finished.lock().unwrap().clone()
    }

    #[test]
    fn a_public_url_gives_the_origin_and_the_host() {
        for (url, origin, host) in [
            (
                "https://tada.example.org/",
                "https://tada.example.org",
                "tada.example.org",
            ),
            (
                "http://localhost:5173",
                "http://localhost:5173",
                "localhost",
            ),
            ("http://[::1]:8080/", "http://[::1]:8080", "[::1]"),
        ] {
            let parsed = PublicUrl::parse(url).unwrap();
            assert_eq!(
                (parsed.origin.as_str(), parsed.host.as_str()),
                (origin, host)
            );
        }
    }

    #[test]
    fn a_public_url_with_a_path_or_another_scheme_is_invalid() {
        for url in [
            "https://tada.example.org/app",
            "ftp://tada.example.org",
            "https://",
            "https://anna@tada.example.org",
        ] {
            assert_eq!(PublicUrl::parse(url), Err(InvalidPublicUrl), "{url}");
        }
    }

    #[tokio::test]
    async fn sends_a_magic_link_with_the_token_in_the_fragment() {
        let setup = setup(magic_link(), Ok(()));
        setup.handler.run(&job(1)).await.unwrap();

        let sent = sent(&setup);
        assert_eq!(sent.len(), 1);
        assert_eq!(
            sent[0].text,
            "https://tada.example.org/sign-in/link#token=token-1"
        );
        assert_eq!(sent[0].to, Email::parse("anna@example.org").unwrap());
        assert_eq!(sent[0].message_id, "<abc@tada.example.org>");
        assert_eq!(
            setup.store.issued.lock().unwrap()[0].1,
            NOW + MAGIC_LINK_LIFETIME
        );
        assert_eq!(finished(&setup), [Outcome::Sent]);
    }

    #[tokio::test]
    async fn sends_an_invitation_that_lasts_seven_days() {
        let setup = setup(invitation(), Ok(()));
        setup.handler.run(&job(1)).await.unwrap();

        let sent = sent(&setup);
        assert_eq!(
            sent[0].text,
            "https://tada.example.org/invitation#token=token-1"
        );
        assert_eq!(sent[0].subject, "Fliegerclub Musterhausen");
        assert_eq!(
            setup.store.issued.lock().unwrap()[0].1,
            NOW + INVITATION_LIFETIME
        );
        assert_eq!(INVITATION_LIFETIME, SignedDuration::from_hours(7 * 24));
        assert_eq!(MAGIC_LINK_LIFETIME, SignedDuration::from_mins(15));
    }

    /// The intent ID of `job`.
    const INTENT: Uuid = Uuid::from_u128(4);

    #[tokio::test]
    async fn an_unknown_outcome_is_recorded_and_completes_the_job_with_a_warning() {
        let setup = setup(magic_link(), Err(SendError::Unknown));
        let warning = setup.handler.run(&job(1)).await.unwrap().unwrap();
        assert!(warning.0.contains(&INTENT.to_string()), "{warning:?}");
        assert_eq!(finished(&setup), [Outcome::Unknown]);
        assert_eq!(setup.store.status(), "unknown");
    }

    #[tokio::test]
    async fn a_rejected_message_is_recorded_as_failed_and_reported_without_the_address() {
        let setup = setup(
            magic_link(),
            Err(SendError::Rejected("SMTP status 550".into())),
        );
        let warning = setup.handler.run(&job(1)).await.unwrap().unwrap();
        assert!(warning.0.contains("SMTP status 550"), "{warning:?}");
        assert!(warning.0.contains(&INTENT.to_string()), "{warning:?}");
        assert!(!warning.0.contains("anna"), "{warning:?}");
        assert_eq!(finished(&setup), [Outcome::Failed]);
    }

    #[tokio::test]
    async fn a_temporary_failure_fails_the_job_and_a_retry_creates_a_new_token() {
        let setup = setup(
            magic_link(),
            Err(SendError::Temporary("SMTP status 421".into())),
        );
        assert!(setup.handler.run(&job(1)).await.is_err());
        assert_eq!(setup.store.status(), "pending");
        assert!(setup.handler.run(&job(2)).await.is_err());
        assert_eq!(finished(&setup), []);
        let tokens: Vec<String> = sent(&setup).into_iter().map(|m| m.text).collect();
        assert_eq!(
            tokens,
            [
                "https://tada.example.org/sign-in/link#token=token-1",
                "https://tada.example.org/sign-in/link#token=token-2"
            ]
        );
    }

    #[tokio::test]
    async fn a_temporary_failure_of_the_last_attempt_records_failed() {
        let setup = setup(
            magic_link(),
            Err(SendError::Temporary("SMTP status 421".into())),
        );
        assert!(setup.handler.run(&job(10)).await.is_err());
        assert_eq!(finished(&setup), [Outcome::Failed]);
    }

    #[tokio::test]
    async fn a_send_whose_outcome_was_not_recorded_is_not_sent_again() {
        let store = MemoryStore {
            finish_fails: true,
            ..MemoryStore::with(magic_link())
        };
        let setup = setup_with(store, Ok(()));
        assert!(setup.handler.run(&job(1)).await.is_err());
        assert_eq!(setup.handler.run(&job(2)).await, Ok(None));
        assert_eq!(sent(&setup).len(), 1);
        assert_eq!(setup.store.status(), "unknown");
    }

    #[tokio::test]
    async fn an_obsolete_invitation_is_recorded_as_failed_without_a_token_or_a_mail() {
        let setup = setup(
            PendingIntent {
                obsolete: true,
                ..invitation()
            },
            Ok(()),
        );
        let warning = setup.handler.run(&job(1)).await.unwrap().unwrap();
        assert!(warning.0.contains(&INTENT.to_string()), "{warning:?}");
        assert!(sent(&setup).is_empty());
        assert!(setup.store.issued.lock().unwrap().is_empty());
        assert_eq!(finished(&setup), [Outcome::Failed]);
    }

    #[tokio::test]
    async fn a_finished_intent_sends_nothing() {
        let setup = setup(magic_link(), Ok(()));
        *setup.store.status.lock().unwrap() = "sent";
        assert_eq!(setup.handler.run(&job(2)).await, Ok(None));
        assert!(sent(&setup).is_empty());
    }

    #[tokio::test]
    async fn rejects_an_unknown_payload_version() {
        let setup = setup(magic_link(), Ok(()));
        let job = Job {
            version: 2,
            ..job(1)
        };
        assert!(setup.handler.run(&job).await.is_err());
        assert!(sent(&setup).is_empty());
    }
}
