//! The Telegram gateway (ADR 0011). It turns updates of the Bot API into `app` commands and contains
//! no domain rules.

mod messages;

use std::fmt;
use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use frankenstein::AsyncTelegramApi;
use frankenstein::client_reqwest::Bot;
use frankenstein::methods::{GetUpdatesParams, SendMessageParams};
use frankenstein::types::{AllowedUpdate, ChatType, Message};
use frankenstein::updates::{Update, UpdateContent};
use tada_app::caller::{ServiceCaller, TelegramGateway};
use tada_app::clock::Clock;
use tada_app::telegram::{TelegramLinks, TelegramName, TelegramUserId, claim_link_code};

use crate::messages::Messages;

/// The time that Telegram holds a long-polling request open. It is below the stop limit of ADR 0025.
const POLL_TIMEOUT_SECONDS: u32 = 20;
/// The wait after a failed request to the Bot API.
const RETRY_DELAY: Duration = Duration::from_secs(5);

/// The gateway in long-polling mode, for development (ADR 0011).
pub struct Gateway {
    bot: Bot,
    links: Arc<dyn TelegramLinks>,
    clock: Arc<dyn Clock>,
    caller: ServiceCaller<TelegramGateway>,
    messages: Messages,
}

// `Bot` shows its URL in `Debug`, and the URL contains the bot token (ADR 0035).
impl fmt::Debug for Gateway {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Gateway").finish_non_exhaustive()
    }
}

impl Gateway {
    /// `api_url` is the base URL of the Bot API, for example `https://api.telegram.org`.
    pub fn new(
        api_url: &str,
        token: &str,
        links: Arc<dyn TelegramLinks>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            bot: Bot::new_url(format!("{}/bot{token}", api_url.trim_end_matches('/'))),
            links,
            clock,
            caller: ServiceCaller::new(),
            messages: Messages::new(),
        }
    }

    /// Polls for updates until `stop` completes.
    pub async fn run(&self, stop: impl Future<Output = ()>) {
        tokio::pin!(stop);
        let mut offset: Option<i64> = None;
        loop {
            let params = GetUpdatesParams::builder()
                .maybe_offset(offset)
                .timeout(POLL_TIMEOUT_SECONDS)
                .allowed_updates(vec![AllowedUpdate::Message])
                .build();
            let response = tokio::select! {
                () = &mut stop => return,
                response = self.bot.get_updates(&params) => response,
            };
            match response {
                Ok(response) => {
                    for update in response.result {
                        offset = Some(i64::from(update.update_id) + 1);
                        self.handle(update).await;
                    }
                }
                Err(error) => {
                    tracing::warn!(%error, "the Bot API request failed");
                    tokio::select! {
                        () = &mut stop => return,
                        () = tokio::time::sleep(RETRY_DELAY) => {}
                    }
                }
            }
        }
    }

    async fn handle(&self, update: Update) {
        match self.links.record_update(i64::from(update.update_id)).await {
            Ok(true) => {}
            Ok(false) => return,
            Err(error) => {
                tracing::warn!(%error, "cannot record the update");
                return;
            }
        }
        let UpdateContent::Message(message) = update.content else {
            return;
        };
        // A link code belongs into a private chat only. The bot ignores groups for now.
        if message.chat.type_field != ChatType::Private {
            return;
        }
        let reply = self.reply(&message).await;
        let params = SendMessageParams::builder()
            .chat_id(message.chat.id)
            .text(reply)
            .build();
        if let Err(error) = self.bot.send_message(&params).await {
            tracing::warn!(%error, "cannot send the reply");
        }
    }

    async fn reply(&self, message: &Message) -> String {
        let (Some(text), Some(from)) = (&message.text, &message.from) else {
            return self.messages.get("telegram-help");
        };
        // A deep link sends "/start <code>"; a member can also paste the code.
        let code = text.strip_prefix("/start").unwrap_or(text).trim();
        if code.is_empty() {
            return self.messages.get("telegram-help");
        }
        let Ok(account) = i64::try_from(from.id).map(TelegramUserId) else {
            return self.messages.get("telegram-error");
        };
        let name = TelegramName(match &from.last_name {
            Some(last) => format!("{} {last}", from.first_name),
            None => from.first_name.clone(),
        });
        match claim_link_code(
            &self.caller,
            code,
            account,
            &name,
            self.links.as_ref(),
            self.clock.as_ref(),
        )
        .await
        {
            Ok(true) => {
                tracing::info!("a Telegram account claimed a link code");
                self.messages.get("telegram-link-claimed")
            }
            Ok(false) => self.messages.get("telegram-link-invalid"),
            Err(error) => {
                tracing::warn!(%error, "cannot claim the link code");
                self.messages.get("telegram-error")
            }
        }
    }
}
