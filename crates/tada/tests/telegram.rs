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
use secrecy::SecretString;
use serde_json::{Value, json};
use tada_adapters::clock::SystemClock;
use tada_app::auth::{Authenticated, Authenticator, Credential};
use tada_app::caller::{MemberCaller, OrganizationRole};
use tada_app::caller::{ServiceCaller, TelegramGateway};
use tada_app::domain::ids::UserId;
use tada_app::messenger::{Button, Messenger, SendOutcome};
use tada_app::session::SessionAuthenticator;
use tada_app::telegram::{
    TelegramName, TelegramUserId, accept_link_claim, claim_link_code, confirm_link,
    create_link_code, list_link_requests,
};
use tada_store_pg::testing::TestDatabase;
use tada_telegram::{Gateway, TelegramMessenger, UpdateSource};
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
    tokio::time::timeout(
        Duration::from_secs(20),
        gateway.run(stop, &UpdateSource::Polling),
    )
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
    tokio::time::timeout(
        Duration::from_secs(10),
        gateway.run(stop, &UpdateSource::Polling),
    )
    .await
    .unwrap();

    let replies = api.replies.lock().unwrap().clone();
    assert_eq!(
        replies.len(),
        2,
        "the repeated update got a reply: {replies:?}"
    );
    assert!(replies[0].starts_with("Dieser Code gehört zum tada-Konto"));
    assert!(replies[1].starts_with("Dieser Code ist ungültig"));

    // The Telegram account accepts the claim (ADR 0011, reverse phishing).
    let replies = converse(&test, vec![update(3, "/bestaetigen")], 1).await;
    assert!(replies[0].starts_with("Danke. Bestätigen Sie die Verknüpfung"));
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

/// The attack (reverse phishing): a member sends a victim the deep link with the code of the
/// member. The bot names the tada account of the code, and the link needs the consent of the
/// Telegram account in the chat before the member can confirm it.
#[tokio::test]
async fn the_telegram_account_sees_the_tada_account_and_consents_first() {
    support::logs::install();
    let test = TestDatabase::start().await;
    let (_, user, cookie) = test.member("testwil", OrganizationRole::Owner).await;
    let attacker = authenticate(&test, &cookie).await;
    let code = create_link_code(&attacker, &test.database, &SystemClock)
        .await
        .unwrap();
    let name: String = test
        .scalar(&format!(
            "SELECT display_name FROM app_user WHERE id = '{}'",
            user.as_uuid()
        ))
        .await;

    let replies = converse(&test, vec![update(1, &format!("/start {}", code.code))], 1).await;
    assert!(replies[0].contains(&name), "{}", replies[0]);
    assert!(replies[0].contains("testwil"), "{}", replies[0]);
    // A display name is not unique; the masked address of the account is a second sign.
    assert!(replies[0].contains("m…@example.org"), "{}", replies[0]);
    assert!(replies[0].contains("/bestaetigen"), "{}", replies[0]);
    let requests = list_link_requests(&attacker, &test.database, &SystemClock)
        .await
        .unwrap();
    assert!(
        requests.is_empty(),
        "no consent of the Telegram account yet"
    );

    let replies = converse(&test, vec![update(2, "/bestaetigen")], 1).await;
    assert!(
        replies[0].starts_with("Danke. Bestätigen Sie die Verknüpfung"),
        "{}",
        replies[0]
    );
    let requests = list_link_requests(&attacker, &test.database, &SystemClock)
        .await
        .unwrap();
    assert_eq!(requests.len(), 1);
}

/// The Telegram account rejects a claim with /trennen; the member then has nothing to confirm.
#[tokio::test]
async fn the_telegram_account_rejects_a_claim() {
    support::logs::install();
    let test = TestDatabase::start().await;
    let (_, _, cookie) = test.member("testwil", OrganizationRole::Owner).await;
    let member = authenticate(&test, &cookie).await;
    let code = create_link_code(&member, &test.database, &SystemClock)
        .await
        .unwrap();

    let replies = converse(
        &test,
        vec![
            update(1, &format!("/start {}", code.code)),
            update(2, "/trennen"),
            update(3, "/bestaetigen"),
        ],
        3,
    )
    .await;
    assert!(
        replies[1].contains("nicht mit tada verknüpft"),
        "{}",
        replies[1]
    );
    assert!(
        replies[2].starts_with("Es gibt nichts zu bestätigen"),
        "{}",
        replies[2]
    );
    assert!(
        list_link_requests(&member, &test.database, &SystemClock)
            .await
            .unwrap()
            .is_empty()
    );
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
        .is_some()
    );
    assert!(
        accept_link_claim(
            &gateway,
            TelegramUserId(7130429),
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

/// The attack: a stolen session links the Telegram account of the thief, which then acts as the
/// member. The confirmation needs a sign-in of at most 15 minutes ago.
#[tokio::test]
async fn the_confirmation_needs_a_recent_sign_in() {
    support::logs::install();
    let test = TestDatabase::start().await;
    let (organization, user, _) = test.member("testwil", OrganizationRole::Owner).await;
    let old = jiff::Timestamp::now() - jiff::SignedDuration::from_mins(16);
    let cookie = test.sign_in(user, Some(organization), old).await;
    let router = support::session_router(&test, Arc::new(SystemClock));

    let (status, _, code) = call(
        &router,
        &cookie,
        Method::POST,
        "/api/v1/telegram/link-codes",
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
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
        .is_some()
    );
    assert!(
        accept_link_claim(
            &gateway,
            TelegramUserId(7130429),
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
    let path = format!(
        "/api/v1/telegram/link-requests/{}/confirm",
        page["items"][0]["id"].as_str().unwrap()
    );

    let (status, _, problem) = call(&router, &cookie, Method::POST, &path).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{problem}");
    assert_eq!(problem["code"], "recent-sign-in-required");
    assert_eq!(
        test.scalar::<i64>("SELECT count(*) FROM telegram_identity")
            .await,
        0
    );
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
        self.linked_member_with_cookie(role, account).await.0
    }

    /// `linked_member` with the session cookie of the member.
    async fn linked_member_with_cookie(
        &self,
        role: Option<&str>,
        account: i64,
    ) -> (UserId, String) {
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
                &SystemClock
            )
            .await
            .unwrap()
            .is_some()
        );
        assert!(
            accept_link_claim(
                &ServiceCaller::<TelegramGateway>::new(),
                TelegramUserId(account),
                &self.test.database,
                &SystemClock
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
        (user, cookie)
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

/// A member sees the link in the web client and removes it; the account then acts as nobody.
#[tokio::test]
async fn a_member_unlinks_the_telegram_account_in_the_web_client() {
    let club = Club::start().await;
    let (_, cookie) = club
        .linked_member_with_cookie(Some("event-contributor"), ACCOUNT)
        .await;
    let (status, link) = call_json(
        &club.router,
        &cookie,
        Method::GET,
        "/api/v1/telegram/link",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(link["link"]["telegram_user_id"], ACCOUNT);

    let (status, _) = call_json(
        &club.router,
        &cookie,
        Method::POST,
        "/api/v1/telegram/link/remove",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (_, link) = call_json(
        &club.router,
        &cookie,
        Method::GET,
        "/api/v1/telegram/link",
        None,
    )
    .await;
    assert_eq!(link, json!({"link": null}));
    let replies = converse(
        &club.test,
        vec![update(1, "/vorschlag TEST30 date_window 2030-06..2030-07")],
        1,
    )
    .await;
    assert!(replies[0].contains("nicht verknüpft"), "{}", replies[0]);
}

/// A lost or wrong link ends from Telegram too: /trennen removes the link of the account.
#[tokio::test]
async fn the_bot_unlinks_the_account() {
    let club = Club::start().await;
    club.linked_member(Some("event-contributor"), ACCOUNT).await;

    let replies = converse(&club.test, vec![update(1, "/trennen")], 1).await;
    assert!(
        replies[0].contains("nicht mit tada verknüpft"),
        "{}",
        replies[0]
    );
    assert_eq!(
        club.test
            .scalar::<i64>("SELECT count(*) FROM telegram_identity")
            .await,
        0
    );
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
async fn an_update_that_does_not_decode_is_skipped_alone_and_stays_out_of_the_log() {
    support::logs::install();
    let test = TestDatabase::start().await;
    // The date is not a number, so the Bot API client cannot decode the batch.
    // The update after it in the same batch decodes and gets its reply.
    let mut broken = update(5, "/vorschlag TEST30 venue Geheimnis im Hangar 3");
    broken["message"]["date"] = json!("gestern");

    let api = FakeBotApi::default();
    api.batches
        .lock()
        .unwrap()
        .push_back(vec![broken, update(6, "not-a-code")]);
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
    let (replied, seen, asked) = (
        api.replied.clone(),
        api.replies.clone(),
        api.offsets.clone(),
    );
    let stop = async move {
        while seen.lock().unwrap().is_empty() {
            replied.notified().await;
        }
        // Wait for the next request, which shows the new offset.
        while asked.lock().unwrap().len() < 2 {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    };
    tokio::time::timeout(
        Duration::from_secs(20),
        gateway.run(stop, &UpdateSource::Polling),
    )
    .await
    .unwrap();

    support::logs::assert_clean(&[
        "Geheimnis",
        "gestern",
        "Testperson",
        "Muster",
        "7130429",
        "test-token",
    ]);
    let replies = api.replies.lock().unwrap().clone();
    assert_eq!(replies.len(), 1, "{replies:?}");
    assert!(replies[0].starts_with("Dieser Code ist ungültig"));
    let offsets = api.offsets.lock().unwrap().clone();
    assert_eq!(
        offsets[1],
        json!(7),
        "the next request must ask for the updates after the batch: {offsets:?}"
    );
}

/// A fake Bot API that records each call and answers each method with `answer`.
#[derive(Clone)]
struct RecordingBotApi {
    /// The method and the JSON body of each call, in order.
    calls: Arc<Mutex<Vec<(String, Value)>>>,
    answer: Arc<dyn Fn(&str) -> Answer + Send + Sync>,
}

/// The answer of the fake to one call.
enum Answer {
    Json(StatusCode, Value),
    /// No answer before the client gives up.
    Hang,
}

/// A successful answer with `result`.
fn ok(result: Value) -> Answer {
    Answer::Json(StatusCode::OK, json!({"ok": true, "result": result}))
}

/// A refusal of the Bot API with the HTTP status `code`.
fn refusal(code: u16) -> Answer {
    Answer::Json(
        StatusCode::from_u16(code).unwrap(),
        json!({"ok": false, "error_code": code, "description": "refused"}),
    )
}

/// The answer of a working Bot API to each method that the gateway calls.
fn working(method: &str) -> Answer {
    match method {
        "sendMessage" => ok(json!({
            "message_id": 99, "date": 0, "chat": {"id": ACCOUNT, "type": "private"}, "text": "x"
        })),
        "getUpdates" => ok(json!([])),
        "getWebhookInfo" => {
            ok(json!({"url": "", "has_custom_certificate": false, "pending_update_count": 0}))
        }
        _ => ok(json!(true)),
    }
}

async fn record(
    State(api): State<RecordingBotApi>,
    axum::extract::Path((_, method)): axum::extract::Path<(String, String)>,
    body: axum::body::Bytes,
) -> axum::response::Response {
    use axum::response::IntoResponse;
    let body = serde_json::from_slice(&body).unwrap_or(Value::Null);
    api.calls.lock().unwrap().push((method.clone(), body));
    if method == "getUpdates" {
        // A long poll without updates does not answer at once.
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    match (api.answer)(&method) {
        Answer::Json(status, body) => (status, Json(body)).into_response(),
        Answer::Hang => {
            tokio::time::sleep(Duration::from_secs(30)).await;
            StatusCode::GATEWAY_TIMEOUT.into_response()
        }
    }
}

impl RecordingBotApi {
    /// Starts the fake. Returns it and its base URL.
    async fn start(answer: impl Fn(&str) -> Answer + Send + Sync + 'static) -> (Self, String) {
        let api = Self {
            calls: Arc::default(),
            answer: Arc::new(answer),
        };
        let server = Router::new()
            .route("/{bot}/{method}", post(record))
            .with_state(api.clone());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, server).await.unwrap() });
        (api, format!("http://{address}"))
    }

    /// The bodies of the calls of `method`.
    fn calls_of(&self, method: &str) -> Vec<Value> {
        self.calls
            .lock()
            .unwrap()
            .iter()
            .filter(|(called, _)| called == method)
            .map(|(_, body)| body.clone())
            .collect()
    }
}

/// The invented webhook secret of the tests.
const WEBHOOK_SECRET: &str = "test-webhook-secret-4f1c";

fn webhook_source() -> UpdateSource {
    UpdateSource::Webhook {
        secret: SecretString::from(WEBHOOK_SECRET),
        public_url: support::public_url(),
    }
}

/// The listener of the `telegram` role for `gateway`, with the database as its only store.
fn role_router(test: &TestDatabase, gateway: &Arc<Gateway>, source: &UpdateSource) -> Router {
    tada::telegram::routes(
        gateway.clone(),
        source,
        vec![Arc::new(test.database.clone())],
    )
}

/// A `POST /telegram/webhook` with `secret` in the secret header, if any.
async fn deliver(router: &Router, secret: Option<&str>, body: Vec<u8>) -> StatusCode {
    let mut request = axum::http::Request::post("/telegram/webhook")
        .header(header::CONTENT_TYPE, "application/json");
    if let Some(secret) = secret {
        request = request.header("X-Telegram-Bot-Api-Secret-Token", secret);
    }
    let response = router
        .clone()
        .oneshot(request.body(Body::from(body)).unwrap())
        .await
        .unwrap();
    response.status()
}

async fn get_status(router: &Router, path: &str) -> StatusCode {
    let request = axum::http::Request::get(path).body(Body::empty()).unwrap();
    router.clone().oneshot(request).await.unwrap().status()
}

#[tokio::test]
async fn webhook_rejects_bad_secret_and_replay() {
    support::logs::install();
    let test = TestDatabase::start().await;
    let (api, url) = RecordingBotApi::start(working).await;
    let gateway = Arc::new(Gateway::new(
        &url,
        "test-token",
        Arc::new(test.database.clone()),
        Arc::new(SystemClock),
    ));
    let router = role_router(&test, &gateway, &webhook_source());
    let body = serde_json::to_vec(&update(41, "not-a-code")).unwrap();

    assert_eq!(
        deliver(&router, None, body.clone()).await,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        deliver(&router, Some("test-webhook-secret-4f1d"), body.clone()).await,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        deliver(&router, Some(""), body.clone()).await,
        StatusCode::UNAUTHORIZED
    );
    assert!(
        api.calls_of("sendMessage").is_empty(),
        "a request with a bad secret got a reply"
    );

    // The rejected requests did not record the update, so the first good delivery gets its reply.
    assert_eq!(
        deliver(&router, Some(WEBHOOK_SECRET), body.clone()).await,
        StatusCode::OK
    );
    assert_eq!(api.calls_of("sendMessage").len(), 1);
    // Telegram delivers an update again if it saw no answer; the replay changes nothing.
    assert_eq!(
        deliver(&router, Some(WEBHOOK_SECRET), body).await,
        StatusCode::OK
    );
    assert_eq!(
        api.calls_of("sendMessage").len(),
        1,
        "the replay got a reply"
    );

    // A button press is not served yet: no reply and no error.
    let press = json!({"update_id": 42, "callback_query": {
        "id": "4382", "chat_instance": "1",
        "from": {"id": ACCOUNT, "is_bot": false, "first_name": "Testperson"},
        "data": "x"
    }});
    let press = serde_json::to_vec(&press).unwrap();
    assert_eq!(
        deliver(&router, Some(WEBHOOK_SECRET), press).await,
        StatusCode::OK
    );
    assert_eq!(api.calls_of("sendMessage").len(), 1);
    // A body that is not an update gets no retry from Telegram and no reply.
    assert_eq!(
        deliver(
            &router,
            Some(WEBHOOK_SECRET),
            b"{\"update_id\":\"Geheimnis\"}".to_vec()
        )
        .await,
        StatusCode::OK
    );

    support::logs::assert_clean(&[
        WEBHOOK_SECRET,
        "test-token",
        "Testperson",
        "7130429",
        "Geheimnis",
    ]);
}

#[tokio::test]
async fn oversized_body_is_rejected() {
    support::logs::install();
    let test = TestDatabase::start().await;
    let (api, url) = RecordingBotApi::start(working).await;
    let gateway = Arc::new(Gateway::new(
        &url,
        "test-token",
        Arc::new(test.database.clone()),
        Arc::new(SystemClock),
    ));
    let router = role_router(&test, &gateway, &webhook_source());
    let mut big = update(43, "not-a-code");
    big["message"]["caption"] = json!("x".repeat(1024 * 1024));

    assert_eq!(
        deliver(
            &router,
            Some(WEBHOOK_SECRET),
            serde_json::to_vec(&big).unwrap()
        )
        .await,
        StatusCode::PAYLOAD_TOO_LARGE
    );
    assert!(api.calls_of("sendMessage").is_empty());
    // The update is not recorded: a smaller delivery of the same ID still gets its reply.
    let small = serde_json::to_vec(&update(43, "not-a-code")).unwrap();
    assert_eq!(
        deliver(&router, Some(WEBHOOK_SECRET), small).await,
        StatusCode::OK
    );
    assert_eq!(api.calls_of("sendMessage").len(), 1);
}

#[tokio::test]
async fn readyz_fails_with_revoked_token() {
    support::logs::install();
    let test = TestDatabase::start().await;

    // A working Bot API: the first long poll makes the role ready.
    let (api, url) = RecordingBotApi::start(working).await;
    let gateway = Arc::new(Gateway::new(
        &url,
        "test-token",
        Arc::new(test.database.clone()),
        Arc::new(SystemClock),
    ));
    let router = role_router(&test, &gateway, &UpdateSource::Polling);
    let polled = {
        let api = api.clone();
        async move {
            while api.calls_of("getUpdates").len() < 2 {
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        }
    };
    tokio::time::timeout(
        Duration::from_secs(20),
        gateway.run(polled, &UpdateSource::Polling),
    )
    .await
    .unwrap();
    assert_eq!(get_status(&router, "/readyz").await, StatusCode::OK);

    // A revoked token: each call gets 401, so no Bot API call succeeds.
    let (api, url) = RecordingBotApi::start(|_| refusal(401)).await;
    let gateway = Arc::new(Gateway::new(
        &url,
        "test-token",
        Arc::new(test.database.clone()),
        Arc::new(SystemClock),
    ));
    let router = role_router(&test, &gateway, &UpdateSource::Polling);
    let refused = {
        let api = api.clone();
        async move {
            while api.calls_of("getUpdates").is_empty() {
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        }
    };
    tokio::time::timeout(
        Duration::from_secs(20),
        gateway.run(refused, &UpdateSource::Polling),
    )
    .await
    .unwrap();
    assert_eq!(
        get_status(&router, "/readyz").await,
        StatusCode::SERVICE_UNAVAILABLE
    );
    assert_eq!(get_status(&router, "/healthz").await, StatusCode::OK);
    support::logs::assert_clean(&["test-token"]);
}

#[tokio::test]
async fn healthz_in_both_modes() {
    support::logs::install();
    let test = TestDatabase::start().await;
    let (api, url) = RecordingBotApi::start(working).await;
    let gateway = Arc::new(Gateway::new(
        &url,
        "test-token",
        Arc::new(test.database.clone()),
        Arc::new(SystemClock),
    ));

    // Polling: the health routes, and no webhook route.
    let router = role_router(&test, &gateway, &UpdateSource::Polling);
    assert_eq!(get_status(&router, "/healthz").await, StatusCode::OK);
    let body = serde_json::to_vec(&update(44, "not-a-code")).unwrap();
    assert_eq!(
        deliver(&router, Some(WEBHOOK_SECRET), body).await,
        StatusCode::NOT_FOUND
    );
    let polled = {
        let api = api.clone();
        async move {
            while api.calls_of("getUpdates").is_empty() {
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        }
    };
    tokio::time::timeout(
        Duration::from_secs(20),
        gateway.run(polled, &UpdateSource::Polling),
    )
    .await
    .unwrap();
    assert_eq!(
        api.calls_of("getUpdates")[0]["allowed_updates"],
        json!(["message", "callback_query"])
    );

    // Webhook: the role registers the webhook with its secret at start.
    let source = webhook_source();
    let router = role_router(&test, &gateway, &source);
    assert_eq!(get_status(&router, "/healthz").await, StatusCode::OK);
    let registered = {
        let api = api.clone();
        async move {
            while api.calls_of("setWebhook").is_empty() {
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        }
    };
    tokio::time::timeout(Duration::from_secs(20), gateway.run(registered, &source))
        .await
        .unwrap();
    let webhook = &api.calls_of("setWebhook")[0];
    assert_eq!(webhook["url"], "https://tada.example.org/telegram/webhook");
    assert_eq!(webhook["secret_token"], WEBHOOK_SECRET);
    assert_eq!(
        webhook["allowed_updates"],
        json!(["message", "callback_query"])
    );
    assert_eq!(get_status(&router, "/readyz").await, StatusCode::OK);
    support::logs::assert_clean(&[WEBHOOK_SECRET, "test-token"]);
}

/// A recipient of the send tests, invented.
const RECIPIENT: TelegramUserId = TelegramUserId(7130429);

#[tokio::test]
async fn send_maps_403_to_blocked() {
    support::logs::install();
    for (answer, expected) in [
        (200, SendOutcome::Sent),
        (403, SendOutcome::Blocked),
        (400, SendOutcome::Failed),
        (429, SendOutcome::Failed),
        (500, SendOutcome::Unknown),
        (502, SendOutcome::Unknown),
    ] {
        let (_, url) = RecordingBotApi::start(move |method| {
            if answer == 200 {
                working(method)
            } else {
                refusal(answer)
            }
        })
        .await;
        let messenger = TelegramMessenger::new(&url, "test-token");
        assert_eq!(
            messenger.send(RECIPIENT, "Hallo", &[]).await,
            expected,
            "status {answer}"
        );
    }
    // No Bot API at the address: the request did not leave, so nothing was sent.
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let closed = format!("http://{}", listener.local_addr().unwrap());
    drop(listener);
    let messenger = TelegramMessenger::new(&closed, "test-token");
    assert_eq!(
        messenger.send(RECIPIENT, "Hallo", &[]).await,
        SendOutcome::Failed
    );
    support::logs::assert_clean(&["test-token", "7130429"]);
}

#[tokio::test]
async fn send_timeout_is_unknown() {
    support::logs::install();
    let (api, url) = RecordingBotApi::start(|_| Answer::Hang).await;
    let messenger =
        TelegramMessenger::new(&url, "test-token").with_timeout(Duration::from_millis(300));
    let outcome = tokio::time::timeout(
        Duration::from_secs(10),
        messenger.send(RECIPIENT, "Hallo", &[]),
    )
    .await
    .unwrap();
    assert_eq!(outcome, SendOutcome::Unknown);
    assert_eq!(api.calls_of("sendMessage").len(), 1, "the request left");
}

#[tokio::test]
async fn messages_are_plain_text() {
    support::logs::install();
    let test = TestDatabase::start().await;
    let (api, url) = RecordingBotApi::start(working).await;
    let messenger = TelegramMessenger::new(&url, "test-token");
    let title = "<b>Zelt</b> [mark all done](https://example.org) *jetzt*";
    let rows = vec![vec![
        Button::Callback {
            label: "Erledigt".to_owned(),
            data: "d:AAEC".to_owned(),
        },
        Button::Url {
            label: "Öffnen".to_owned(),
            url: "https://tada.example.org/events".to_owned(),
        },
    ]];
    assert_eq!(
        messenger.send(RECIPIENT, title, &rows).await,
        SendOutcome::Sent
    );

    let sent = &api.calls_of("sendMessage")[0];
    assert_eq!(sent["chat_id"], json!(7130429));
    assert_eq!(sent["text"], title);
    assert!(sent.get("parse_mode").is_none(), "{sent}");
    assert!(sent.get("entities").is_none(), "{sent}");
    assert_eq!(
        sent["reply_markup"]["inline_keyboard"],
        json!([[
            {"text": "Erledigt", "callback_data": "d:AAEC"},
            {"text": "Öffnen", "url": "https://tada.example.org/events"}
        ]])
    );

    // The replies of the gateway are plain text too.
    let gateway = Arc::new(Gateway::new(
        &url,
        "test-token",
        Arc::new(test.database.clone()),
        Arc::new(SystemClock),
    ));
    let router = role_router(&test, &gateway, &webhook_source());
    let body = serde_json::to_vec(&update(45, "not-a-code")).unwrap();
    assert_eq!(
        deliver(&router, Some(WEBHOOK_SECRET), body).await,
        StatusCode::OK
    );
    let reply = &api.calls_of("sendMessage")[1];
    assert!(reply.get("parse_mode").is_none(), "{reply}");
    // A message without buttons has no keyboard.
    messenger.send(RECIPIENT, "Hallo", &[]).await;
    assert!(api.calls_of("sendMessage")[2].get("reply_markup").is_none());
}

/// The platform stops the role within 25 seconds (ADR 0025): a hung Bot API call must not hold it.
#[tokio::test]
async fn stop_ends_the_run_while_a_bot_api_call_hangs() {
    support::logs::install();
    let test = TestDatabase::start().await;

    // Webhook mode: the call that keeps `/readyz` current hangs.
    let (api, url) = RecordingBotApi::start(|method| match method {
        "getWebhookInfo" => Answer::Hang,
        _ => working(method),
    })
    .await;
    let gateway = Gateway::new(
        &url,
        "test-token",
        Arc::new(test.database.clone()),
        Arc::new(SystemClock),
    );
    let hung = {
        let api = api.clone();
        async move {
            while api.calls_of("getWebhookInfo").is_empty() {
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        }
    };
    tokio::time::timeout(
        Duration::from_secs(10),
        gateway.run(hung, &webhook_source()),
    )
    .await
    .expect("the hung heartbeat held the stop");

    // Polling mode: the reply to an update hangs.
    let delivered = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let (api, url) = RecordingBotApi::start(move |method| match method {
        "sendMessage" => Answer::Hang,
        "getUpdates" if !delivered.swap(true, std::sync::atomic::Ordering::SeqCst) => {
            ok(json!([update(51, "not-a-code")]))
        }
        _ => working(method),
    })
    .await;
    let gateway = Gateway::new(
        &url,
        "test-token",
        Arc::new(test.database.clone()),
        Arc::new(SystemClock),
    );
    let hung = {
        let api = api.clone();
        async move {
            while api.calls_of("sendMessage").is_empty() {
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        }
    };
    tokio::time::timeout(
        Duration::from_secs(10),
        gateway.run(hung, &UpdateSource::Polling),
    )
    .await
    .expect("the hung reply held the stop");
}

/// Telegram delivers an update again after an error. The dedupe makes that safe, so an update that
/// the database cannot record gets an error, not a silent loss.
#[tokio::test]
async fn a_webhook_update_that_cannot_be_recorded_gets_an_error() {
    support::logs::install();
    let test = TestDatabase::start().await;
    let (api, url) = RecordingBotApi::start(working).await;
    let gateway = Arc::new(Gateway::new(
        &url,
        "test-token",
        Arc::new(test.database.clone()),
        Arc::new(SystemClock),
    ));
    let router = role_router(&test, &gateway, &webhook_source());
    test.database.close().await;
    let body = serde_json::to_vec(&update(52, "not-a-code")).unwrap();
    assert_eq!(
        deliver(&router, Some(WEBHOOK_SECRET), body).await,
        StatusCode::INTERNAL_SERVER_ERROR
    );
    assert!(api.calls_of("sendMessage").is_empty());
}
