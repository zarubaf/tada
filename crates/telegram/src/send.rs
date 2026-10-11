//! The `Messenger` port over the Bot API (ADRs 0011 and 0076).

use std::fmt;
use std::time::Duration;

use async_trait::async_trait;
use frankenstein::AsyncTelegramApi;
use frankenstein::Error;
use frankenstein::client_reqwest::Bot;
use frankenstein::methods::SendMessageParams;
use frankenstein::types::{InlineKeyboardButton, InlineKeyboardMarkup, ReplyMarkup};
use tada_app::messenger::{Button, Messenger, SendOutcome};
use tada_app::telegram::TelegramUserId;

use crate::bot;
use crate::bot_error::BotFailure;

/// The time after which a send counts as `Unknown`.
const SEND_TIMEOUT: Duration = Duration::from_secs(15);

/// Sends plain-text messages with inline keyboards to private chats.
pub struct TelegramMessenger {
    bot: Bot,
    timeout: Duration,
}

// `Bot` shows its URL in `Debug`, and the URL contains the bot token (ADR 0035).
impl fmt::Debug for TelegramMessenger {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TelegramMessenger")
            .field("timeout", &self.timeout)
            .finish_non_exhaustive()
    }
}

impl TelegramMessenger {
    /// `api_url` is the base URL of the Bot API, for example `https://api.telegram.org`.
    pub fn new(api_url: &str, token: &str) -> Self {
        Self {
            bot: bot(api_url, token),
            timeout: SEND_TIMEOUT,
        }
    }

    /// Replaces the time after which a send counts as `Unknown`.
    #[must_use]
    pub fn with_timeout(self, timeout: Duration) -> Self {
        Self { timeout, ..self }
    }
}

#[async_trait]
impl Messenger for TelegramMessenger {
    async fn send(&self, to: TelegramUserId, text: &str, rows: &[Vec<Button>]) -> SendOutcome {
        // No parse mode: a title in the text cannot become a link or markup (ADR 0076).
        let params = SendMessageParams::builder()
            .chat_id(to.0)
            .text(text)
            .maybe_reply_markup(keyboard(rows))
            .build();
        match tokio::time::timeout(self.timeout, self.bot.send_message(&params)).await {
            Ok(Ok(_)) => SendOutcome::Sent,
            Ok(Err(error)) => {
                tracing::warn!(error = %BotFailure(&error), "cannot send the message");
                outcome_of(&error)
            }
            Err(_) => {
                tracing::warn!("the send timed out");
                SendOutcome::Unknown
            }
        }
    }
}

/// The inline keyboard of `rows`, or none if there is no button.
fn keyboard(rows: &[Vec<Button>]) -> Option<ReplyMarkup> {
    if rows.iter().all(Vec::is_empty) {
        return None;
    }
    let inline_keyboard = rows
        .iter()
        .map(|row| row.iter().map(button).collect())
        .collect();
    Some(ReplyMarkup::InlineKeyboardMarkup(
        InlineKeyboardMarkup::builder()
            .inline_keyboard(inline_keyboard)
            .build(),
    ))
}

fn button(button: &Button) -> InlineKeyboardButton {
    match button {
        Button::Callback { label, data } => InlineKeyboardButton::builder()
            .text(label)
            .callback_data(data)
            .build(),
        Button::Url { label, url } => InlineKeyboardButton::builder().text(label).url(url).build(),
    }
}

/// The outcome of a failed send. Only an error before the request left is a sure `Failed`.
fn outcome_of(error: &Error) -> SendOutcome {
    match error {
        Error::Api(response) => match response.error_code {
            403 => SendOutcome::Blocked,
            400..=499 => SendOutcome::Failed,
            _ => SendOutcome::Unknown,
        },
        Error::JsonEncode { .. } => SendOutcome::Failed,
        Error::HttpReqwest(error) if error.is_connect() || error.is_builder() => {
            SendOutcome::Failed
        }
        // A timeout, a broken connection or an answer that does not decode, for example the HTML
        // page of a proxy after a server error.
        _ => SendOutcome::Unknown,
    }
}
