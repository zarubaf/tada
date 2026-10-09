//! The Telegram gateway (ADR 0011). It turns updates of the Bot API into `app` commands and contains
//! no domain rules.

mod bot_error;
mod command;
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
use tada_app::documents::DocumentStore;
use tada_app::domain::facts::ValueError;
use tada_app::domain::sources::SourceText;
use tada_app::events::EventStore;
use tada_app::facts::FactStore;
use tada_app::identity::IdentityStore;
use tada_app::problem::{CommandError, ProblemCode};
use tada_app::proposals::{ProposalStore, ProposeError, ProposeStores};
use tada_app::sources::SourceStore;
use tada_app::telegram::{
    FactMessage, TelegramActError, TelegramLinks, TelegramName, TelegramUserId, claim_link_code,
    member_for, propose_fact,
};

use crate::bot_error::{BotFailure, salvage};
use crate::command::{Incomplete, ProposeCommand, parse_propose};
use crate::messages::Messages;

/// The ports that the gateway reads and writes. One database adapter implements all of them.
pub trait Ports:
    TelegramLinks + IdentityStore + EventStore + FactStore + ProposalStore + SourceStore + DocumentStore
{
}

impl<T> Ports for T where
    T: TelegramLinks
        + IdentityStore
        + EventStore
        + FactStore
        + ProposalStore
        + SourceStore
        + DocumentStore
{
}

/// The time that Telegram holds a long-polling request open. It is below the stop limit of ADR 0025.
const POLL_TIMEOUT_SECONDS: u32 = 20;
/// The wait after a failed request to the Bot API.
const RETRY_DELAY: Duration = Duration::from_secs(5);

/// The gateway in long-polling mode, for development (ADR 0011).
pub struct Gateway {
    bot: Bot,
    ports: Arc<dyn Ports>,
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
    pub fn new(api_url: &str, token: &str, ports: Arc<dyn Ports>, clock: Arc<dyn Clock>) -> Self {
        Self {
            bot: Bot::new_url(format!("{}/bot{token}", api_url.trim_end_matches('/'))),
            ports,
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
            let updates = match response {
                Ok(response) => response.result,
                Err(error) => {
                    tracing::warn!(error = %BotFailure(&error), "the Bot API request failed");
                    match salvage(&error) {
                        // An answer that does not move the offset forward must not loop without a pause.
                        Some(batch) if offset.is_none_or(|current| batch.next_offset > current) => {
                            if batch.skipped > 0 {
                                tracing::warn!(
                                    skipped = batch.skipped,
                                    "the gateway skips updates that do not decode"
                                );
                            }
                            offset = Some(batch.next_offset);
                            batch.updates
                        }
                        _ => {
                            tokio::select! {
                                () = &mut stop => return,
                                () = tokio::time::sleep(RETRY_DELAY) => {}
                            }
                            continue;
                        }
                    }
                }
            };
            for update in updates {
                offset = offset.max(Some(i64::from(update.update_id) + 1));
                self.handle(update).await;
            }
        }
    }

    async fn handle(&self, update: Update) {
        match self.ports.record_update(i64::from(update.update_id)).await {
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
            tracing::warn!(error = %BotFailure(&error), "cannot send the reply");
        }
    }

    async fn reply(&self, message: &Message) -> String {
        let (Some(text), Some(from)) = (&message.text, &message.from) else {
            return self.messages.get("telegram-help");
        };
        let Ok(account) = i64::try_from(from.id).map(TelegramUserId) else {
            return self.messages.get("telegram-error");
        };
        let source = SourceText::normalize(text);
        if let Some(command) = parse_propose(source.as_str()) {
            return match command {
                Ok(command) => self.propose(account, &source, command).await,
                Err(Incomplete) => self.messages.get("telegram-proposal-usage"),
            };
        }
        // A deep link sends "/start <code>"; a member can also paste the code.
        let code = text.strip_prefix("/start").unwrap_or(text).trim();
        if code.is_empty() {
            return self.messages.get("telegram-help");
        }
        let name = TelegramName(match &from.last_name {
            Some(last) => format!("{} {last}", from.first_name),
            None => from.first_name.clone(),
        });
        match claim_link_code(
            &self.caller,
            code,
            account,
            &name,
            self.ports.as_ref(),
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

    /// The command `/vorschlag`: the linked member proposes a value for a field of an event.
    async fn propose(
        &self,
        account: TelegramUserId,
        source: &SourceText,
        command: ProposeCommand<'_>,
    ) -> String {
        let ports = self.ports.as_ref();
        let result = async {
            let acting = member_for(
                &self.caller,
                account,
                command.event_key,
                ports,
                ports,
                ports,
            )
            .await?;
            let stores = ProposeStores {
                identity: ports,
                facts: ports,
                proposals: ports,
                sources: ports,
                documents: ports,
            };
            let message = FactMessage {
                source,
                field_key: command.field_key,
                value: command.value,
            };
            propose_fact(
                &acting.caller,
                &acting.event,
                message,
                &self.messages.get("telegram-proposal-reason"),
                stores,
                self.clock.as_ref(),
            )
            .await?;
            Ok::<_, TelegramActError>(acting.event)
        }
        .await;
        match result {
            Ok(event) => {
                tracing::info!("a linked member proposed a fact by Telegram");
                self.messages
                    .get_with("telegram-proposal-created", "event", event.name.as_str())
            }
            Err(error) => self.refusal(&error),
        }
    }

    /// The German reply to a refused command. The log has the problem code, never the message.
    fn refusal(&self, error: &TelegramActError) -> String {
        let code = error.code();
        if let Some(store_error) = error.store_error() {
            tracing::warn!(%store_error, "cannot run the Telegram command");
        } else {
            tracing::info!(code = code.as_str(), "the Telegram command was refused");
        }
        match error {
            TelegramActError::NotLinked => self.messages.get("telegram-not-linked"),
            TelegramActError::Ambiguous => self.messages.get("telegram-ambiguous"),
            TelegramActError::UnknownField => self.messages.get("telegram-unknown-field"),
            TelegramActError::Value(ValueError::NoTextForm) => {
                self.messages.get("telegram-value-web-only")
            }
            TelegramActError::Value(_) => self.messages.get("telegram-value-invalid"),
            TelegramActError::Propose(ProposeError::Invalid(_)) => {
                self.messages.problem(ProblemCode::ValidationFailed)
            }
            _ => self.messages.problem(code),
        }
    }
}
