//! The Telegram linking spike end to end (ADR 0011): a fake Bot API, the gateway and PostgreSQL.

// The helpers of this test file are not `#[test]` functions, so clippy.toml does not cover them.
#![allow(clippy::unwrap_used)]

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::body::Body;
use axum::extract::State;
use axum::http::{Request, StatusCode, header};
use axum::routing::post;
use axum::{Json, Router};
use serde_json::{Value, json};
use tada_adapters::clock::SystemClock;
use tada_api::ApiState;
use tada_app::caller::{MemberCaller, OrganizationRole};
use tada_app::caller::{ServiceCaller, TelegramGateway};
use tada_app::telegram::{
    TelegramName, TelegramUserId, claim_link_code, create_link_code, list_link_requests,
};
use tada_store_pg::dev::DevAuthenticator;
use tada_store_pg::dev::{DEV_ORGANIZATION_ID, DEV_USER_ID};
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

/// A private message from the invented account 4242.
fn update(update_id: u32, text: &str) -> Value {
    json!({"update_id": update_id, "message": {
        "message_id": update_id, "date": 0, "text": text,
        "chat": {"id": 4242, "type": "private", "first_name": "Testperson"},
        "from": {"id": 4242, "is_bot": false, "first_name": "Testperson", "last_name": "Muster"}
    }})
}

#[tokio::test]
async fn a_code_sent_to_the_bot_becomes_a_request_that_the_member_sees() {
    let test = TestDatabase::start().await;
    test.database.ensure_dev_organization().await.unwrap();
    let member = MemberCaller::new(DEV_USER_ID, DEV_ORGANIZATION_ID, OrganizationRole::Owner);
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
    assert_eq!(requests[0].telegram_user_id.0, 4242);
    assert_eq!(requests[0].telegram_name.0, "Testperson Muster");
}

async fn call(router: &Router, method: &str, path: &str) -> (StatusCode, Option<String>, Value) {
    let request = Request::builder()
        .method(method)
        .uri(path)
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
    let test = TestDatabase::start().await;
    test.database.ensure_dev_organization().await.unwrap();
    let router = tada_api::router(
        ApiState {
            dependencies: Vec::new(),
            authenticator: Arc::new(DevAuthenticator),
            events: Arc::new(test.database.clone()),
            telegram: Arc::new(test.database.clone()),
            clock: Arc::new(SystemClock),
            trusted_proxies: Vec::new(),
            identity: Arc::new(test.database.clone()),
            event_members: Arc::new(test.database.clone()),
        },
        None,
    );

    let (status, cache, code) = call(&router, "POST", "/api/v1/telegram/link-codes").await;
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
            TelegramUserId(4242),
            &name,
            &test.database,
            &SystemClock
        )
        .await
        .unwrap()
    );

    let (_, _, page) = call(&router, "GET", "/api/v1/telegram/link-requests").await;
    let request = &page["items"][0];
    assert_eq!(request["telegram_name"], "Testperson");
    let path = format!(
        "/api/v1/telegram/link-requests/{}/confirm",
        request["id"].as_str().unwrap()
    );

    let (status, _, link) = call(&router, "POST", &path).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(link["telegram_user_id"], 4242);
    let (status, _, problem) = call(&router, "POST", &path).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(problem["code"], "not-found");
}
