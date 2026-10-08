//! Document uploads and downloads over HTTP, with PostgreSQL, Garage and real sessions (ADR 0009, ADR 0043).

// The helpers of this test file are not `#[test]` functions, so clippy.toml does not cover them.
#![allow(clippy::unwrap_used)]

mod support;

use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::http::{HeaderMap, Method, Request, StatusCode, header};
use serde_json::Value;
use support::SESSION_COOKIE;
use support::files::{executable, pdf, png};
use tada_adapters::clock::SystemClock;
use tada_adapters::storage::testing::TestGarage;
use tada_api::ApiState;
use tada_app::caller::{MemberCaller, OrganizationRole};
use tada_app::domain::identity::EventRole;
use tada_app::domain::ids::{EventId, OrganizationId};
use tada_app::event_members::add_event_member;
use tada_app::session::SessionAuthenticator;
use tada_store_pg::testing::TestDatabase;
use tower::ServiceExt;

/// The upload limit of the tests. It is above the default body limit of axum, 2 MB (ADR 0043).
const LIMIT: u64 = 3 * 1024 * 1024;

/// The API on PostgreSQL and Garage, with one organization, one event and its owner.
struct Api {
    router: Router,
    test: TestDatabase,
    _garage: TestGarage,
    organization: OrganizationId,
    event: EventId,
    owner: MemberCaller,
    owner_cookie: String,
}

impl Api {
    async fn start() -> Self {
        let (test, garage) = tokio::join!(TestDatabase::start(), TestGarage::start());
        let (organization, owner, owner_cookie) =
            test.member("testwil", OrganizationRole::Owner).await;
        let event = test.create_event(organization, "OPEN30").await;
        let database = Arc::new(test.database.clone());
        let clock = Arc::new(SystemClock);
        let authenticator = Arc::new(SessionAuthenticator::new(
            database.clone(),
            database,
            clock.clone(),
        ));
        let state = ApiState {
            blobs: Arc::new(garage.storage.clone()),
            upload_max_bytes: LIMIT,
            ..support::api_state(&test, authenticator, clock)
        };
        Self {
            router: tada_api::router(state, None),
            test,
            _garage: garage,
            organization,
            event,
            owner: MemberCaller::new(owner, organization, OrganizationRole::Owner),
            owner_cookie,
        }
    }

    /// A new member of the organization with the event role `role` in the event. Returns its cookie.
    async fn with_role(&self, role: EventRole) -> String {
        let (_, user, cookie) = self.test.member("testwil", OrganizationRole::Member).await;
        add_event_member(
            &self.owner,
            self.event,
            user,
            role,
            &self.test.database,
            &self.test.database,
            &SystemClock,
        )
        .await
        .unwrap();
        cookie
    }

    async fn raw(&self, request: Request<Body>) -> (StatusCode, HeaderMap, Vec<u8>) {
        let response = self.router.clone().oneshot(request).await.unwrap();
        let (parts, body) = response.into_parts();
        let bytes = axum::body::to_bytes(body, usize::MAX).await.unwrap();
        (parts.status, parts.headers, bytes.to_vec())
    }

    async fn json(&self, request: Request<Body>) -> (StatusCode, Value) {
        let (status, _, body) = self.raw(request).await;
        let value = if body.is_empty() {
            Value::Null
        } else {
            serde_json::from_slice(&body).unwrap()
        };
        (status, value)
    }

    async fn get(&self, cookie: &str, path: &str) -> (StatusCode, Value) {
        self.json(get(cookie, path)).await
    }

    /// Uploads `content` as a new document of the event.
    async fn upload(&self, cookie: &str, name: &str, content: &[u8]) -> (StatusCode, Value) {
        let path = format!("/api/v1/events/{}/documents", self.event);
        self.json(upload(cookie, &path, name, content)).await
    }

    async fn download(&self, cookie: &str, path: &str) -> (StatusCode, HeaderMap, Vec<u8>) {
        self.raw(get(cookie, path)).await
    }
}

fn get(cookie: &str, path: &str) -> Request<Body> {
    support::request(Method::GET, path)
        .header(header::COOKIE, format!("{SESSION_COOKIE}={cookie}"))
        .body(Body::empty())
        .unwrap()
}

/// A raw upload request. The file name is percent-encoded, as a client sends it.
fn upload(cookie: &str, path: &str, name: &str, content: &[u8]) -> Request<Body> {
    let encoded: String = name
        .bytes()
        .map(|byte| {
            if byte.is_ascii_alphanumeric() || byte == b'.' {
                char::from(byte).to_string()
            } else {
                format!("%{byte:02X}")
            }
        })
        .collect();
    support::request(Method::POST, path)
        .header(header::COOKIE, format!("{SESSION_COOKIE}={cookie}"))
        .header(header::CONTENT_TYPE, "application/octet-stream")
        .header("x-file-name", encoded)
        .body(Body::from(content.to_vec()))
        .unwrap()
}

fn content_path(document: &Value) -> String {
    format!(
        "/api/v1/document-versions/{}/content",
        document["newest_version"]["id"].as_str().unwrap()
    )
}

#[tokio::test]
async fn uploads_a_pdf_as_doc_001_and_reads_it_back() {
    let api = Api::start().await;
    let content = pdf("Programm Open Day");
    let (status, document) = api
        .upload(&api.owner_cookie, "Programm Übersicht.pdf", &content)
        .await;
    assert_eq!(status, StatusCode::CREATED, "{document}");
    assert_eq!(document["readable_id"], "DOC-001");
    assert_eq!(document["name"], "Programm Übersicht.pdf");
    let version = &document["newest_version"];
    assert_eq!(version["number"], 1);
    assert_eq!(version["media_type"], "application/pdf");
    assert_eq!(version["size_bytes"], content.len());
    assert_eq!(version["sha256"].as_str().unwrap().len(), 64);

    let id = document["id"].as_str().unwrap();
    let (status, read) = api
        .get(&api.owner_cookie, &format!("/api/v1/documents/{id}"))
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(read, document);

    let list = format!("/api/v1/events/{}/documents", api.event);
    let (status, page) = api
        .get(&api.owner_cookie, &format!("{list}?q=%C3%BCbersicht"))
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(page["items"][0]["id"], id);
    let (_, page) = api.get(&api.owner_cookie, &format!("{list}?q=Plan")).await;
    assert_eq!(page["items"], Value::Array(Vec::new()));

    let (status, headers, body) = api
        .download(&api.owner_cookie, &content_path(&document))
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, content);
    assert_eq!(headers[header::CONTENT_TYPE], "application/pdf");
    assert_eq!(
        headers[header::CONTENT_DISPOSITION],
        "attachment; filename=\"Programm _bersicht.pdf\"; \
         filename*=UTF-8''Programm%20%C3%9Cbersicht.pdf"
    );
    assert_eq!(headers[header::X_CONTENT_TYPE_OPTIONS], "nosniff");
    assert_eq!(
        headers[header::CONTENT_SECURITY_POLICY],
        "default-src 'none'; sandbox"
    );
    assert_eq!(headers[header::CACHE_CONTROL], "private, no-store");
}

#[tokio::test]
async fn a_new_version_keeps_the_first_one() {
    let api = Api::start().await;
    let (_, document) = api
        .upload(&api.owner_cookie, "Programm.pdf", &pdf("Version 1"))
        .await;
    let versions = format!(
        "/api/v1/documents/{}/versions",
        document["id"].as_str().unwrap()
    );
    let (status, updated) = api
        .json(upload(
            &api.owner_cookie,
            &versions,
            "Programm 2.pdf",
            &pdf("Version 2"),
        ))
        .await;
    assert_eq!(status, StatusCode::CREATED, "{updated}");
    assert_eq!(updated["readable_id"], "DOC-001");
    assert_eq!(updated["newest_version"]["number"], 2);

    let (status, list) = api.get(&api.owner_cookie, &versions).await;
    assert_eq!(status, StatusCode::OK);
    let numbers: Vec<_> = list["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|version| version["number"].as_u64().unwrap())
        .collect();
    assert_eq!(numbers, [1, 2]);
    let (_, _, first) = api
        .download(&api.owner_cookie, &content_path(&document))
        .await;
    assert_eq!(first, pdf("Version 1"));
}

#[tokio::test]
async fn rejects_a_renamed_executable_with_415() {
    let api = Api::start().await;
    let (status, problem) = api
        .upload(&api.owner_cookie, "Programm.pdf", &executable())
        .await;
    assert_eq!(status, StatusCode::UNSUPPORTED_MEDIA_TYPE);
    assert_eq!(problem["code"], "unsupported-media-type");
}

#[tokio::test]
async fn rejects_a_body_that_is_not_an_octet_stream_or_has_no_file_name() {
    let api = Api::start().await;
    let path = format!("/api/v1/events/{}/documents", api.event);
    let mut request = upload(&api.owner_cookie, &path, "Programm.pdf", &pdf("Programm"));
    request.headers_mut().insert(
        header::CONTENT_TYPE,
        "multipart/form-data; boundary=x".parse().unwrap(),
    );
    let (status, problem) = api.json(request).await;
    assert_eq!(status, StatusCode::UNSUPPORTED_MEDIA_TYPE);
    assert_eq!(problem["code"], "unsupported-media-type");

    for name in [None, Some("%C3")] {
        let mut request = upload(&api.owner_cookie, &path, "Programm.pdf", &pdf("Programm"));
        match name {
            None => request.headers_mut().remove("x-file-name"),
            Some(name) => request
                .headers_mut()
                .insert("x-file-name", name.parse().unwrap()),
        };
        let (status, problem) = api.json(request).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{name:?}");
        assert_eq!(problem["code"], "malformed-request");
    }
}

#[tokio::test]
async fn accepts_a_file_over_the_default_body_limit_of_axum() {
    let api = Api::start().await;
    let mut content = pdf("Plan");
    content.resize(5 * 1024 * 1024 / 2, b' ');
    let (status, document) = api.upload(&api.owner_cookie, "Plan.pdf", &content).await;
    assert_eq!(status, StatusCode::CREATED, "{document}");
    assert_eq!(document["newest_version"]["size_bytes"], content.len());
}

#[tokio::test]
async fn rejects_a_file_over_the_limit_with_413() {
    let api = Api::start().await;
    let mut content = pdf("Gross");
    content.resize(usize::try_from(LIMIT).unwrap() + 1, b' ');
    // Without `Content-Length`, the command counts the bytes of the stream.
    let (status, problem) = api.upload(&api.owner_cookie, "Gross.pdf", &content).await;
    assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);
    assert_eq!(problem["code"], "payload-too-large");

    // A declared size over the limit fails before the server reads the body.
    let path = format!("/api/v1/events/{}/documents", api.event);
    let mut request = upload(&api.owner_cookie, &path, "Gross.pdf", &content);
    request
        .headers_mut()
        .insert(header::CONTENT_LENGTH, content.len().into());
    let (status, problem) = api.json(request).await;
    assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);
    assert_eq!(problem["code"], "payload-too-large");

    let count: i64 = api
        .test
        .scalar("SELECT count(*) FROM document_version")
        .await;
    assert_eq!(count, 0);
}

#[tokio::test]
async fn rejects_an_upload_over_the_quota_with_quota_exceeded() {
    let api = Api::start().await;
    let content = pdf("Programm");
    let quota = content.len() as i64 + 10;
    let _: i64 = api
        .test
        .scalar(&format!(
            "WITH changed AS (UPDATE organization SET storage_quota_bytes = {quota} RETURNING 1)
             SELECT count(*) FROM changed"
        ))
        .await;
    let (status, _) = api
        .upload(&api.owner_cookie, "Programm.pdf", &content)
        .await;
    assert_eq!(status, StatusCode::CREATED);

    let (status, problem) = api.upload(&api.owner_cookie, "Plan.pdf", &content).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(problem["code"], "validation-failed");
    assert_eq!(
        problem["errors"],
        serde_json::json!([{"pointer": "/file", "code": "quota-exceeded"}])
    );
}

#[tokio::test]
async fn shows_only_pdf_and_text_inline() {
    let api = Api::start().await;
    let (_, image) = api.upload(&api.owner_cookie, "Plan.png", &png()).await;
    let (status, headers, _) = api
        .download(
            &api.owner_cookie,
            &format!("{}?disposition=inline", content_path(&image)),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(headers[header::CONTENT_TYPE], "image/png");
    assert!(
        headers[header::CONTENT_DISPOSITION]
            .to_str()
            .unwrap()
            .starts_with("attachment;")
    );
    assert_eq!(headers[header::X_CONTENT_TYPE_OPTIONS], "nosniff");

    let (_, document) = api
        .upload(&api.owner_cookie, "Programm.pdf", &pdf("Programm"))
        .await;
    let (_, headers, _) = api
        .download(
            &api.owner_cookie,
            &format!("{}?disposition=inline", content_path(&document)),
        )
        .await;
    assert!(
        headers[header::CONTENT_DISPOSITION]
            .to_str()
            .unwrap()
            .starts_with("inline;")
    );
    assert_eq!(
        headers[header::CONTENT_SECURITY_POLICY],
        "default-src 'none'; sandbox"
    );

    let (status, _, _) = api
        .download(
            &api.owner_cookie,
            &format!("{}?disposition=script", content_path(&document)),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn a_viewer_can_download_but_cannot_upload() {
    let api = Api::start().await;
    let (_, document) = api
        .upload(&api.owner_cookie, "Programm.pdf", &pdf("Programm"))
        .await;
    let viewer = api.with_role(EventRole::EventViewer).await;

    let (status, _, body) = api.download(&viewer, &content_path(&document)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, pdf("Programm"));

    let (status, problem) = api.upload(&viewer, "Plan.pdf", &pdf("Plan")).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(problem["code"], "forbidden");
    let versions = format!(
        "/api/v1/documents/{}/versions",
        document["id"].as_str().unwrap()
    );
    let (status, _) = api
        .json(upload(&viewer, &versions, "Neu.pdf", &pdf("Neu")))
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let contributor = api.with_role(EventRole::EventContributor).await;
    let (status, _) = api.upload(&contributor, "Plan.pdf", &pdf("Plan")).await;
    assert_eq!(status, StatusCode::CREATED);
}

#[tokio::test]
async fn another_organization_gets_404_on_each_document_route() {
    let api = Api::start().await;
    let (_, document) = api
        .upload(&api.owner_cookie, "Programm.pdf", &pdf("Programm"))
        .await;
    let (other, _, cookie) = api.test.member("andere", OrganizationRole::Owner).await;
    assert_ne!(other, api.organization);
    let id = document["id"].as_str().unwrap();

    for path in [
        format!("/api/v1/documents/{id}"),
        format!("/api/v1/documents/{id}/versions"),
        format!("/api/v1/events/{}/documents", api.event),
        content_path(&document),
    ] {
        let (status, problem) = api.get(&cookie, &path).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{path}");
        assert_eq!(problem["code"], "not-found");
    }
    let (status, _) = api
        .json(upload(
            &cookie,
            &format!("/api/v1/documents/{id}/versions"),
            "Fremd.pdf",
            &pdf("Fremd"),
        ))
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = api.upload(&cookie, "Fremd.pdf", &pdf("Fremd")).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}
