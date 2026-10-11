//! The Telegram gateway in webhook mode on the test database, with a fake Bot API: a test delivers updates as
//! Telegram would, and reads the answers to the callback queries.

use std::sync::{Arc, Mutex};

use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::{StatusCode, header};
use axum::routing::post;
use axum::{Json, Router};
use secrecy::SecretString;
use serde_json::{Value, json};
use tada_app::clock::Clock;
use tada_app::domain::ids::DigestItemId;
use tada_app::domain::reminders::{Answer, CallbackData};
use tada_store_pg::testing::TestDatabase;
use tada_telegram::{Gateway, UpdateSource};
use tower::ServiceExt;

/// The invented webhook secret of the tests.
const SECRET: &str = "test-webhook-secret-answers";

/// A fake Bot API that records the body of each `answerCallbackQuery` call.
#[derive(Clone, Default)]
struct BotApi {
    answers: Arc<Mutex<Vec<Value>>>,
}

async fn bot_method(
    State(api): State<BotApi>,
    Path((_, method)): Path<(String, String)>,
    Json(body): Json<Value>,
) -> Json<Value> {
    if method == "answerCallbackQuery" {
        api.answers.lock().unwrap().push(body);
    }
    Json(json!({"ok": true, "result": true}))
}

/// The webhook route of one `telegram` process with its own fake Bot API.
pub struct Webhook {
    api: BotApi,
    router: Router,
    next_update: Mutex<u32>,
}

impl Webhook {
    /// A gateway on `test` that reads the time from `clock`.
    pub async fn start(test: &TestDatabase, clock: Arc<dyn Clock>) -> Self {
        let api = BotApi::default();
        let server = Router::new()
            .route("/{bot}/{method}", post(bot_method))
            .with_state(api.clone());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, server).await.unwrap() });
        let gateway = Arc::new(Gateway::new(
            &format!("http://{address}"),
            "test-token",
            Arc::new(test.database.clone()),
            clock,
        ));
        let source = UpdateSource::Webhook {
            secret: SecretString::from(SECRET),
            public_url: super::public_url(),
        };
        let router =
            tada::telegram::routes(gateway, &source, vec![Arc::new(test.database.clone())]);
        Self {
            api,
            router,
            next_update: Mutex::new(1),
        }
    }

    /// A press of the button `answer` of `item` by `from` on a message in the chat `chat`. Returns the text of
    /// the answer to the callback query.
    pub async fn press(
        &self,
        from: i64,
        chat: Value,
        answer: Answer,
        item: DigestItemId,
    ) -> String {
        self.press_on(
            from,
            json!({"message_id": 5, "date": 1, "chat": chat, "text": "Digest"}),
            answer,
            item,
        )
        .await
    }

    pub async fn press_on(
        &self,
        from: i64,
        message: Value,
        answer: Answer,
        item: DigestItemId,
    ) -> String {
        let data = CallbackData { answer, item }.encode();
        self.deliver_callback(from, message, &data).await
    }

    pub async fn deliver_callback(&self, from: i64, message: Value, data: &str) -> String {
        let id = {
            let mut next = self.next_update.lock().unwrap();
            *next += 1;
            *next
        };
        let query = format!("query-{id}");
        let update = json!({"update_id": id, "callback_query": {
            "id": query, "chat_instance": "instance-1", "data": data, "message": message,
            "from": {"id": from, "is_bot": false, "first_name": "Testperson"}
        }});
        assert_eq!(self.deliver(&update).await, StatusCode::OK);
        let answers = self.api.answers.lock().unwrap().clone();
        let answer = answers
            .iter()
            .find(|answer| answer["callback_query_id"] == query.as_str())
            .unwrap_or_else(|| panic!("no answer to the callback query: {answers:?}"));
        answer["text"].as_str().unwrap_or_default().to_owned()
    }

    pub async fn deliver(&self, update: &Value) -> StatusCode {
        let request = axum::http::Request::post("/telegram/webhook")
            .header(header::CONTENT_TYPE, "application/json")
            .header("X-Telegram-Bot-Api-Secret-Token", SECRET)
            .body(Body::from(update.to_string()))
            .unwrap();
        self.router.clone().oneshot(request).await.unwrap().status()
    }
}

/// The private chat of `account` with the bot.
pub fn private(account: i64) -> Value {
    json!({"id": account, "type": "private", "first_name": "Testperson"})
}
