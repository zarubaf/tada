//! The outbound Telegram port (ADRs 0011 and 0076).
//!
//! The worker sends through it, and the `telegram` crate implements it.
//! The `telegram` process role only receives updates.

use async_trait::async_trait;

use crate::telegram::TelegramUserId;

/// One button of an inline keyboard.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Button {
    /// A press sends `data` back to the bot, at most 64 bytes.
    Callback { label: String, data: String },
    /// A press opens `url`.
    Url { label: String, url: String },
}

/// The result of one send. The caller decides about a retry; an `Unknown` send is never retried
/// without a check (ADR 0011).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SendOutcome {
    /// Telegram took the message.
    Sent,
    /// The recipient blocked the bot (HTTP 403).
    Blocked,
    /// Telegram refused the message, or the request did not leave. Nothing was sent.
    Failed,
    /// The request left, but no clear answer came: a timeout or a server error.
    /// Telegram can have delivered the message.
    Unknown,
}

#[async_trait]
pub trait Messenger: Send + Sync {
    /// Sends `text` as plain text, without a parse mode, to the private chat of `to`.
    /// Each entry of `rows` is one row of buttons below the text.
    async fn send(&self, to: TelegramUserId, text: &str, rows: &[Vec<Button>]) -> SendOutcome;
}
