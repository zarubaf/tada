//! The retention settings of an organization and the daily job that redacts closed events and old
//! sources after the set periods (ADR 0071).

// The helpers of this test file are not `#[test]` functions, so clippy.toml does not cover them.
#![allow(clippy::unwrap_used)]

mod support;

use axum::body::Body;
use axum::http::{Method, StatusCode, header};
use jiff::Timestamp;
use serde_json::{Value, json};
use support::{MailApp, NoObjectStorage, SESSION_COOKIE};
use tada_app::audit::AuditEvent;
use tada_app::caller::{JobRunner, MemberCaller, OrgScope, ServiceCaller};
use tada_app::clock::Clock;
use tada_app::domain::RecordVersion;
use tada_app::domain::identity::{DisplayName, Email, OrganizationRole};
use tada_app::domain::ids::{OrganizationId, UserId};
use tada_app::domain::redaction::{RedactionTarget, RedactionTargetKind};
use tada_app::problem::ProblemCode;
use tada_app::redaction::redact_for_retention;
use tada_app::retention::{Retention, RetentionPeriods, RetentionReport, RetentionStore, run};
use tada_app::store::StoreError;
use tada_store_pg::Database;
use uuid::Uuid;

const PATH: &str = "/api/v1/organization/retention";
const SET: &str = "/api/v1/organization/retention/set";

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

    /// The session cookie of a new member.
    async fn member(&self, name: &str, role: OrganizationRole) -> String {
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
        self.app
            .test
            .sign_in(id, Some(self.testwil), self.app.clock.now())
            .await
    }

    async fn call(
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
        let (response, value) = self.app.send(request.unwrap()).await;
        (response.status(), value)
    }

    async fn get(&self, cookie: &str) -> (StatusCode, Value) {
        self.call(cookie, Method::GET, PATH, None).await
    }

    async fn put(&self, cookie: &str, body: &Value) -> (StatusCode, Value) {
        self.call(cookie, Method::POST, SET, Some(body)).await
    }

    async fn set(&self, closed_event: i64, source: i64) {
        self.app
            .test
            .execute(&[&format!(
                "UPDATE organization SET retention_closed_event_months = {closed_event},
                     retention_source_months = {source}"
            )])
            .await;
    }

    /// An event that closed `months` months ago.
    async fn closed_event(&self, key: &str, months: i64) -> Uuid {
        self.closed_event_at(key, &format!("now() - make_interval(months => {months})"))
            .await
    }

    /// An event that closed at the SQL time `closed_at`.
    async fn closed_event_at(&self, key: &str, closed_at: &str) -> Uuid {
        let event = self.app.test.create_event(self.testwil, key).await;
        self.app
            .test
            .execute(&[&format!(
                "UPDATE event SET status = 'closed', closed_at = {closed_at} WHERE id = '{event}'"
            )])
            .await;
        event.as_uuid()
    }

    /// A text source that is `months` months old, of `event` or of no event.
    async fn source(&self, event: Option<Uuid>, months: i64) -> Uuid {
        self.source_at(event, &format!("now() - make_interval(months => {months})"))
            .await
    }

    /// A text source that was created at the SQL time `created_at`.
    async fn source_at(&self, event: Option<Uuid>, created_at: &str) -> Uuid {
        let id = Uuid::now_v7();
        let event = event.map_or("NULL".to_owned(), |event| format!("'{event}'"));
        self.app
            .test
            .execute(&[
                "SET LOCAL session_replication_role = replica",
                &format!(
                    "INSERT INTO source_item (id, organization_id, event_id, kind, created_at)
                     VALUES ('{id}', '{org}', {event}, 'member-text', {created_at})",
                    org = self.testwil
                ),
                &format!(
                    "INSERT INTO source_version
                         (id, organization_id, source_item_id, kind, channel, author_actor, text, sha256, captured_at)
                     VALUES ('{v}', '{org}', '{id}', 'member-text', 'web',
                             '{{}}', 'Der Vorstand trifft sich im Mai.', decode(repeat('ab', 32), 'hex'), now())",
                    v = Uuid::now_v7(),
                    org = self.testwil
                ),
            ])
            .await;
        id
    }

    async fn run(&self) -> RetentionReport {
        self.run_at(Timestamp::now()).await
    }

    async fn run_at(&self, now: Timestamp) -> RetentionReport {
        let db = &self.app.test.database;
        run(db, db, &NoObjectStorage, now).await.unwrap()
    }

    async fn event_redacted(&self, event: Uuid) -> bool {
        self.app
            .test
            .scalar::<bool>(&format!(
                "SELECT redacted_at IS NOT NULL FROM event WHERE id = '{event}'"
            ))
            .await
    }

    async fn source_redacted(&self, source: Uuid) -> bool {
        self.app
            .test
            .scalar::<bool>(&format!(
                "SELECT redacted_at IS NOT NULL FROM source_item WHERE id = '{source}'"
            ))
            .await
    }
}

fn periods(closed_event: Value, source: Value, version: i64) -> Value {
    json!({"closed_event_months": closed_event, "source_months": source, "version": version})
}

/// The body of a change with the version that the caller read.
fn change(closed_event: Value, source: Value, version: i64) -> Value {
    json!({"closed_event_months": closed_event, "source_months": source, "expected_version": version})
}

#[tokio::test]
async fn owner_sets_retention_admin_reads_member_forbidden() {
    let app = App::start().await;
    let owner = app.member("Olga Owner", OrganizationRole::Owner).await;
    let admin = app.member("Adam Admin", OrganizationRole::Admin).await;
    let member = app.member("Mia Member", OrganizationRole::Member).await;

    // Retention is off until an owner sets a period.
    let (status, read) = app.get(&admin).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(read, periods(Value::Null, Value::Null, 1));

    let body = change(json!(6), Value::Null, 1);
    let (status, set) = app.put(&owner, &body).await;
    assert_eq!(status, StatusCode::OK, "{set}");
    assert_eq!(set, periods(json!(6), Value::Null, 2));
    let (_, read) = app.get(&admin).await;
    assert_eq!(read, set);

    let (status, problem) = app.put(&admin, &change(json!(6), Value::Null, 2)).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(problem["code"], "forbidden");
    let (status, problem) = app.get(&member).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(problem["code"], "forbidden");
    let (status, problem) = app.put(&member, &body).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(problem["code"], "forbidden");

    // The audit log holds the two numbers and no content.
    let detail = app
        .app
        .test
        .scalar::<Value>(
            "SELECT detail FROM audit_event WHERE action = 'organization.set_retention'",
        )
        .await;
    assert_eq!(
        detail,
        json!({"closed_event_months": 6, "source_months": null})
    );

    // Null switches a period off again.
    let (status, off) = app.put(&owner, &change(Value::Null, json!(120), 2)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(off, periods(Value::Null, json!(120), 3));
    let (_, read) = app.get(&owner).await;
    assert_eq!(read, off);
}

/// A short period redacts data for good on the next run, so a stolen session older than 15 minutes cannot set it.
#[tokio::test]
async fn setting_the_periods_needs_a_recent_sign_in() {
    let app = App::start().await;
    let owner = app.member("Olga Owner", OrganizationRole::Owner).await;
    app.app.clock.advance(jiff::SignedDuration::from_mins(16));

    let (status, problem) = app.put(&owner, &change(json!(1), json!(1), 1)).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{problem}");
    assert_eq!(problem["code"], "recent-sign-in-required");
    // The owner still reads the periods, and nothing changed.
    let (status, read) = app.get(&owner).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(read, periods(Value::Null, Value::Null, 1));
}

#[tokio::test]
async fn a_stale_version_conflicts() {
    let app = App::start().await;
    let first = app.member("Olga Owner", OrganizationRole::Owner).await;
    let second = app.member("Otto Owner", OrganizationRole::Owner).await;
    let (status, _) = app.put(&first, &change(json!(6), Value::Null, 1)).await;
    assert_eq!(status, StatusCode::OK);
    let (status, problem) = app.put(&second, &change(json!(12), Value::Null, 1)).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(problem["code"], "record-version-conflict");
    let (_, read) = app.get(&second).await;
    assert_eq!(read, periods(json!(6), Value::Null, 2));
}

#[tokio::test]
async fn retention_out_of_range_is_validation_failed() {
    let app = App::start().await;
    let owner = app.member("Olga Owner", OrganizationRole::Owner).await;
    for (body, pointer) in [
        (change(json!(0), Value::Null, 1), "/closed_event_months"),
        (change(Value::Null, json!(121), 1), "/source_months"),
        (change(json!(-1), Value::Null, 1), "/closed_event_months"),
    ] {
        let (status, problem) = app.put(&owner, &body).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
        assert_eq!(problem["code"], "validation-failed");
        assert_eq!(problem["errors"][0]["pointer"], pointer);
    }
    let (_, read) = app.get(&owner).await;
    assert_eq!(read, periods(Value::Null, Value::Null, 1));
    assert_eq!(
        app.app
            .test
            .scalar::<i64>(
                "SELECT count(*) FROM audit_event WHERE action = 'organization.set_retention'"
            )
            .await,
        0
    );
}

#[tokio::test]
async fn retention_redacts_expired_closed_event() {
    let app = App::start().await;
    app.set(6, 6).await;
    let expired = app.closed_event("OLD30", 7).await;
    let report = app.run().await;
    assert_eq!(
        report,
        RetentionReport {
            events: 1,
            failed: vec![],
            ..RetentionReport::default()
        }
    );
    assert!(app.event_redacted(expired).await);
    // The event keeps its row and its status, and the audit log names the retention job.
    assert_eq!(
        app.app
            .test
            .scalar::<String>(&format!("SELECT status FROM event WHERE id = '{expired}'"))
            .await,
        "closed"
    );
    assert_eq!(
        app.app
            .test
            .scalar::<Value>("SELECT detail FROM audit_event WHERE action = 'event.redact'")
            .await,
        json!({"reason": "retention"})
    );
}

#[tokio::test]
async fn retention_redacts_old_sources_without_event() {
    let app = App::start().await;
    app.set(6, 6).await;
    let old = app.source(None, 7).await;
    let report = app.run().await;
    assert_eq!(
        report,
        RetentionReport {
            events: 0,
            sources: 1,
            failed: vec![],
            ..RetentionReport::default()
        }
    );
    assert!(app.source_redacted(old).await);
    assert!(
        app.app
            .test
            .scalar::<bool>(&format!(
                "SELECT text IS NULL FROM source_version WHERE source_item_id = '{old}'"
            ))
            .await
    );
}

#[tokio::test]
async fn retention_skips_active_and_unset() {
    let app = App::start().await;
    app.set(1, 1).await;
    let active = app
        .app
        .test
        .create_event(app.testwil, "ACT30")
        .await
        .as_uuid();
    app.app
        .test
        .execute(&[&format!(
            "UPDATE event SET created_at = now() - interval '24 months' WHERE id = '{active}'"
        )])
        .await;
    let recent = app.closed_event("NEW30", 0).await;
    let of_active_event = app.source(Some(active), 24).await;
    let young = app.source(None, 0).await;

    // A second organization with old rows and no period.
    let other = app.app.test.create_organization("flugplatz").await;
    let other_event = app.app.test.create_event(other, "OLD30").await.as_uuid();
    app.app
        .test
        .execute(&[&format!(
            "UPDATE event SET status = 'closed', closed_at = now() - interval '60 months'
             WHERE id = '{other_event}'"
        )])
        .await;

    assert_eq!(
        app.run().await,
        RetentionReport {
            events: 0,
            sources: 0,
            failed: vec![],
            ..RetentionReport::default()
        }
    );
    for event in [active, recent, other_event] {
        assert!(!app.event_redacted(event).await);
    }
    for source in [of_active_event, young] {
        assert!(!app.source_redacted(source).await);
    }
}

#[tokio::test]
async fn retention_resumes_after_partial_run() {
    let app = App::start().await;
    app.set(6, 6).await;
    let first = app.closed_event("OLD30", 8).await;
    let second = app.closed_event("OLD31", 9).await;
    let source = app.source(None, 8).await;

    // An earlier run redacted the first event, then stopped.
    let db = &app.app.test.database;
    let done = redact_for_retention(
        MemberCaller::new(
            UserId::from_uuid(Uuid::now_v7()),
            app.testwil,
            OrganizationRole::Owner,
        )
        .scope(),
        RedactionTarget {
            kind: RedactionTargetKind::Event,
            id: first,
        },
        db,
        &NoObjectStorage,
    )
    .await;
    assert!(done.is_ok());

    assert_eq!(
        app.run().await,
        RetentionReport {
            events: 1,
            sources: 1,
            failed: vec![],
            ..RetentionReport::default()
        }
    );
    assert!(app.event_redacted(first).await);
    assert!(app.event_redacted(second).await);
    assert!(app.source_redacted(source).await);
    // A second run finds nothing.
    assert_eq!(app.run().await, RetentionReport::default());
}

/// The store with one more target that cannot be redacted: its event does not exist.
#[derive(Debug)]
struct WithBrokenTarget<'a> {
    inner: &'a Database,
    organization: OrganizationId,
    broken: Uuid,
}

#[async_trait::async_trait]
impl RetentionStore for WithBrokenTarget<'_> {
    async fn get(&self, scope: OrgScope) -> Result<Retention, StoreError> {
        self.inner.get(scope).await
    }

    async fn set(
        &self,
        scope: OrgScope,
        periods: RetentionPeriods,
        expected_version: RecordVersion,
        audit: &AuditEvent,
    ) -> Result<Option<Retention>, StoreError> {
        self.inner
            .set(scope, periods, expected_version, audit)
            .await
    }

    async fn due_for_retention(
        &self,
        caller: &ServiceCaller<JobRunner>,
        now: Timestamp,
    ) -> Result<Vec<(OrganizationId, RedactionTarget)>, StoreError> {
        let mut due = vec![(
            self.organization,
            RedactionTarget {
                kind: RedactionTargetKind::Event,
                id: self.broken,
            },
        )];
        due.extend(self.inner.due_for_retention(caller, now).await?);
        Ok(due)
    }
}

#[tokio::test]
async fn a_failing_target_does_not_stop_the_others() {
    let app = App::start().await;
    app.set(6, 6).await;
    let event = app.closed_event("OLD30", 8).await;
    let source = app.source(None, 8).await;
    let broken = Uuid::now_v7();
    let db = &app.app.test.database;
    let store = WithBrokenTarget {
        inner: db,
        organization: app.testwil,
        broken,
    };

    let report = run(&store, db, &NoObjectStorage, Timestamp::now())
        .await
        .unwrap();
    // The failure comes first in the list, and the others are redacted after it.
    assert_eq!(report.events, 1);
    assert_eq!(report.sources, 1);
    assert_eq!(report.failed.len(), 1);
    assert_eq!(report.failed[0].target.id, broken);
    assert_eq!(report.failed[0].code, ProblemCode::NotFound);
    assert!(!report.completed(), "the worker marks the sweep failed");
    assert!(app.event_redacted(event).await);
    assert!(app.source_redacted(source).await);
}

fn at(time: &str) -> String {
    format!("timestamptz '{time}'")
}

/// The period ends at exactly `closed_at` plus n calendar months: the row is due then, and not one second before.
#[tokio::test]
async fn retention_pins_the_boundary_of_the_period() {
    let app = App::start().await;
    app.set(6, 6).await;
    let now: Timestamp = "2030-08-15T12:00:00Z".parse().unwrap();
    let exact = app
        .closed_event_at("EXACT30", &at("2030-02-15T12:00:00Z"))
        .await;
    let inside = app
        .closed_event_at("INSIDE30", &at("2030-02-15T12:00:01Z"))
        .await;
    let exact_source = app.source_at(None, &at("2030-02-15T12:00:00Z")).await;
    let inside_source = app.source_at(None, &at("2030-02-15T12:00:01Z")).await;

    let report = app.run_at(now).await;
    assert_eq!(
        report,
        RetentionReport {
            events: 1,
            sources: 1,
            ..RetentionReport::default()
        }
    );
    assert!(app.event_redacted(exact).await);
    assert!(!app.event_redacted(inside).await);
    assert!(app.source_redacted(exact_source).await);
    assert!(!app.source_redacted(inside_source).await);
    // One second later the rest is due.
    let report = app.run_at(now + jiff::SignedDuration::from_secs(1)).await;
    assert_eq!((report.events, report.sources), (1, 1));
}

/// A month counts as a calendar month: 31 January plus one month is 28 February in 2030.
#[tokio::test]
async fn retention_counts_calendar_months_at_the_end_of_a_month() {
    let app = App::start().await;
    app.set(1, 1).await;
    let now: Timestamp = "2030-02-28T12:00:00Z".parse().unwrap();
    let due = app
        .closed_event_at("JAN30", &at("2030-01-31T12:00:00Z"))
        .await;
    let not_due = app
        .closed_event_at("JAN31", &at("2030-01-31T12:00:01Z"))
        .await;
    let due_source = app.source_at(None, &at("2030-01-31T12:00:00Z")).await;
    let not_due_source = app.source_at(None, &at("2030-01-31T12:00:01Z")).await;

    let report = app.run_at(now).await;
    assert_eq!((report.events, report.sources), (1, 1));
    assert!(app.event_redacted(due).await);
    assert!(!app.event_redacted(not_due).await);
    assert!(app.source_redacted(due_source).await);
    assert!(!app.source_redacted(not_due_source).await);
}

/// How evidence of an event cites a source.
#[derive(Debug, Clone, Copy)]
enum Citation {
    EvidenceLink,
    ProposalEvidence,
    RecordEvidence,
}

impl App {
    /// Evidence of `event` that cites the version of `source`, in the way `citation`.
    async fn cite(&self, source: Uuid, event: Uuid, citation: Citation) {
        let org = self.testwil;
        let version = self
            .app
            .test
            .scalar::<Uuid>(&format!(
                "SELECT id FROM source_version WHERE source_item_id = '{source}'"
            ))
            .await;
        let quote = format!("'{version}', 0, 10, 'Der Vorstand'");
        let proposal = Uuid::now_v7();
        let insert_proposal = format!(
            "INSERT INTO proposal
                 (id, organization_id, changeset_id, event_id, operation, operation_version, target_kind, target_id,
                  reason, created_at)
             VALUES ('{proposal}', '{org}', '{changeset}', '{event}', '{{\"kind\": \"set_fact\"}}', 1, 'fact',
                     '{target}', 'Die Quelle nennt es.', now())",
            changeset = Uuid::now_v7(),
            target = Uuid::now_v7(),
        );
        let evidence = match citation {
            Citation::EvidenceLink => {
                let fact = Uuid::now_v7();
                let fact_version = Uuid::now_v7();
                vec![
                    format!(
                        "INSERT INTO fact (id, organization_id, event_id, field_id, version)
                         VALUES ('{fact}', '{org}', '{event}', '{field}', 1)",
                        field = Uuid::now_v7()
                    ),
                    format!(
                        "INSERT INTO fact_version
                             (id, organization_id, fact_id, number, state, value, approximate, created_at, accepted_by)
                         VALUES ('{fact_version}', '{org}', '{fact}', 1, 'accepted', '{{}}', false, now(), '{{}}')"
                    ),
                    format!(
                        "INSERT INTO evidence_link
                             (id, organization_id, fact_version_id, source_version_id, start_offset, end_offset, quote)
                         VALUES ('{}', '{org}', '{fact_version}', {quote})",
                        Uuid::now_v7()
                    ),
                ]
            }
            Citation::ProposalEvidence => vec![
                insert_proposal,
                format!(
                    "INSERT INTO proposal_evidence
                         (id, organization_id, proposal_id, source_version_id, start_offset, end_offset, quote)
                     VALUES ('{}', '{org}', '{proposal}', {quote})",
                    Uuid::now_v7()
                ),
            ],
            Citation::RecordEvidence => vec![
                insert_proposal,
                format!(
                    "INSERT INTO record_evidence
                         (id, organization_id, action_id, record_version, proposal_id, source_version_id,
                          start_offset, end_offset, quote)
                     VALUES ('{}', '{org}', '{}', 1, '{proposal}', {quote})",
                    Uuid::now_v7(),
                    Uuid::now_v7()
                ),
            ],
        };
        let mut statements = vec!["SET LOCAL session_replication_role = replica"];
        statements.extend(evidence.iter().map(String::as_str));
        self.app.test.execute(&statements).await;
    }
}

/// A source of no event that evidence of an active event cites follows the event (ADR 0071): retention leaves it
/// alone while the event is active, and redacts it once the event is closed.
#[tokio::test]
async fn retention_skips_a_source_that_an_active_event_cites() {
    let app = App::start().await;
    app.set(1, 1).await;
    let event = app
        .app
        .test
        .create_event(app.testwil, "ACT30")
        .await
        .as_uuid();
    let mut cited = Vec::new();
    for citation in [
        Citation::EvidenceLink,
        Citation::ProposalEvidence,
        Citation::RecordEvidence,
    ] {
        let source = app.source(None, 5).await;
        app.cite(source, event, citation).await;
        cited.push(source);
    }

    assert_eq!(app.run().await, RetentionReport::default());
    for source in &cited {
        assert!(!app.source_redacted(*source).await);
    }

    // The run listed the sources while the event was closed; then the event was reopened.
    app.app
        .test
        .execute(&[&format!(
            "UPDATE event SET status = 'closed', closed_at = now() WHERE id = '{event}'"
        )])
        .await;
    let due: Vec<_> = cited
        .iter()
        .map(|&id| {
            (
                app.testwil,
                RedactionTarget {
                    kind: RedactionTargetKind::SourceItem,
                    id,
                },
            )
        })
        .collect();
    let db = &app.app.test.database;
    assert_eq!(
        db.due_for_retention(&ServiceCaller::<JobRunner>::new(), Timestamp::now())
            .await
            .unwrap(),
        due,
        "a closed event no longer holds the sources"
    );
    app.app
        .test
        .execute(&[&format!(
            "UPDATE event SET status = 'active', closed_at = NULL WHERE id = '{event}'"
        )])
        .await;
    let stale = Stale {
        inner: db,
        due: due.clone(),
    };
    let report = run(&stale, db, &NoObjectStorage, Timestamp::now())
        .await
        .unwrap();
    assert_eq!(
        report,
        RetentionReport {
            skipped: 3,
            ..RetentionReport::default()
        }
    );
    for source in &cited {
        assert!(!app.source_redacted(*source).await);
    }

    // The event closes for good: its sources are due.
    app.app
        .test
        .execute(&[&format!(
            "UPDATE event SET status = 'closed', closed_at = now() WHERE id = '{event}'"
        )])
        .await;
    assert_eq!(
        app.run().await,
        RetentionReport {
            sources: 3,
            ..RetentionReport::default()
        }
    );
    for source in &cited {
        assert!(app.source_redacted(*source).await);
    }
}

/// A store that lists targets which the run found due earlier.
#[derive(Debug)]
struct Stale<'a> {
    inner: &'a Database,
    due: Vec<(OrganizationId, RedactionTarget)>,
}

#[async_trait::async_trait]
impl RetentionStore for Stale<'_> {
    async fn get(&self, scope: OrgScope) -> Result<Retention, StoreError> {
        self.inner.get(scope).await
    }

    async fn set(
        &self,
        scope: OrgScope,
        periods: RetentionPeriods,
        expected_version: RecordVersion,
        audit: &AuditEvent,
    ) -> Result<Option<Retention>, StoreError> {
        self.inner
            .set(scope, periods, expected_version, audit)
            .await
    }

    async fn due_for_retention(
        &self,
        _: &ServiceCaller<JobRunner>,
        _: Timestamp,
    ) -> Result<Vec<(OrganizationId, RedactionTarget)>, StoreError> {
        Ok(self.due.clone())
    }
}

/// A source that an event holds by the time of its redaction, and an event that was reopened, stay as they are.
#[tokio::test]
async fn retention_skips_a_target_that_no_longer_qualifies() {
    let app = App::start().await;
    app.set(1, 1).await;
    let event = app.closed_event("OLD30", 5).await;
    let source = app.source(None, 5).await;
    let reopened = app.closed_event("OLD31", 5).await;
    // Both were due when the run listed them; then the source got an event and the event was reopened.
    app.app
        .test
        .execute(&[
            "SET LOCAL session_replication_role = replica",
            &format!("UPDATE source_item SET event_id = '{event}' WHERE id = '{source}'"),
            &format!(
                "UPDATE event SET status = 'active', closed_at = NULL WHERE id = '{reopened}'"
            ),
        ])
        .await;
    let db = &app.app.test.database;
    let store = Stale {
        inner: db,
        due: vec![
            (
                app.testwil,
                RedactionTarget {
                    kind: RedactionTargetKind::SourceItem,
                    id: source,
                },
            ),
            (
                app.testwil,
                RedactionTarget {
                    kind: RedactionTargetKind::Event,
                    id: reopened,
                },
            ),
        ],
    };

    let report = run(&store, db, &NoObjectStorage, Timestamp::now())
        .await
        .unwrap();
    assert_eq!(
        report,
        RetentionReport {
            skipped: 2,
            ..RetentionReport::default()
        }
    );
    assert!(report.completed());
    assert!(!app.source_redacted(source).await);
    assert!(!app.event_redacted(reopened).await);
}
