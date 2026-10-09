//! Invitations end to end: the mail, the preview and the acceptance (ADR 0008, ADR 0056).

// The helpers of this test file are not `#[test]` functions, so clippy.toml does not cover them.
#![allow(clippy::unwrap_used)]

mod support;

use std::ops::Deref;

use axum::body::Body;
use axum::http::{Method, Response, StatusCode, header};
use jiff::SignedDuration;
use serde_json::{Value, json};
use support::{MailApp, SESSION_COOKIE, session_cookie};
use tada::bootstrap::{BootstrapCommand, execute};
use tada_app::domain::identity::{
    DisplayName, Email, OrganizationName, OrganizationRole, OrganizationSlug,
};
use tada_app::domain::ids::{OrganizationId, UserId};

const LINK: &str = "https://tada.example.org/invitation#token=";
const SECOND: SignedDuration = SignedDuration::from_secs(1);

/// The shared test application with the steps of an invitation.
struct App(MailApp);

impl Deref for App {
    type Target = MailApp;

    fn deref(&self) -> &MailApp {
        &self.0
    }
}

impl App {
    async fn start() -> Self {
        Self(MailApp::start().await)
    }

    async fn post(&self, path: &str, body: &Value) -> (Response<Body>, Value) {
        let request = support::request(Method::POST, path)
            .header(header::CONTENT_TYPE, "application/json")
            .header(header::USER_AGENT, "Firefox");
        self.send(request.body(Body::from(body.to_string())).unwrap())
            .await
    }

    async fn get(&self, path: &str, cookie: &str) -> (Response<Body>, Value) {
        let request = support::request(Method::GET, path)
            .header(header::COOKIE, format!("{SESSION_COOKIE}={cookie}"));
        self.send(request.body(Body::empty()).unwrap()).await
    }

    async fn preview(&self, token: &str) -> (Response<Body>, Value) {
        self.post("/api/v1/invitations/preview", &json!({"token": token}))
            .await
    }

    async fn accept(&self, token: &str) -> (Response<Body>, Value) {
        self.post("/api/v1/invitations/accept", &json!({"token": token}))
            .await
    }

    /// Invites `email` into `organization` with `role` and returns the token of the mail.
    async fn invite(
        &self,
        organization: OrganizationId,
        email: &str,
        role: OrganizationRole,
    ) -> String {
        self.test
            .queue_invitation(
                organization,
                &Email::parse(email).unwrap(),
                &DisplayName::parse("Anna Muster").unwrap(),
                role,
            )
            .await;
        self.mailed_token(LINK).await
    }

    async fn user(&self, email: &str) -> UserId {
        self.test
            .create_user(
                &DisplayName::parse("Anna Alt").unwrap(),
                &Email::parse(email).unwrap(),
            )
            .await
    }

    async fn count(&self, sql: &str) -> i64 {
        self.test.scalar(sql).await
    }
}

/// A problem without the fields that differ for each request.
fn without_request_id(mut problem: Value) -> Value {
    problem.as_object_mut().unwrap().remove("request_id");
    problem.as_object_mut().unwrap().remove("instance");
    problem
}

#[tokio::test]
async fn the_first_owner_previews_and_accepts_the_bootstrap_invitation() {
    let app = App::start().await;
    let outcome = execute(
        &app.test.database,
        &support::public_url(),
        app.clock.as_ref(),
        BootstrapCommand {
            organization_slug: OrganizationSlug::parse("testwil").unwrap(),
            organization_name: OrganizationName::parse("Open Day Testwil").unwrap(),
            owner_email: Email::parse("owner@example.org").unwrap(),
            print_link: false,
        },
        None,
    )
    .await
    .unwrap();
    let tada_app::bootstrap::BootstrapOutcome::InvitationQueued {
        organization_id, ..
    } = outcome
    else {
        panic!("no invitation: {outcome:?}");
    };
    let token = app.mailed_token(LINK).await;

    let (response, preview) = app.preview(&token).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    assert!(session_cookie(&response).is_none());
    assert_eq!(
        preview,
        json!({"organization_name": "Open Day Testwil", "privacy_notice": null, "role": "owner"})
    );

    let (response, session) = app.accept(&token).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    let set_cookie = response.headers()[header::SET_COOKIE].to_str().unwrap();
    let cookie = session_cookie(&response).unwrap();
    assert_eq!(
        set_cookie,
        format!(
            "{SESSION_COOKIE}={cookie}; HttpOnly; Secure; SameSite=Lax; Path=/; Max-Age=7776000"
        )
    );
    app.test.assert_no_plaintext(&cookie).await;
    app.test.assert_no_plaintext(&token).await;

    let organization = json!({
        "organization_id": organization_id.as_uuid(),
        "name": "Open Day Testwil",
        "role": "owner",
    });
    assert_eq!(session["display_name"], "owner");
    assert_eq!(session["organization"], organization);
    assert_eq!(session["memberships"], json!([organization]));

    let (response, read) = app.get("/api/v1/session", &cookie).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(read, session);
    let (response, _) = app.get("/api/v1/events", &cookie).await;
    assert_eq!(response.status(), StatusCode::OK);

    assert_eq!(
        app.count(
            "SELECT count(*) FROM invitation WHERE status = 'accepted' AND accepted_at IS NOT NULL"
        )
        .await,
        1
    );
    assert_eq!(app.count("SELECT count(*) FROM invitation_token").await, 0);
    let audit: i64 = app
        .count(&format!(
            "SELECT count(*) FROM audit_event
             WHERE action = 'invitation.accept' AND record_kind = 'invitation'
               AND actor_kind = 'member' AND channel = 'web'
               AND organization_id = '{}'
               AND actor_id = '{}'",
            organization_id.as_uuid(),
            session["user_id"].as_str().unwrap(),
        ))
        .await;
    assert_eq!(audit, 1);

    support::logs::assert_clean(&[&token, &cookie, "owner@example.org", "Firefox"]);
    support::logs::assert_route_logged("/api/v1/invitations/accept");
}

#[tokio::test]
async fn a_preview_does_not_use_the_token() {
    let app = App::start().await;
    let testwil = app.test.create_organization("testwil").await;
    let token = app
        .invite(testwil, "anna@example.org", OrganizationRole::Admin)
        .await;

    for _ in 0..2 {
        let (response, preview) = app.preview(&token).await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            preview,
            json!({"organization_name": "testwil", "privacy_notice": null, "role": "admin"})
        );
    }
    let (response, _) = app.accept(&token).await;
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn an_invitation_works_once() {
    let app = App::start().await;
    let testwil = app.test.create_organization("testwil").await;
    let token = app
        .invite(testwil, "anna@example.org", OrganizationRole::Member)
        .await;

    let (first, _) = app.accept(&token).await;
    assert_eq!(first.status(), StatusCode::OK);
    let (second, used) = app.accept(&token).await;
    assert_eq!(second.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(used["code"], "unauthenticated");
    assert!(session_cookie(&second).is_none());
    let (preview, _) = app.preview(&token).await;
    assert_eq!(preview.status(), StatusCode::UNAUTHORIZED);

    let (_, unknown) = app.accept("unknown").await;
    assert_eq!(
        without_request_id(used),
        without_request_id(unknown),
        "no detail difference"
    );
    let (response, unknown) = app.preview("unknown").await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(unknown["code"], "unauthenticated");
}

#[tokio::test]
async fn an_invitation_expires_after_7_days() {
    let app = App::start().await;
    let testwil = app.test.create_organization("testwil").await;
    let token = app
        .invite(testwil, "anna@example.org", OrganizationRole::Member)
        .await;
    app.clock
        .advance(SignedDuration::from_hours(7 * 24) - SECOND);
    let (response, _) = app.preview(&token).await;
    assert_eq!(response.status(), StatusCode::OK);
    let (response, _) = app.accept(&token).await;
    assert_eq!(response.status(), StatusCode::OK);

    let token = app
        .invite(testwil, "ben@example.org", OrganizationRole::Member)
        .await;
    app.clock.advance(SignedDuration::from_hours(7 * 24));
    for (response, problem) in [app.preview(&token).await, app.accept(&token).await] {
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(problem["code"], "unauthenticated");
        assert!(session_cookie(&response).is_none());
    }
    assert_eq!(
        app.count("SELECT count(*) FROM email_identity WHERE email = 'ben@example.org'")
            .await,
        0
    );
}

#[tokio::test]
async fn acceptance_creates_a_missing_user_and_reuses_an_existing_one() {
    let app = App::start().await;
    let testwil = app.test.create_organization("testwil").await;
    let musterhausen = app.test.create_organization("musterhausen").await;
    let anna = app.user("anna@example.org").await;
    app.test
        .add_membership(musterhausen, anna, OrganizationRole::Member)
        .await;

    let token = app
        .invite(testwil, "anna@example.org", OrganizationRole::Member)
        .await;
    let (response, session) = app.accept(&token).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(session["user_id"], json!(anna.as_uuid()));
    assert_eq!(
        session["display_name"], "Anna Alt",
        "the user keeps the name"
    );
    assert_eq!(
        session["organization"]["organization_id"],
        json!(testwil.as_uuid()),
        "the session is in the organization of the invitation"
    );
    assert_eq!(session["memberships"].as_array().unwrap().len(), 2);

    let token = app
        .invite(testwil, "ben@example.org", OrganizationRole::Member)
        .await;
    let (response, session) = app.accept(&token).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_ne!(session["user_id"], json!(anna.as_uuid()));
    assert_eq!(session["display_name"], "Anna Muster");
    assert_eq!(app.count("SELECT count(*) FROM app_user").await, 2);
    assert_eq!(app.count("SELECT count(*) FROM email_identity").await, 2);
}

#[tokio::test]
async fn an_invitation_never_lowers_a_role() {
    let app = App::start().await;
    let testwil = app.test.create_organization("testwil").await;
    let anna = app.user("anna@example.org").await;
    app.test
        .add_membership(testwil, anna, OrganizationRole::Admin)
        .await;

    let token = app
        .invite(testwil, "anna@example.org", OrganizationRole::Member)
        .await;
    let (response, session) = app.accept(&token).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(session["organization"]["role"], "admin");

    let token = app
        .invite(testwil, "anna@example.org", OrganizationRole::Owner)
        .await;
    let (response, session) = app.accept(&token).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(session["organization"]["role"], "owner");
    assert_eq!(
        app.count("SELECT count(*) FROM organization_membership")
            .await,
        1
    );

    // The audit log names Anna and records only the raise, with role names (ADR 0061).
    let details: Value = app
        .test
        .scalar(&format!(
            "SELECT jsonb_agg(detail ORDER BY occurred_at, id) FROM audit_event
             WHERE action = 'invitation.accept' AND subject_user_id = '{}'",
            anna.as_uuid()
        ))
        .await;
    assert_eq!(
        details,
        json!([null, {"old_role": "admin", "new_role": "owner"}])
    );
}

#[tokio::test]
async fn each_invitation_response_forbids_the_referrer() {
    let app = App::start().await;
    // `App::send` checks the header of each response, also of problems.
    app.preview("unknown").await;
    app.accept("unknown").await;
    app.post("/api/v1/invitations/accept", &json!({})).await;
}
