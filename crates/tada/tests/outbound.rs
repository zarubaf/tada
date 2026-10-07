//! Outbound intents end to end: the intent and the job in PostgreSQL, the send job, and the mail.

// The helpers of this test file are not `#[test]` functions, so clippy.toml does not cover them.
#![allow(clippy::unwrap_used)]

use std::sync::Arc;
use std::time::Duration;

use tada_adapters::clock::SystemClock;
use tada_adapters::mail::{FluentMailTexts, MemoryMailer};
use tada_app::domain::identity::{DisplayName, Email};
use tada_app::jobs::{Handlers, Ran, run_next};
use tada_app::outbound::{Purpose, SendOutbound};
use tada_store_pg::testing::TestDatabase;
use uuid::Uuid;

const LEASE: Duration = Duration::from_secs(60);
const LINK: &str = "https://tada.example.org/sign-in/link#token=";

struct Worker {
    test: TestDatabase,
    mailer: Arc<MemoryMailer>,
    handlers: Handlers,
}

impl Worker {
    async fn start() -> Self {
        let test = TestDatabase::start().await;
        let mailer = Arc::new(MemoryMailer::new());
        let handler = SendOutbound::new(
            Arc::new(test.database.clone()),
            mailer.clone(),
            Arc::new(FluentMailTexts::new().unwrap()),
            Arc::new(SystemClock),
            "https://tada.example.org",
            "tada.example.org",
        );
        Self {
            test,
            mailer,
            handlers: Handlers::default().with(Arc::new(handler)),
        }
    }

    async fn run_next(&self) -> Ran {
        run_next(&self.test.database, &self.handlers, Uuid::now_v7(), LEASE)
            .await
            .unwrap()
    }
}

#[tokio::test]
async fn a_rolled_back_intent_sends_nothing_and_a_committed_one_sends_one_link() {
    let worker = Worker::start().await;
    let user_id = worker
        .test
        .create_user(
            &DisplayName::parse("Anna Muster").unwrap(),
            &Email::parse("anna@example.org").unwrap(),
        )
        .await;
    let purpose = Purpose::MagicLink { user_id };

    worker.test.queue_outbound(&purpose, false).await;
    assert_eq!(
        worker.run_next().await,
        Ran::Idle,
        "the rollback left no job"
    );
    assert!(worker.mailer.sent().is_empty());

    worker.test.queue_outbound(&purpose, true).await;
    assert!(matches!(worker.run_next().await, Ran::Completed(_)));
    let sent = worker.mailer.sent();
    assert_eq!(sent.len(), 1);
    let start = sent[0].text.find(LINK).unwrap() + LINK.len();
    let token: String = sent[0].text[start..]
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
        .collect();
    assert_eq!(token.len(), 43, "256 bits in Base64");
    assert!(sent[0].message_id.ends_with("@tada.example.org>"));
    worker.test.assert_no_plaintext(&token).await;
    assert_eq!(worker.run_next().await, Ran::Idle);
}
