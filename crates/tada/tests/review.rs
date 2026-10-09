//! The event profile, the field catalog and the review of changesets over HTTP, with real sessions (ADRs 0049, 0050).

// The helpers of this test file are not `#[test]` functions, so clippy.toml does not cover them.
#![allow(clippy::unwrap_used)]

mod support;

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Method, StatusCode, header};
use jiff::{SignedDuration, Timestamp};
use serde_json::{Value, json};
use support::{SESSION_COOKIE, TestClock};
use tada_app::caller::{MemberCaller, OrganizationRole};
use tada_app::clock::Clock;
use tada_app::domain::ids::{OrganizationId, UserId};
use tada_app::proposals::{NewChangeset, ProposeStores, create_changeset};
use tada_store_pg::testing::TestDatabase;
use uuid::Uuid;

const SOURCE: &str = "Das Open Day findet im Mai oder Juni 2030 statt. Wir erwarten ungefähr 20000 Leute. \
                      Der Ort ist noch offen. Wer klärt die Bewilligung?";

struct Api {
    router: axum::Router,
    test: TestDatabase,
    clock: Arc<TestClock>,
}

impl Api {
    async fn start() -> Self {
        let test = TestDatabase::start().await;
        // Whole seconds: the database keeps microseconds, so a stored time equals the time of the clock.
        let start = Timestamp::from_second(Timestamp::now().as_second()).unwrap();
        let clock = Arc::new(TestClock::new(start));
        let router = support::session_router(&test, clock.clone());
        Self {
            router,
            test,
            clock,
        }
    }

    /// A new member of the organization `slug`, signed in at the time of the clock.
    async fn member(&self, slug: &str, role: OrganizationRole) -> Member {
        let (organization, user, _) = self.test.member(slug, role).await;
        let cookie = self.sign_in(user, organization).await;
        Member {
            organization,
            user,
            cookie,
        }
    }

    async fn sign_in(&self, user: UserId, organization: OrganizationId) -> String {
        self.test
            .sign_in(user, Some(organization), self.clock.now())
            .await
    }

    async fn send(
        &self,
        cookie: &str,
        method: Method,
        path: &str,
        body: Option<&Value>,
    ) -> (StatusCode, Value) {
        let request = support::request(method, path)
            .header(header::COOKIE, format!("{SESSION_COOKIE}={cookie}"));
        let request = match body {
            Some(body) => request
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(body.to_string())),
            None => request.body(Body::empty()),
        };
        let (response, value) = support::send(&self.router, request.unwrap()).await;
        (response.status(), value)
    }

    async fn get(&self, cookie: &str, path: &str) -> (StatusCode, Value) {
        self.send(cookie, Method::GET, path, None).await
    }

    async fn post(&self, cookie: &str, path: &str, body: &Value) -> (StatusCode, Value) {
        self.send(cookie, Method::POST, path, Some(body)).await
    }

    /// The member creates an event, becomes its event manager, and gets its ID.
    async fn create_event(&self, cookie: &str, key: &str) -> String {
        let (status, event) = self
            .post(
                cookie,
                "/api/v1/events",
                &json!({"key": key, "name": "Open Day Testwil"}),
            )
            .await;
        assert_eq!(status, StatusCode::CREATED);
        event["id"].as_str().unwrap().to_owned()
    }

    async fn add_to_event(&self, manager: &str, event: &str, member: &Member, role: &str) {
        let (status, _) = self
            .post(
                manager,
                &format!("/api/v1/events/{event}/memberships"),
                &json!({"user_id": member.user.as_uuid(), "event_role": role}),
            )
            .await;
        assert_eq!(status, StatusCode::CREATED);
    }

    /// The ID of the field `key` in the field catalog of the event.
    async fn field(&self, cookie: &str, event: &str, key: &str) -> String {
        let (status, fields) = self
            .get(cookie, &format!("/api/v1/events/{event}/fields"))
            .await;
        assert_eq!(status, StatusCode::OK);
        fields["items"]
            .as_array()
            .unwrap()
            .iter()
            .find(|field| field["key"] == key)
            .unwrap()["id"]
            .as_str()
            .unwrap()
            .to_owned()
    }

    /// A changeset with one proposal that sets the venue of the event to an assumption.
    async fn venue_changeset(&self, cookie: &str, event: &str, venue: &str) -> (StatusCode, Value) {
        let field = self.field(cookie, event, "venue").await;
        self.post(
            cookie,
            &format!("/api/v1/events/{event}/changesets"),
            &venue_body(event, &field, venue),
        )
        .await
    }
}

/// The body of a changeset with one proposal that sets the venue field `field` to an assumption.
fn venue_body(event: &str, field: &str, venue: &str) -> Value {
    json!({
        "source_text": SOURCE,
        "proposals": [proposal(
            json!({"kind": "set-fact", "event_id": event, "field_id": field, "state": {
                "state": "assumption", "value": {"type": "text", "text": venue},
            }}),
            "Der Ort ist noch offen.",
        )],
    })
}

struct Member {
    organization: OrganizationId,
    user: UserId,
    cookie: String,
}

/// A proposal with a new ID, whose evidence is the passage `quote` of `SOURCE`.
fn proposal(operation: Value, quote: &str) -> Value {
    let start = SOURCE.find(quote).unwrap();
    let start = SOURCE[..start].chars().count();
    let end = start + quote.chars().count();
    json!({
        "id": Uuid::now_v7(),
        "operation": operation,
        "evidence": [{"start": start, "end": end, "quote": quote}],
        "reason": "The member wrote it.",
    })
}

fn ids(page: &Value) -> Vec<String> {
    page["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item["id"].as_str().unwrap().to_owned())
        .collect()
}

mod facts {
    use super::*;

    #[tokio::test]
    async fn an_applied_changeset_fills_the_event_profile_and_an_unknown_stays_unknown() {
        let api = Api::start().await;
        let owner = api.member("testwil", OrganizationRole::Owner).await;
        let event = api.create_event(&owner.cookie, "TEST30").await;
        let (status, fields) = api
            .get(&owner.cookie, &format!("/api/v1/events/{event}/fields"))
            .await;
        assert_eq!(status, StatusCode::OK);
        let window = fields["items"]
            .as_array()
            .unwrap()
            .iter()
            .find(|field| field["key"] == "date_window")
            .unwrap();
        assert_eq!(window["value_type"], json!({"type": "date-window"}));
        assert_eq!(
            window["label"],
            json!({"kind": "message", "id": "field-date_window"})
        );
        assert_eq!(
            window["value_schema"]["properties"]["type"],
            json!({"const": "date-window"})
        );

        let date_window = api.field(&owner.cookie, &event, "date_window").await;
        let visitors = api.field(&owner.cookie, &event, "visitor_estimate").await;
        let venue = api.field(&owner.cookie, &event, "venue").await;
        let changeset_id = Uuid::now_v7();
        let body = json!({
            "id": changeset_id,
            "source_text": SOURCE,
            "proposals": [
                proposal(
                    json!({"kind": "set-fact", "event_id": event, "field_id": date_window, "state": {
                        "state": "accepted",
                        "value": {"type": "date-window", "start": "2030-05-01", "end": "2030-06-30", "granularity": "month"},
                    }}),
                    "im Mai oder Juni 2030",
                ),
                proposal(
                    json!({"kind": "set-fact", "event_id": event, "field_id": visitors, "state": {
                        "state": "assumption", "approximate": true,
                        "value": {"type": "quantity", "min": "20000", "max": "20000"},
                    }}),
                    "ungefähr 20000 Leute",
                ),
                proposal(
                    json!({"kind": "set-fact", "event_id": event, "field_id": venue, "state": {"state": "unknown"}}),
                    "Der Ort ist noch offen.",
                ),
                proposal(
                    json!({"kind": "create-open-question", "id": Uuid::now_v7(), "event_id": event,
                           "text": "Wer klärt die Bewilligung?", "owner": owner.user.as_uuid()}),
                    "Wer klärt die Bewilligung?",
                ),
            ],
        });
        let path = format!("/api/v1/events/{event}/changesets");
        let (status, created) = api.post(&owner.cookie, &path, &body).await;
        assert_eq!(status, StatusCode::CREATED, "{created}");
        assert_eq!(created["id"], changeset_id.to_string());
        assert_eq!(created["proposal_ids"].as_array().unwrap().len(), 4);
        let (status, again) = api.post(&owner.cookie, &path, &body).await;
        assert_eq!(status, StatusCode::OK, "a retry finds the changeset");
        assert_eq!(again, created);

        let (status, profile) = api
            .get(&owner.cookie, &format!("/api/v1/events/{event}/profile"))
            .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(profile["facts"], json!([]));
        assert_eq!(profile["proposals"].as_array().unwrap().len(), 3);

        let (status, changeset) = api
            .get(&owner.cookie, &format!("/api/v1/changesets/{changeset_id}"))
            .await;
        assert_eq!(status, StatusCode::OK);
        let window_proposal = changeset["proposals"]
            .as_array()
            .unwrap()
            .iter()
            .find(|proposal| proposal["operation"]["field_id"] == date_window.as_str())
            .unwrap();
        assert_eq!(window_proposal["status"], "open");
        assert_eq!(window_proposal["stale"], false);
        assert_eq!(
            window_proposal["evidence"][0]["excerpt"],
            json!({
                "before": "Das Open Day findet ",
                "quote": "im Mai oder Juni 2030",
                "after": " statt. Wir erwarten ungefähr 20000 Leute. Der Ort ist noch offen. Wer klärt die Bewilligung?",
            })
        );
        assert!(window_proposal.get("current").is_none());

        let selected: Vec<Value> = changeset["proposals"]
            .as_array()
            .unwrap()
            .iter()
            .map(|proposal| proposal["id"].clone())
            .collect();
        let (status, applied) = api
            .post(
                &owner.cookie,
                &format!("/api/v1/changesets/{changeset_id}/apply"),
                &json!({"selected": selected}),
            )
            .await;
        assert_eq!(status, StatusCode::OK, "{applied}");
        assert_eq!(applied["open_questions"][0]["local_id"], "QST-001");

        let (status, profile) = api
            .get(&owner.cookie, &format!("/api/v1/events/{event}/profile"))
            .await;
        assert_eq!(status, StatusCode::OK);
        let fact = |key: &str| {
            profile["facts"]
                .as_array()
                .unwrap()
                .iter()
                .find(|fact| fact["field_key"] == key)
                .unwrap()
                .clone()
        };
        let window = fact("date_window");
        assert_eq!(window["state"], "accepted");
        assert_eq!(window["version"], 1);
        assert_eq!(
            window["value"],
            json!({"type": "date-window", "start": "2030-05-01", "end": "2030-06-30", "granularity": "month"})
        );
        assert_eq!(window["approximate"], false);
        assert_eq!(
            window["evidence"][0]["passage"]["quote"],
            "im Mai oder Juni 2030"
        );
        let visitors = fact("visitor_estimate");
        assert_eq!(visitors["state"], "assumption");
        assert_eq!(
            visitors["value"],
            json!({"type": "quantity", "min": "20000", "max": "20000"})
        );
        assert_eq!(visitors["approximate"], true);
        let venue = fact("venue");
        assert_eq!(venue["state"], "unknown");
        assert!(
            venue.get("value").is_none(),
            "an unknown has no value, not null"
        );
        assert_eq!(profile["proposals"], json!([]));
        assert_eq!(profile["open_questions"][0]["local_id"], "QST-001");
        assert_eq!(
            profile["open_questions"][0]["text"],
            "Wer klärt die Bewilligung?"
        );

        let (status, inbox) = api
            .get(&owner.cookie, "/api/v1/changesets?status=open")
            .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(inbox["items"], json!([]));
    }
}

mod review {
    use super::*;

    #[tokio::test]
    async fn an_apply_with_a_stale_expected_version_conflicts_and_the_review_shows_it() {
        let api = Api::start().await;
        let owner = api.member("testwil", OrganizationRole::Owner).await;
        let event = api.create_event(&owner.cookie, "TEST30").await;
        let (_, first) = api
            .venue_changeset(&owner.cookie, &event, "Flugfeld Testwil")
            .await;
        let (_, second) = api
            .venue_changeset(&owner.cookie, &event, "Halle Testwil")
            .await;

        let apply = |changeset: &Value| {
            let path = format!(
                "/api/v1/changesets/{}/apply",
                changeset["id"].as_str().unwrap()
            );
            let body = json!({"selected": changeset["proposal_ids"]});
            (path, body)
        };
        let (path, body) = apply(&first);
        let (status, _) = api.post(&owner.cookie, &path, &body).await;
        assert_eq!(status, StatusCode::OK);
        let (path, body) = apply(&second);
        let (status, problem) = api.post(&owner.cookie, &path, &body).await;
        assert_eq!(status, StatusCode::CONFLICT);
        assert_eq!(problem["code"], "record-version-conflict");

        let id = second["id"].as_str().unwrap();
        let (status, changeset) = api
            .get(&owner.cookie, &format!("/api/v1/changesets/{id}"))
            .await;
        assert_eq!(status, StatusCode::OK);
        let proposal = &changeset["proposals"][0];
        assert_eq!(proposal["status"], "conflict");
        assert_eq!(proposal["conflict_reason"], "fact-changed");
        assert_eq!(proposal["stale"], false);
        assert_eq!(proposal["current"]["version"], 1);
        assert_eq!(
            proposal["current"]["value"],
            json!({"type": "text", "text": "Flugfeld Testwil"})
        );
        assert_eq!(proposal["current"]["approximate"], false);
    }

    #[tokio::test]
    async fn a_value_goes_back_as_an_edit_in_the_shape_that_the_review_reads() {
        let api = Api::start().await;
        let owner = api.member("testwil", OrganizationRole::Owner).await;
        let event = api.create_event(&owner.cookie, "TEST30").await;
        let (_, created) = api.venue_changeset(&owner.cookie, &event, "Flugfeld").await;
        let id = created["id"].as_str().unwrap();
        let (_, changeset) = api
            .get(&owner.cookie, &format!("/api/v1/changesets/{id}"))
            .await;
        let proposal = &changeset["proposals"][0];
        let operation = &proposal["operation"];
        assert_eq!(operation["kind"], "set-fact");
        assert_eq!(operation["state"], "assumption");

        // The reviewer confirms the value as read: the state, the value and the mark "approximate".
        let state = json!({
            "state": "accepted",
            "value": operation["value"],
            "approximate": operation["approximate"],
        });
        let (status, applied) = api
            .post(
                &owner.cookie,
                &format!("/api/v1/changesets/{id}/apply"),
                &json!({"selected": [proposal["id"]], "edits": [{"proposal_id": proposal["id"], "state": state}]}),
            )
            .await;
        assert_eq!(status, StatusCode::OK, "{applied}");
        assert_eq!(applied["proposals"][0]["status"], "accepted-with-edit");
        let (_, profile) = api
            .get(&owner.cookie, &format!("/api/v1/events/{event}/profile"))
            .await;
        assert_eq!(profile["facts"][0]["state"], "accepted");
        assert_eq!(profile["facts"][0]["value"], operation["value"]);
    }

    #[tokio::test]
    async fn a_proposal_older_than_fourteen_days_is_stale() {
        let api = Api::start().await;
        let owner = api.member("testwil", OrganizationRole::Owner).await;
        let event = api.create_event(&owner.cookie, "TEST30").await;
        let (_, changeset) = api.venue_changeset(&owner.cookie, &event, "Flugfeld").await;
        let id = changeset["id"].as_str().unwrap();

        api.clock.advance(SignedDuration::from_hours(14 * 24));
        // The session has the same idle timeout, so the owner signs in again.
        let cookie = api.sign_in(owner.user, owner.organization).await;
        let (_, review) = api.get(&cookie, &format!("/api/v1/changesets/{id}")).await;
        assert_eq!(review["proposals"][0]["stale"], false, "exactly 14 days");

        api.clock.advance(SignedDuration::from_secs(1));
        let (_, review) = api.get(&cookie, &format!("/api/v1/changesets/{id}")).await;
        assert_eq!(review["proposals"][0]["stale"], true);
        let (_, page) = api
            .get(
                &cookie,
                &format!("/api/v1/events/{event}/changesets?status=open"),
            )
            .await;
        assert_eq!(page["items"][0]["id"], id);
        assert_eq!(page["items"][0]["stale"], true);
        assert_eq!(page["items"][0]["open_proposals"], 1);
    }

    #[tokio::test]
    async fn a_viewer_cannot_create_a_changeset_and_a_contributor_cannot_review_it() {
        let api = Api::start().await;
        let owner = api.member("testwil", OrganizationRole::Owner).await;
        let viewer = api.member("testwil", OrganizationRole::Member).await;
        let contributor = api.member("testwil", OrganizationRole::Member).await;
        let event = api.create_event(&owner.cookie, "TEST30").await;
        api.add_to_event(&owner.cookie, &event, &viewer, "event-viewer")
            .await;
        api.add_to_event(&owner.cookie, &event, &contributor, "event-contributor")
            .await;

        let (status, problem) = api
            .venue_changeset(&viewer.cookie, &event, "Flugfeld")
            .await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(problem["code"], "forbidden");

        let (status, changeset) = api
            .venue_changeset(&contributor.cookie, &event, "Flugfeld")
            .await;
        assert_eq!(status, StatusCode::CREATED);
        let id = changeset["id"].as_str().unwrap();
        let (status, _) = api
            .get(&contributor.cookie, &format!("/api/v1/changesets/{id}"))
            .await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        // The same rule hides the changeset in the Review Inbox instead of refusing the list.
        let (status, inbox) = api
            .get(&contributor.cookie, "/api/v1/changesets?status=open")
            .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(inbox["items"], json!([]));
        // The list of the event shows only what the routing gives the contributor (ADR 0067): nothing here.
        let (status, page) = api
            .get(
                &contributor.cookie,
                &format!("/api/v1/events/{event}/changesets?status=open"),
            )
            .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(page["items"], json!([]));
        let (status, problem) = api
            .post(
                &contributor.cookie,
                &format!("/api/v1/changesets/{id}/reject"),
                &json!({"proposal_ids": changeset["proposal_ids"]}),
            )
            .await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(problem["code"], "forbidden");

        let (status, rejected) = api
            .post(
                &owner.cookie,
                &format!("/api/v1/changesets/{id}/reject"),
                &json!({"proposal_ids": changeset["proposal_ids"]}),
            )
            .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(rejected["proposals"][0]["status"], "rejected");
        let (_, review) = api
            .get(&owner.cookie, &format!("/api/v1/changesets/{id}"))
            .await;
        assert_eq!(review["proposals"][0]["status"], "rejected");
    }

    #[tokio::test]
    async fn a_member_of_another_organization_finds_nothing() {
        let api = Api::start().await;
        let owner = api.member("testwil", OrganizationRole::Owner).await;
        let stranger = api.member("musterhausen", OrganizationRole::Owner).await;
        let event = api.create_event(&owner.cookie, "TEST30").await;
        let (_, changeset) = api.venue_changeset(&owner.cookie, &event, "Flugfeld").await;
        let id = changeset["id"].as_str().unwrap();
        let selection = json!({"selected": changeset["proposal_ids"]});
        let rejection = json!({"proposal_ids": changeset["proposal_ids"]});

        for path in [
            format!("/api/v1/events/{event}/profile"),
            format!("/api/v1/events/{event}/fields"),
            format!("/api/v1/events/{event}/changesets?status=open"),
            format!("/api/v1/changesets/{id}"),
        ] {
            let (status, problem) = api.get(&stranger.cookie, &path).await;
            assert_eq!(status, StatusCode::NOT_FOUND, "{path}");
            assert_eq!(problem["code"], "not-found", "{path}");
        }
        let venue = api.field(&owner.cookie, &event, "venue").await;
        let (status, _) = api
            .post(
                &stranger.cookie,
                &format!("/api/v1/events/{event}/changesets"),
                &venue_body(&event, &venue, "Flugfeld"),
            )
            .await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        for (path, body) in [
            (format!("/api/v1/changesets/{id}/apply"), &selection),
            (format!("/api/v1/changesets/{id}/reject"), &rejection),
        ] {
            let (status, problem) = api.post(&stranger.cookie, &path, body).await;
            assert_eq!(status, StatusCode::NOT_FOUND, "{path}");
            assert_eq!(problem["code"], "not-found", "{path}");
        }
        let (status, inbox) = api
            .get(&stranger.cookie, "/api/v1/changesets?status=open")
            .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(inbox["items"], json!([]));
        let (_, inbox) = api
            .get(&owner.cookie, "/api/v1/changesets?status=open")
            .await;
        assert_eq!(ids(&inbox), [id]);
    }

    #[tokio::test]
    async fn the_inbox_shows_an_organization_changeset_to_an_owner_and_nothing_to_a_contributor() {
        let api = Api::start().await;
        let owner = api.member("testwil", OrganizationRole::Owner).await;
        let contributor = api.member("testwil", OrganizationRole::Member).await;
        let event = api.create_event(&owner.cookie, "TEST30").await;
        api.add_to_event(&owner.cookie, &event, &contributor, "event-contributor")
            .await;
        let (_, own) = api
            .venue_changeset(&contributor.cookie, &event, "Flugfeld")
            .await;
        api.clock.advance(SignedDuration::from_secs(1));

        // The web has no form for a new event, so the changeset of the organization comes from the command.
        let new_event = Uuid::now_v7();
        let input: NewChangeset = serde_json::from_value(json!({
            "source_text": "Fly-in Musterhausen 2031",
            "proposals": [{
                "id": Uuid::now_v7(),
                "operation": {"kind": "create-event", "id": new_event, "key": "FLY31", "name": "Fly-in Musterhausen"},
                "evidence": [{"start": 0, "end": 19, "quote": "Fly-in Musterhausen"}],
                "reason": "The member names a new event.",
            }],
        }))
        .unwrap();
        let database = &api.test.database;
        let stores = ProposeStores {
            identity: database,
            facts: database,
            proposals: database,
            sources: database,
            documents: database,
            workstreams: database,
            parties: database,
            work: database,
        };
        let caller = MemberCaller::new(owner.user, owner.organization, OrganizationRole::Owner);
        create_changeset(&caller, input, stores, api.clock.as_ref())
            .await
            .unwrap();

        let (status, inbox) = api
            .get(&owner.cookie, "/api/v1/changesets?status=open&limit=1")
            .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(ids(&inbox), [own["id"].as_str().unwrap()]);
        let cursor = inbox["next_cursor"].as_str().unwrap();
        let (_, next) = api
            .get(
                &owner.cookie,
                &format!("/api/v1/changesets?status=open&limit=1&cursor={cursor}"),
            )
            .await;
        let organization_changeset = &next["items"][0];
        assert!(organization_changeset.get("event_id").is_none());
        assert_eq!(organization_changeset["author"]["kind"], "member");
        assert!(next.get("next_cursor").is_none());

        let (status, inbox) = api
            .get(&contributor.cookie, "/api/v1/changesets?status=open")
            .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(inbox["items"], json!([]));
    }
}

mod work {
    use super::*;

    #[tokio::test]
    async fn a_proposal_through_http_creates_a_conditional_commitment_that_stays_conditional() {
        let api = Api::start().await;
        let owner = api.member("testwil", OrganizationRole::Owner).await;
        let contributor = api.member("testwil", OrganizationRole::Member).await;
        let event = api.create_event(&owner.cookie, "TEST30").await;
        api.add_to_event(&owner.cookie, &event, &contributor, "event-contributor")
            .await;

        // The contributor proposes a new supplier and its conditional commitment.
        let (person, commitment) = (Uuid::now_v7(), Uuid::now_v7());
        let mut new_person = proposal(
            json!({"kind": "create-person", "id": person, "name": "Moritz Muster"}),
            "Wer klärt die Bewilligung?",
        );
        let mut promise = proposal(
            json!({
                "kind": "create-commitment", "id": commitment, "event_id": event,
                "text": "Klärt die Bewilligung", "promisor": {"person": person},
                "owner": contributor.user.as_uuid(), "due_date": "2030-04-30",
                "condition": "wenn der Ort feststeht",
            }),
            "Der Ort ist noch offen.",
        );
        new_person["id"] = json!(Uuid::now_v7());
        promise["depends_on"] = json!([new_person["id"]]);
        let (status, changeset) = api
            .post(
                &contributor.cookie,
                &format!("/api/v1/events/{event}/changesets"),
                &json!({"source_text": SOURCE, "proposals": [new_person, promise]}),
            )
            .await;
        assert_eq!(status, StatusCode::CREATED, "{changeset}");
        let changeset = changeset["id"].as_str().unwrap();

        let (status, review) = api
            .get(&owner.cookie, &format!("/api/v1/changesets/{changeset}"))
            .await;
        assert_eq!(status, StatusCode::OK);
        let operation = review["proposals"]
            .as_array()
            .unwrap()
            .iter()
            .map(|proposal| &proposal["operation"])
            .find(|operation| operation["kind"] == "create-commitment")
            .unwrap();
        assert_eq!(operation["promisor"], json!({"person": person}));
        assert_eq!(operation["due_date"], "2030-04-30");

        let (status, result) = api
            .post(
                &owner.cookie,
                &format!("/api/v1/changesets/{changeset}/apply"),
                &json!({"selected": [promise["id"]]}),
            )
            .await;
        assert_eq!(status, StatusCode::OK, "{result}");
        assert_eq!(
            result["commitments"],
            json!([{"id": commitment, "local_id": "COM-001"}])
        );
        assert_eq!(
            result["persons"],
            json!([{"id": person, "local_id": "PER-001"}])
        );
        assert_eq!(result["actions"], json!([]));
        assert_eq!(result["institutions"], json!([]));

        let path = format!("/api/v1/events/{event}/commitments/{commitment}");
        let (status, read) = api.get(&contributor.cookie, &path).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(read["status"], "conditional");
        assert_eq!(read["condition"], "wenn der Ort feststeht");
        assert_eq!(read["evidence"][0]["quote"], "Der Ort ist noch offen.");
        assert_eq!(read["evidence"][0]["record_version"], 1);

        // The new person shows its evidence to a reader of the source (ADR 0069).
        let (status, read_person) = api
            .get(&contributor.cookie, &format!("/api/v1/persons/{person}"))
            .await;
        assert_eq!(status, StatusCode::OK, "{read_person}");
        assert_eq!(
            read_person["evidence"][0]["quote"],
            "Wer klärt die Bewilligung?"
        );
        // A member of another event reads the person, but not the source of its evidence.
        let other_event = api.create_event(&owner.cookie, "TEST31").await;
        let outsider = api.member("testwil", OrganizationRole::Member).await;
        api.add_to_event(&owner.cookie, &other_event, &outsider, "event-viewer")
            .await;
        let (status, outside) = api
            .get(&outsider.cookie, &format!("/api/v1/persons/{person}"))
            .await;
        assert_eq!(status, StatusCode::OK, "{outside}");
        assert_eq!(outside["name"], "Moritz Muster");
        assert_eq!(outside["evidence"], json!([]));

        // Only "make firm" or an accepted proposal of that change makes it firm (ADR 0068).
        let (status, problem) = api
            .send(
                &contributor.cookie,
                Method::PATCH,
                &path,
                Some(&json!({"status": "firm", "expected_version": 1})),
            )
            .await;
        assert_eq!(problem["code"], "invalid-transition", "{status} {problem}");
        let (_, read) = api.get(&contributor.cookie, &path).await;
        assert_eq!(read["status"], "conditional");
        assert_eq!(read["version"], 1);
    }
}

/// Review routing (ADR 0067): a proposal goes to the person who owns the work.
mod routing {
    use super::*;

    /// An event with an event manager, a workstream lead, the author of the proposals and another contributor.
    struct Routed {
        api: Api,
        event: String,
        workstream: String,
        /// An event manager without an organization role that manages each event.
        manager: Member,
        lead: Member,
        author: Member,
        other: Member,
    }

    impl Routed {
        async fn start() -> Self {
            let api = Api::start().await;
            let owner = api.member("testwil", OrganizationRole::Owner).await;
            let event = api.create_event(&owner.cookie, "TEST30").await;
            let manager = api.member("testwil", OrganizationRole::Member).await;
            let lead = api.member("testwil", OrganizationRole::Member).await;
            let author = api.member("testwil", OrganizationRole::Member).await;
            let other = api.member("testwil", OrganizationRole::Member).await;
            api.add_to_event(&owner.cookie, &event, &manager, "event-manager")
                .await;
            for member in [&lead, &author, &other] {
                api.add_to_event(&owner.cookie, &event, member, "event-contributor")
                    .await;
            }
            let (status, workstream) = api
                .post(
                    &manager.cookie,
                    &format!("/api/v1/events/{event}/workstreams"),
                    &json!({"name": "Bodenbetrieb", "lead_user_id": lead.user.as_uuid()}),
                )
                .await;
            assert_eq!(status, StatusCode::CREATED, "{workstream}");
            let workstream = workstream["id"].as_str().unwrap().to_owned();
            Self {
                api,
                event,
                workstream,
                manager,
                lead,
                author,
                other,
            }
        }

        /// The author proposes `proposals`. Returns the ID of the changeset.
        async fn propose(&self, proposals: Vec<Value>) -> String {
            let (status, changeset) = self
                .api
                .post(
                    &self.author.cookie,
                    &format!("/api/v1/events/{}/changesets", self.event),
                    &json!({"source_text": SOURCE, "proposals": proposals}),
                )
                .await;
            assert_eq!(status, StatusCode::CREATED, "{changeset}");
            changeset["id"].as_str().unwrap().to_owned()
        }

        /// A proposal of a new commitment of the author from the new person `person`, in `workstream`.
        fn commitment(&self, person: Uuid, workstream: Option<&str>) -> Value {
            proposal(
                json!({
                    "kind": "create-commitment", "id": Uuid::now_v7(), "event_id": self.event,
                    "text": "Liefert den Generator", "promisor": {"person": person},
                    "owner": self.author.user.as_uuid(), "workstream": workstream,
                }),
                "Der Ort ist noch offen.",
            )
        }

        fn new_person(person: Uuid) -> Value {
            proposal(
                json!({"kind": "create-person", "id": person, "name": "Moritz Muster"}),
                "Wer klärt die Bewilligung?",
            )
        }

        /// A changeset with a new supplier and a commitment from it in the workstream.
        async fn supplier_changeset(&self) -> (String, Value, Value) {
            let person = Uuid::now_v7();
            let supplier = Self::new_person(person);
            let mut promise = self.commitment(person, Some(&self.workstream));
            promise["depends_on"] = json!([supplier["id"]]);
            let changeset = self.propose(vec![supplier.clone(), promise.clone()]).await;
            (changeset, supplier, promise)
        }

        async fn inbox(&self, member: &Member) -> Vec<String> {
            let (status, inbox) = self
                .api
                .get(&member.cookie, "/api/v1/changesets?status=open")
                .await;
            assert_eq!(status, StatusCode::OK, "{inbox}");
            ids(&inbox)
        }

        async fn detail(&self, member: &Member, changeset: &str) -> (StatusCode, Value) {
            self.api
                .get(&member.cookie, &format!("/api/v1/changesets/{changeset}"))
                .await
        }

        async fn apply(
            &self,
            member: &Member,
            changeset: &str,
            selected: &[&Value],
        ) -> (StatusCode, Value) {
            let ids: Vec<&Value> = selected.iter().map(|proposal| &proposal["id"]).collect();
            self.api
                .post(
                    &member.cookie,
                    &format!("/api/v1/changesets/{changeset}/apply"),
                    &json!({"selected": ids}),
                )
                .await
        }

        /// An action of the event that `owner` owns, created by the manager.
        async fn action(&self, owner: &Member) -> String {
            let (status, action) = self
                .api
                .post(
                    &self.manager.cookie,
                    &format!("/api/v1/events/{}/actions", self.event),
                    &json!({"title": "Generator bestellen", "owner_user_id": owner.user.as_uuid()}),
                )
                .await;
            assert_eq!(status, StatusCode::CREATED, "{action}");
            action["id"].as_str().unwrap().to_owned()
        }

        fn action_status(&self, action: &str) -> Value {
            proposal(
                json!({
                    "kind": "change-action-status", "event_id": self.event, "action_id": action,
                    "status": "in-progress", "expected_version": 1,
                }),
                "Wer klärt die Bewilligung?",
            )
        }

        async fn remove_from_event(&self, member: &Member) {
            let (status, problem) = self
                .api
                .post(
                    &self.manager.cookie,
                    &format!(
                        "/api/v1/events/{}/memberships/{}/remove",
                        self.event,
                        member.user.as_uuid()
                    ),
                    &json!({"expected_version": 1}),
                )
                .await;
            assert_eq!(status, StatusCode::NO_CONTENT, "{problem}");
        }
    }

    fn proposal_of<'a>(changeset: &'a Value, id: &Value) -> &'a Value {
        changeset["proposals"]
            .as_array()
            .unwrap()
            .iter()
            .find(|proposal| proposal["id"] == *id)
            .unwrap()
    }

    #[tokio::test]
    async fn a_change_of_an_action_goes_to_its_owner() {
        let r = Routed::start().await;
        let action = r.action(&r.other).await;
        let change = r.action_status(&action);
        let changeset = r.propose(vec![change.clone()]).await;

        assert_eq!(r.inbox(&r.other).await, [changeset.as_str()]);
        assert_eq!(r.inbox(&r.lead).await, Vec::<String>::new());
        assert_eq!(r.inbox(&r.manager).await, Vec::<String>::new());
        let (status, review) = r.detail(&r.other, &changeset).await;
        assert_eq!(status, StatusCode::OK, "{review}");
        let shown = proposal_of(&review, &change["id"]);
        assert_eq!(shown["routed_to_me"], true);
        assert_eq!(shown["overdue"], false);
        let (status, _) = r.detail(&r.lead, &changeset).await;
        assert_eq!(status, StatusCode::FORBIDDEN);

        let (status, applied) = r.apply(&r.other, &changeset, &[&change]).await;
        assert_eq!(status, StatusCode::OK, "{applied}");
        let (_, read) = r
            .api
            .get(
                &r.other.cookie,
                &format!("/api/v1/events/{}/actions/{action}", r.event),
            )
            .await;
        assert_eq!(read["status"], "in-progress");
    }

    #[tokio::test]
    async fn a_new_commitment_in_a_workstream_goes_to_the_lead() {
        let r = Routed::start().await;
        let (changeset, _, promise) = r.supplier_changeset().await;

        assert_eq!(r.inbox(&r.lead).await, [changeset.as_str()]);
        assert_eq!(r.inbox(&r.manager).await, Vec::<String>::new());
        assert_eq!(r.inbox(&r.author).await, Vec::<String>::new());
        let (_, review) = r.detail(&r.lead, &changeset).await;
        assert_eq!(proposal_of(&review, &promise["id"])["routed_to_me"], true);
        let (_, review) = r.detail(&r.manager, &changeset).await;
        assert_eq!(proposal_of(&review, &promise["id"])["routed_to_me"], false);
    }

    #[tokio::test]
    async fn a_new_action_without_a_workstream_goes_to_the_managers() {
        let r = Routed::start().await;
        let action = proposal(
            json!({
                "kind": "create-action", "id": Uuid::now_v7(), "event_id": r.event,
                "title": "Bewilligung klären", "owner": r.lead.user.as_uuid(),
            }),
            "Wer klärt die Bewilligung?",
        );
        let changeset = r.propose(vec![action.clone()]).await;

        assert_eq!(r.inbox(&r.manager).await, [changeset.as_str()]);
        assert_eq!(r.inbox(&r.lead).await, Vec::<String>::new());
        let (_, review) = r.detail(&r.manager, &changeset).await;
        assert_eq!(proposal_of(&review, &action["id"])["routed_to_me"], true);
        let (status, _) = r.apply(&r.lead, &changeset, &[&action]).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn a_new_supplier_goes_to_the_lead_of_the_commitment_that_needs_it() {
        let r = Routed::start().await;
        let (changeset, supplier, promise) = r.supplier_changeset().await;

        let (_, review) = r.detail(&r.lead, &changeset).await;
        assert_eq!(proposal_of(&review, &supplier["id"])["routed_to_me"], true);
        let (status, applied) = r.apply(&r.lead, &changeset, &[&promise]).await;
        assert_eq!(status, StatusCode::OK, "{applied}");
        assert_eq!(applied["persons"][0]["local_id"], "PER-001");
        assert_eq!(applied["commitments"][0]["local_id"], "COM-001");
        assert_eq!(r.inbox(&r.lead).await, Vec::<String>::new());
    }

    #[tokio::test]
    async fn the_manager_inbox_hides_routed_proposals_until_they_are_overdue() {
        let r = Routed::start().await;
        let (changeset, _, promise) = r.supplier_changeset().await;
        assert_eq!(r.inbox(&r.manager).await, Vec::<String>::new());

        r.api.clock.advance(SignedDuration::from_hours(3 * 24));
        let manager = Member {
            cookie: r.api.sign_in(r.manager.user, r.manager.organization).await,
            ..r.manager
        };
        assert_eq!(r.inbox(&manager).await, Vec::<String>::new(), "day 3");
        let (_, review) = r.detail(&manager, &changeset).await;
        assert_eq!(proposal_of(&review, &promise["id"])["overdue"], false);

        r.api.clock.advance(SignedDuration::from_hours(24));
        let manager = Member {
            cookie: r.api.sign_in(manager.user, manager.organization).await,
            ..manager
        };
        assert_eq!(r.inbox(&manager).await, [changeset.as_str()], "day 4");
        let (_, review) = r.detail(&manager, &changeset).await;
        let shown = proposal_of(&review, &promise["id"]);
        assert_eq!(shown["overdue"], true);
        assert_eq!(shown["routed_to_me"], false);
    }

    #[tokio::test]
    async fn a_manager_can_still_apply_a_routed_proposal() {
        let r = Routed::start().await;
        let (changeset, _, promise) = r.supplier_changeset().await;
        let (status, applied) = r.apply(&r.manager, &changeset, &[&promise]).await;
        assert_eq!(status, StatusCode::OK, "{applied}");
        assert_eq!(applied["commitments"][0]["local_id"], "COM-001");
    }

    #[tokio::test]
    async fn a_replaced_lead_loses_the_proposal_at_once() {
        let r = Routed::start().await;
        let (changeset, _, promise) = r.supplier_changeset().await;
        assert_eq!(r.inbox(&r.lead).await, [changeset.as_str()]);

        let (status, changed) = r
            .api
            .send(
                &r.manager.cookie,
                Method::PATCH,
                &format!("/api/v1/events/{}/workstreams/{}", r.event, r.workstream),
                Some(&json!({"lead_user_id": r.other.user.as_uuid(), "expected_version": 1})),
            )
            .await;
        assert_eq!(status, StatusCode::OK, "{changed}");

        assert_eq!(r.inbox(&r.lead).await, Vec::<String>::new());
        let (status, _) = r.detail(&r.lead, &changeset).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        let (status, problem) = r.apply(&r.lead, &changeset, &[&promise]).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(problem["code"], "forbidden");
        assert_eq!(r.inbox(&r.other).await, [changeset.as_str()]);
        let (status, applied) = r.apply(&r.other, &changeset, &[&promise]).await;
        assert_eq!(status, StatusCode::OK, "{applied}");
    }

    #[tokio::test]
    async fn a_member_who_loses_the_role_loses_the_proposal_at_once() {
        let r = Routed::start().await;
        let action = r.action(&r.other).await;
        let change = r.action_status(&action);
        let changeset = r.propose(vec![change.clone()]).await;
        assert_eq!(r.inbox(&r.other).await, [changeset.as_str()]);

        r.remove_from_event(&r.other).await;

        assert_eq!(r.inbox(&r.other).await, Vec::<String>::new());
        let (status, _) = r.detail(&r.other, &changeset).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        let (status, _) = r.apply(&r.other, &changeset, &[&change]).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        let (_, work) = r.api.get(&r.other.cookie, "/api/v1/me/work").await;
        assert_eq!(work["review_count"], 0);
        // The proposal has no reviewer besides the managers now, so they see it at once.
        assert_eq!(r.inbox(&r.manager).await, [changeset.as_str()]);
        let (_, work) = r.api.get(&r.manager.cookie, "/api/v1/me/work").await;
        assert_eq!(work["review_count"], 1);
        let (_, review) = r.detail(&r.manager, &changeset).await;
        assert_eq!(proposal_of(&review, &change["id"])["routed_to_me"], true);
    }

    #[tokio::test]
    async fn a_lead_who_loses_the_role_passes_the_proposal_to_the_managers() {
        let r = Routed::start().await;
        let (changeset, _, promise) = r.supplier_changeset().await;
        assert_eq!(r.inbox(&r.manager).await, Vec::<String>::new());

        r.remove_from_event(&r.lead).await;

        assert_eq!(r.inbox(&r.manager).await, [changeset.as_str()]);
        let (_, work) = r.api.get(&r.manager.cookie, "/api/v1/me/work").await;
        assert_eq!(work["review_count"], 2);
        let (_, review) = r.detail(&r.manager, &changeset).await;
        assert_eq!(proposal_of(&review, &promise["id"])["routed_to_me"], true);
    }

    #[tokio::test]
    async fn the_inbox_counts_the_open_proposals_of_the_caller() {
        let r = Routed::start().await;
        let venue = r.api.field(&r.manager.cookie, &r.event, "venue").await;
        let fact = venue_body(&r.event, &venue, "Flugfeld")["proposals"][0].clone();
        let person = Uuid::now_v7();
        let supplier = Routed::new_person(person);
        let mut promise = r.commitment(person, Some(&r.workstream));
        promise["depends_on"] = json!([supplier["id"]]);
        r.propose(vec![fact, supplier, promise]).await;

        for (member, count) in [(&r.lead, 2), (&r.manager, 1)] {
            let (_, inbox) = r
                .api
                .get(&member.cookie, "/api/v1/changesets?status=open")
                .await;
            assert_eq!(inbox["items"][0]["open_proposals"], count, "{inbox}");
            let (_, work) = r.api.get(&member.cookie, "/api/v1/me/work").await;
            assert_eq!(work["review_count"], count);
        }
    }

    #[tokio::test]
    async fn a_lead_cannot_apply_a_selection_with_a_fact_proposal() {
        let r = Routed::start().await;
        let venue = r.api.field(&r.manager.cookie, &r.event, "venue").await;
        let fact = venue_body(&r.event, &venue, "Flugfeld")["proposals"][0].clone();
        let person = Uuid::now_v7();
        let supplier = Routed::new_person(person);
        let mut promise = r.commitment(person, Some(&r.workstream));
        promise["depends_on"] = json!([supplier["id"]]);
        let changeset = r
            .propose(vec![fact.clone(), supplier, promise.clone()])
            .await;

        let (status, problem) = r.apply(&r.lead, &changeset, &[&promise, &fact]).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{problem}");
        assert_eq!(problem["code"], "forbidden");
        let (status, problem) = r
            .api
            .post(
                &r.lead.cookie,
                &format!("/api/v1/changesets/{changeset}/reject"),
                &json!({"proposal_ids": [fact["id"]]}),
            )
            .await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{problem}");
        let (_, review) = r.detail(&r.manager, &changeset).await;
        for proposal in review["proposals"].as_array().unwrap() {
            assert_eq!(proposal["status"], "open");
        }
        let (_, commitments) = r
            .api
            .get(
                &r.lead.cookie,
                &format!("/api/v1/events/{}/commitments", r.event),
            )
            .await;
        assert_eq!(commitments["items"], json!([]));
    }

    #[tokio::test]
    async fn the_lead_sees_the_changeset_and_a_contributor_does_not() {
        let r = Routed::start().await;
        let (changeset, _, _) = r.supplier_changeset().await;

        let (status, review) = r.detail(&r.lead, &changeset).await;
        assert_eq!(status, StatusCode::OK, "{review}");
        assert_eq!(review["proposals"].as_array().unwrap().len(), 2);
        let (status, problem) = r.detail(&r.other, &changeset).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(problem["code"], "forbidden");
        let (status, page) = r
            .api
            .get(
                &r.lead.cookie,
                &format!("/api/v1/events/{}/changesets?status=open", r.event),
            )
            .await;
        assert_eq!(status, StatusCode::OK, "{page}");
        assert_eq!(ids(&page), [changeset.as_str()]);
        let (status, page) = r
            .api
            .get(
                &r.other.cookie,
                &format!("/api/v1/events/{}/changesets?status=open", r.event),
            )
            .await;
        assert_eq!(status, StatusCode::OK, "{page}");
        assert_eq!(page["items"], json!([]));
    }

    #[tokio::test]
    async fn my_work_counts_my_reviews() {
        let r = Routed::start().await;
        r.supplier_changeset().await;
        let action = r.action(&r.lead).await;
        r.propose(vec![r.action_status(&action)]).await;

        let count = |member: &Member| {
            let cookie = member.cookie.clone();
            let api = &r.api;
            async move {
                let (status, work) = api.get(&cookie, "/api/v1/me/work").await;
                assert_eq!(status, StatusCode::OK, "{work}");
                work["review_count"].clone()
            }
        };
        // Two proposals of the supplier changeset and the change of the action of the lead.
        assert_eq!(count(&r.lead).await, 3);
        assert_eq!(count(&r.manager).await, 0);
        assert_eq!(count(&r.other).await, 0);
    }
}
