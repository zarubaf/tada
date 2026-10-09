//! Members and invitations over HTTP, with real sessions and the mail of the worker (ADR 0056).

// The helpers of this test file are not `#[test]` functions, so clippy.toml does not cover them.
#![allow(clippy::unwrap_used)]

mod support;

use std::ops::Deref;

use axum::body::Body;
use axum::http::{Method, StatusCode, header};
use serde_json::{Value, json};
use support::{MailApp, SESSION_COOKIE};
use tada_app::clock::Clock;
use tada_app::domain::identity::{DisplayName, Email, OrganizationRole};
use tada_app::domain::ids::{OrganizationId, UserId};

const LINK: &str = "https://tada.example.org/invitation#token=";

struct App {
    app: MailApp,
    testwil: OrganizationId,
}

impl Deref for App {
    type Target = MailApp;

    fn deref(&self) -> &MailApp {
        &self.app
    }
}

/// A member with a session in Testwil.
struct Member {
    id: UserId,
    cookie: String,
}

impl App {
    async fn start() -> Self {
        let app = MailApp::start().await;
        let testwil = app.test.create_organization("testwil").await;
        Self { app, testwil }
    }

    /// A new member of `organization` with a session there, at the time of the test clock.
    async fn member_of(
        &self,
        organization: OrganizationId,
        name: &str,
        role: OrganizationRole,
    ) -> Member {
        let email = format!("{}@example.org", name.to_lowercase().replace(' ', "."));
        let id = self
            .test
            .create_user(
                &DisplayName::parse(name).unwrap(),
                &Email::parse(&email).unwrap(),
            )
            .await;
        self.test.add_membership(organization, id, role).await;
        let cookie = self
            .test
            .sign_in(id, Some(organization), self.clock.now())
            .await;
        Member { id, cookie }
    }

    async fn member(&self, name: &str, role: OrganizationRole) -> Member {
        self.member_of(self.testwil, name, role).await
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
        let (response, value) = self.send(request.unwrap()).await;
        (response.status(), value)
    }

    async fn get(&self, member: &Member, path: &str) -> (StatusCode, Value) {
        self.call(Some(&member.cookie), Method::GET, path, None)
            .await
    }

    async fn post(&self, member: &Member, path: &str, body: &Value) -> (StatusCode, Value) {
        self.call(Some(&member.cookie), Method::POST, path, Some(body))
            .await
    }

    async fn invite(&self, member: &Member, email: &str, role: &str) -> (StatusCode, Value) {
        self.post(
            member,
            "/api/v1/invitations",
            &json!({"email": email, "display_name": "Anna Muster", "role": role}),
        )
        .await
    }

    async fn remove(&self, member: &Member, user: UserId) -> (StatusCode, Value) {
        self.post(
            member,
            &format!("/api/v1/members/{}/remove", user.as_uuid()),
            &json!({"expected_version": 1}),
        )
        .await
    }

    async fn accept(&self, token: &str) -> (StatusCode, Value) {
        let body = json!({"token": token});
        self.call(
            None,
            Method::POST,
            "/api/v1/invitations/accept",
            Some(&body),
        )
        .await
    }

    async fn count(&self, sql: &str) -> i64 {
        self.test.scalar(sql).await
    }
}

#[tokio::test]
async fn an_admin_invites_an_admin_but_not_an_owner() {
    let app = App::start().await;
    let admin = app.member("Adam Admin", OrganizationRole::Admin).await;

    let (status, invitation) = app.invite(&admin, " Anna@Example.org ", "admin").await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(invitation["email"], "anna@example.org");
    assert_eq!(invitation["display_name"], "Anna Muster");
    assert_eq!(invitation["role"], "admin");
    assert_eq!(invitation["created_at"], "2030-05-18T08:00:00Z");
    assert_eq!(
        app.count("SELECT count(*) FROM outbound_intent WHERE purpose = 'invitation'")
            .await,
        1
    );

    let (status, problem) = app.invite(&admin, "berta@example.org", "owner").await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(problem["code"], "forbidden");
    let member = app.member("Mia Member", OrganizationRole::Member).await;
    let (status, _) = app.invite(&member, "berta@example.org", "member").await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(app.count("SELECT count(*) FROM invitation").await, 1);

    let (status, page) = app.get(&admin, "/api/v1/invitations").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(page["items"], json!([invitation]));
    let (status, _) = app.get(&member, "/api/v1/invitations").await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn a_retry_returns_the_invitation_and_a_member_cannot_be_invited() {
    let app = App::start().await;
    let owner = app.member("Olga Owner", OrganizationRole::Owner).await;
    let id = uuid::Uuid::now_v7();
    let body = json!({"id": id, "email": "anna@example.org", "display_name": "Anna Muster", "role": "member"});
    let (status, first) = app.post(&owner, "/api/v1/invitations", &body).await;
    assert_eq!(status, StatusCode::CREATED);
    let (status, retry) = app.post(&owner, "/api/v1/invitations", &body).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(retry, first);

    let (status, problem) = app.invite(&owner, "olga.owner@example.org", "admin").await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        problem["errors"],
        json!([{"pointer": "/email", "code": "already-member"}])
    );
}

/// A new invitation of the same address revokes the pending one, and a revoked token stops working.
#[tokio::test]
async fn a_revoked_or_replaced_invitation_cannot_be_accepted() {
    let app = App::start().await;
    let owner = app.member("Olga Owner", OrganizationRole::Owner).await;
    let (_, first) = app.invite(&owner, "anna@example.org", "member").await;
    let first_token = app.mailed_token(LINK).await;
    let (_, second) = app.invite(&owner, "anna@example.org", "admin").await;
    let second_token = app.mailed_token(LINK).await;
    assert_ne!(first_token, second_token);
    assert_eq!(
        app.count("SELECT count(*) FROM audit_event WHERE action = 'invitation.replace'")
            .await,
        1
    );

    let (status, problem) = app.accept(&first_token).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "{problem}");

    let path = format!(
        "/api/v1/invitations/{}/revoke",
        second["id"].as_str().unwrap()
    );
    let (status, _) = app
        .call(Some(&owner.cookie), Method::POST, &path, None)
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, problem) = app
        .call(Some(&owner.cookie), Method::POST, &path, None)
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(problem["code"], "not-found");
    let (status, _) = app.accept(&second_token).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    let (_, page) = app.get(&owner, "/api/v1/invitations").await;
    assert_eq!(page["items"], json!([]));
    assert_ne!(first["id"], second["id"]);
}

#[tokio::test]
async fn each_member_lists_the_members_and_only_owners_and_admins_see_the_addresses() {
    let app = App::start().await;
    let owner = app.member("Olga Owner", OrganizationRole::Owner).await;
    let anna = app.member("Anna Muster", OrganizationRole::Member).await;
    app.member("Berta Beispiel", OrganizationRole::Admin).await;

    let (status, page) = app.get(&anna, "/api/v1/members?limit=2").await;
    assert_eq!(status, StatusCode::OK);
    let items = page["items"].as_array().unwrap();
    assert_eq!(items[0]["display_name"], "Anna Muster");
    assert_eq!(items[0]["user_id"], anna.id.as_uuid().to_string());
    assert_eq!(items[0]["role"], "member");
    assert_eq!(items[0]["version"], 1);
    assert_eq!(items[1]["display_name"], "Berta Beispiel");
    assert!(items.iter().all(|item| item.get("email").is_none()));
    let cursor = page["next_cursor"].as_str().unwrap();
    let (_, rest) = app
        .get(&anna, &format!("/api/v1/members?limit=2&cursor={cursor}"))
        .await;
    assert_eq!(rest["items"][0]["display_name"], "Olga Owner");
    assert!(rest.get("next_cursor").is_none());

    let (_, page) = app.get(&owner, "/api/v1/members").await;
    assert_eq!(page["items"][0]["email"], "anna.muster@example.org");
    let (status, problem) = app.get(&owner, "/api/v1/members?cursor=x").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(problem["code"], "malformed-request");
}

/// A removal ends all sessions of the member, also a session in another organization: tada cannot
/// tell a stolen session from the member's own one. The member signs in again with a magic link.
#[tokio::test]
async fn a_removed_member_is_signed_out_everywhere() {
    let app = App::start().await;
    let owner = app.member("Olga Owner", OrganizationRole::Owner).await;
    let anna = app.member("Anna Muster", OrganizationRole::Member).await;
    let musterhausen = app.test.create_organization("musterhausen").await;
    app.test
        .add_membership(musterhausen, anna.id, OrganizationRole::Member)
        .await;
    let elsewhere = app
        .test
        .sign_in(anna.id, Some(musterhausen), app.clock.now())
        .await;
    let (status, _) = app.get(&anna, "/api/v1/members").await;
    assert_eq!(status, StatusCode::OK);

    let (status, problem) = app
        .post(
            &owner,
            &format!("/api/v1/members/{}/remove", anna.id.as_uuid()),
            &json!({"expected_version": 2}),
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(problem["code"], "record-version-conflict");
    let (status, _) = app.remove(&owner, anna.id).await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    for cookie in [&anna.cookie, &elsewhere] {
        let (status, problem) = app
            .call(Some(cookie), Method::GET, "/api/v1/session", None)
            .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(problem["code"], "unauthenticated");
    }
    assert_eq!(
        app.count(&format!(
            "SELECT count(*) FROM session WHERE user_id = '{}'",
            anna.id.as_uuid()
        ))
        .await,
        0
    );

    let (status, _) = app.remove(&owner, anna.id).await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    support::logs::assert_clean(&[
        &owner.cookie,
        &anna.cookie,
        "olga.owner@example.org",
        "anna.muster@example.org",
        "Olga Owner",
        "Anna Muster",
        &anna.id.as_uuid().to_string(),
    ]);
    support::logs::assert_route_logged("/api/v1/members/{user_id}/remove");
}

/// The attack: a stolen session outlives the removal of its member. After a new invitation it
/// would choose the organization again. The removal ends it, so it stays out.
#[tokio::test]
async fn a_stolen_session_does_not_come_back_with_a_new_invitation() {
    let app = App::start().await;
    let owner = app.member("Olga Owner", OrganizationRole::Owner).await;
    let anna = app.member("Anna Muster", OrganizationRole::Member).await;
    let stolen = anna.cookie.clone();

    let (status, _) = app.remove(&owner, anna.id).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, _) = app
        .invite(&owner, "anna.muster@example.org", "member")
        .await;
    assert_eq!(status, StatusCode::CREATED);
    let token = app.mailed_token(LINK).await;
    let (status, _) = app.accept(&token).await;
    assert_eq!(status, StatusCode::OK);

    let body = json!({"organization_id": app.testwil.as_uuid()});
    let (status, problem) = app
        .call(
            Some(&stolen),
            Method::POST,
            "/api/v1/session/organization",
            Some(&body),
        )
        .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "{problem}");
    let (status, _) = app
        .call(Some(&stolen), Method::GET, "/api/v1/members", None)
        .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

/// The attack: an admin who expects the removal invites an own second address as admin. The
/// removal revokes the pending invitations of the admin, so the link does not bring the admin back.
#[tokio::test]
async fn a_removal_revokes_the_pending_invitations_of_the_member() {
    let app = App::start().await;
    let owner = app.member("Olga Owner", OrganizationRole::Owner).await;
    let admin = app.member("Adam Admin", OrganizationRole::Admin).await;
    let (status, own) = app.invite(&admin, "adam.alt@example.org", "admin").await;
    assert_eq!(status, StatusCode::CREATED);
    let token = app.mailed_token(LINK).await;
    let (status, other) = app.invite(&owner, "berta@example.org", "member").await;
    assert_eq!(status, StatusCode::CREATED);

    let (status, _) = app.remove(&owner, admin.id).await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let (status, problem) = app.accept(&token).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "{problem}");
    let (_, page) = app.get(&owner, "/api/v1/invitations").await;
    assert_eq!(
        page["items"],
        json!([other]),
        "the invitation of the owner stays"
    );
    assert_eq!(
        app.count(&format!(
            "SELECT count(*) FROM audit_event WHERE action = 'invitation.revoke' AND record_id = '{}'",
            own["id"].as_str().unwrap()
        ))
        .await,
        1
    );
}

/// An admin cannot give the role owner, so an admin cannot revoke an owner invitation either
/// (ADR 0056). An owner can.
#[tokio::test]
async fn an_admin_cannot_revoke_an_owner_invitation() {
    let app = App::start().await;
    let owner = app.member("Olga Owner", OrganizationRole::Owner).await;
    let admin = app.member("Adam Admin", OrganizationRole::Admin).await;
    let (_, invitation) = app.invite(&owner, "otto@example.org", "owner").await;
    let path = format!(
        "/api/v1/invitations/{}/revoke",
        invitation["id"].as_str().unwrap()
    );

    let (status, problem) = app
        .call(Some(&admin.cookie), Method::POST, &path, None)
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{problem}");
    assert_eq!(problem["code"], "forbidden");
    let (_, page) = app.get(&owner, "/api/v1/invitations").await;
    assert_eq!(page["items"], json!([invitation]));

    let (status, _) = app
        .call(Some(&owner.cookie), Method::POST, &path, None)
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
}

/// The attack: an admin invites the address of a pending owner invitation again, as member. The new
/// invitation would replace, and so revoke and lower, the owner invitation. Only an owner can.
#[tokio::test]
async fn an_admin_cannot_replace_an_owner_invitation() {
    let app = App::start().await;
    let owner = app.member("Olga Owner", OrganizationRole::Owner).await;
    let admin = app.member("Adam Admin", OrganizationRole::Admin).await;
    let (_, invitation) = app.invite(&owner, "otto@example.org", "owner").await;

    for role in ["member", "admin"] {
        let (status, problem) = app.invite(&admin, "otto@example.org", role).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{problem}");
        assert_eq!(problem["code"], "forbidden");
    }
    let (_, page) = app.get(&owner, "/api/v1/invitations").await;
    assert_eq!(page["items"], json!([invitation]));

    let (status, _) = app.invite(&owner, "otto@example.org", "admin").await;
    assert_eq!(status, StatusCode::CREATED);
}

/// The remedy for a stolen session without a removal: the member signs out everywhere, also the
/// last owner, whom a removal refuses. The current session ends too.
#[tokio::test]
async fn a_member_signs_out_everywhere() {
    let app = App::start().await;
    let olga = app.member("Olga Owner", OrganizationRole::Owner).await;
    let musterhausen = app.test.create_organization("musterhausen").await;
    app.test
        .add_membership(musterhausen, olga.id, OrganizationRole::Member)
        .await;
    let stolen = app
        .test
        .sign_in(olga.id, Some(musterhausen), app.clock.now())
        .await;

    let (status, _) = app
        .call(
            Some(&olga.cookie),
            Method::POST,
            "/api/v1/session/sign-out-everywhere",
            None,
        )
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    for cookie in [&olga.cookie, &stolen] {
        let (status, _) = app
            .call(Some(cookie), Method::GET, "/api/v1/session", None)
            .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }
    assert_eq!(
        app.count("SELECT count(*) FROM organization_membership WHERE role = 'owner'")
            .await,
        1,
        "the membership stays"
    );
    assert_eq!(
        app.count(&format!(
            "SELECT count(*) FROM audit_event WHERE action = 'organization_membership.end_sessions' AND subject_user_id = '{}'",
            olga.id.as_uuid()
        ))
        .await,
        1
    );
}

/// An owner or an admin ends the sessions of another member without a removal, up to the own role:
/// only an owner ends the sessions of an owner. A member cannot end the sessions of others.
#[tokio::test]
async fn an_owner_or_admin_ends_the_sessions_of_a_member() {
    let app = App::start().await;
    let olga = app.member("Olga Owner", OrganizationRole::Owner).await;
    let otto = app.member("Otto Owner", OrganizationRole::Owner).await;
    let adam = app.member("Adam Admin", OrganizationRole::Admin).await;
    let anna = app.member("Anna Muster", OrganizationRole::Member).await;
    let end = |member: UserId| format!("/api/v1/members/{}/sessions/end", member.as_uuid());

    let (status, problem) = app
        .call(Some(&anna.cookie), Method::POST, &end(adam.id), None)
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{problem}");
    let (status, _) = app
        .call(Some(&adam.cookie), Method::POST, &end(otto.id), None)
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _) = app
        .call(Some(&otto.cookie), Method::GET, "/api/v1/session", None)
        .await;
    assert_eq!(status, StatusCode::OK, "a refusal changes nothing");

    let (status, _) = app
        .call(Some(&adam.cookie), Method::POST, &end(anna.id), None)
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, _) = app
        .call(Some(&anna.cookie), Method::GET, "/api/v1/session", None)
        .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    let (status, _) = app
        .call(Some(&olga.cookie), Method::POST, &end(otto.id), None)
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, _) = app
        .call(Some(&otto.cookie), Method::GET, "/api/v1/session", None)
        .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(
        app.count("SELECT count(*) FROM organization_membership")
            .await,
        4,
        "no membership ends"
    );

    let stranger = UserId::from_uuid(uuid::Uuid::now_v7());
    let (status, _) = app
        .call(Some(&olga.cookie), Method::POST, &end(stranger), None)
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn the_last_owner_cannot_leave_and_an_admin_cannot_remove_an_owner() {
    let app = App::start().await;
    let owner = app.member("Olga Owner", OrganizationRole::Owner).await;
    let admin = app.member("Adam Admin", OrganizationRole::Admin).await;
    let member = app.member("Mia Member", OrganizationRole::Member).await;

    let (status, problem) = app.remove(&owner, owner.id).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(problem["code"], "invalid-transition");
    let (status, problem) = app.remove(&admin, owner.id).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(problem["code"], "forbidden");
    let (status, _) = app.remove(&member, admin.id).await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    // A member can leave.
    let (status, _) = app.remove(&member, member.id).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, _) = app.remove(&admin, admin.id).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert_eq!(
        app.count("SELECT count(*) FROM organization_membership")
            .await,
        1
    );
}
