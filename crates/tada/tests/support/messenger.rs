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
