//! The Telegram linking spike end to end (ADR 0011): a fake Bot API, the gateway and PostgreSQL.

// The helpers of this test file are not `#[test]` functions, so clippy.toml does not cover them.
#![allow(clippy::unwrap_used)]

mod support;

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::body::Body;
use axum::extract::State;
use axum::http::{Method, StatusCode, header};
use axum::routing::post;
use axum::{Json, Router};
use serde_json::{Value, json};
use tada_adapters::clock::SystemClock;
use tada_app::auth::{Authenticator, Credential};
use tada_app::caller::{MemberCaller, OrganizationRole};
use tada_app::caller::{ServiceCaller, TelegramGateway};
use tada_app::session::SessionAuthenticator;
use tada_app::telegram::{
    TelegramName, TelegramUserId, claim_link_code, create_link_code, list_link_requests,
};
use tada_store_pg::testing::TestDatabase;
use tada_telegram::Gateway;
use tokio::sync::Notify;
use tower::ServiceExt;

#[derive(Clone, Default)]
struct FakeBotApi {
    /// Each `getUpdates` call returns the next batch; then an empty batch.
    batches: Arc<Mutex<VecDeque<Vec<Value>>>>,
    replies: Arc<Mutex<Vec<String>>>,
    replied: Arc<Notify>,
}

async fn get_updates(State(api): State<FakeBotApi>) -> Json<Value> {
    let batch = api.batches.lock().unwrap().pop_front();
    if batch.is_none() {
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    Json(json!({"ok": true, "result": batch.unwrap_or_default()}))
}

async fn send_message(State(api): State<FakeBotApi>, Json(body): Json<Value>) -> Json<Value> {
    api.replies
        .lock()
        .unwrap()
        .push(body["text"].as_str().unwrap().to_owned());
    api.replied.notify_one();
    Json(json!({"ok": true, "result": {
        "message_id": 99, "date": 0, "chat": {"id": body["chat_id"], "type": "private"}, "text": body["text"]
    }}))
}

/// A private message from the invented account 7130429.
fn update(update_id: u32, text: &str) -> Value {
    json!({"update_id": update_id, "message": {
        "message_id": update_id, "date": 0, "text": text,
        "chat": {"id": 7130429, "type": "private", "first_name": "Testperson"},
        "from": {"id": 7130429, "is_bot": false, "first_name": "Testperson", "last_name": "Muster"}
    }})
}

#[tokio::test]
async fn a_code_sent_to_the_bot_becomes_a_request_that_the_member_sees() {
    support::logs::install();
    let test = TestDatabase::start().await;
    let (_, _, cookie) = test.member("testwil", OrganizationRole::Owner).await;
    let member = authenticate(&test, &cookie).await;
    let code = create_link_code(&member, &test.database, &SystemClock)
        .await
        .unwrap();

    let api = FakeBotApi::default();
    api.batches.lock().unwrap().extend([
        vec![
            update(1, &format!("/start {}", code.code)),
            update(2, "not-a-code"),
        ],
        // Telegram can deliver an update again, for example after a restart.
        vec![update(1, &format!("/start {}", code.code))],
    ]);
    let server = Router::new()
        .route("/{bot}/getUpdates", post(get_updates))
        .route("/{bot}/sendMessage", post(send_message))
        .with_state(api.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, server).await.unwrap() });

    let gateway = Gateway::new(
        &format!("http://{address}"),
        "test-token",
        Arc::new(test.database.clone()),
        Arc::new(SystemClock),
    );
    let replied = api.replied.clone();
    let replies = api.replies.clone();
    let stop = async move {
        while replies.lock().unwrap().len() < 2 {
            replied.notified().await;
        }
        // Time for a third reply, which must not come.
        tokio::time::sleep(Duration::from_millis(300)).await;
    };
    tokio::time::timeout(Duration::from_secs(10), gateway.run(stop))
        .await
        .unwrap();

    let replies = api.replies.lock().unwrap().clone();
    assert_eq!(
        replies.len(),
        2,
        "the repeated update got a reply: {replies:?}"
    );
    assert!(replies[0].starts_with("Danke. Bestätigen Sie die Verknüpfung"));
    assert!(replies[1].starts_with("Dieser Code ist ungültig"));

    let requests = list_link_requests(&member, &test.database, &SystemClock)
        .await
        .unwrap();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].telegram_user_id.0, 7130429);
    assert_eq!(requests[0].telegram_name.0, "Testperson Muster");

    support::logs::assert_clean(&[
        &code.code,
        &cookie,
        "Testperson",
        "Muster",
        "test-token",
        "7130429",
    ]);
}

/// The member of a session cookie, as the session authenticator finds it.
async fn authenticate(test: &TestDatabase, cookie: &str) -> MemberCaller {
    let database = Arc::new(test.database.clone());
    SessionAuthenticator::new(database.clone(), database, Arc::new(SystemClock))
        .authenticate(Some(Credential::Session(cookie)))
        .await
        .unwrap()
}

async fn call(
    router: &Router,
    cookie: &str,
    method: Method,
    path: &str,
) -> (StatusCode, Option<String>, Value) {
    let request = support::request(method, path)
        .header(
            header::COOKIE,
            format!("{}={cookie}", support::SESSION_COOKIE),
        )
        .body(Body::empty())
        .unwrap();
    let response = router.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let cache = response
        .headers()
        .get(header::CACHE_CONTROL)
        .map(|value| value.to_str().unwrap().to_owned());
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    (
        status,
        cache,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

#[tokio::test]
async fn the_member_confirms_the_link_in_the_web_client() {
    support::logs::install();
    let test = TestDatabase::start().await;
    let (_, _, cookie) = test.member("testwil", OrganizationRole::Owner).await;
    let router = support::session_router(&test, Arc::new(SystemClock));

    let (status, cache, code) = call(
        &router,
        &cookie,
        Method::POST,
        "/api/v1/telegram/link-codes",
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(
        cache.as_deref(),
        Some("no-store"),
        "the code must not stay in a cache"
    );
    let code = code["code"].as_str().unwrap().to_owned();

    let gateway = ServiceCaller::<TelegramGateway>::new();
    let name = TelegramName("Testperson".to_owned());
    assert!(
        claim_link_code(
            &gateway,
            &code,
            TelegramUserId(7130429),
            &name,
            &test.database,
            &SystemClock
        )
        .await
        .unwrap()
    );

    let (_, _, page) = call(
        &router,
        &cookie,
        Method::GET,
        "/api/v1/telegram/link-requests",
    )
    .await;
    let request = &page["items"][0];
    assert_eq!(request["telegram_name"], "Testperson");
    let path = format!(
        "/api/v1/telegram/link-requests/{}/confirm",
        request["id"].as_str().unwrap()
    );

    let (status, _, link) = call(&router, &cookie, Method::POST, &path).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(link["telegram_user_id"], 7130429);
    let (status, _, problem) = call(&router, &cookie, Method::POST, &path).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(problem["code"], "not-found");

    support::logs::assert_clean(&[&code, &cookie, "Testperson", "7130429"]);
    support::logs::assert_route_logged("/api/v1/telegram/link-requests/{request_id}/confirm");
}
