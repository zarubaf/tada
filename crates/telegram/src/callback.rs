//! The press of an answer button of a digest (ADR 0076): a `callback_query` update.

use frankenstein::AsyncTelegramApi;
use frankenstein::methods::AnswerCallbackQueryParams;
use frankenstein::types::{CallbackQuery, ChatType, MaybeInaccessibleMessage};
use tada_app::domain::reminders::CallbackData;
use tada_app::pm::answer::{AnswerOutcome, AnswerPorts, answer};
use tada_app::telegram::TelegramUserId;
use tada_app::work::WorkPorts;

use crate::Gateway;
use crate::bot_error::BotFailure;

impl Gateway {
    /// Handles a press and always answers the query, so that the client stops its wait.
    pub(crate) async fn press(&self, query: &CallbackQuery, origin: Origin) {
        let text = match origin {
            // The bot serves private chats only; the answer has no text.
            Origin::Group => None,
            Origin::Foreign => Some(self.messages.get("telegram-answer-outdated")),
            Origin::Private(account) => {
                Some(self.messages.get(self.answer_press(account, query).await))
            }
        };
        let params = AnswerCallbackQueryParams::builder()
            .callback_query_id(query.id.clone())
            .maybe_text(text)
            .build();
        match self.bot.answer_callback_query(&params).await {
            Ok(_) => self.bot_api.succeeded(),
            Err(error) => {
                tracing::warn!(error = %BotFailure(&error), "cannot answer the button press");
            }
        }
    }

    /// The message ID of the answer to a press in the private chat of `account`.
    async fn answer_press(&self, account: TelegramUserId, query: &CallbackQuery) -> &'static str {
        let Some(data) = query.data.as_deref().and_then(CallbackData::decode) else {
            return "telegram-answer-outdated";
        };
        let ports = self.ports.as_ref();
        let ports = AnswerPorts {
            links: ports,
            answers: ports,
            work: WorkPorts {
                identity: ports,
                work: ports,
                milestones: ports,
                decisions: ports,
                workstreams: ports,
                parties: ports,
                clock: self.clock.as_ref(),
            },
        };
        match answer(&self.caller, account, data, self.clock.now(), ports).await {
            Ok(outcome) => {
                tracing::info!(outcome = ?outcome, "a member pressed a digest button");
                match outcome {
                    AnswerOutcome::Recorded => "telegram-answer-recorded",
                    AnswerOutcome::Changed => "telegram-answer-changed",
                    AnswerOutcome::Outdated => "telegram-answer-outdated",
                }
            }
            Err(error) => {
                tracing::warn!(%error, "cannot handle the button press");
                "telegram-error"
            }
        }
    }
}

/// Where a press comes from.
pub(crate) enum Origin {
    /// A chat that is not private, or an inline message. The bot ignores it.
    Group,
    /// A private chat, but a message that tada did not send to the presser: a forwarded message or the chat of
    /// another account.
    Foreign,
    /// The digest in the private chat of the account.
    Private(TelegramUserId),
}

pub(crate) fn origin(query: &CallbackQuery) -> Origin {
    let Some(message) = query.message.as_ref() else {
        return Origin::Group;
    };
    let (chat, forwarded) = match message {
        MaybeInaccessibleMessage::Message(message) => {
            (&*message.chat, message.forward_origin.is_some())
        }
        MaybeInaccessibleMessage::InaccessibleMessage(message) => (&message.chat, false),
    };
    if chat.type_field != ChatType::Private {
        return Origin::Group;
    }
    match i64::try_from(query.from.id) {
        Ok(account) if chat.id == account && !forwarded => Origin::Private(TelegramUserId(account)),
        _ => Origin::Foreign,
    }
}
