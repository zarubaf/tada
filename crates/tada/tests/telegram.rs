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
use tada_app::auth::{Authenticated, Authenticator, Credential};
use tada_app::caller::{MemberCaller, OrganizationRole};
use tada_app::caller::{ServiceCaller, TelegramGateway};
use tada_app::domain::ids::UserId;
use tada_app::session::SessionAuthenticator;
use tada_app::telegram::{
    TelegramName, TelegramUserId, claim_link_code, confirm_link, create_link_code,
    list_link_requests,
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
    /// The `offset` of each `getUpdates` call.
    offsets: Arc<Mutex<Vec<Value>>>,
}

async fn get_updates(State(api): State<FakeBotApi>, Json(body): Json<Value>) -> Json<Value> {
    api.offsets.lock().unwrap().push(body["offset"].clone());
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

/// The invented Telegram account of the tests.
const ACCOUNT: i64 = 7130429;

/// A private message from the invented account 7130429.
fn update(update_id: u32, text: &str) -> Value {
    update_from(ACCOUNT, update_id, text)
}

/// A private message from the account `account`.
fn update_from(account: i64, update_id: u32, text: &str) -> Value {
    json!({"update_id": update_id, "message": {
        "message_id": update_id, "date": 0, "text": text,
        "chat": {"id": account, "type": "private", "first_name": "Testperson"},
        "from": {"id": account, "is_bot": false, "first_name": "Testperson", "last_name": "Muster"}
    }})
}

/// Runs the gateway against a fake Bot API until it sent `replies` replies. Returns the replies.
async fn converse(test: &TestDatabase, updates: Vec<Value>, replies: usize) -> Vec<String> {
    let api = FakeBotApi::default();
    api.batches.lock().unwrap().push_back(updates);
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
    let (replied, seen) = (api.replied.clone(), api.replies.clone());
    let stop = async move {
        while seen.lock().unwrap().len() < replies {
            replied.notified().await;
        }
        // Time for a reply too many, which must not come.
        tokio::time::sleep(Duration::from_millis(300)).await;
    };
    tokio::time::timeout(Duration::from_secs(20), gateway.run(stop))
        .await
        .unwrap();
    api.replies.lock().unwrap().clone()
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
    let authenticated =
        SessionAuthenticator::new(database.clone(), database, Arc::new(SystemClock))
            .authenticate(Some(Credential::Session(cookie)))
            .await
            .unwrap();
    let Authenticated::Member(member) = authenticated else {
        panic!("a session gives a member");
    };
    member
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

/// A club with the event TEST30, its manager and the web API.
struct Club {
    test: TestDatabase,
    router: Router,
    manager: String,
    event: String,
}

impl Club {
    async fn start() -> Self {
        support::logs::install();
        let test = TestDatabase::start().await;
        let router = support::session_router(&test, Arc::new(SystemClock));
        let (_, _, manager) = test.member("testwil", OrganizationRole::Owner).await;
        let (status, event) = call_json(
            &router,
            &manager,
            Method::POST,
            "/api/v1/events",
            Some(&json!({"key": "TEST30", "name": "Open Day Testwil"})),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        let event = event["id"].as_str().unwrap().to_owned();
        Self {
            test,
            router,
            manager,
            event,
        }
    }

    async fn get(&self, cookie: &str, path: &str) -> Value {
        call_json(&self.router, cookie, Method::GET, path, None)
            .await
            .1
    }

    async fn post(&self, cookie: &str, path: &str, body: &Value) -> (StatusCode, Value) {
        call_json(&self.router, cookie, Method::POST, path, Some(body)).await
    }

    /// A member of the club with the event role `role` (or none), linked to the Telegram account `account`.
    /// Returns the user ID.
    async fn linked_member(&self, role: Option<&str>, account: i64) -> UserId {
        let (_, user, cookie) = self.test.member("testwil", OrganizationRole::Member).await;
        if let Some(role) = role {
            let (status, _) = self
                .post(
                    &self.manager,
                    &format!("/api/v1/events/{}/memberships", self.event),
                    &json!({"user_id": user.as_uuid(), "event_role": role}),
                )
                .await;
            assert_eq!(status, StatusCode::CREATED);
        }
        let member = authenticate(&self.test, &cookie).await;
        let code = create_link_code(&member, &self.test.database, &SystemClock)
            .await
            .unwrap();
        let name = TelegramName("Testperson".to_owned());
        assert!(
            claim_link_code(
                &ServiceCaller::<TelegramGateway>::new(),
                &code.code,
                TelegramUserId(account),
                &name,
                &self.test.database,
                &SystemClock,
            )
            .await
            .unwrap()
        );
        let request = list_link_requests(&member, &self.test.database, &SystemClock)
            .await
            .unwrap()
            .remove(0);
        confirm_link(&member, request.id, &self.test.database, &SystemClock)
            .await
            .unwrap();
        user
    }

    /// The open changesets of the event.
    async fn open_changesets(&self) -> Vec<Value> {
        let page = self
            .get(
                &self.manager,
                &format!("/api/v1/events/{}/changesets?status=open", self.event),
            )
            .await;
        page["items"].as_array().unwrap().clone()
    }
}

async fn call_json(
    router: &Router,
    cookie: &str,
    method: Method,
    path: &str,
    body: Option<&Value>,
) -> (StatusCode, Value) {
    let request = support::request(method, path).header(
        header::COOKIE,
        format!("{}={cookie}", support::SESSION_COOKIE),
    );
    let request = match body {
        Some(body) => request
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(body.to_string())),
        None => request.body(Body::empty()),
    };
    let (response, value) = support::send(router, request.unwrap()).await;
    (response.status(), value)
}

#[tokio::test]
async fn a_reviewed_telegram_change_shows_in_web_queries() {
    let club = Club::start().await;
    let user = club.linked_member(Some("event-contributor"), ACCOUNT).await;

    let replies = converse(
        &club.test,
        vec![update(1, "/vorschlag TEST30 date_window 2030-06..2030-07")],
        1,
    )
    .await;
    assert_eq!(replies.len(), 1, "{replies:?}");
    assert!(replies[0].contains("Open Day Testwil"), "{}", replies[0]);
    assert!(replies[0].contains("Anlassleitung"), "{}", replies[0]);

    let changesets = club.open_changesets().await;
    assert_eq!(changesets.len(), 1);
    let id = changesets[0]["id"].as_str().unwrap();
    let review = club
        .get(&club.manager, &format!("/api/v1/changesets/{id}"))
        .await;
    assert_eq!(review["author"]["channel"], "telegram");
    assert_eq!(review["author"]["kind"], "member");
    assert_eq!(review["author"]["id"], user.as_uuid().to_string());
    let proposal = &review["proposals"][0];
    assert_eq!(proposal["operation"]["kind"], "set-fact");
    assert_eq!(
        proposal["evidence"][0]["excerpt"]["quote"],
        "2030-06..2030-07"
    );

    let profile_path = format!("/api/v1/events/{}/profile", club.event);
    let profile = club.get(&club.manager, &profile_path).await;
    assert_eq!(
        profile["facts"],
        json!([]),
        "a proposal is not accepted state"
    );

    let (status, applied) = club
        .post(
            &club.manager,
            &format!("/api/v1/changesets/{id}/apply"),
            &json!({"selected": [proposal["id"]]}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{applied}");
    let profile = club.get(&club.manager, &profile_path).await;
    assert_eq!(
        profile["facts"][0]["value"],
        json!({"type": "date-window", "start": "2030-06-01", "end": "2030-07-31", "granularity": "month"})
    );

    // A second proposal for the field that has a value now must expect version 1.
    // With a stale version the apply would conflict.
    let replies = converse(
        &club.test,
        vec![update(2, "/vorschlag TEST30 date_window 2030-08..2030-09")],
        1,
    )
    .await;
    assert!(replies[0].contains("Anlassleitung"), "{replies:?}");
    let changesets = club.open_changesets().await;
    assert_eq!(changesets.len(), 1);
    let id = changesets[0]["id"].as_str().unwrap();
    let review = club
        .get(&club.manager, &format!("/api/v1/changesets/{id}"))
        .await;
    assert_eq!(review["proposals"][0]["stale"], false);
    assert_eq!(review["proposals"][0]["operation"]["expected_version"], 1);
    let (status, applied) = club
        .post(
            &club.manager,
            &format!("/api/v1/changesets/{id}/apply"),
            &json!({"selected": [review["proposals"][0]["id"]]}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{applied}");
    let profile = club.get(&club.manager, &profile_path).await;
    assert_eq!(profile["facts"][0]["version"], 2);
    assert_eq!(
        profile["facts"][0]["value"],
        json!({"type": "date-window", "start": "2030-08-01", "end": "2030-09-30", "granularity": "month"})
    );

    support::logs::assert_clean(&[
        "7130429",
        "Testperson",
        "test-token",
        "/vorschlag",
        "2030-06..2030-07",
        "2030-08..2030-09",
    ]);
}

#[tokio::test]
async fn an_unlinked_account_a_viewer_and_a_removed_member_get_a_refusal_and_no_changeset() {
    let club = Club::start().await;
    club.linked_member(Some("event-viewer"), 7130430).await;
    let removed = club.linked_member(Some("event-contributor"), 7130431).await;
    let (status, _) = club
        .post(
            &club.manager,
            &format!(
                "/api/v1/events/{}/memberships/{}/remove",
                club.event,
                removed.as_uuid()
            ),
            &json!({"expected_version": 1}),
        )
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let command = "/vorschlag TEST30 date_window 2030-06..2030-07";

    let replies = converse(
        &club.test,
        vec![
            update_from(ACCOUNT, 1, command),
            update_from(7130430, 2, command),
            update_from(7130431, 3, command),
            // A viewer gets the refusal before any hint about the value.
            update_from(7130430, 4, "/vorschlag TEST30 date_window morgen"),
        ],
        4,
    )
    .await;
    assert_eq!(replies.len(), 4, "{replies:?}");
    assert!(replies[3].contains("keine Berechtigung"), "{}", replies[3]);
    assert!(replies[0].contains("nicht verknüpft"), "{}", replies[0]);
    assert!(replies[1].contains("keine Berechtigung"), "{}", replies[1]);
    assert!(replies[2].contains("nicht sehen"), "{}", replies[2]);
    assert!(club.open_changesets().await.is_empty());
    support::logs::assert_clean(&["7130430", "7130431", "2030-06..2030-07", "morgen"]);
}

#[tokio::test]
async fn a_bad_command_gets_a_german_hint_and_no_changeset() {
    let club = Club::start().await;
    club.linked_member(Some("event-contributor"), ACCOUNT).await;
    let replies = converse(
        &club.test,
        vec![
            update(1, "/vorschlag"),
            update(2, "/vorschlag TEST30 no_such_field 1"),
            update(3, "/vorschlag TEST30 date_window morgen"),
            update(4, "/vorschlag OTHER1 date_window 2030-06..2030-07"),
        ],
        4,
    )
    .await;
    assert_eq!(replies.len(), 4, "{replies:?}");
    assert!(replies[0].starts_with("So schlagen Sie"), "{}", replies[0]);
    assert!(replies[1].contains("Feld"), "{}", replies[1]);
    assert!(replies[2].contains("Wert"), "{}", replies[2]);
    assert!(replies[3].contains("nicht sehen"), "{}", replies[3]);
    assert!(club.open_changesets().await.is_empty());
}

#[tokio::test]
async fn an_event_key_in_two_organizations_of_the_member_is_ambiguous() {
    let club = Club::start().await;
    let user = club.linked_member(Some("event-contributor"), ACCOUNT).await;
    // The same key in a second organization of the same user.
    let other = club.test.create_organization("otherwil").await;
    club.test.create_event(other, "TEST30").await;
    club.test
        .add_membership(other, user, OrganizationRole::Owner)
        .await;

    let replies = converse(
        &club.test,
        vec![update(1, "/vorschlag TEST30 date_window 2030-06..2030-07")],
        1,
    )
    .await;
    assert!(
        replies[0].contains("mehreren Organisationen"),
        "{replies:?}"
    );
    assert!(club.open_changesets().await.is_empty());
}

#[tokio::test]
async fn an_update_that_does_not_decode_stays_out_of_the_log() {
    support::logs::install();
    let test = TestDatabase::start().await;
    // The date is not a number, so the Bot API client cannot decode the batch.
    let mut broken = update(5, "/vorschlag TEST30 venue Geheimnis im Hangar 3");
    broken["message"]["date"] = json!("gestern");

    let api = FakeBotApi::default();
    api.batches
        .lock()
        .unwrap()
        .extend([vec![broken], vec![update(6, "not-a-code")]]);
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
    let (replied, seen) = (api.replied.clone(), api.replies.clone());
    let stop = async move {
        while seen.lock().unwrap().is_empty() {
            replied.notified().await;
        }
    };
    tokio::time::timeout(Duration::from_secs(20), gateway.run(stop))
        .await
        .unwrap();

    support::logs::assert_clean(&["Geheimnis", "Testperson", "Muster", "7130429", "test-token"]);
    let replies = api.replies.lock().unwrap().clone();
    assert_eq!(replies.len(), 1, "{replies:?}");
    assert!(replies[0].starts_with("Dieser Code ist ungültig"));
}
