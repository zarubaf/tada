//! A restart keeps the sessions, the records and the files (demonstration step 9 of the roadmap),
//! and no table holds a token in plain text (ADR 0008).
//!
//! Each "process" of this test has its own connection pool, router and worker, as a `serve` and a `worker`
//! process have. The restart drops them and builds new ones on the same database and the same object storage.

// The helpers of this test file are not `#[test]` functions, so clippy.toml does not cover them.
#![allow(clippy::unwrap_used)]

mod support;

use std::num::NonZeroU64;
use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::body::Body;
use axum::http::{Method, StatusCode, header};
use jiff::Timestamp;
use serde_json::{Value, json};
use support::SESSION_COOKIE;
use support::files::pdf;
use tada_adapters::clock::SystemClock;
use tada_adapters::mail::{FluentMailTexts, MemoryMailer};
use tada_adapters::storage::testing::TestGarage;
use tada_api::ApiState;
use tada_app::domain::identity::{DisplayName, Email, OrganizationRole};
use tada_app::domain::ids::OrganizationId;
use tada_app::jobs::{Handlers, Ran, run_next};
use tada_app::outbound::SendOutbound;
use tada_app::session::SessionAuthenticator;
use tada_store_pg::Database;
use tada_store_pg::testing::TestDatabase;
use tower::ServiceExt;
use uuid::Uuid;

const SOURCE: &str = "Das Open Day Testwil findet im Juni 2030 auf dem Flugfeld statt.";
const SIGN_IN_LINK: &str = "https://tada.example.org/sign-in/link#token=";
const INVITATION_LINK: &str = "https://tada.example.org/invitation#token=";

/// One `serve` process and one `worker` process with their own connection pool.
struct Process {
    database: Database,
    router: Router,
    mailer: Arc<MemoryMailer>,
    handlers: Handlers,
}

impl Process {
    fn start(test: &TestDatabase, garage: &TestGarage) -> Self {
        let database = test.connect();
        let shared = Arc::new(database.clone());
        let clock = Arc::new(SystemClock);
        let authenticator = Arc::new(SessionAuthenticator::new(
            shared.clone(),
            shared.clone(),
            clock.clone(),
        ));
        let state = ApiState {
            blobs: Arc::new(garage.storage.clone()),
            upload_max_bytes: NonZeroU64::new(1024 * 1024).unwrap(),
            ..support::api_state_on(&database, authenticator, clock.clone())
        };
        let mailer = Arc::new(MemoryMailer::new());
        let handler = SendOutbound::new(
            shared,
            mailer.clone(),
            Arc::new(FluentMailTexts::new().unwrap()),
            clock,
            support::public_url(),
        );
        Self {
            database,
            router: tada_api::router(state, None),
            mailer,
            handlers: Handlers::default().with(Arc::new(handler)),
        }
    }

    /// Stops the process: the pool closes all its connections.
    async fn stop(self) {
        self.database.close().await;
    }

    async fn call(
        &self,
        cookie: Option<&str>,
        method: Method,
        path: &str,
        body: Option<&Value>,
    ) -> (StatusCode, Value) {
        let mut request = support::request(method, path);
        if let Some(cookie) = cookie {
            request = request.header(header::COOKIE, format!("{SESSION_COOKIE}={cookie}"));
        }
        let request = match body {
            Some(body) => request
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(body.to_string())),
            None => request.body(Body::empty()),
        };
        let (response, value) = support::send(&self.router, request.unwrap()).await;
        (response.status(), value)
    }

    async fn get(&self, cookie: &str, path: &str) -> Value {
        let (status, value) = self.call(Some(cookie), Method::GET, path, None).await;
        assert_eq!(status, StatusCode::OK, "{path}: {value}");
        value
    }

    async fn post(&self, cookie: Option<&str>, path: &str, body: &Value) -> Value {
        let (status, value) = self.call(cookie, Method::POST, path, Some(body)).await;
        assert!(status.is_success(), "{path}: {status} {value}");
        value
    }

    async fn upload(&self, cookie: &str, path: &str, content: Vec<u8>) -> Value {
        let request = support::request(Method::POST, path)
            .header(header::COOKIE, format!("{SESSION_COOKIE}={cookie}"))
            .header(header::CONTENT_TYPE, "application/octet-stream")
            .header("x-file-name", "Programm.pdf")
            .body(Body::from(content))
            .unwrap();
        let (response, value) = support::send(&self.router, request).await;
        assert_eq!(response.status(), StatusCode::CREATED, "{value}");
        value
    }

    async fn download(&self, cookie: &str, version: &str) -> Vec<u8> {
        let request = support::request(
            Method::GET,
            &format!("/api/v1/document-versions/{version}/content"),
        )
        .header(header::COOKIE, format!("{SESSION_COOKIE}={cookie}"))
        .body(Body::empty())
        .unwrap();
        let response = self.router.clone().oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec()
    }

    /// Proposes one operation with the passage `quote` of `SOURCE` as evidence and applies it.
    async fn propose_and_apply(&self, cookie: &str, event: &str, operation: Value, quote: &str) {
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
        let changeset = self
            .post(
                Some(cookie),
                &format!("/api/v1/events/{event}/changesets"),
                &body,
            )
            .await;
        let path = format!(
            "/api/v1/changesets/{}/apply",
            changeset["id"].as_str().unwrap()
        );
        self.post(
            Some(cookie),
            &path,
            &json!({"selected": changeset["proposal_ids"]}),
        )
        .await;
    }

    /// Runs the worker until no job is left, then reads the token of the last mail after `link`.
    async fn mailed_token(&self, link: &str) -> String {
        while run_next(
            &self.database,
            &self.handlers,
            Uuid::now_v7(),
            Duration::from_secs(60),
        )
        .await
        .unwrap()
            != Ran::Idle
        {}
        let sent = self.mailer.sent();
        let text = &sent.last().unwrap().text;
        let start = text.find(link).unwrap() + link.len();
        text[start..]
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
            .collect()
    }
}

/// The records and files of the first process, as its API shows them.
#[derive(Debug, PartialEq)]
struct Shown {
    session: Value,
    profile: Value,
    upload: Value,
    upload_content: Vec<u8>,
    draft: Value,
    rendering: Value,
}

async fn shown(process: &Process, cookie: &str, event: &str, upload: &str, draft: &str) -> Shown {
    let upload_document = process
        .get(cookie, &format!("/api/v1/documents/{upload}"))
        .await;
    let version = upload_document["newest_version"]["id"].as_str().unwrap();
    let upload_content = process.download(cookie, version).await;
    let draft_document = process
        .get(cookie, &format!("/api/v1/documents/{draft}"))
        .await;
    let draft_version = draft_document["newest_version"]["id"].as_str().unwrap();
    Shown {
        session: process.get(cookie, "/api/v1/session").await,
        profile: process
            .get(cookie, &format!("/api/v1/events/{event}/profile"))
            .await,
        rendering: process
            .get(
                cookie,
                &format!("/api/v1/document-versions/{draft_version}/rendering"),
            )
            .await,
        upload: upload_document,
        upload_content,
        draft: draft_document,
    }
}

/// The plain texts of the tokens of the first process.
struct Tokens {
    session: String,
    magic_link: String,
    invitation: String,
    link_code: String,
    api_token: String,
}

/// Fills the database through the API of the first process: an event, a fact, an upload, an approved draft,
/// and each kind of token. Returns the event, the upload, the draft and the tokens.
async fn fill(
    process: &Process,
    test: &TestDatabase,
    organization: OrganizationId,
    cookie: &str,
) -> (String, String, String, Tokens) {
    let event = process
        .post(
            Some(cookie),
            "/api/v1/events",
            &json!({"key": "OPEN30", "name": "Open Day Testwil"}),
        )
        .await;
    let event = event["id"].as_str().unwrap().to_owned();

    let fields = process
        .get(cookie, &format!("/api/v1/events/{event}/fields"))
        .await;
    let field = fields["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|field| field["key"] == "date_window")
        .unwrap()["id"]
        .clone();
    let operation = json!({
        "kind": "set-fact", "event_id": event, "field_id": field, "expected_version": null,
        "state": {"state": "accepted", "value": {"type": "date-window",
            "start": "2030-06-01", "end": "2030-06-30", "granularity": "month"}},
    });
    process
        .propose_and_apply(cookie, &event, operation, "im Juni 2030")
        .await;
    let profile = process
        .get(cookie, &format!("/api/v1/events/{event}/profile"))
        .await;
    let fact = profile["facts"][0]["id"].as_str().unwrap().to_owned();

    let upload = process
        .upload(
            cookie,
            &format!("/api/v1/events/{event}/documents"),
            pdf("Programm Testwil"),
        )
        .await;
    let upload = upload["id"].as_str().unwrap().to_owned();

    let draft = Uuid::now_v7().to_string();
    let operation = json!({
        "kind": "create-document-draft", "event_id": event,
        "document": {"new": {"id": draft, "name": "Vorläufiges Konzept"}},
        "markdown": format!("Das Open Day Testwil ist [](tada:fact/{fact}?v=1).\n"),
    });
    process
        .propose_and_apply(cookie, &event, operation, "Das Open Day Testwil")
        .await;
    let document = process
        .get(cookie, &format!("/api/v1/documents/{draft}"))
        .await;
    let version = document["newest_version"]["id"].as_str().unwrap();
    process
        .post(
            Some(cookie),
            &format!("/api/v1/document-versions/{version}/approve"),
            &json!({"expected_version": 1}),
        )
        .await;

    let link_code = process
        .post(Some(cookie), "/api/v1/telegram/link-codes", &json!({}))
        .await;
    let expires = Timestamp::now() + jiff::SignedDuration::from_hours(24);
    let api_token = process
        .post(
            Some(cookie),
            "/api/v1/tokens",
            &json!({"name": "Claude Code", "scope": "read", "expires_at": expires.to_string(),
                    "notice_version_confirmed": 1}),
        )
        .await;
    process
        .post(
            Some(cookie),
            "/api/v1/invitations",
            &json!({"email": "ben@example.org", "display_name": "Ben Beispiel", "role": "member"}),
        )
        .await;
    let invitation = process.mailed_token(INVITATION_LINK).await;

    let anna = test
        .create_user(
            &DisplayName::parse("Anna Muster").unwrap(),
            &Email::parse("anna@example.org").unwrap(),
        )
        .await;
    test.add_membership(organization, anna, OrganizationRole::Member)
        .await;
    process
        .post(
            None,
            "/api/v1/sign-in/requests",
            &json!({"email": "anna@example.org"}),
        )
        .await;
    let magic_link = process.mailed_token(SIGN_IN_LINK).await;

    let tokens = Tokens {
        session: cookie.to_owned(),
        magic_link,
        invitation,
        link_code: link_code["code"].as_str().unwrap().to_owned(),
        api_token: api_token["secret"].as_str().unwrap().to_owned(),
    };
    (event, upload, draft, tokens)
}

#[tokio::test]
async fn a_restart_keeps_the_session_the_records_and_the_files_and_no_table_holds_a_token() {
    support::logs::install();
    let (test, garage) = tokio::join!(TestDatabase::start(), TestGarage::start());
    let (organization, _, cookie) = test.member("testwil", OrganizationRole::Owner).await;

    let first = Process::start(&test, &garage);
    let (event, upload, draft, tokens) = fill(&first, &test, organization, &cookie).await;
    let before = shown(&first, &cookie, &event, &upload, &draft).await;
    assert_eq!(before.profile["facts"][0]["state"], "accepted");
    assert_eq!(before.draft["newest_version"]["status"], "approved");
    first.stop().await;

    let second = Process::start(&test, &garage);
    assert_eq!(
        shown(&second, &cookie, &event, &upload, &draft).await,
        before
    );

    // The final token scan: no table holds a token of any kind in plain text.
    for token in [
        &tokens.session,
        &tokens.magic_link,
        &tokens.invitation,
        &tokens.link_code,
        &tokens.api_token,
    ] {
        test.assert_no_plaintext(token).await;
    }

    // The links that the first process sent still work after the restart.
    let (status, preview) = second
        .call(
            None,
            Method::POST,
            "/api/v1/invitations/preview",
            Some(&json!({"token": tokens.invitation})),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{preview}");
    let (status, session) = second
        .call(
            None,
            Method::POST,
            "/api/v1/sign-in/magic-link",
            Some(&json!({"token": tokens.magic_link})),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{session}");

    support::logs::assert_clean(&[
        &tokens.session,
        &tokens.magic_link,
        &tokens.invitation,
        &tokens.link_code,
        &tokens.api_token,
        "anna@example.org",
        "ben@example.org",
    ]);
}
