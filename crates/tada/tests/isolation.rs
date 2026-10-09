//! The isolation of organizations (ADR 0006, ADR 0039): each API operation, each MCP tool, each job
//! kind and the export.
//!
//! Two organizations, each with two events and members in the roles owner and member.
//! The actors act in organization A. One actor is also an owner of organization B and filled it, so
//! the session or token of organization A is the only thing that keeps B away from them.
//! A read of a record of B gets 404 or shows nothing of B. A write of a record of B is refused, and
//! the rows of B stay the same. The test reads the database, not only the status.
//!
//! The coverage test reads the operations of `tada_api::openapi()`, and the main test reads the
//! `tools/list` of the MCP server and the job kinds of the queue. Each new operation, tool or job
//! kind needs a case here, or a reason in `NOT_ORGANIZATION_SCOPED`.

// The helpers of this test file are not `#[test]` functions, so clippy.toml does not cover them.
#![allow(clippy::unwrap_used)]

mod support;

use std::collections::BTreeSet;
use std::sync::Arc;
use std::time::Duration;

use axum::body::Body;
use axum::http::{Method, StatusCode, header};
use jiff::{SignedDuration, Timestamp};
use secrecy::SecretString;
use serde_json::{Value, json};
use support::export::{contains, export, files};
use support::files::pdf;
use support::organization::{Client, SOURCE, fill, organization, router};
use tada_adapters::clock::SystemClock;
use tada_adapters::mail::{FluentMailTexts, MemoryMailer};
use tada_adapters::storage::testing::TestGarage;
use tada_app::caller::{OrganizationRole, ServiceCaller, TelegramGateway};
use tada_app::domain::ids::{OrganizationId, UserId};
use tada_app::jobs::{Handlers, Ran, run_next};
use tada_app::outbound::SEND_JOB;
use tada_app::telegram::{TelegramName, TelegramUserId, accept_link_claim, claim_link_code};
use tada_store_pg::Database;
use tada_store_pg::testing::TestDatabase;
use uuid::Uuid;

/// The operations that no organization scopes, with the reason.
/// Each other operation of the OpenAPI document needs a case in `attempts`.
const NOT_ORGANIZATION_SCOPED: &[(&str, &str)] = &[
    (
        "get_telegram_link",
        "A Telegram link belongs to the user, not to an organization, and shows only the own link.",
    ),
    (
        "remove_telegram_link",
        "A Telegram link belongs to the user, not to an organization, and the member removes only the own link.",
    ),
    (
        "request_sign_in",
        "It names an email address before any session, and each address gets the same answer.",
    ),
    (
        "preview_magic_link",
        "The token of the link names the user, before any session; it shows a masked address only.",
    ),
    (
        "redeem_magic_link",
        "The token of the link selects the user; the session gets its organization afterward.",
    ),
    ("sign_out", "It deletes the session of the caller only."),
    (
        "preview_invitation",
        "The token of the invitation selects the invitation and its organization, not the session.",
    ),
    (
        "accept_invitation",
        "The token of the invitation selects the invitation and its organization, not the session.",
    ),
    (
        "get_token_notice",
        "It returns the version of the token notice, which is the same in each organization.",
    ),
];

/// The job kinds that the isolation test runs. Each kind of a handler of the worker needs a case.
const JOB_KINDS: &[&str] = &[SEND_JOB];

/// The slug and the name of organization B, and a word that only the texts of B hold.
/// The checks ignore the case of letters.
const B_MARKER: &str = "musterhausen";

/// The records that the cases use. `Default` gives empty values for the coverage test.
#[derive(Debug, Clone, Default)]
struct Targets {
    a_event: String,
    a_second: String,
    a_upload: String,
    a_draft: String,
    a_draft_version: String,
    a_fact: String,
    a_open_changeset: String,
    /// The user of the actor with the role member in A.
    a_member: String,
    /// The current record versions of the switch and of the privacy notice of A.
    a_feature_version: i64,
    a_privacy_version: i64,
    /// The ID of the shipped field `date_window`; each organization uses it.
    date_field: String,
    b_organization: String,
    b_event: String,
    b_second: String,
    b_upload: String,
    b_upload_version: String,
    b_draft: String,
    b_draft_version: String,
    b_fact: String,
    b_source_version: String,
    b_changeset: String,
    b_proposal: String,
    b_token: String,
    b_invitation: String,
    b_link_request: String,
    /// A person and an institution of B.
    b_person: String,
    b_institution: String,
    /// A member of B only, with a role in the first event of B.
    b_member: String,
}

/// What a request sends.
#[derive(Debug, Clone)]
enum Payload {
    None,
    Json(Value),
    File(Vec<u8>),
}

/// What the response of an attempt must be. Each attempt also checks that the response shows no
/// record of B and that the rows of B stay the same.
#[derive(Debug, Clone, PartialEq)]
enum Expect {
    /// The path names a record of B: 404 (a member can also get 403 before the lookup).
    NotFound,
    /// The body refers to a record of B: a client error.
    Refused,
    /// A read in A: a success that shows the record `shows` of A to an owner.
    Reads { shows: Option<String> },
    /// A write in A: a success for an owner.
    Writes,
}

/// Which actors make an attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Actors {
    All,
    /// Only the actor without a membership in B; the member of both organizations can choose B.
    SingleOrganization,
}

#[derive(Debug, Clone)]
struct Attempt {
    method: Method,
    path: String,
    payload: Payload,
    expect: Expect,
    actors: Actors,
}

fn get(path: String, expect: Expect) -> Attempt {
    Attempt {
        method: Method::GET,
        path,
        payload: Payload::None,
        expect,
        actors: Actors::All,
    }
}

fn post(path: String, body: Value, expect: Expect) -> Attempt {
    Attempt {
        method: Method::POST,
        path,
        payload: Payload::Json(body),
        expect,
        actors: Actors::All,
    }
}

fn patch(path: String, body: Value, expect: Expect) -> Attempt {
    Attempt {
        method: Method::PATCH,
        path,
        payload: Payload::Json(body),
        expect,
        actors: Actors::All,
    }
}

fn command(path: String, expect: Expect) -> Attempt {
    Attempt {
        method: Method::POST,
        path,
        payload: Payload::None,
        expect,
        actors: Actors::All,
    }
}

fn upload(path: String) -> Attempt {
    Attempt {
        method: Method::POST,
        path,
        payload: Payload::File(pdf("Programm Testwil drei")),
        expect: Expect::NotFound,
        actors: Actors::All,
    }
}

fn single_organization(attempt: Attempt) -> Attempt {
    Attempt {
        actors: Actors::SingleOrganization,
        ..attempt
    }
}

fn reads(shows: &str) -> Expect {
    Expect::Reads {
        shows: Some(shows.to_owned()),
    }
}

const READS: Expect = Expect::Reads { shows: None };

/// The body of a changeset with one proposal and one passage of `SOURCE` as evidence.
/// `source_version` names another source version as the source of the passage.
fn changeset(operation: Value, source_version: Option<&str>) -> Value {
    let quote = "Das Open Day";
    json!({
        "source_text": SOURCE,
        "proposals": [{
            "id": Uuid::now_v7(),
            "operation": operation,
            "evidence": [{"start": 0, "end": quote.chars().count(), "quote": quote,
                          "source_version_id": source_version}],
            "reason": "The member wrote it.",
        }],
    })
}

/// A new draft of a document in the event with the Markdown `markdown`.
fn draft(event: &str, markdown: String) -> Value {
    json!({
        "kind": "create-document-draft", "event_id": event,
        "document": {"new": {"id": Uuid::now_v7(), "name": "Konzept Testwil"}},
        "markdown": markdown,
    })
}

/// A proposal that sets the date window of the event to May 2030.
fn date(event: &str, field: &str) -> Value {
    json!({
        "kind": "set-fact", "event_id": event, "field_id": field, "expected_version": null,
        "state": {"state": "accepted", "value": {"type": "date-window",
            "start": "2030-05-01", "end": "2030-05-31", "granularity": "month"}},
    })
}

/// The isolation cases of an operation of the API, or `None` if it has none.
#[allow(clippy::too_many_lines)]
fn attempts(operation: &str, t: &Targets) -> Option<Vec<Attempt>> {
    let api = |path: String| format!("/api/v1{path}");
    let attempts = match operation {
        // Events.
        "list_events" => vec![get(api("/events".into()), reads(&t.a_event))],
        "create_event" => vec![post(
            api("/events".into()),
            json!({"key": "ISO30", "name": "Open Day Testwil"}),
            Expect::Writes,
        )],
        "get_event" => vec![
            get(api(format!("/events/{}", t.b_event)), Expect::NotFound),
            get(api(format!("/events/{}", t.a_event)), reads(&t.a_event)),
        ],
        // Telegram link requests.
        "create_telegram_link_code" => {
            vec![command(api("/telegram/link-codes".into()), Expect::Writes)]
        }
        "list_telegram_link_requests" => vec![get(api("/telegram/link-requests".into()), READS)],
        "confirm_telegram_link" => vec![command(
            api(format!(
                "/telegram/link-requests/{}/confirm",
                t.b_link_request
            )),
            Expect::NotFound,
        )],
        // The session: the member of both organizations sees both and can choose B by design.
        "get_session" => vec![single_organization(get(api("/session".into()), READS))],
        "choose_organization" => vec![single_organization(post(
            api("/session/organization".into()),
            json!({"organization_id": t.b_organization}),
            Expect::Refused,
        ))],
        // Event memberships.
        "list_event_memberships" => vec![get(
            api(format!("/events/{}/memberships", t.b_event)),
            Expect::NotFound,
        )],
        "add_event_membership" => vec![
            post(
                api(format!("/events/{}/memberships", t.b_event)),
                json!({"user_id": t.a_member, "event_role": "event-viewer"}),
                Expect::NotFound,
            ),
            post(
                api(format!("/events/{}/memberships", t.a_event)),
                json!({"user_id": t.b_member, "event_role": "event-viewer"}),
                Expect::Refused,
            ),
        ],
        "change_event_role" => vec![post(
            api(format!(
                "/events/{}/memberships/{}/change-role",
                t.b_event, t.b_member
            )),
            json!({"event_role": "event-manager", "expected_version": 1}),
            Expect::NotFound,
        )],
        "remove_event_membership" => vec![post(
            api(format!(
                "/events/{}/memberships/{}/remove",
                t.b_event, t.b_member
            )),
            json!({"expected_version": 1}),
            Expect::NotFound,
        )],
        // Workstreams. An unknown ID in B stands for a workstream of B: the lookup is by event.
        "list_workstreams" => vec![
            get(
                api(format!("/events/{}/workstreams", t.b_event)),
                Expect::NotFound,
            ),
            get(api(format!("/events/{}/workstreams", t.a_event)), READS),
        ],
        "create_workstream" => vec![
            post(
                api(format!("/events/{}/workstreams", t.b_event)),
                json!({"name": "Gelände", "lead_user_id": t.a_member}),
                Expect::NotFound,
            ),
            post(
                api(format!("/events/{}/workstreams", t.a_event)),
                json!({"name": "Gelände", "lead_user_id": t.b_member}),
                Expect::Refused,
            ),
        ],
        "change_workstream" => vec![patch(
            api(format!(
                "/events/{}/workstreams/{}",
                t.b_event,
                Uuid::now_v7()
            )),
            json!({"name": "Bar", "expected_version": 1}),
            Expect::NotFound,
        )],
        // Actions and commitments. An unknown ID in B stands for a record of B: the lookup is by event.
        "list_actions" => vec![
            get(
                api(format!("/events/{}/actions", t.b_event)),
                Expect::NotFound,
            ),
            get(
                api(format!("/events/{}/actions?owner=me", t.a_event)),
                READS,
            ),
        ],
        "my_work" => vec![get(api("/me/work".to_owned()), READS)],
        "create_action" => vec![
            post(
                api(format!("/events/{}/actions", t.b_event)),
                json!({"title": "Zaun stellen", "owner_user_id": t.a_member}),
                Expect::NotFound,
            ),
            post(
                api(format!("/events/{}/actions", t.a_event)),
                json!({"title": "Zaun stellen", "owner_user_id": t.b_member}),
                Expect::Refused,
            ),
        ],
        "get_action" => vec![get(
            api(format!("/events/{}/actions/{}", t.b_event, Uuid::now_v7())),
            Expect::NotFound,
        )],
        "change_action" => vec![patch(
            api(format!("/events/{}/actions/{}", t.b_event, Uuid::now_v7())),
            json!({"title": "Bar", "expected_version": 1}),
            Expect::NotFound,
        )],
        "list_commitments" => vec![
            get(
                api(format!("/events/{}/commitments", t.b_event)),
                Expect::NotFound,
            ),
            get(api(format!("/events/{}/commitments", t.a_event)), READS),
        ],
        "create_commitment" => vec![
            post(
                api(format!("/events/{}/commitments", t.b_event)),
                json!({
                    "text": "Strom ab Freitag",
                    "promisor": {"kind": "institution", "id": t.b_institution},
                    "owner_user_id": t.a_member,
                }),
                Expect::NotFound,
            ),
            post(
                api(format!("/events/{}/commitments", t.a_event)),
                json!({
                    "text": "Strom ab Freitag",
                    "promisor": {"kind": "institution", "id": t.b_institution},
                    "owner_user_id": t.a_member,
                }),
                Expect::Refused,
            ),
            post(
                api(format!("/events/{}/commitments", t.a_event)),
                json!({
                    "text": "Strom ab Freitag",
                    "promisor": {"kind": "person", "id": t.b_person},
                    "owner_user_id": t.a_member,
                }),
                Expect::Refused,
            ),
        ],
        "get_commitment" => vec![get(
            api(format!(
                "/events/{}/commitments/{}",
                t.b_event,
                Uuid::now_v7()
            )),
            Expect::NotFound,
        )],
        "change_commitment" => vec![patch(
            api(format!(
                "/events/{}/commitments/{}",
                t.b_event,
                Uuid::now_v7()
            )),
            json!({"text": "Bar", "expected_version": 1}),
            Expect::NotFound,
        )],
        "make_commitment_firm" => vec![post(
            api(format!(
                "/events/{}/commitments/{}/firm",
                t.b_event,
                Uuid::now_v7()
            )),
            json!({"reason": "Signed", "expected_version": 1}),
            Expect::NotFound,
        )],
        // Members and invitations.
        "list_members" => vec![get(api("/members".into()), reads(&t.a_member))],
        "remove_member" => vec![post(
            api(format!("/members/{}/remove", t.b_member)),
            json!({"expected_version": 1}),
            Expect::NotFound,
        )],
        "list_invitations" => vec![get(api("/invitations".into()), READS)],
        "invite_member" => vec![post(
            api("/invitations".into()),
            json!({"email": "neu@example.org", "display_name": "Neu Testwil", "role": "member"}),
            Expect::Writes,
        )],
        "revoke_invitation" => vec![command(
            api(format!("/invitations/{}/revoke", t.b_invitation)),
            Expect::NotFound,
        )],
        // Persons and institutions.
        "list_persons" => vec![get(api("/persons".into()), READS)],
        "create_person" => vec![post(
            api("/persons".into()),
            json!({"name": "Beat Muster", "user_id": t.b_member}),
            Expect::Refused,
        )],
        "get_person" => vec![get(
            api(format!("/persons/{}", t.b_person)),
            Expect::NotFound,
        )],
        "change_person" => vec![patch(
            api(format!("/persons/{}", t.b_person)),
            json!({"name": "Anna Beispiel", "expected_version": 1}),
            Expect::NotFound,
        )],
        "list_institutions" => vec![get(api("/institutions".into()), READS)],
        "create_institution" => vec![post(
            api("/institutions".into()),
            json!({"name": "Testwil Generatoren AG", "kind": "company"}),
            Expect::Writes,
        )],
        "get_institution" => vec![get(
            api(format!("/institutions/{}", t.b_institution)),
            Expect::NotFound,
        )],
        "change_institution" => vec![patch(
            api(format!("/institutions/{}", t.b_institution)),
            json!({"name": "Testwil Generatoren AG", "expected_version": 1}),
            Expect::NotFound,
        )],
        // Documents and downloads.
        "list_documents" => vec![
            get(
                api(format!("/events/{}/documents", t.b_event)),
                Expect::NotFound,
            ),
            get(
                api(format!("/events/{}/documents?q=Programm", t.a_event)),
                reads(&t.a_upload),
            ),
        ],
        "upload_document" => vec![upload(api(format!("/events/{}/documents", t.b_event)))],
        "get_document" => vec![
            get(api(format!("/documents/{}", t.b_upload)), Expect::NotFound),
            get(api(format!("/documents/{}", t.b_draft)), Expect::NotFound),
        ],
        "list_document_versions" => vec![get(
            api(format!("/documents/{}/versions", t.b_upload)),
            Expect::NotFound,
        )],
        "upload_document_version" => {
            vec![upload(api(format!("/documents/{}/versions", t.b_upload)))]
        }
        "download_document_version" => vec![get(
            api(format!("/document-versions/{}/content", t.b_upload_version)),
            Expect::NotFound,
        )],
        "render_document_version" => vec![get(
            api(format!(
                "/document-versions/{}/rendering",
                t.b_draft_version
            )),
            Expect::NotFound,
        )],
        "approve_document_version" => vec![post(
            api(format!("/document-versions/{}/approve", t.b_draft_version)),
            json!({"expected_version": 1}),
            Expect::NotFound,
        )],
        "diff_document_versions" => vec![
            get(
                api(format!(
                    "/documents/{}/diff?from={}&to={}",
                    t.b_draft, t.b_draft_version, t.b_draft_version
                )),
                Expect::NotFound,
            ),
            get(
                api(format!(
                    "/documents/{}/diff?from={}&to={}",
                    t.a_draft, t.b_draft_version, t.a_draft_version
                )),
                Expect::NotFound,
            ),
        ],
        // Facts.
        "get_event_profile" => vec![
            get(
                api(format!("/events/{}/profile", t.b_event)),
                Expect::NotFound,
            ),
            get(
                api(format!("/events/{}/profile", t.a_event)),
                reads(&t.a_fact),
            ),
        ],
        "list_fields" => vec![get(
            api(format!("/events/{}/fields", t.b_event)),
            Expect::NotFound,
        )],
        // Changesets, apply and drafts with citations.
        "list_event_changesets" => vec![
            get(
                api(format!("/events/{}/changesets?status=open", t.b_event)),
                Expect::NotFound,
            ),
            get(
                api(format!("/events/{}/changesets?status=open", t.b_second)),
                Expect::NotFound,
            ),
        ],
        "create_changeset" => vec![
            post(
                api(format!("/events/{}/changesets", t.b_second)),
                changeset(date(&t.b_second, &t.date_field), None),
                Expect::NotFound,
            ),
            post(
                api(format!("/events/{}/changesets", t.a_second)),
                changeset(date(&t.a_second, &t.date_field), Some(&t.b_source_version)),
                Expect::Refused,
            ),
            post(
                api(format!("/events/{}/changesets", t.a_event)),
                changeset(
                    draft(
                        &t.a_event,
                        format!("Das Open Day ist am [](tada:fact/{}?v=1).\n", t.b_fact),
                    ),
                    None,
                ),
                Expect::Refused,
            ),
            post(
                api(format!("/events/{}/changesets", t.a_event)),
                changeset(
                    draft(
                        &t.a_event,
                        format!(
                            "Das Open Day ist [im Mai](tada:source/{}#0-5).\n",
                            t.b_source_version
                        ),
                    ),
                    None,
                ),
                Expect::Refused,
            ),
        ],
        "list_changesets" => vec![get(
            api("/changesets?status=open".into()),
            reads(&t.a_open_changeset),
        )],
        "get_changeset" => vec![get(
            api(format!("/changesets/{}", t.b_changeset)),
            Expect::NotFound,
        )],
        "apply_changeset" => vec![post(
            api(format!("/changesets/{}/apply", t.b_changeset)),
            json!({"selected": [t.b_proposal]}),
            Expect::NotFound,
        )],
        "reject_proposals" => vec![post(
            api(format!("/changesets/{}/reject", t.b_changeset)),
            json!({"proposal_ids": [t.b_proposal]}),
            Expect::NotFound,
        )],
        // Tokens and the switches of the organization.
        "list_tokens" => vec![get(api("/tokens".into()), READS)],
        "create_token" => vec![post(
            api("/tokens".into()),
            json!({"name": "Isolation", "scope": "read", "notice_version_confirmed": 1,
                   "expires_at": (Timestamp::now() + SignedDuration::from_hours(24)).to_string()}),
            Expect::Writes,
        )],
        "revoke_token" => vec![command(
            api(format!("/tokens/{}/revoke", t.b_token)),
            Expect::NotFound,
        )],
        "list_organization_features" => vec![get(api("/organization/features".into()), READS)],
        "set_organization_feature" => {
            let path = api("/organization/features/mcp-tokens/set".into());
            vec![
                post(
                    path.clone(),
                    json!({"enabled": false, "expected_version": t.a_feature_version}),
                    Expect::Writes,
                ),
                post(
                    path,
                    json!({"enabled": true, "expected_version": t.a_feature_version + 1}),
                    Expect::Writes,
                ),
            ]
        }
        // The privacy notice.
        "get_privacy_notice" => vec![get(api("/organization/privacy-notice".into()), READS)],
        "set_privacy_notice" => vec![post(
            api("/organization/privacy-notice/set".into()),
            json!({"markdown": "Datenschutz Testwil", "expected_version": t.a_privacy_version}),
            Expect::Writes,
        )],
        _ => return None,
    };
    Some(attempts)
}

/// The operation IDs of the OpenAPI document.
fn operations() -> Vec<String> {
    let document = serde_json::to_value(tada_api::openapi()).unwrap();
    let mut operations = Vec::new();
    for item in document["paths"].as_object().unwrap().values() {
        for operation in item.as_object().unwrap().values() {
            if let Some(id) = operation.get("operationId").and_then(Value::as_str) {
                operations.push(id.to_owned());
            }
        }
    }
    assert!(operations.len() > 40, "{operations:?}");
    operations
}

/// A new operation fails here until it has an isolation case or a reason why no organization scopes it.
#[test]
fn each_operation_has_an_isolation_case_or_a_reason() {
    let targets = Targets::default();
    let mut missing = Vec::new();
    let mut both = Vec::new();
    for operation in operations() {
        let scoped = attempts(&operation, &targets).is_some();
        let exempt = NOT_ORGANIZATION_SCOPED
            .iter()
            .any(|(name, _)| *name == operation);
        match (scoped, exempt) {
            (false, false) => missing.push(operation),
            (true, true) => both.push(operation),
            _ => {}
        }
    }
    assert!(
        missing.is_empty(),
        "operations without an isolation case: {missing:?}"
    );
    assert!(
        both.is_empty(),
        "operations with a case and a reason: {both:?}"
    );
    let operations = operations();
    for (name, _) in NOT_ORGANIZATION_SCOPED {
        assert!(
            operations.iter().any(|operation| operation == name),
            "{name} is not an operation"
        );
    }
}

/// The handlers of the worker, with a mailer into memory.
fn worker_handlers(database: &Database, mailer: Arc<MemoryMailer>) -> Handlers {
    tada::worker::handlers(
        database,
        mailer,
        Arc::new(FluentMailTexts::new().unwrap()),
        Arc::new(SystemClock),
        support::public_url(),
    )
}

/// A new job kind of the worker fails here until it has an isolation case.
#[tokio::test]
async fn each_job_kind_of_the_worker_has_an_isolation_case() {
    // The pool connects at the first query; this test makes none.
    let database = Database::connect_lazy(
        "postgres://tada@127.0.0.1:1/tada",
        &SecretString::from("unused"),
    )
    .unwrap();
    let kinds = worker_handlers(&database, Arc::new(MemoryMailer::new())).kinds();
    let covered: BTreeSet<&str> = JOB_KINDS.iter().copied().collect();
    assert_eq!(kinds, covered, "each job kind needs an isolation case");
}

/// The UUIDs in `text`, in the hyphenated form.
fn uuids(text: &str) -> BTreeSet<String> {
    let bytes = text.as_bytes();
    let mut found = BTreeSet::new();
    let mut start = 0;
    while start + 36 <= bytes.len() {
        match Uuid::try_parse_ascii(&bytes[start..start + 36]) {
            Ok(id) if bytes[start + 8] == b'-' => {
                found.insert(id.to_string());
                start += 36;
            }
            _ => start += 1,
        }
    }
    found
}

/// One actor in organization A, with a session and an MCP token of A.
struct Actor {
    name: &'static str,
    client: Client,
    /// True for an owner of A. An owner must succeed where a member can get 403.
    owner: bool,
    /// True for the member of both organizations.
    in_both: bool,
    token: String,
}

/// Two filled organizations on one database and one Garage.
struct World {
    test: TestDatabase,
    garage: TestGarage,
    a: OrganizationId,
    b: OrganizationId,
    owner_a: Client,
    /// The member of both organizations. They filled B in a session of B.
    both: UserId,
    /// The UUIDs of the rows of B that no row of A holds.
    b_ids: BTreeSet<String>,
    /// The rows of B before the attempts.
    b_rows: String,
    targets: Targets,
}

/// The rows of each table with an `organization_id`, and the organization row, of `organization`.
async fn rows(test: &TestDatabase, organization: OrganizationId) -> String {
    let tables: String = test
        .scalar(
            "SELECT string_agg(quote_ident(table_name), ',' ORDER BY table_name)
             FROM information_schema.columns
             WHERE table_schema = 'public' AND column_name = 'organization_id'",
        )
        .await;
    let mut selects = vec![format!(
        "SELECT 'organization' AS t, to_jsonb(r)::text AS row FROM organization r WHERE r.id = '{organization}'"
    )];
    for table in tables.split(',') {
        selects.push(format!(
            "SELECT '{table}', to_jsonb(r)::text FROM {table} r WHERE r.organization_id = '{organization}'"
        ));
    }
    test.scalar(&format!(
        "SELECT coalesce(string_agg(t || ' ' || row, E'\\n' ORDER BY t, row), '') FROM ({}) s",
        selects.join(" UNION ALL ")
    ))
    .await
}

/// The UUIDs of the rows of `organization` that no row of `other` and no shipped field holds.
async fn own_ids(
    test: &TestDatabase,
    organization: OrganizationId,
    other: OrganizationId,
) -> BTreeSet<String> {
    let shared: String = test
        .scalar(
            "SELECT coalesce(string_agg(to_jsonb(f)::text, ','), '') FROM field_definition f
             WHERE f.organization_id IS NULL",
        )
        .await;
    let mut excluded = uuids(&rows(test, other).await);
    excluded.extend(uuids(&shared));
    let ids: BTreeSet<String> = uuids(&rows(test, organization).await)
        .difference(&excluded)
        .cloned()
        .collect();
    assert!(ids.len() > 20, "{ids:?}");
    ids
}

/// The ID of the first item of a list.
fn first_id(list: &Value) -> String {
    list["items"][0]["id"].as_str().unwrap().to_owned()
}

impl World {
    #[allow(clippy::too_many_lines)]
    async fn start() -> Self {
        support::logs::install();
        let (test, garage) = tokio::join!(TestDatabase::start(), TestGarage::start());
        let (a, _, owner_a) = organization(&test, &garage, "testwil").await;
        let filled_a = fill(&test, &owner_a, a, 1001).await;
        // The member of both organizations is the owner who fills B.
        let (b, both, both_in_b) = organization(&test, &garage, "musterhausen").await;
        let filled_b = fill(&test, &both_in_b, b, 1002).await;
        test.add_membership(a, both, OrganizationRole::Owner).await;

        // Organization B: a member of B only, an event and a changeset with the text of B, a
        // privacy notice of B and an open Telegram link request.
        let (_, b_member, _) = test.member("musterhausen", OrganizationRole::Member).await;
        both_in_b
            .post(
                &format!("/api/v1/events/{}/memberships", filled_b.event),
                &json!({"user_id": b_member.to_string(), "event_role": "event-viewer"}),
            )
            .await;
        let b_only = both_in_b
            .create_event("ONLYB31", "Fly-in Musterhausen")
            .await;
        let fields = both_in_b
            .get(&format!("/api/v1/events/{b_only}/fields"))
            .await;
        let date_field = fields["items"]
            .as_array()
            .unwrap()
            .iter()
            .find(|field| field["key"] == "date_window")
            .unwrap()["id"]
            .as_str()
            .unwrap()
            .to_owned();
        let text = "Das Fly-in Musterhausen ist im Mai 2030.";
        let b_changeset = both_in_b
            .post(
                &format!("/api/v1/events/{b_only}/changesets"),
                &json!({
                    "source_text": text,
                    "proposals": [{
                        "id": Uuid::now_v7(),
                        "operation": date(&b_only, &date_field),
                        "evidence": [{"start": 4, "end": 23, "quote": "Fly-in Musterhausen"}],
                        "reason": "The member wrote it.",
                    }],
                }),
            )
            .await;
        both_in_b
            .post(
                "/api/v1/organization/privacy-notice/set",
                &json!({"markdown": "Datenschutz Musterhausen", "expected_version": 1}),
            )
            .await;
        assert!(
            claim_link_code(
                &ServiceCaller::<TelegramGateway>::new(),
                &filled_b.secrets[2],
                TelegramUserId(1003),
                &TelegramName("Testperson Musterhausen".to_owned()),
                &test.database,
                &SystemClock
            )
            .await
            .unwrap()
            .is_some()
        );
        assert!(
            accept_link_claim(
                &ServiceCaller::<TelegramGateway>::new(),
                TelegramUserId(1003),
                &test.database,
                &SystemClock
            )
            .await
            .unwrap()
        );

        let b_person = both_in_b
            .post("/api/v1/persons", &json!({"name": "Beat Musterhausen"}))
            .await;
        let b_institution = both_in_b
            .post(
                "/api/v1/institutions",
                &json!({"name": "Musterhausen Bau AG", "kind": "company"}),
            )
            .await;

        let b_profile = both_in_b
            .get(&format!("/api/v1/events/{}/profile", filled_b.event))
            .await;
        let b_upload_versions = both_in_b
            .get(&format!("/api/v1/documents/{}/versions", filled_b.upload))
            .await;
        let b_draft = both_in_b
            .get(&format!("/api/v1/documents/{}", filled_b.draft))
            .await;
        let a_profile = owner_a
            .get(&format!("/api/v1/events/{}/profile", filled_a.event))
            .await;
        let a_draft = owner_a
            .get(&format!("/api/v1/documents/{}", filled_a.draft))
            .await;
        let a_open = owner_a.get("/api/v1/changesets?status=open").await;
        let targets = Targets {
            a_event: filled_a.event.clone(),
            a_second: filled_a.second.clone(),
            a_upload: filled_a.upload.clone(),
            a_draft: filled_a.draft.clone(),
            a_draft_version: a_draft["newest_version"]["id"].as_str().unwrap().to_owned(),
            a_fact: a_profile["facts"][0]["id"].as_str().unwrap().to_owned(),
            a_open_changeset: first_id(&a_open),
            date_field,
            b_organization: b.to_string(),
            b_event: filled_b.event.clone(),
            b_second: filled_b.second.clone(),
            b_upload: filled_b.upload.clone(),
            b_upload_version: first_id(&b_upload_versions),
            b_draft: filled_b.draft.clone(),
            b_draft_version: b_draft["newest_version"]["id"].as_str().unwrap().to_owned(),
            b_fact: b_profile["facts"][0]["id"].as_str().unwrap().to_owned(),
            b_source_version: b_profile["facts"][0]["evidence"][0]["source_version_id"]
                .as_str()
                .unwrap()
                .to_owned(),
            b_changeset: b_changeset["id"].as_str().unwrap().to_owned(),
            b_proposal: b_changeset["proposal_ids"][0].as_str().unwrap().to_owned(),
            b_token: first_id(&both_in_b.get("/api/v1/tokens").await),
            b_invitation: first_id(&both_in_b.get("/api/v1/invitations").await),
            b_link_request: first_id(&both_in_b.get("/api/v1/telegram/link-requests").await),
            b_member: b_member.to_string(),
            b_person: b_person["id"].as_str().unwrap().to_owned(),
            b_institution: b_institution["id"].as_str().unwrap().to_owned(),
            ..Targets::default()
        };
        let b_ids = own_ids(&test, b, a).await;
        let b_rows = rows(&test, b).await;
        Self {
            test,
            garage,
            a,
            b,
            owner_a,
            both,
            b_ids,
            b_rows,
            targets,
        }
    }

    /// The actors in A: the member of both organizations in a session of A, and a member of A only
    /// with the role event manager in the first event of A. Returns them and the user of the member.
    async fn actors(&self) -> (Vec<Actor>, UserId) {
        let both = Client {
            router: router(&self.test, &self.garage),
            cookie: self
                .test
                .sign_in(self.both, Some(self.a), Timestamp::now())
                .await,
        };
        let (_, member, cookie) = self.test.member("testwil", OrganizationRole::Member).await;
        self.owner_a
            .post(
                &format!("/api/v1/events/{}/memberships", self.targets.a_event),
                &json!({"user_id": member.to_string(), "event_role": "event-manager"}),
            )
            .await;
        let actors = vec![
            Actor {
                name: "the member of both organizations",
                client: both,
                owner: true,
                in_both: true,
                token: String::new(),
            },
            Actor {
                name: "a member of A",
                client: Client {
                    router: router(&self.test, &self.garage),
                    cookie,
                },
                owner: false,
                in_both: false,
                token: String::new(),
            },
        ];
        (actors, member)
    }

    /// The current record versions of A that the cases send.
    async fn targets(&self, member: &str) -> Targets {
        let features = self.owner_a.get("/api/v1/organization/features").await;
        let privacy = self
            .owner_a
            .get("/api/v1/organization/privacy-notice")
            .await;
        Targets {
            a_member: member.to_owned(),
            a_feature_version: features["items"][0]["version"].as_i64().unwrap(),
            a_privacy_version: privacy["version"].as_i64().unwrap(),
            ..self.targets.clone()
        }
    }

    /// Fails if `text` shows a record or a text of B.
    fn assert_hides_b(&self, label: &str, text: &str) {
        assert!(
            !text.to_lowercase().contains(B_MARKER),
            "{label}: shows a text of B: {text}"
        );
        for id in &self.b_ids {
            let simple = id.replace('-', "");
            assert!(
                !text.contains(id.as_str()) && !text.contains(&simple),
                "{label}: shows {id} of B: {text}"
            );
        }
    }

    /// Fails if a row of B changed.
    async fn assert_b_unchanged(&self, label: &str) {
        let now = rows(&self.test, self.b).await;
        assert!(now == self.b_rows, "{label}: changed the rows of B");
    }

    async fn attempt(&self, actor: &Actor, operation: &str, attempt: &Attempt) {
        let label = format!(
            "{operation} {} {} by {}",
            attempt.method, attempt.path, actor.name
        );
        let request = support::request(attempt.method.clone(), &attempt.path);
        let (request, body) = match &attempt.payload {
            Payload::None => (request, Body::empty()),
            Payload::Json(value) => (
                request.header(header::CONTENT_TYPE, "application/json"),
                Body::from(value.to_string()),
            ),
            Payload::File(bytes) => (
                request
                    .header(header::CONTENT_TYPE, "application/octet-stream")
                    .header("x-file-name", "Programm.pdf"),
                Body::from(bytes.clone()),
            ),
        };
        let (status, body) = actor.client.call(request, body).await;
        self.assert_hides_b(&label, &body.to_string());
        let allowed_forbidden = !actor.owner && status == StatusCode::FORBIDDEN;
        match &attempt.expect {
            Expect::NotFound => assert!(
                status == StatusCode::NOT_FOUND || allowed_forbidden,
                "{label}: {status} {body}"
            ),
            Expect::Refused => assert!(status.is_client_error(), "{label}: {status} {body}"),
            Expect::Reads { shows } => {
                assert!(
                    status.is_success() || allowed_forbidden,
                    "{label}: {status} {body}"
                );
                if let (Some(id), true) = (shows, actor.owner) {
                    assert!(body.to_string().contains(id.as_str()), "{label}: {body}");
                }
            }
            // A member can be refused, for example with 403, but never fail with a server error.
            Expect::Writes => {
                assert!(
                    status.is_success() || (!actor.owner && status.is_client_error()),
                    "{label}: {status} {body}"
                );
            }
        }
        self.assert_b_unchanged(&label).await;
    }

    /// Calls the MCP tool `name` with the token of the actor and returns the tool result.
    async fn tool(&self, actor: &Actor, name: &str, arguments: &Value) -> Value {
        let label = format!("MCP {name} {arguments} by {}", actor.name);
        let body = json!({
            "jsonrpc": "2.0", "id": 1, "method": "tools/call",
            "params": {"name": name, "arguments": arguments},
        });
        let (status, body) = self.mcp(&actor.token, &body).await;
        assert_eq!(status, StatusCode::OK, "{label}: {body}");
        self.assert_hides_b(&label, &body.to_string());
        self.assert_b_unchanged(&label).await;
        body["result"].clone()
    }

    async fn mcp(&self, token: &str, body: &Value) -> (StatusCode, Value) {
        // An MCP client sends no `Origin`; `support::request` would add one to a POST.
        let request = support::request(Method::GET, "/mcp")
            .method(Method::POST)
            .header(header::HOST, "tada.example.org")
            .header(header::CONTENT_TYPE, "application/json")
            .header(header::ACCEPT, "application/json, text/event-stream")
            .header(header::AUTHORIZATION, format!("Bearer {token}"))
            .body(Body::from(body.to_string()))
            .unwrap();
        let (response, body) = support::send(&self.owner_a.router, request).await;
        (response.status(), body)
    }
}

/// The names of the tools of the MCP server.
async fn tool_names(world: &World, token: &str) -> BTreeSet<String> {
    let (status, body) = world
        .mcp(
            token,
            &json!({"jsonrpc": "2.0", "id": 1, "method": "tools/list"}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    body["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|tool| tool["name"].as_str().unwrap().to_owned())
        .collect()
}

/// What a tool call must give.
enum ToolExpect {
    /// A refusal with a problem code.
    Refused,
    /// A result that shows the record `shows` of A.
    Shows(String),
    /// A result.
    Answers,
}

/// The isolation cases of the MCP tools.
fn tool_attempts(t: &Targets) -> Vec<(&'static str, Value, ToolExpect)> {
    let quote = "Das Open Day";
    let evidence = |source: Option<&str>| json!([{"start": 0, "end": quote.chars().count(), "quote": quote, "source_version_id": source}]);
    let proposal = |operation: Value, source: Option<&str>| {
        json!({
            "id": Uuid::now_v7(), "operation": operation, "evidence": evidence(source),
            "reason": "The member wrote it.",
        })
    };
    vec![
        (
            "list_events",
            json!({}),
            ToolExpect::Shows(t.a_event.clone()),
        ),
        // Both organizations have events with the keys OPEN30 and FLY30, and only B has ONLYB31.
        (
            "get_event_schema",
            json!({"event_key": "OPEN30"}),
            ToolExpect::Answers,
        ),
        (
            "get_event_schema",
            json!({"event_key": "ONLYB31"}),
            ToolExpect::Refused,
        ),
        (
            "get_event_profile",
            json!({"event_key": "OPEN30"}),
            ToolExpect::Shows(t.a_fact.clone()),
        ),
        (
            "get_event_profile",
            json!({"event_key": "ONLYB31"}),
            ToolExpect::Refused,
        ),
        (
            "search_sources",
            json!({"query": "Flugfeld"}),
            ToolExpect::Answers,
        ),
        (
            "search_sources",
            json!({"query": B_MARKER}),
            ToolExpect::Answers,
        ),
        (
            "search_sources",
            json!({"query": "Flugfeld", "event_key": "ONLYB31"}),
            ToolExpect::Refused,
        ),
        (
            "get_source_passage",
            json!({"source_version_id": t.b_source_version, "start": 0, "end": 5}),
            ToolExpect::Refused,
        ),
        (
            "list_documents",
            json!({"event_key": "OPEN30"}),
            ToolExpect::Shows(t.a_upload.clone()),
        ),
        (
            "list_documents",
            json!({"event_key": "ONLYB31"}),
            ToolExpect::Refused,
        ),
        (
            "get_document_version",
            json!({"version_id": t.b_draft_version}),
            ToolExpect::Refused,
        ),
        (
            "propose_changeset",
            json!({"id": Uuid::now_v7(), "event_id": t.b_second, "source_text": SOURCE,
                   "proposals": [proposal(date(&t.b_second, &t.date_field), None)]}),
            ToolExpect::Refused,
        ),
        (
            "propose_changeset",
            json!({"id": Uuid::now_v7(), "event_id": t.a_second, "source_text": SOURCE,
                   "proposals": [proposal(date(&t.a_second, &t.date_field), Some(&t.b_source_version))]}),
            ToolExpect::Refused,
        ),
        (
            "propose_changeset",
            json!({"id": Uuid::now_v7(), "event_id": t.a_event, "source_text": SOURCE,
                   "proposals": [proposal(draft(&t.a_event,
                       format!("Das Open Day ist am [](tada:fact/{}?v=1).\n", t.b_fact)), None)]}),
            ToolExpect::Refused,
        ),
    ]
}

/// Acceptance: two organizations and several events are isolated in each API operation, each MCP
/// tool, each job kind and the export, also for a member of both organizations.
#[tokio::test]
#[allow(clippy::too_many_lines)]
async fn each_operation_tool_job_and_export_keeps_the_isolation_of_organizations() {
    let world = World::start().await;
    let (mut actors, member) = world.actors().await;

    // The API.
    for actor in &actors {
        for operation in operations() {
            let targets = world.targets(&member.to_string()).await;
            let Some(attempts) = attempts(&operation, &targets) else {
                continue;
            };
            for attempt in attempts {
                if attempt.actors == Actors::SingleOrganization && actor.in_both {
                    continue;
                }
                world.attempt(actor, &operation, &attempt).await;
            }
        }
    }
    // The session of the member of both organizations still acts in A.
    let session = actors[0].client.get("/api/v1/session").await;
    assert_eq!(
        session["organization"]["organization_id"],
        world.a.to_string(),
        "{session}"
    );

    // The MCP tools, with a `propose` token of A of each actor.
    for actor in &mut actors {
        let token = actor
            .client
            .post(
                "/api/v1/tokens",
                &json!({"name": "Isolation", "scope": "propose", "notice_version_confirmed": 1,
                        "expires_at": (Timestamp::now() + SignedDuration::from_hours(24)).to_string()}),
            )
            .await;
        actor.token = token["secret"].as_str().unwrap().to_owned();
    }
    let targets = world.targets("").await;
    let cases = tool_attempts(&targets);
    let covered: BTreeSet<String> = cases.iter().map(|(name, ..)| (*name).to_owned()).collect();
    assert_eq!(
        tool_names(&world, &actors[0].token).await,
        covered,
        "each MCP tool needs an isolation case"
    );
    for actor in &actors {
        for (name, arguments, expect) in &cases {
            let result = world.tool(actor, name, arguments).await;
            let label = format!("MCP {name} {arguments} by {}", actor.name);
            match expect {
                ToolExpect::Refused => {
                    assert_eq!(result["isError"], true, "{label}: {result}");
                }
                ToolExpect::Shows(id) => {
                    assert_eq!(result["isError"], false, "{label}: {result}");
                    if actor.owner {
                        assert!(
                            result.to_string().contains(id.as_str()),
                            "{label}: {result}"
                        );
                    }
                }
                ToolExpect::Answers => {
                    assert_eq!(result["isError"], false, "{label}: {result}");
                }
            }
        }
    }

    // The export: the export of each organization holds no record of the other.
    let a_ids = own_ids(&world.test, world.a, world.b).await;
    let directory = tempfile::tempdir().unwrap();
    for (slug, other_ids, marker) in [
        ("testwil", &world.b_ids, Some(B_MARKER)),
        ("musterhausen", &a_ids, None),
    ] {
        let output = directory.path().join(slug);
        export(&world.test, &world.garage, slug, &output).await;
        for (path, content) in files(&output) {
            if let Some(marker) = marker {
                let content = String::from_utf8_lossy(&content).to_lowercase();
                assert!(!content.contains(marker), "{slug}: {path} holds {marker}");
            }
            for id in other_ids {
                assert!(
                    !contains(&content, id) && !contains(&content, &id.replace('-', "")),
                    "{slug}: {path} holds {id} of the other organization"
                );
            }
        }
    }
    world.assert_b_unchanged("the export").await;

    // The jobs: each job kind that the operations queued has a case. This guards a kind that the
    // worker would run without a handler; `each_job_kind_of_the_worker_has_an_isolation_case`
    // checks the handlers.
    let kinds: String = world
        .test
        .scalar("SELECT coalesce(string_agg(DISTINCT kind, ','), '') FROM job")
        .await;
    for kind in kinds.split(',') {
        assert!(
            JOB_KINDS.contains(&kind),
            "the job kind {kind} needs an isolation case"
        );
    }
    // The send job of an invitation of A runs in A only: it sends the mail of A and leaves B as it is.
    let mismatched: i64 = world
        .test
        .scalar(
            "SELECT count(*) FROM job j JOIN outbound_intent i ON i.id = (j.payload->>'intent_id')::uuid
             WHERE j.organization_id IS DISTINCT FROM i.organization_id",
        )
        .await;
    assert_eq!(
        mismatched, 0,
        "each send job names the organization of its intent"
    );
    let a_job: Uuid = world
        .test
        .scalar(&format!(
            "SELECT id FROM job WHERE kind = '{SEND_JOB}' AND organization_id = '{}' ORDER BY id LIMIT 1",
            world.a
        ))
        .await;
    // The jobs of B wait until the job of A ran.
    world
        .test
        .scalar::<i64>(&format!(
            "WITH u AS (UPDATE job SET run_at = now() + interval '1 hour'
             WHERE organization_id = '{}' RETURNING 1) SELECT count(*) FROM u",
            world.b
        ))
        .await;
    let b_rows = rows(&world.test, world.b).await;
    let mailer = Arc::new(MemoryMailer::new());
    let handlers = worker_handlers(&world.test.database, mailer.clone());
    let ran = run_next(
        &world.test.database,
        &handlers,
        Uuid::now_v7(),
        Duration::from_secs(60),
    )
    .await
    .unwrap();
    assert_eq!(ran, Ran::Completed(a_job));
    let sent = mailer.sent();
    assert_eq!(sent.len(), 1);
    assert!(sent[0].text.contains("testwil"), "{}", sent[0].text);
    assert!(!sent[0].text.contains("musterhausen"), "{}", sent[0].text);
    assert_eq!(
        rows(&world.test, world.b).await,
        b_rows,
        "the send job of A changed the rows of B"
    );
}
