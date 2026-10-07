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
use crate::jobs::{Job, JobFailed, JobHandler};
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

    /// Records the outcome of a pending intent.
    async fn finish(&self, intent_id: Uuid, outcome: Outcome) -> Result<(), StoreError>;
}

/// The handler of the send job.
///
/// Each attempt creates a new token, because the store keeps only the hash of the old one.
/// The old token expires unused.
pub struct SendOutbound {
    store: Arc<dyn OutboundStore>,
    mailer: Arc<dyn Mailer>,
    texts: Arc<dyn MailTexts>,
    clock: Arc<dyn Clock>,
    public_url: String,
    host: String,
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
    /// `public_url` is `TADA_PUBLIC_URL`, the start of each link (ADR 0042).
    /// `host` is its host, the right part of each `Message-ID`.
    pub fn new(
        store: Arc<dyn OutboundStore>,
        mailer: Arc<dyn Mailer>,
        texts: Arc<dyn MailTexts>,
        clock: Arc<dyn Clock>,
        public_url: &str,
        host: &str,
    ) -> Self {
        Self {
            store,
            mailer,
            texts,
            clock,
            public_url: public_url.trim_end_matches('/').to_owned(),
            host: host.to_owned(),
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
            message_id: format!("<{}@{}>", intent.message_id, self.host),
        })
    }

    /// The token goes into the fragment, so that the browser never sends it in a GET request (ADR 0008).
    fn link(&self, path: &str, token: &SecretString) -> String {
        format!("{}{path}#token={}", self.public_url, token.expose_secret())
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

    async fn run(&self, job: &Job) -> Result<(), JobFailed> {
        let intent_id = intent_id(job)?;
        let Some(intent) = self
            .store
            .load_pending(intent_id)
            .await
            .map_err(store_failed)?
        else {
            // An earlier attempt recorded the outcome already.
            return Ok(());
        };
        let message = self.message(intent).await?;
        let outcome = match self.mailer.send(&message).await {
            Ok(()) => Outcome::Sent,
            Err(SendError::Rejected(_)) => Outcome::Failed,
            Err(SendError::Unknown) => Outcome::Unknown,
            // The queue retries with a backoff, and the intent stays pending.
            Err(error @ SendError::Temporary(_)) => return Err(JobFailed(error.to_string())),
        };
        self.store
            .finish(intent_id, outcome)
            .await
            .map_err(store_failed)
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

    /// One pending intent. Each issued token is `token-<n>`.
    #[derive(Default)]
    struct MemoryStore {
        pending: Mutex<Option<PendingIntent>>,
        issued: Mutex<Vec<(String, Timestamp)>>,
        finished: Mutex<Vec<Outcome>>,
    }

    impl MemoryStore {
        fn with(intent: PendingIntent) -> Self {
            Self {
                pending: Mutex::new(Some(intent)),
                ..Self::default()
            }
        }

        fn issue(&self, expires_at: Timestamp) -> SecretString {
            let mut issued = self.issued.lock().unwrap();
            let token = format!("token-{}", issued.len() + 1);
            issued.push((token.clone(), expires_at));
            SecretString::from(token)
        }
    }

    #[async_trait]
    impl OutboundStore for MemoryStore {
        async fn load_pending(&self, _: Uuid) -> Result<Option<PendingIntent>, StoreError> {
            Ok(self.pending.lock().unwrap().clone())
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
            self.pending.lock().unwrap().take();
            self.finished.lock().unwrap().push(outcome);
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
        }
    }

    struct Setup {
        store: Arc<MemoryStore>,
        mailer: Arc<TestMailer>,
        handler: SendOutbound,
    }

    fn setup(intent: PendingIntent, result: Result<(), SendError>) -> Setup {
        let store = Arc::new(MemoryStore::with(intent));
        let mailer = Arc::new(TestMailer::answering(result));
        let handler = SendOutbound::new(
            store.clone(),
            mailer.clone(),
            Arc::new(LinkTexts),
            Arc::new(FixedClock),
            "https://tada.example.org",
            "tada.example.org",
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

    #[tokio::test]
    async fn an_unknown_outcome_is_recorded_and_completes_the_job() {
        let setup = setup(magic_link(), Err(SendError::Unknown));
        assert_eq!(setup.handler.run(&job(1)).await, Ok(()));
        assert_eq!(finished(&setup), [Outcome::Unknown]);
    }

    #[tokio::test]
    async fn a_rejected_message_is_recorded_as_failed_and_completes_the_job() {
        let setup = setup(
            magic_link(),
            Err(SendError::Rejected("SMTP status 550".into())),
        );
        assert_eq!(setup.handler.run(&job(1)).await, Ok(()));
        assert_eq!(finished(&setup), [Outcome::Failed]);
    }

    #[tokio::test]
    async fn a_temporary_failure_fails_the_job_and_a_retry_creates_a_new_token() {
        let setup = setup(
            magic_link(),
            Err(SendError::Temporary("SMTP status 421".into())),
        );
        assert!(setup.handler.run(&job(1)).await.is_err());
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
    async fn a_finished_intent_sends_nothing() {
        let setup = setup(magic_link(), Ok(()));
        setup.store.pending.lock().unwrap().take();
        assert_eq!(setup.handler.run(&job(2)).await, Ok(()));
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
