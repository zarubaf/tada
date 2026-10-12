//! The Telegram gateway (ADR 0011). It turns updates of the Bot API into `app` commands and contains
//! no domain rules. It also implements the `Messenger` port and the digest texts that the worker sends with
//! (ADR 0076).

mod bot_error;
mod callback;
mod command;
mod digest_texts;
mod health;
mod messages;
mod send;
mod webhook;

use std::fmt;
use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use frankenstein::AsyncTelegramApi;
use frankenstein::client_reqwest::Bot;
use frankenstein::methods::{GetUpdatesParams, SendMessageParams, SetWebhookParams};
use frankenstein::types::{AllowedUpdate, ChatType, Message};
use frankenstein::updates::{Update, UpdateContent};
use secrecy::{ExposeSecret, SecretString};
use tada_app::caller::{ServiceCaller, TelegramGateway};
use tada_app::clock::Clock;
use tada_app::documents::DocumentStore;
use tada_app::domain::facts::ValueError;
use tada_app::domain::sources::SourceText;
use tada_app::events::EventStore;
use tada_app::facts::FactStore;
use tada_app::identity::IdentityStore;
use tada_app::parties::PartyStore;
use tada_app::pm::answer::DigestAnswers;
use tada_app::problem::{CommandError, ProblemCode};
use tada_app::proposals::{ProposalStore, ProposeError, ProposeStores};
use tada_app::public_url::PublicUrl;
use tada_app::sign_in::email_hint;
use tada_app::sources::SourceStore;
use tada_app::telegram::{
    FactMessage, TelegramActError, TelegramLinks, TelegramName, TelegramUserId, accept_link_claim,
    claim_link_code, is_linked_account, member_for, propose_fact, unlink_account,
};

use tada_app::work::WorkStore;
use tada_app::workstreams::WorkstreamStore;

use crate::bot_error::{BotFailure, salvage};
use crate::command::{Incomplete, LinkCommand, ProposeCommand, parse_link_command, parse_propose};
use crate::messages::Messages;
use crate::webhook::WEBHOOK_PATH;

pub use crate::digest_texts::FluentDigestTexts;
pub use crate::health::BotApiCheck;
pub use crate::send::TelegramMessenger;
pub use crate::webhook::routes;

/// The ports that the gateway reads and writes. One database adapter implements all of them.
pub trait Ports:
    TelegramLinks
    + IdentityStore
    + EventStore
    + FactStore
    + ProposalStore
    + SourceStore
    + DocumentStore
    + WorkstreamStore
    + PartyStore
    + WorkStore
    + DigestAnswers
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
        + WorkstreamStore
        + PartyStore
        + WorkStore
        + DigestAnswers
{
}

/// The time that Telegram holds a long-polling request open. It is below the stop limit of ADR 0025.
const POLL_TIMEOUT_SECONDS: u32 = 20;
/// The wait after a failed request to the Bot API.
const RETRY_DELAY: Duration = Duration::from_secs(5);
/// The time between two calls in webhook mode, which keep `/readyz` current (ADR 0076).
const HEARTBEAT: Duration = Duration::from_secs(60);
/// The updates that the gateway asks for in both modes. Telegram does not send the others.
const ALLOWED_UPDATES: [AllowedUpdate; 2] = [AllowedUpdate::Message, AllowedUpdate::CallbackQuery];

/// How the updates reach the gateway (ADR 0076). `Debug` hides the secret.
#[derive(Debug, Clone)]
pub enum UpdateSource {
    /// The gateway asks for updates with long polling.
    Polling,
    /// Telegram posts the updates to `routes` below `public_url`, with `secret` in a header.
    Webhook {
        secret: SecretString,
        public_url: PublicUrl,
    },
}

/// The database could not record the update, so the gateway did not handle it.
#[derive(Debug)]
pub(crate) struct NotRecorded;

/// The client of the Bot API at `api_url` for the bot with `token`.
fn bot(api_url: &str, token: &str) -> Bot {
    Bot::new_url(format!("{}/bot{token}", api_url.trim_end_matches('/')))
}

/// The gateway: it receives updates by long polling or by webhook (ADRs 0011 and 0076).
pub struct Gateway {
    bot: Bot,
    bot_api: BotApiCheck,
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
            bot: bot(api_url, token),
            bot_api: BotApiCheck::default(),
            ports,
            clock,
            caller: ServiceCaller::new(),
            messages: Messages::new(),
        }
    }

    /// The check for `/readyz`: it fails if no Bot API call of this gateway succeeded for 5 minutes.
    pub fn bot_api_check(&self) -> BotApiCheck {
        self.bot_api.clone()
    }

    /// Receives updates from `source` until `stop` completes. In webhook mode, `routes` delivers them.
    pub async fn run(&self, stop: impl Future<Output = ()>, source: &UpdateSource) {
        match source {
            UpdateSource::Polling => self.poll(stop).await,
            UpdateSource::Webhook { secret, public_url } => {
                self.keep_webhook(stop, secret, public_url).await;
            }
        }
    }

    /// Sets the webhook, then calls the Bot API from time to time until `stop` completes.
    async fn keep_webhook(
        &self,
        stop: impl Future<Output = ()>,
        secret: &SecretString,
        public_url: &PublicUrl,
    ) {
        tokio::pin!(stop);
        let params = SetWebhookParams::builder()
            .url(format!("{}{WEBHOOK_PATH}", public_url.origin()))
            .secret_token(secret.expose_secret())
            .allowed_updates(ALLOWED_UPDATES.to_vec())
            .build();
        loop {
            let response = tokio::select! {
                () = &mut stop => return,
                response = self.bot.set_webhook(&params) => response,
            };
            match response {
                Ok(_) => {
                    self.bot_api.succeeded();
                    tracing::info!("the webhook is set");
                    break;
                }
                Err(error) => {
                    tracing::warn!(error = %BotFailure(&error), "cannot set the webhook");
                    tokio::select! {
                        () = &mut stop => return,
                        () = tokio::time::sleep(RETRY_DELAY) => {}
                    }
                }
            }
        }
        loop {
            let response = tokio::select! {
                () = &mut stop => return,
                response = self.bot.get_webhook_info() => response,
            };
            match response {
                Ok(_) => self.bot_api.succeeded(),
                Err(error) => {
                    tracing::warn!(error = %BotFailure(&error), "the Bot API request failed");
                }
            }
            tokio::select! {
                () = &mut stop => return,
                () = tokio::time::sleep(HEARTBEAT) => {}
            }
        }
    }

    /// Polls for updates until `stop` completes.
    async fn poll(&self, stop: impl Future<Output = ()>) {
        tokio::pin!(stop);
        let mut offset: Option<i64> = None;
        loop {
            let params = GetUpdatesParams::builder()
                .maybe_offset(offset)
                .timeout(POLL_TIMEOUT_SECONDS)
                .allowed_updates(ALLOWED_UPDATES.to_vec())
                .build();
            let response = tokio::select! {
                () = &mut stop => return,
                response = self.bot.get_updates(&params) => response,
            };
            let updates = match response {
                Ok(response) => {
                    self.bot_api.succeeded();
                    response.result
                }
                Err(error) => {
                    tracing::warn!(error = %BotFailure(&error), "the Bot API request failed");
                    if matches!(&error, frankenstein::Error::Api(response) if response.error_code == 409)
                    {
                        // Polling never deletes a webhook: with a shared token that would take the
                        // updates away from the installation in webhook mode.
                        tracing::warn!(
                            "a webhook is set for this bot; polling works after a `deleteWebhook` call"
                        );
                    }
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
                // A stop drops the current update; the dedupe keeps a later delivery from a second run.
                tokio::select! {
                    () = &mut stop => return,
                    _ = self.handle(update) => {}
                }
            }
        }
    }

    /// Handles one update of either mode once: the database drops a repeated update ID.
    async fn handle(&self, update: Update) -> Result<(), NotRecorded> {
        match self.ports.record_update(i64::from(update.update_id)).await {
            Ok(true) => {}
            Ok(false) => return Ok(()),
            Err(error) => {
                tracing::warn!(%error, "cannot record the update");
                return Err(NotRecorded);
            }
        }
        // The gateway asks for no other kind of update.
        match update.content {
            UpdateContent::Message(message) => {
                // Only the private chat shows that the account no longer blocks the bot.
                if message.chat.type_field == ChatType::Private
                    && let Some(account) = message
                        .from
                        .as_ref()
                        .and_then(|from| i64::try_from(from.id).ok())
                {
                    self.reached(TelegramUserId(account)).await;
                }
                self.answer(&message).await;
            }
            UpdateContent::CallbackQuery(query) => {
                let origin = callback::origin(&query);
                if let callback::Origin::Private(account) = origin {
                    self.reached(account).await;
                }
                self.press(&query, origin).await;
            }
            _ => {}
        }
        Ok(())
    }

    /// The account sent an update from its private chat with the bot, so a send to it can work again (ADR 0076).
    async fn reached(&self, account: TelegramUserId) {
        if let Err(error) = self.ports.reached(account).await {
            tracing::warn!(%error, "cannot clear the blocked mark of the link");
        }
    }

    /// Replies to a message in a private chat.
    async fn answer(&self, message: &Message) {
        // A link code belongs into a private chat only. The bot ignores groups for now.
        if message.chat.type_field != ChatType::Private {
            return;
        }
        let reply = self.reply(message).await;
        let params = SendMessageParams::builder()
            .chat_id(message.chat.id)
            .text(reply)
            .build();
        match self.bot.send_message(&params).await {
            Ok(_) => self.bot_api.succeeded(),
            Err(error) => tracing::warn!(error = %BotFailure(&error), "cannot send the reply"),
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
        if let Some(command) = parse_link_command(source.as_str()) {
            return self.link_command(account, command).await;
        }
        // A linked account has no code to claim. Free text or a used deep link must not look like a
        // wrong code.
        match is_linked_account(&self.caller, account, self.ports.as_ref()).await {
            Ok(true) => return self.messages.get("telegram-linked-help"),
            Ok(false) => {}
            Err(error) => {
                tracing::warn!(%error, "cannot look up the link of the account");
                return self.messages.get("telegram-error");
            }
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
            Ok(Some(target)) => {
                tracing::info!("a Telegram account claimed a link code");
                // The account sees whom it would link to before it accepts (reverse phishing).
                self.messages.get_with_all(
                    "telegram-link-claimed",
                    &[
                        ("account", &target.user_name),
                        ("email", &email_hint(&target.email)),
                        ("organization", &target.organization_name),
                    ],
                )
            }
            Ok(None) => self.messages.get("telegram-link-invalid"),
            Err(error) => {
                tracing::warn!(%error, "cannot claim the link code");
                self.messages.get("telegram-error")
            }
        }
    }

    /// The commands `/bestaetigen` (also `/bestätigen`) and `/trennen` of the link (ADR 0011).
    async fn link_command(&self, account: TelegramUserId, command: LinkCommand) -> String {
        let ports = self.ports.as_ref();
        let result = match command {
            LinkCommand::Accept => {
                accept_link_claim(&self.caller, account, ports, self.clock.as_ref())
                    .await
                    .map(|accepted| {
                        if accepted {
                            tracing::info!("a Telegram account accepted a link claim");
                            "telegram-link-accepted"
                        } else {
                            "telegram-link-nothing-to-accept"
                        }
                    })
            }
            LinkCommand::Unlink => unlink_account(&self.caller, account, ports)
                .await
                .map(|()| {
                    tracing::info!("a Telegram account ended its link");
                    "telegram-unlinked"
                }),
        };
        match result {
            Ok(id) => self.messages.get(id),
            Err(error) => {
                tracing::warn!(%error, "cannot run the link command");
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
                workstreams: ports,
                parties: ports,
                work: ports,
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
