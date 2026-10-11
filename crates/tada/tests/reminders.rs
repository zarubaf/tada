//! The reminder policy of an organization and the reminder settings of an account, through the API (ADR 0076).

// The helpers of this test file are not `#[test]` functions, so clippy.toml does not cover them.
#![allow(clippy::unwrap_used)]

mod support;

use axum::body::Body;
use axum::http::{Method, StatusCode, header};
use serde_json::{Value, json};
use support::{MailApp, SESSION_COOKIE};
use tada_app::clock::Clock;
use tada_app::domain::identity::{DisplayName, Email, OrganizationRole};
use tada_app::domain::ids::{OrganizationId, UserId};

const POLICY: &str = "/api/v1/organization/pm-policy";
const SET_POLICY: &str = "/api/v1/organization/pm-policy/set";
const REMINDERS: &str = "/api/v1/account/reminders";
const SET_REMINDERS: &str = "/api/v1/account/reminders/set";

struct App {
    app: MailApp,
    testwil: OrganizationId,
}

impl App {
    async fn start() -> Self {
        let app = MailApp::start().await;
        let testwil = app.test.create_organization("testwil").await;
        Self { app, testwil }
    }

    /// A new member and the session cookie.
    async fn member(&self, name: &str, role: OrganizationRole) -> (UserId, String) {
        let email = format!("{}@example.org", name.to_lowercase().replace(' ', "."));
        let id = self
            .app
            .test
            .create_user(
                &DisplayName::parse(name).unwrap(),
                &Email::parse(&email).unwrap(),
            )
            .await;
        self.app.test.add_membership(self.testwil, id, role).await;
        let cookie = self
            .app
            .test
            .sign_in(id, Some(self.testwil), self.app.clock.now())
            .await;
        (id, cookie)
    }

    async fn call(&self, cookie: &str, path: &str, body: Option<&Value>) -> (StatusCode, Value) {
        let method = if body.is_some() {
            Method::POST
        } else {
            Method::GET
        };
        let request = support::request(method, path)
            .header(header::COOKIE, format!("{SESSION_COOKIE}={cookie}"));
        let request = match body {
            Some(body) => request
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(body.to_string())),
            None => request.body(Body::empty()),
        };
        let (response, value) = self.app.send(request.unwrap()).await;
        (response.status(), value)
    }

    async fn audits(&self, action: &str) -> i64 {
        self.app
            .test
            .scalar(&format!(
                "SELECT count(*) FROM audit_event WHERE action = '{action}' AND detail IS NULL"
            ))
            .await
    }
}

fn policy(local_time: &str, time_zone: &str, expected_version: i64) -> Value {
    json!({
        "local_time": local_time, "time_zone": time_zone, "weekdays": 0b001_1111,
        "quiet_start": "21:00", "quiet_end": "07:00", "cooldown_hours": 24,
        "follow_up_working_days": 3, "look_ahead_working_days": 0,
        "paused": false, "ai_enabled": true, "expected_version": expected_version,
    })
}

#[tokio::test]
async fn an_owner_or_admin_sets_the_policy_and_its_schedule() {
    let app = App::start().await;
    let (_, owner) = app.member("Olga Owner", OrganizationRole::Owner).await;
    let (_, admin) = app.member("Adam Admin", OrganizationRole::Admin).await;
    let (_, member) = app.member("Mia Member", OrganizationRole::Member).await;

    // The defaults of an organization that changed nothing.
    let (status, read) = app.call(&admin, POLICY, None).await;
    assert_eq!(status, StatusCode::OK, "{read}");
    assert_eq!(
        read,
        json!({
            "local_time": "08:30:00", "time_zone": "Europe/Zurich", "weekdays": 31,
            "quiet_start": "20:00:00", "quiet_end": "08:00:00", "cooldown_hours": 20,
            "follow_up_working_days": 2, "look_ahead_working_days": 2,
            "paused": false, "ai_enabled": false, "version": 1,
        })
    );

    // A member neither reads nor changes the policy.
    let (status, problem) = app.call(&member, POLICY, None).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(problem["code"], "forbidden");
    let (status, problem) = app
        .call(
            &member,
            SET_POLICY,
            Some(&policy("07:45", "Europe/Berlin", 1)),
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(problem["code"], "forbidden");

    let (status, set) = app
        .call(
            &admin,
            SET_POLICY,
            Some(&policy("07:45", "Europe/Berlin", 1)),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{set}");
    assert_eq!(set["local_time"], "07:45:00");
    assert_eq!(set["time_zone"], "Europe/Berlin");
    assert_eq!(set["version"], 2);
    let (_, read) = app.call(&owner, POLICY, None).await;
    assert_eq!(read, set);

    // A stale version conflicts and changes nothing.
    let (status, problem) = app
        .call(
            &owner,
            SET_POLICY,
            Some(&policy("06:00", "Europe/Zurich", 1)),
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(problem["code"], "record-version-conflict");
    let (status, _) = app
        .call(
            &owner,
            SET_POLICY,
            Some(&policy("06:00", "Europe/Zurich", 2)),
        )
        .await;
    assert_eq!(status, StatusCode::OK);

    // The schedule of the daily check follows the policy.
    let schedule: String = app
        .app
        .test
        .scalar(&format!(
            "SELECT local_time::text || ' ' || time_zone || ' ' || missed_run_limit FROM schedule
             WHERE organization_id = '{}' AND kind = 'pm-check'",
            app.testwil
        ))
        .await;
    assert_eq!(schedule, "06:00:00 Europe/Zurich 1");
    assert_eq!(app.audits("organization.set_pm_policy").await, 2);
}

#[tokio::test]
async fn invalid_policy_values_are_field_errors() {
    let app = App::start().await;
    let (_, owner) = app.member("Olga Owner", OrganizationRole::Owner).await;
    let mut body = policy("07:45", "Mars/Olympus", 1);
    body["cooldown_hours"] = json!(0);
    body["look_ahead_working_days"] = json!(11);
    let (status, problem) = app.call(&owner, SET_POLICY, Some(&body)).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{problem}");
    assert_eq!(problem["code"], "validation-failed");
    let errors: Vec<(&str, &str)> = problem["errors"]
        .as_array()
        .unwrap()
        .iter()
        .map(|error| {
            (
                error["pointer"].as_str().unwrap(),
                error["code"].as_str().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        errors,
        [
            ("/time_zone", "invalid-time-zone"),
            ("/cooldown_hours", "out-of-range"),
            ("/look_ahead_working_days", "out-of-range"),
        ]
    );
    let (_, read) = app.call(&owner, POLICY, None).await;
    assert_eq!(read["version"], 1);
    assert_eq!(app.audits("organization.set_pm_policy").await, 0);
    let schedules: i64 = app.app.test.scalar("SELECT count(*) FROM schedule").await;
    assert_eq!(schedules, 0);
}

#[tokio::test]
async fn a_member_opts_in_with_a_confirmed_telegram_link_only() {
    let app = App::start().await;
    let (user, member) = app.member("Mia Member", OrganizationRole::Member).await;

    let (status, read) = app.call(&member, REMINDERS, None).await;
    assert_eq!(status, StatusCode::OK, "{read}");
    assert_eq!(
        read,
        json!({"opted_in_at": null, "time_zone": "Europe/Zurich", "telegram_linked": false})
    );

    // Without a link, the opt-in fails and changes nothing.
    let opt_in = json!({"opt_in": true, "time_zone": "America/New_York"});
    let (status, problem) = app.call(&member, SET_REMINDERS, Some(&opt_in)).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{problem}");
    assert_eq!(problem["code"], "validation-failed");
    assert_eq!(problem["errors"][0]["pointer"], "/opt_in");
    assert_eq!(problem["errors"][0]["code"], "telegram-not-linked");
    let (_, unchanged) = app.call(&member, REMINDERS, None).await;
    assert_eq!(unchanged, read);

    // A time zone that is not an IANA name is a field error.
    let wrong = json!({"opt_in": false, "time_zone": "UTC+2"});
    let (status, problem) = app.call(&member, SET_REMINDERS, Some(&wrong)).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{problem}");
    assert_eq!(problem["errors"][0]["pointer"], "/time_zone");
    assert_eq!(problem["errors"][0]["code"], "invalid-time-zone");

    // With a confirmed link, the opt-in works, and a second opt-in keeps the first time.
    app.app
        .test
        .execute(&[&format!(
            "INSERT INTO telegram_identity (telegram_user_id, user_id, linked_at)
             VALUES (424242, '{user}', now())"
        )])
        .await;
    let (status, first) = app.call(&member, SET_REMINDERS, Some(&opt_in)).await;
    assert_eq!(status, StatusCode::OK, "{first}");
    assert_eq!(first["time_zone"], "America/New_York");
    assert_eq!(first["telegram_linked"], true);
    assert!(first["opted_in_at"].is_string());
    app.app.clock.advance(jiff::SignedDuration::from_mins(5));
    let (_, again) = app.call(&member, SET_REMINDERS, Some(&opt_in)).await;
    assert_eq!(again, first);

    // An opt-out clears the time.
    let opt_out = json!({"opt_in": false, "time_zone": "America/New_York"});
    let (status, out) = app.call(&member, SET_REMINDERS, Some(&opt_out)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(out["opted_in_at"], Value::Null);
    assert_eq!(app.audits("account.set_reminders").await, 3);
    // The settings belong to the user, so the events name no organization.
    let scoped: i64 = app
        .app
        .test
        .scalar(
            "SELECT count(*) FROM audit_event
             WHERE action = 'account.set_reminders' AND organization_id IS NOT NULL",
        )
        .await;
    assert_eq!(scoped, 0);
}
