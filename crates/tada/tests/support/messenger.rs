//! A `Messenger` that keeps each message in memory instead of a call to the Bot API.

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use tada_app::messenger::{Button, Messenger, SendOutcome};
use tada_app::telegram::TelegramUserId;

/// One message that the fake took.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sent {
    pub to: i64,
    pub text: String,
    pub rows: Vec<Vec<Button>>,
}

/// Answers the sends with `outcomes` in order, and each later send with the last one. With `crash`, it keeps the
/// message and then panics, as a worker that stops after the Bot API call and before it records the outcome.
#[derive(Debug)]
pub struct FakeMessenger {
    outcomes: Vec<SendOutcome>,
    crash: bool,
    sent: Mutex<Vec<Sent>>,
}

impl FakeMessenger {
    pub fn new(outcome: SendOutcome) -> Arc<Self> {
        Self::answering(&[outcome])
    }

    pub fn answering(outcomes: &[SendOutcome]) -> Arc<Self> {
        Arc::new(Self {
            outcomes: outcomes.to_vec(),
            crash: false,
            sent: Mutex::default(),
        })
    }

    pub fn crashing() -> Arc<Self> {
        Arc::new(Self {
            outcomes: vec![SendOutcome::Sent],
            crash: true,
            sent: Mutex::default(),
        })
    }

    pub fn sent(&self) -> Vec<Sent> {
        self.sent
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }
}

#[async_trait]
impl Messenger for FakeMessenger {
    async fn send(&self, to: TelegramUserId, text: &str, rows: &[Vec<Button>]) -> SendOutcome {
        let call = {
            let mut sent = self
                .sent
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            sent.push(Sent {
                to: to.0,
                text: text.to_owned(),
                rows: rows.to_vec(),
            });
            sent.len() - 1
        };
        assert!(!self.crash, "the worker stops after the Bot API call");
        self.outcomes[call.min(self.outcomes.len() - 1)]
    }
}

/// The digest ports of a worker that sends into memory and has no model.
pub fn digest_ports(messenger: Arc<FakeMessenger>) -> tada::worker::DigestSend {
    tada::worker::DigestSend {
        messenger,
        ai: None,
    }
}

/// The job handlers of a worker on `database` that sends digests into `messenger`, with the AI `ai`.
pub fn digest_worker(
    database: &tada_store_pg::Database,
    messenger: Arc<FakeMessenger>,
    ai: Option<tada_app::pm::send::DigestAi>,
    clock: Arc<dyn tada_app::clock::Clock>,
) -> tada_app::jobs::Handlers {
    tada::worker::handlers(
        database,
        Arc::new(tada_adapters::mail::MemoryMailer::new()),
        Arc::new(tada_adapters::mail::FluentMailTexts::new().unwrap()),
        clock,
        super::public_url(),
        Arc::new(super::NoObjectStorage),
        tada::worker::DigestSend { messenger, ai },
    )
}

/// Runs the next due job with `handlers`.
pub async fn run_one(
    database: &tada_store_pg::Database,
    handlers: &tada_app::jobs::Handlers,
) -> tada_app::jobs::Ran {
    tada_app::jobs::run_next(
        database,
        handlers,
        uuid::Uuid::now_v7(),
        std::time::Duration::from_secs(60),
    )
    .await
    .unwrap()
}

/// Runs the due jobs with `handlers` until none is due, and returns what each run did.
pub async fn run_all(
    database: &tada_store_pg::Database,
    handlers: &tada_app::jobs::Handlers,
) -> Vec<tada_app::jobs::Ran> {
    let mut ran = Vec::new();
    loop {
        match run_one(database, handlers).await {
            tada_app::jobs::Ran::Idle => return ran,
            other => ran.push(other),
        }
    }
}

/// The run limits of the `pm-digest` task with their defaults.
pub const AI_LIMITS: tada_app::model::AiLimits = tada_app::model::AiLimits {
    max_turns: 6,
    max_cost: tada_app::ai_cost::Chf::from_millirappen(5_000),
    run_timeout: jiff::SignedDuration::from_secs(60),
    call_timeout: jiff::SignedDuration::from_secs(20),
    call_retries: 2,
};

/// The AI of a worker on `database` with the model `model` and a cap of CHF 25 a month.
pub fn digest_ai(
    database: &tada_store_pg::Database,
    model: Arc<tada_adapters::fake_model::FakeModel>,
) -> tada_app::pm::send::DigestAi {
    tada_app::pm::send::DigestAi {
        model,
        cost: Arc::new(database.clone()),
        setup: tada_app::ai_cost::AiSetup {
            configured: true,
            default_cap: tada_app::ai_cost::Chf::from_millirappen(2_500_000),
        },
        limits: AI_LIMITS,
    }
}
