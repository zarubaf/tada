//! An organization that its owner fills through the API, on a database with a shared Garage.
//! The export test uses it.

use std::num::NonZeroU64;
use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::http::{Method, StatusCode, header};
use jiff::Timestamp;
use serde_json::{Value, json};
use tada_adapters::clock::SystemClock;
use tada_adapters::storage::testing::TestGarage;
use tada_api::ApiState;
use tada_app::caller::{OrganizationRole, ServiceCaller, TelegramGateway};
use tada_app::domain::identity::{DisplayName, Email};
use tada_app::domain::ids::{OrganizationId, UserId};
use tada_app::session::SessionAuthenticator;
use tada_app::telegram::{TelegramName, TelegramUserId, claim_link_code};
use tada_store_pg::testing::TestDatabase;
use uuid::Uuid;

use super::SESSION_COOKIE;
use super::files::pdf;

/// The source text of each changeset of `Client::propose`.
pub const SOURCE: &str = "Das Open Day findet im Mai 2030 auf dem Flugfeld statt.";

/// The API of one database on a shared Garage.
pub fn router(test: &TestDatabase, garage: &TestGarage) -> Router {
    let database = Arc::new(test.database.clone());
    let clock = Arc::new(SystemClock);
    let authenticator = Arc::new(SessionAuthenticator::new(
        database.clone(),
        database,
        clock.clone(),
    ));
    let state = ApiState {
        blobs: Arc::new(garage.storage.clone()),
        upload_max_bytes: NonZeroU64::new(1024 * 1024).unwrap(),
        ..super::api_state(test, authenticator, clock)
    };
    tada_api::router(state, None)
}

/// A member of one organization who calls the API.
pub struct Client {
    pub router: Router,
    pub cookie: String,
}

impl Client {
    pub async fn call(
        &self,
        request: axum::http::request::Builder,
        body: Body,
    ) -> (StatusCode, Value) {
        let request = request
            .header(header::COOKIE, format!("{SESSION_COOKIE}={}", self.cookie))
            .body(body)
            .unwrap();
        let (response, value) = super::send(&self.router, request).await;
        (response.status(), value)
    }

    pub async fn get(&self, path: &str) -> Value {
        let (status, value) = self
            .call(super::request(Method::GET, path), Body::empty())
            .await;
        assert_eq!(status, StatusCode::OK, "{path}: {value}");
        value
    }

    pub async fn post(&self, path: &str, body: &Value) -> Value {
        let request =
            super::request(Method::POST, path).header(header::CONTENT_TYPE, "application/json");
        let (status, value) = self.call(request, Body::from(body.to_string())).await;
        assert!(status.is_success(), "{path}: {status} {value}");
        value
    }

    pub async fn upload(&self, path: &str, name: &str, content: Vec<u8>) -> Value {
        let request = super::request(Method::POST, path)
            .header(header::CONTENT_TYPE, "application/octet-stream")
            .header("x-file-name", name);
        let (status, value) = self.call(request, Body::from(content)).await;
        assert_eq!(status, StatusCode::CREATED, "{value}");
        value
    }

    pub async fn create_event(&self, key: &str, name: &str) -> String {
        let event = self
            .post("/api/v1/events", &json!({"key": key, "name": name}))
            .await;
        event["id"].as_str().unwrap().to_owned()
    }

    /// Proposes one operation with the passage `quote` of `SOURCE` as evidence. Returns the changeset.
    pub async fn propose(&self, event: &str, operation: Value, quote: &str) -> Value {
        let start = SOURCE[..SOURCE.find(quote).unwrap()].chars().count();
        let body = json!({
            "source_text": SOURCE,
            "proposals": [{
                "id": Uuid::now_v7(),
                "operation": operation,
                "evidence": [{"start": start, "end": start + quote.chars().count(), "quote": quote}],
                "reason": "The member wrote it.",
            }],
        });
        self.post(&format!("/api/v1/events/{event}/changesets"), &body)
            .await
    }

    pub async fn apply(&self, changeset: &Value) {
        let path = format!(
            "/api/v1/changesets/{}/apply",
            changeset["id"].as_str().unwrap()
        );
        self.post(&path, &json!({"selected": changeset["proposal_ids"]}))
            .await;
    }

    /// A proposal that sets the date window of the event to May 2030.
    pub async fn date_operation(&self, event: &str) -> Value {
        let fields = self.get(&format!("/api/v1/events/{event}/fields")).await;
        let field = fields["items"]
            .as_array()
            .unwrap()
            .iter()
            .find(|field| field["key"] == "date_window")
            .unwrap()["id"]
            .clone();
        json!({
            "kind": "set-fact", "event_id": event, "field_id": field, "expected_version": null,
            "state": {"state": "accepted", "value": {"type": "date-window",
                "start": "2030-05-01", "end": "2030-05-31", "granularity": "month"}},
        })
    }
}

/// An organization with an owner who calls the API.
pub async fn organization(
    test: &TestDatabase,
    garage: &TestGarage,
    slug: &str,
) -> (OrganizationId, UserId, Client) {
    let (organization, user, cookie) = test.member(slug, OrganizationRole::Owner).await;
    let client = Client {
        router: router(test, garage),
        cookie,
    };
    (organization, user, client)
}

/// The records of a filled organization that the tests use.
pub struct Filled {
    /// The first event, with an applied changeset, an upload and an approved draft.
    pub event: String,
    /// The second event, with an open changeset.
    pub second: String,
    pub upload: String,
    pub draft: String,
    /// The tokens and link codes of the fill, and the session of the client.
    pub secrets: Vec<String>,
}

/// Fills an organization: two events, facts, an applied and an open changeset, an upload with two
/// versions, an approved draft, a Telegram link of the account `telegram_user`, an open link code,
/// an API token and an invitation.
pub async fn fill(
    test: &TestDatabase,
    client: &Client,
    organization: OrganizationId,
    telegram_user: i64,
) -> Filled {
    let event = client.create_event("OPEN30", "Open Day Testwil").await;
    let second = client.create_event("FLY30", "Fly-in Testwil").await;

    let operation = client.date_operation(&event).await;
    let applied = client.propose(&event, operation, "im Mai 2030").await;
    client.apply(&applied).await;
    let operation = client.date_operation(&second).await;
    client.propose(&second, operation, "im Mai 2030").await;
    let profile = client.get(&format!("/api/v1/events/{event}/profile")).await;
    let fact = profile["facts"][0]["id"].as_str().unwrap().to_owned();

    let documents = format!("/api/v1/events/{event}/documents");
    let upload = client
        .upload(&documents, "Programm.pdf", pdf("Programm Testwil eins"))
        .await;
    let upload = upload["id"].as_str().unwrap().to_owned();
    client
        .upload(
            &format!("/api/v1/documents/{upload}/versions"),
            "Programm.pdf",
            pdf("Programm Testwil zwei"),
        )
        .await;

    let draft = Uuid::now_v7();
    let operation = json!({
        "kind": "create-document-draft", "event_id": event,
        "document": {"new": {"id": draft, "name": "Konzept Testwil"}},
        "markdown": format!("Das Open Day ist am [](tada:fact/{fact}?v=1).\n"),
    });
    let changeset = client.propose(&event, operation, "Das Open Day").await;
    client.apply(&changeset).await;
    let document = client.get(&format!("/api/v1/documents/{draft}")).await;
    let version = document["newest_version"]["id"].as_str().unwrap();
    client
        .post(
            &format!("/api/v1/document-versions/{version}/approve"),
            &json!({"expected_version": 1}),
        )
        .await;

    let code = client.post("/api/v1/telegram/link-codes", &json!({})).await;
    let code = code["code"].as_str().unwrap().to_owned();
    assert!(
        claim_link_code(
            &ServiceCaller::<TelegramGateway>::new(),
            &code,
            TelegramUserId(telegram_user),
            &TelegramName("Testperson Testwil".to_owned()),
            &test.database,
            &SystemClock,
        )
        .await
        .unwrap()
    );
    let requests = client.get("/api/v1/telegram/link-requests").await;
    let request = requests["items"][0]["id"].as_str().unwrap();
    client
        .post(
            &format!("/api/v1/telegram/link-requests/{request}/confirm"),
            &json!({}),
        )
        .await;
    let open_code = client.post("/api/v1/telegram/link-codes", &json!({})).await;

    let expires = Timestamp::now() + jiff::SignedDuration::from_hours(24);
    let token = client
        .post(
            "/api/v1/tokens",
            &json!({"name": "Claude Code", "scope": "read", "expires_at": expires.to_string(),
                    "notice_version_confirmed": 1}),
        )
        .await;

    test.queue_invitation(
        organization,
        &Email::parse("invited@example.org").unwrap(),
        &DisplayName::parse("Invited Testwil").unwrap(),
        OrganizationRole::Member,
    )
    .await;

    Filled {
        event,
        second,
        upload,
        draft: draft.to_string(),
        secrets: vec![
            client.cookie.clone(),
            code,
            open_code["code"].as_str().unwrap().to_owned(),
            token["secret"].as_str().unwrap().to_owned(),
        ],
    }
}
