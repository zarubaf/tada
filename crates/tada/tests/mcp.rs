//! The MCP server (ADR 0040) end to end: JSON-RPC over HTTP, the token authenticator, the `app` queries and PostgreSQL.

// The helpers of this test file are not `#[test]` functions, so clippy.toml does not cover them.
#![allow(clippy::unwrap_used)]

mod support;

use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use jiff::SignedDuration;
use secrecy::ExposeSecret;
use serde_json::{Value, json};
use tada_app::caller::{MemberCaller, OrganizationRole};
use tada_app::clock::Clock;
use tada_app::domain::RecordVersion;
use tada_app::domain::facts::core_catalog;
use tada_app::domain::identity::{DisplayName, Email, EventRole};
use tada_app::domain::ids::{EventId, InstitutionId, OrganizationId, ProposalId, UserId};
use tada_app::domain::parties::Party;
use tada_app::domain::sources::SourceText;
use tada_app::event_members::{add_event_member, change_event_role};
use tada_app::parties::{NewInstitution, NewPerson, create_institution, create_person};
use tada_app::proposals::{Changeset, Created, NewChangeset, ProposeStores, create_changeset};
use tada_app::review::{ApplyInput, ReviewStores, apply_changeset};
use tada_app::search::{SearchRequest, search_sources};
use tada_app::sources::SourceStore;
use tada_app::tokens::{
    NOTICE_VERSION, TokenAuthenticator, TokenError, TokenRequest, TokenScope, create_token,
};
use tada_app::work::{NewAction, NewCommitment, WorkPorts, create_action, create_commitment};
use tada_app::workstreams::{NewWorkstream, create_workstream};
use tada_mcp::McpState;
use tada_store_pg::testing::TestDatabase;
use tower::ServiceExt;
use uuid::Uuid;

use crate::support::{TestClock, logs};

/// The text of each changeset of the tests. The word `Flugfeld` also stands in the texts that the member cannot read.
const SOURCE: &str =
    "Das Open Day findet im Mai 2030 auf dem Flugfeld statt. Der Ort ist noch offen.";
/// The text of a new event that the organization changeset of the owner creates.
const ORGANIZATION_SOURCE: &str = "Neuer Anlass: Hangarfest im Juni 2030.";

/// Two organizations. In Testwil, Anna is a viewer of OPEN30 and has no role in SECRET30.
/// Musterhausen has the event FLY31.
struct Mcp {
    router: Router,
    test: TestDatabase,
    clock: Arc<TestClock>,
    testwil: OrganizationId,
    owner: MemberCaller,
    /// An owner of Musterhausen.
    other_owner: MemberCaller,
    anna: UserId,
    anna_caller: MemberCaller,
    open_day: EventId,
    secret: EventId,
    fly_in: EventId,
    /// The secret of Anna's `read` token.
    token: String,
}

async fn user(
    test: &TestDatabase,
    organization: OrganizationId,
    name: &str,
    role: OrganizationRole,
) -> UserId {
    let email = format!("{}@example.org", name.to_lowercase());
    let user = test
        .create_user(
            &DisplayName::parse(name).unwrap(),
            &Email::parse(&email).unwrap(),
        )
        .await;
    test.add_membership(organization, user, role).await;
    user
}

fn core_field(key: &str) -> Uuid {
    core_catalog()
        .into_iter()
        .find(|field| field.key.as_str() == key)
        .unwrap()
        .id
        .as_uuid()
}

/// A passage of `text` that quotes `quote`.
fn passage(text: &str, quote: &str) -> Value {
    let byte = text.find(quote).unwrap();
    let start = text[..byte].chars().count();
    json!({"start": start, "end": start + quote.chars().count(), "quote": quote})
}

impl Mcp {
    async fn start() -> Self {
        logs::install();
        let test = TestDatabase::start().await;
        let clock = Arc::new(TestClock::new("2030-05-18T08:00:00Z".parse().unwrap()));
        let testwil = test.create_organization("testwil").await;
        let musterhausen = test.create_organization("musterhausen").await;
        let open_day = test.create_event(testwil, "OPEN30").await;
        let secret = test.create_event(testwil, "SECRET30").await;
        let fly_in = test.create_event(musterhausen, "FLY31").await;
        let olga = user(&test, testwil, "Olga", OrganizationRole::Owner).await;
        let anna = user(&test, testwil, "Anna", OrganizationRole::Member).await;
        let owner = MemberCaller::new(olga, testwil, OrganizationRole::Owner);
        let otto = user(&test, musterhausen, "Otto", OrganizationRole::Owner).await;
        let other_owner = MemberCaller::new(otto, musterhausen, OrganizationRole::Owner);
        let anna_caller =
            MemberCaller::new(anna, testwil, OrganizationRole::Member).with_sign_in(clock.now());
        let request = TokenRequest {
            name: "Claude Code".to_owned(),
            scope: TokenScope::Read,
            expires_at: clock.now() + SignedDuration::from_hours(24),
            notice_version_confirmed: NOTICE_VERSION,
        };
        let created = create_token(
            &anna_caller,
            request,
            &test.database,
            &test.database,
            &*clock,
        )
        .await
        .unwrap();
        let database = Arc::new(test.database.clone());
        let api = support::api_state(
            &test,
            Arc::new(TokenAuthenticator::new(
                database.clone(),
                database.clone(),
                clock.clone(),
            )),
            clock.clone(),
        );
        let mcp = McpState {
            authenticator: Arc::new(TokenAuthenticator::new(
                database.clone(),
                database.clone(),
                clock.clone(),
            )),
            events: database.clone(),
            identity: database.clone(),
            facts: database.clone(),
            sources: database.clone(),
            proposals: database.clone(),
            documents: database.clone(),
            clock: clock.clone(),
            public_url: support::public_url(),
            workstreams: database.clone(),
            parties: database.clone(),
            work: database,
        };
        let router = tada::serve::routes(api, mcp, None);
        let mcp = Self {
            router,
            test,
            clock,
            testwil,
            owner,
            other_owner,
            anna,
            anna_caller,
            open_day,
            secret,
            fly_in,
            token: created.secret.expose_secret().to_owned(),
        };
        mcp.add_role(open_day, EventRole::EventViewer).await;
        mcp
    }

    /// Sends a JSON-RPC request with `token` and returns the status and the JSON body.
    async fn rpc(
        &self,
        token: Option<&str>,
        origin: Option<&str>,
        body: &Value,
    ) -> (StatusCode, Value) {
        let (status, _, body) = self.rpc_traced(token, origin, body).await;
        (status, body)
    }

    /// Like `rpc`, and also returns the request ID of the response header.
    async fn rpc_traced(
        &self,
        token: Option<&str>,
        origin: Option<&str>,
        body: &Value,
    ) -> (StatusCode, Uuid, Value) {
        // An MCP client sends no `Origin`; `support::request` would add one to a POST.
        let mut request = support::request(Method::GET, "/mcp")
            .method(Method::POST)
            .header(header::HOST, "tada.example.org")
            .header(header::CONTENT_TYPE, "application/json")
            .header(header::ACCEPT, "application/json, text/event-stream");
        if let Some(token) = token {
            request = request.header(header::AUTHORIZATION, format!("Bearer {token}"));
        }
        if let Some(origin) = origin {
            request = request.header(header::ORIGIN, origin);
        }
        let request = request.body(Body::from(body.to_string())).unwrap();
        send_traced(&self.router, request).await
    }

    /// Calls the tool `name` with Anna's token and returns the JSON-RPC response.
    async fn call(&self, name: &str, arguments: Value) -> Value {
        let body = json!({
            "jsonrpc": "2.0", "id": 1, "method": "tools/call",
            "params": {"name": name, "arguments": arguments},
        });
        let (status, body) = self.rpc(Some(&self.token), None, &body).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        body
    }

    /// The structured result of a successful tool call.
    async fn result(&self, name: &str, arguments: Value) -> Value {
        let body = self.call(name, arguments).await;
        assert_eq!(body["result"]["isError"], false, "{body}");
        body["result"]["structuredContent"].clone()
    }

    /// Creates a changeset of the owner with `proposals` and returns it.
    async fn propose(&self, event: Option<EventId>, text: &str, proposals: Value) -> Changeset {
        let input: NewChangeset = serde_json::from_value(json!({
            "event_id": event.map(EventId::as_uuid), "source_text": text, "proposals": proposals,
        }))
        .unwrap();
        let stores = ProposeStores {
            identity: &self.test.database,
            facts: &self.test.database,
            proposals: &self.test.database,
            sources: &self.test.database,
            documents: &self.test.database,
            workstreams: &self.test.database,
            parties: &self.test.database,
            work: &self.test.database,
        };
        match create_changeset(&self.owner, input, stores, &*self.clock)
            .await
            .unwrap()
        {
            Created::New(changeset) => changeset,
            Created::Existing(_) => panic!("not new"),
        }
    }

    /// Accepts the proposals `selected` of `changeset` as the owner.
    async fn apply(&self, changeset: &Changeset, selected: &[Uuid]) {
        let stores = ReviewStores {
            identity: &self.test.database,
            facts: &self.test.database,
            proposals: &self.test.database,
            review: &self.test.database,
            sources: &self.test.database,
            workstreams: &self.test.database,
            work: &self.test.database,
            parties: &self.test.database,
        };
        let input = ApplyInput {
            selected: selected
                .iter()
                .copied()
                .map(ProposalId::from_uuid)
                .collect(),
            edits: Vec::new(),
        };
        apply_changeset(&self.owner, changeset.id, input, stores, &*self.clock)
            .await
            .unwrap();
    }

    /// Stores `text` as the words of `author` in the event `event`.
    async fn add_text(&self, author: &MemberCaller, event: EventId, text: &str) {
        let text = SourceText::normalize(text);
        self.test
            .database
            .add_member_text(
                author.scope(),
                event,
                &text,
                &author.actor(),
                self.clock.now(),
            )
            .await
            .unwrap();
    }

    /// Gives Anna the event role `role` in `event`.
    async fn add_role(&self, event: EventId, role: EventRole) {
        let database = &self.test.database;
        add_event_member(
            &self.owner,
            event,
            self.anna,
            role,
            database,
            database,
            &*self.clock,
        )
        .await
        .unwrap();
    }

    /// The source versions that the search of `caller` finds for `query`.
    async fn search_as(&self, caller: &MemberCaller, query: &str) -> Vec<Value> {
        let request = SearchRequest {
            event_key: None,
            query: query.to_owned(),
            limit: None,
        };
        let database = &self.test.database;
        search_sources(caller, request, database, database, database)
            .await
            .unwrap()
            .iter()
            .map(|hit| json!(hit.source_version_id.as_uuid()))
            .collect()
    }
}

/// The structured content of a tool result with `isError`: a refusal of the call, with its problem code.
fn problem(body: &Value) -> &Value {
    assert!(body["error"].is_null(), "not a tool result: {body}");
    assert_eq!(body["result"]["isError"], true, "{body}");
    let content = &body["result"]["structuredContent"];
    assert!(content["errors"].is_array(), "{body}");
    content
}

/// Sends a request and returns the status and the JSON body. Each response must forbid the referrer (ADR 0008).
async fn send(router: &Router, request: Request<Body>) -> (StatusCode, Value) {
    let (status, _, body) = send_traced(router, request).await;
    (status, body)
}

/// Like `send`, and also returns the request ID of the response header.
async fn send_traced(router: &Router, request: Request<Body>) -> (StatusCode, Uuid, Value) {
    let response = router.clone().oneshot(request).await.unwrap();
    assert_eq!(response.headers()[header::REFERRER_POLICY], "no-referrer");
    let request_id = response.headers()["x-request-id"]
        .to_str()
        .unwrap()
        .parse()
        .unwrap();
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    if bytes.is_empty() {
        return (status, request_id, Value::Null);
    }
    let body = serde_json::from_slice(&bytes)
        .unwrap_or_else(|_| panic!("{status}: not JSON: {}", String::from_utf8_lossy(&bytes)));
    (status, request_id, body)
}

fn set_fact(event: EventId, field: &str, value: Value) -> Value {
    json!({
        "kind": "set-fact", "event_id": event.as_uuid(), "field_id": core_field(field),
        "state": {"state": "accepted", "value": value},
    })
}

fn proposal(id: Uuid, operation: Value, depends_on: &[Uuid], text: &str, quote: &str) -> Value {
    json!({
        "id": id, "operation": operation, "depends_on": depends_on,
        "evidence": [passage(text, quote)], "reason": "The member says so.",
    })
}

fn may_2030() -> Value {
    json!({"type": "date-window", "start": "2030-05-01", "end": "2030-05-31", "granularity": "month"})
}

#[tokio::test]
async fn a_request_without_a_valid_token_or_with_a_foreign_origin_is_rejected() {
    let mcp = Mcp::start().await;
    let list = json!({"jsonrpc": "2.0", "id": 1, "method": "tools/list"});

    let (status, body) = mcp.rpc(None, None, &list).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body["code"], "unauthenticated");
    assert_eq!(
        body["type"],
        "https://github.com/zarubaf/tada/blob/main/doc/problems.md#unauthenticated"
    );
    assert_eq!(body["status"], 401);
    let (status, _) = mcp.rpc(Some("tada_pat_unknown"), None, &list).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    let (status, body) = mcp
        .rpc(Some(&mcp.token), Some("https://evil.example"), &list)
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(body["code"], "forbidden");
    assert_eq!(
        body["type"],
        "https://github.com/zarubaf/tada/blob/main/doc/problems.md#forbidden"
    );

    let (status, body) = mcp
        .rpc(Some(&mcp.token), Some(support::PUBLIC_URL), &list)
        .await;
    assert_eq!(status, StatusCode::OK);
    let names: Vec<&str> = body["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|tool| tool["name"].as_str().unwrap())
        .collect();
    assert_eq!(names.len(), 12, "{names:?}");
    for name in [
        "list_events",
        "get_event_schema",
        "get_event_profile",
        "search_sources",
        "get_source_passage",
        "list_documents",
        "get_document_version",
        "list_workstreams",
        "list_actions",
        "list_commitments",
        "search_parties",
        "propose_changeset",
    ] {
        assert!(names.contains(&name), "{names:?}");
    }
    // A member accepts and rejects in the Review Inbox only (ADR 0040).
    for name in &names {
        for verb in ["accept", "reject", "apply", "delete"] {
            assert!(!name.starts_with(verb), "{name}");
        }
    }

    // A session cookie is no way in: the server takes only tokens.
    let (_, _, cookie) = mcp.test.member("testwil", OrganizationRole::Owner).await;
    let request = support::request(Method::POST, "/mcp")
        .header(header::HOST, "tada.example.org")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::ACCEPT, "application/json, text/event-stream")
        .header(
            header::COOKIE,
            format!("{}={cookie}", support::SESSION_COOKIE),
        )
        .body(Body::from(list.to_string()))
        .unwrap();
    let (status, _) = send(&mcp.router, request).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    logs::assert_route_logged("/mcp");
    logs::assert_clean(&[&mcp.token, &cookie, "anna@example.org", "olga@example.org"]);
}

#[tokio::test]
async fn list_events_shows_only_the_events_of_the_member_in_its_organization() {
    let mcp = Mcp::start().await;
    let result = mcp.result("list_events", json!({})).await;
    let keys: Vec<&str> = result["events"]
        .as_array()
        .unwrap()
        .iter()
        .map(|event| event["event_key"].as_str().unwrap())
        .collect();
    assert_eq!(keys, ["OPEN30"]);
    assert_eq!(result["more"], false);
}

#[tokio::test]
async fn the_profile_keeps_open_proposals_apart_from_accepted_facts() {
    let mcp = Mcp::start().await;
    let (date, venue) = (Uuid::now_v7(), Uuid::now_v7());
    let changeset = mcp
        .propose(
            Some(mcp.open_day),
            SOURCE,
            json!([
                proposal(
                    date,
                    set_fact(mcp.open_day, "date_window", may_2030()),
                    &[],
                    SOURCE,
                    "im Mai 2030"
                ),
                proposal(
                    venue,
                    set_fact(
                        mcp.open_day,
                        "venue",
                        json!({"type": "text", "text": "Flugfeld"})
                    ),
                    &[],
                    SOURCE,
                    "auf dem Flugfeld"
                ),
            ]),
        )
        .await;
    mcp.apply(&changeset, &[date]).await;

    let profile = mcp
        .result("get_event_profile", json!({"event_key": "OPEN30"}))
        .await;
    let accepted = profile["accepted"].as_array().unwrap();
    assert_eq!(accepted.len(), 1, "{profile}");
    assert_eq!(accepted[0]["field_key"], "date_window");
    assert_eq!(accepted[0]["value"], may_2030());
    assert_eq!(accepted[0]["version"], 1);
    let evidence = &accepted[0]["evidence"][0];
    assert_eq!(
        evidence["source_version_id"],
        changeset.source_version_id.as_uuid().to_string()
    );
    assert_eq!(evidence["quote"], "im Mai 2030");
    assert_eq!(evidence["captured_at"], "2030-05-18T08:00:00Z");
    let olga = mcp.owner.user_id().as_uuid();
    assert_eq!(
        accepted[0]["accepted_by"],
        json!({"kind": "member", "id": olga, "channel": "web"})
    );
    assert_eq!(accepted[0]["accepted_at"], "2030-05-18T08:00:00Z");

    let open = profile["open_proposals"].as_array().unwrap();
    assert_eq!(open.len(), 1, "{profile}");
    assert_eq!(open[0]["proposal_id"], venue.to_string());
    assert_eq!(open[0]["field_key"], "venue");
    assert_eq!(
        open[0]["value"],
        json!({"type": "text", "text": "Flugfeld"})
    );
    assert!(profile["assumptions"].as_array().unwrap().is_empty());
    assert!(profile["unknowns"].as_array().unwrap().is_empty());

    // The citation of the evidence gives the same quote.
    let quote = mcp
        .result(
            "get_source_passage",
            json!({"source_version_id": evidence["source_version_id"], "start": evidence["start"], "end": evidence["end"]}),
        )
        .await;
    assert_eq!(quote["quote"], "im Mai 2030");

    let schema = mcp
        .result("get_event_schema", json!({"event_key": "OPEN30"}))
        .await;
    let audience = schema["fields"]
        .as_array()
        .unwrap()
        .iter()
        .find(|field| field["key"] == "audience")
        .unwrap();
    assert_eq!(
        audience["value_schema"]["properties"]["keys"]["items"]["enum"],
        json!(["public", "members", "invited"])
    );
    assert!(schema["catalog_version"].as_u64().unwrap() >= 1);

    // An event without a role is not found, as an event that does not exist.
    let body = mcp
        .call("get_event_profile", json!({"event_key": "SECRET30"}))
        .await;
    assert_eq!(problem(&body)["code"], "not-found", "{body}");
    let body = mcp
        .call("get_event_profile", json!({"event_key": "NONE30"}))
        .await;
    assert_eq!(problem(&body)["code"], "not-found", "{body}");
}

#[tokio::test]
async fn search_never_returns_a_source_of_another_organization_or_of_an_event_without_a_role() {
    let mcp = Mcp::start().await;
    mcp.add_text(&mcp.owner, mcp.open_day, SOURCE).await;
    mcp.add_text(&mcp.owner, mcp.secret, "Das Flugfeld ist geheim.")
        .await;
    mcp.add_text(&mcp.other_owner, mcp.fly_in, "Das Flugfeld Musterhausen.")
        .await;

    let result = mcp
        .result("search_sources", json!({"query": "Flugfeld"}))
        .await;
    let hits = result["hits"].as_array().unwrap();
    assert_eq!(hits.len(), 1, "{result}");
    assert!(hits[0]["snippet"].as_str().unwrap().contains("Flugfeld"));
    let open_day_source = hits[0]["source_version_id"].clone();

    // The owner reads all sources of Testwil, but never one of Musterhausen.
    let owner_hits = mcp.search_as(&mcp.owner, "Flugfeld").await;
    assert_eq!(owner_hits.len(), 2);
    assert!(owner_hits.contains(&open_day_source));

    // A source that the member cannot read has no passage either.
    for source in mcp
        .search_as(&mcp.owner, "geheim")
        .await
        .into_iter()
        .chain(mcp.search_as(&mcp.other_owner, "Musterhausen").await)
    {
        let body = mcp
            .call(
                "get_source_passage",
                json!({"source_version_id": source, "start": 0, "end": 3}),
            )
            .await;
        assert_eq!(problem(&body)["code"], "not-found", "{body}");
    }

    let result = mcp
        .result(
            "search_sources",
            json!({"event_key": "OPEN30", "query": "Flugfeld"}),
        )
        .await;
    assert_eq!(result["hits"].as_array().unwrap().len(), 1, "{result}");
    let body = mcp
        .call(
            "search_sources",
            json!({"event_key": "SECRET30", "query": "Flugfeld"}),
        )
        .await;
    assert_eq!(problem(&body)["code"], "not-found", "{body}");
    let body = mcp.call("search_sources", json!({"query": " "})).await;
    assert_eq!(problem(&body)["code"], "validation-failed", "{body}");
    assert_eq!(
        problem(&body)["errors"],
        json!([{"pointer": "/query", "code": "empty"}]),
        "a JSON pointer as in the API: {body}"
    );
}

#[tokio::test]
async fn a_member_reads_the_organization_source_that_the_facts_of_its_event_cite() {
    let mcp = Mcp::start().await;
    let (event, create_id, window) = (Uuid::now_v7(), Uuid::now_v7(), Uuid::now_v7());
    let june = json!({"type": "date-window", "start": "2030-06-01", "end": "2030-06-30", "granularity": "month"});
    let create =
        json!({"kind": "create-event", "id": event, "key": "FEST30", "name": "Hangarfest"});
    let set = set_fact(EventId::from_uuid(event), "date_window", june);
    let changeset = mcp
        .propose(
            None,
            ORGANIZATION_SOURCE,
            json!([
                proposal(create_id, create, &[], ORGANIZATION_SOURCE, "Hangarfest"),
                proposal(
                    window,
                    set,
                    &[create_id],
                    ORGANIZATION_SOURCE,
                    "im Juni 2030"
                ),
            ]),
        )
        .await;
    mcp.apply(&changeset, &[create_id, window]).await;

    // The intake text belongs to no event. Without a role in the new event, Anna cannot read it.
    let result = mcp
        .result("search_sources", json!({"query": "Hangarfest"}))
        .await;
    assert!(result["hits"].as_array().unwrap().is_empty(), "{result}");
    let source = changeset.source_version_id.as_uuid();
    let citation = json!({"source_version_id": source, "start": 14, "end": 24});
    let body = mcp.call("get_source_passage", citation.clone()).await;
    assert_eq!(problem(&body)["code"], "not-found", "{body}");

    // As a viewer of the new event, she reads the text that its facts cite.
    mcp.add_role(EventId::from_uuid(event), EventRole::EventViewer)
        .await;
    let result = mcp
        .result("search_sources", json!({"query": "Hangarfest"}))
        .await;
    assert_eq!(
        result["hits"][0]["source_version_id"],
        source.to_string(),
        "{result}"
    );
    let result = mcp
        .result(
            "search_sources",
            json!({"event_key": "FEST30", "query": "Hangarfest"}),
        )
        .await;
    assert_eq!(result["hits"].as_array().unwrap().len(), 1, "{result}");
    assert_eq!(
        mcp.result("get_source_passage", citation).await["quote"],
        "Hangarfest"
    );
    let profile = mcp
        .result("get_event_profile", json!({"event_key": "FEST30"}))
        .await;
    assert_eq!(
        profile["accepted"][0]["evidence"][0]["source_version_id"],
        source.to_string()
    );
}

/// The protocol version that current MCP clients, for example Claude Code, negotiate.
const PROTOCOL_VERSION: &str = "2025-06-18";

/// A POST of a real MCP client: the token, the Accept header of the Streamable HTTP transport and,
/// after the handshake, the negotiated protocol version.
async fn client_post(mcp: &Mcp, protocol: Option<&str>, body: &Value) -> (StatusCode, Value) {
    let mut request = Request::post("/mcp")
        .header(header::HOST, "tada.example.org")
        .header(header::AUTHORIZATION, format!("Bearer {}", mcp.token))
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::ACCEPT, "application/json, text/event-stream")
        .extension(axum::extract::ConnectInfo(std::net::SocketAddr::new(
            support::PEER,
            40000,
        )));
    if let Some(protocol) = protocol {
        request = request.header("mcp-protocol-version", protocol);
    }
    send(
        &mcp.router,
        request.body(Body::from(body.to_string())).unwrap(),
    )
    .await
}

/// The flow of a real MCP client against the stateless server: handshake, tool list and tool call.
#[tokio::test]
async fn a_client_completes_the_handshake_and_calls_a_tool() {
    let mcp = Mcp::start().await;
    let initialize = json!({
        "jsonrpc": "2.0", "id": 0, "method": "initialize",
        "params": {
            "protocolVersion": PROTOCOL_VERSION,
            "capabilities": {"roots": {"listChanged": true}},
            "clientInfo": {"name": "claude-code", "version": "2.0.0"},
        },
    });
    let (status, body) = client_post(&mcp, None, &initialize).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let result = &body["result"];
    assert_eq!(result["protocolVersion"], PROTOCOL_VERSION);
    assert_eq!(result["serverInfo"]["name"], "tada");
    assert!(result["capabilities"]["tools"].is_object(), "{body}");
    assert!(
        result["instructions"]
            .as_str()
            .unwrap()
            .contains("Never fill in")
    );
    let instructions = result["instructions"].as_str().unwrap();
    assert!(instructions.contains("search_parties"), "{instructions}");
    assert!(instructions.contains("condition"), "{instructions}");

    let initialized = json!({"jsonrpc": "2.0", "method": "notifications/initialized"});
    let (status, body) = client_post(&mcp, Some(PROTOCOL_VERSION), &initialized).await;
    assert_eq!(status, StatusCode::ACCEPTED, "{body}");

    let list = json!({"jsonrpc": "2.0", "id": 1, "method": "tools/list"});
    let (status, body) = client_post(&mcp, Some(PROTOCOL_VERSION), &list).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let tools = body["result"]["tools"].as_array().unwrap();
    assert_eq!(tools.len(), 12);
    for tool in tools {
        assert_eq!(tool["inputSchema"]["type"], "object", "{tool}");
        assert_eq!(tool["outputSchema"]["type"], "object", "{tool}");
        let read_only = tool["name"] != "propose_changeset";
        assert_eq!(tool["annotations"]["readOnlyHint"], read_only, "{tool}");
        assert_ne!(tool["annotations"]["destructiveHint"], true, "{tool}");
    }
    // Without the ID of the changeset, a retry after a lost response is refused with `taken`.
    let propose = tools
        .iter()
        .find(|tool| tool["name"] == "propose_changeset")
        .unwrap();
    assert!(
        propose["description"]
            .as_str()
            .unwrap()
            .contains("Always send a new UUIDv7 as the id of the changeset"),
        "{propose}"
    );
    // The schema of app is the one place of the rules of each field (ADR 0040), for example how a draft
    // cites a fact (ADR 0051). The description does not repeat them.
    let schema = propose["inputSchema"].to_string();
    for rule in [
        "[](tada:fact/<fact-uuid>?v=<n>)",
        "lowercase",
        "`https` or `mailto`",
        "a retry with the same id is safe",
    ] {
        assert!(schema.contains(rule), "{rule}: {schema}");
    }
    let description = propose["description"].as_str().unwrap();
    for repeated in ["tada:fact/", "add-choice-value", "mailto", "depends_on"] {
        assert!(!description.contains(repeated), "{repeated}: {description}");
    }
    // The list of documents has no next page, so its description says what `more` means.
    let documents = tools
        .iter()
        .find(|tool| tool["name"] == "list_documents")
        .unwrap();
    assert!(
        documents["description"]
            .as_str()
            .unwrap()
            .contains("If more is true"),
        "{documents}"
    );

    let call = json!({
        "jsonrpc": "2.0", "id": 2, "method": "tools/call",
        "params": {"name": "list_events", "arguments": {}},
    });
    let (status, body) = client_post(&mcp, Some(PROTOCOL_VERSION), &call).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body["result"]["structuredContent"]["events"][0]["event_key"],
        "OPEN30"
    );
    assert_eq!(body["result"]["isError"], false);

    // A client may open a stream with GET or end a session with DELETE; the stateless server has neither.
    for method in [Method::GET, Method::DELETE] {
        let request = Request::builder()
            .method(method.clone())
            .uri("/mcp")
            .header(header::HOST, "tada.example.org")
            .header(header::AUTHORIZATION, format!("Bearer {}", mcp.token))
            .header(header::ACCEPT, "text/event-stream")
            .header("mcp-protocol-version", PROTOCOL_VERSION)
            .body(Body::empty())
            .unwrap();
        let response = mcp.router.clone().oneshot(request).await.unwrap();
        assert_eq!(
            response.status(),
            StatusCode::METHOD_NOT_ALLOWED,
            "{method}"
        );
    }
}

/// A citation and a search write no notice and no member text to the log (ADR 0035).
#[tokio::test]
async fn reads_of_sources_leave_no_text_in_the_log() {
    let mcp = Mcp::start().await;
    mcp.add_text(&mcp.owner, mcp.open_day, SOURCE).await;
    let hits = mcp
        .result("search_sources", json!({"query": "Flugfeld"}))
        .await;
    let hit = &hits["hits"][0];
    let citation = json!({"source_version_id": hit["source_version_id"], "start": 0, "end": 7});
    assert_eq!(
        mcp.result("get_source_passage", citation).await["quote"],
        "Das Ope"
    );
    // The serde message of rmcp repeats the value; the server answers with the problem code only (ADR 0037).
    let wrong =
        json!({"source_version_id": hit["source_version_id"], "start": "Flugfeld", "end": 1});
    let body = mcp.call("get_source_passage", wrong).await;
    let refusal = problem(&body);
    assert_eq!(refusal["code"], "malformed-request", "{body}");
    assert_eq!(refusal["errors"], json!([]), "{body}");
    assert!(!body.to_string().contains("Flugfeld"), "{body}");
    // An unknown tool stays a JSON-RPC error.
    let body = mcp.call("accept_changeset", json!({})).await;
    assert_eq!(body["error"]["code"], -32602, "{body}");

    logs::assert_clean(&["Flugfeld", "Das Ope", "text-search query"]);
}

/// A changeset in `event` with `source_text` and `proposals`, as an agent sends it.
fn changeset(event: EventId, text: &str, proposals: Value) -> Value {
    json!({"event_id": event.as_uuid(), "source_text": text, "proposals": proposals})
}

impl Mcp {
    /// Creates a token of `scope` for Anna and returns its ID and its secret.
    async fn anna_token(&self, scope: TokenScope) -> Result<(Uuid, String), TokenError> {
        let request = TokenRequest {
            name: "Codex".to_owned(),
            scope,
            expires_at: self.clock.now() + SignedDuration::from_hours(24),
            notice_version_confirmed: NOTICE_VERSION,
        };
        let database = &self.test.database;
        // Anna signed in just now (`session::RECENT_SIGN_IN`).
        let anna = self.anna_caller.clone().with_sign_in(self.clock.now());
        let created = create_token(&anna, request, database, database, &*self.clock).await?;
        let secret = created.secret.expose_secret().to_owned();
        Ok((created.token.id.as_uuid(), secret))
    }

    /// Calls `propose_changeset` with `token` and returns the JSON-RPC response.
    async fn propose_with(&self, token: &str, arguments: Value) -> Value {
        let body = json!({
            "jsonrpc": "2.0", "id": 1, "method": "tools/call",
            "params": {"name": "propose_changeset", "arguments": arguments},
        });
        let (status, body) = self.rpc(Some(token), None, &body).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        body
    }

    /// Makes Anna a contributor of SECRET30 and returns the ID and the secret of her new `propose` token.
    async fn contributor_token(&self) -> (Uuid, String) {
        self.add_role(self.secret, EventRole::EventContributor)
            .await;
        self.anna_token(TokenScope::Propose).await.unwrap()
    }
}

/// The intake of ADR 0040: an agent proposes with the member's words, and a reviewer sees it in the Review Inbox.
#[tokio::test]
async fn a_changeset_through_mcp_waits_in_the_review_inbox_with_the_ai_as_author() {
    let mcp = Mcp::start().await;
    let (token_id, token) = mcp.contributor_token().await;
    // A text of the event that a passage cites by its source version.
    mcp.add_text(&mcp.owner, mcp.secret, "Der Ort ist der Hangar 3.")
        .await;
    let hits = mcp
        .result(
            "search_sources",
            json!({"event_key": "SECRET30", "query": "Hangar"}),
        )
        .await;
    let text = hits["hits"][0]["source_version_id"].clone();
    let (date, venue, changeset_id) = (Uuid::now_v7(), Uuid::now_v7(), Uuid::now_v7());
    let venue_evidence =
        json!([{"source_version_id": text, "start": 16, "end": 24, "quote": "Hangar 3"}]);
    let mut arguments = changeset(
        mcp.secret,
        SOURCE,
        json!([
            proposal(
                date,
                set_fact(mcp.secret, "date_window", may_2030()),
                &[],
                SOURCE,
                "im Mai 2030"
            ),
            {
                "id": venue,
                "operation": set_fact(mcp.secret, "venue", json!({"type": "text", "text": "Hangar 3"})),
                "evidence": venue_evidence,
                "reason": "The text of the event names the place.",
            },
        ]),
    );
    arguments["id"] = json!(changeset_id);

    let body = mcp.propose_with(&token, arguments.clone()).await;
    assert_eq!(body["result"]["isError"], false, "{body}");
    let result = &body["result"]["structuredContent"];
    assert_eq!(result["changeset_id"], changeset_id.to_string());
    assert_eq!(result["link"], format!("/inbox/{changeset_id}"));
    assert_eq!(result["existing"], false);
    // A retry with the same ID changes nothing (ADR 0038).
    let body = mcp.propose_with(&token, arguments).await;
    assert_eq!(
        body["result"]["structuredContent"]["existing"], true,
        "{body}"
    );

    // Olga reviews in the web client with a session.
    let cookie = mcp
        .test
        .sign_in(mcp.owner.user_id(), Some(mcp.testwil), mcp.clock.now())
        .await;
    let request = support::request(Method::GET, "/api/v1/changesets?status=open")
        .header(
            header::COOKIE,
            format!("{}={cookie}", support::SESSION_COOKIE),
        )
        .body(Body::empty())
        .unwrap();
    let web = support::session_router(&mcp.test, mcp.clock.clone());
    let (status, inbox) = send(&web, request).await;
    assert_eq!(status, StatusCode::OK, "{inbox}");
    let items = inbox["items"].as_array().unwrap();
    assert_eq!(items.len(), 1, "{inbox}");
    assert_eq!(items[0]["id"], changeset_id.to_string());
    assert_eq!(items[0]["open_proposals"], 2);
    assert_eq!(
        items[0]["author"],
        json!({"kind": "ai", "id": token_id, "principal_id": mcp.anna.as_uuid(), "channel": "api-token"})
    );

    let channel: String = mcp
        .test
        .scalar(&format!(
            "SELECT v.channel FROM source_version v JOIN changeset c ON c.source_version_id = v.id \
             WHERE c.id = '{changeset_id}'"
        ))
        .await;
    assert_eq!(channel, "api-token");

    logs::assert_clean(&[&token, "Flugfeld", "Hangar 3", "im Mai 2030"]);
}

/// One request ID connects an MCP call with its audit records, its answer and its log lines (ADR 0035, ADR 0039).
#[tokio::test]
async fn the_records_and_the_refusals_of_an_mcp_call_name_its_request() {
    let mcp = Mcp::start().await;
    let (_, token) = mcp.contributor_token().await;
    let changeset_id = Uuid::now_v7();
    let mut arguments = changeset(
        mcp.secret,
        SOURCE,
        json!([proposal(
            Uuid::now_v7(),
            set_fact(mcp.secret, "date_window", may_2030()),
            &[],
            SOURCE,
            "im Mai 2030"
        )]),
    );
    arguments["id"] = json!(changeset_id);
    let call = |arguments: Value| {
        json!({
            "jsonrpc": "2.0", "id": 1, "method": "tools/call",
            "params": {"name": "propose_changeset", "arguments": arguments},
        })
    };

    let (status, request_id, body) = mcp.rpc_traced(Some(&token), None, &call(arguments)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["result"]["isError"], false, "{body}");
    let author: Uuid = mcp
        .test
        .scalar(&format!(
            "SELECT (author->>'request_id')::uuid FROM changeset WHERE id = '{changeset_id}'"
        ))
        .await;
    assert_eq!(author, request_id);
    let audit: Uuid = mcp
        .test
        .scalar(&format!(
            "SELECT request_id FROM audit_event \
             WHERE action = 'changeset.create' AND record_id = '{changeset_id}'"
        ))
        .await;
    assert_eq!(audit, request_id);

    // A refusal of a tool names the request.
    let refused = changeset(mcp.secret, SOURCE, json!([]));
    let (_, request_id, body) = mcp.rpc_traced(Some(&token), None, &call(refused)).await;
    assert_eq!(
        problem(&body)["request_id"],
        request_id.to_string(),
        "{body}"
    );

    // A refusal of the guard is a problem with the request ID (ADR 0037).
    let list = json!({"jsonrpc": "2.0", "id": 1, "method": "tools/list"});
    let (status, request_id, body) = mcp.rpc_traced(None, None, &list).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body["request_id"], request_id.to_string(), "{body}");
    assert_eq!(body["instance"], format!("urn:uuid:{request_id}"), "{body}");
}

#[tokio::test]
async fn a_proposal_without_evidence_is_rejected_without_its_text() {
    let mcp = Mcp::start().await;
    let (_, token) = mcp.contributor_token().await;
    let no_evidence = json!([{
        "id": Uuid::now_v7(),
        "operation": set_fact(mcp.secret, "date_window", may_2030()),
        "reason": "The member says so.",
    }]);
    let body = mcp
        .propose_with(&token, changeset(mcp.secret, SOURCE, no_evidence))
        .await;
    let data = &problem(&body);
    assert_eq!(data["code"], "validation-failed", "{body}");
    assert_eq!(
        data["errors"],
        json!([{"pointer": "/proposals/0/evidence", "code": "evidence-missing"}])
    );
    assert!(!body.to_string().contains("Flugfeld"), "{body}");

    let long = "a".repeat(100_001);
    let proposals = json!([proposal(
        Uuid::now_v7(),
        set_fact(mcp.secret, "date_window", may_2030()),
        &[],
        "aaaa",
        "aaaa"
    )]);
    let body = mcp
        .propose_with(&token, changeset(mcp.secret, &long, proposals))
        .await;
    assert_eq!(
        problem(&body)["errors"],
        json!([{"pointer": "/source_text", "code": "length"}]),
        "{body}"
    );
    assert!(!body.to_string().contains(&long[..100]));
}

/// A viewer cannot propose, also through MCP (ADR 0052, ADR 0039).
#[tokio::test]
async fn only_a_propose_token_of_a_member_who_can_propose_in_the_event_proposes() {
    let mcp = Mcp::start().await;
    let proposals = |event: EventId| {
        json!([proposal(
            Uuid::now_v7(),
            set_fact(event, "date_window", may_2030()),
            &[],
            SOURCE,
            "im Mai 2030"
        )])
    };

    // Anna is only a viewer of OPEN30, so she gets no `propose` token at all.
    assert!(matches!(
        mcp.anna_token(TokenScope::Propose).await,
        Err(TokenError::Forbidden)
    ));

    let (_, token) = mcp.contributor_token().await;
    // A contributor of SECRET30 is still a viewer of OPEN30.
    let open_day = changeset(mcp.open_day, SOURCE, proposals(mcp.open_day));
    let body = mcp.propose_with(&token, open_day).await;
    assert_eq!(problem(&body)["code"], "forbidden", "{body}");

    // A `read` token never proposes, also in an event where the member can.
    let secret = changeset(mcp.secret, SOURCE, proposals(mcp.secret));
    let body = mcp.propose_with(&mcp.token, secret.clone()).await;
    assert_eq!(problem(&body)["code"], "forbidden", "{body}");

    let body = mcp.propose_with(&token, secret).await;
    assert_eq!(body["result"]["isError"], false, "{body}");

    // The role counts at each call, not when the token was made.
    let database = &mcp.test.database;
    change_event_role(
        &mcp.owner,
        mcp.secret,
        mcp.anna,
        EventRole::EventViewer,
        RecordVersion::FIRST,
        database,
        database,
    )
    .await
    .unwrap();
    let again = changeset(mcp.secret, SOURCE, proposals(mcp.secret));
    let body = mcp.propose_with(&token, again).await;
    assert_eq!(problem(&body)["code"], "forbidden", "{body}");
}

/// The tool takes each operation of the `app` input, also a document draft (ADR 0051).
/// The draft waits in the Review Inbox, and after its acceptance the agent reads it back to write the next version.
#[tokio::test]
async fn an_agent_proposes_a_document_draft_and_reads_it_back() {
    let mcp = Mcp::start().await;
    let (_, token) = mcp.contributor_token().await;
    let date = Uuid::now_v7();
    let facts = mcp
        .propose(
            Some(mcp.secret),
            SOURCE,
            json!([proposal(
                date,
                set_fact(mcp.secret, "date_window", may_2030()),
                &[],
                SOURCE,
                "im Mai 2030"
            )]),
        )
        .await;
    mcp.apply(&facts, &[date]).await;
    let fact: Uuid = mcp.test.scalar("SELECT id FROM fact").await;
    let document = Uuid::now_v7();
    let markdown = format!("# Konzept\n\nDas Open Day ist am [](tada:fact/{fact}?v=1).\n");
    let draft = json!({
        "kind": "create-document-draft",
        "event_id": mcp.secret.as_uuid(),
        "document": {"new": {"id": document, "name": "Konzept Open Day"}},
        "markdown": markdown,
    });
    let proposal_id = Uuid::now_v7();
    let proposals = json!([proposal(proposal_id, draft, &[], SOURCE, "im Mai 2030")]);
    let body = mcp
        .propose_with(&token, changeset(mcp.secret, SOURCE, proposals))
        .await;
    assert_eq!(body["result"]["isError"], false, "{body}");
    let result = &body["result"]["structuredContent"];
    assert!(
        result["link"].as_str().unwrap().starts_with("/inbox/"),
        "{body}"
    );

    // Olga sees the draft in the Review Inbox with its Markdown and the value that its link cites.
    let changeset_id = result["changeset_id"].as_str().unwrap();
    let cookie = mcp
        .test
        .sign_in(mcp.owner.user_id(), Some(mcp.testwil), mcp.clock.now())
        .await;
    let request = support::request(Method::GET, &format!("/api/v1/changesets/{changeset_id}"))
        .header(
            header::COOKIE,
            format!("{}={cookie}", support::SESSION_COOKIE),
        )
        .body(Body::empty())
        .unwrap();
    let web = support::session_router(&mcp.test, mcp.clock.clone());
    let (status, review) = send(&web, request).await;
    assert_eq!(status, StatusCode::OK, "{review}");
    let rendering = &review["proposals"][0]["draft"];
    assert_eq!(rendering["markdown"], markdown);
    assert_eq!(rendering["lint_warnings"], json!([]));
    assert_eq!(
        rendering["links"][format!("tada:fact/{fact}?v=1")]["state"],
        "accepted"
    );

    let changeset = tada_app::proposals::ProposalStore::get(
        &mcp.test.database,
        mcp.owner.scope(),
        tada_app::domain::ids::ChangesetId::from_uuid(changeset_id.parse().unwrap()),
    )
    .await
    .unwrap()
    .unwrap()
    .0;
    mcp.apply(&changeset, &[proposal_id]).await;

    let listed = mcp
        .result("list_documents", json!({"event_key": "SECRET30"}))
        .await;
    assert_eq!(listed["more"], false);
    let [item] = listed["documents"].as_array().unwrap().as_slice() else {
        panic!("not one document: {listed}");
    };
    assert_eq!(item["document_id"], document.to_string());
    assert_eq!(item["readable_id"], "DOC-001");
    assert_eq!(item["version"], 1);
    assert_eq!(item["newest_version"]["kind"], "draft");
    assert_eq!(item["newest_version"]["status"], "draft");
    let version_id = item["newest_version"]["version_id"].clone();

    let version = mcp
        .result("get_document_version", json!({"version_id": version_id}))
        .await;
    assert_eq!(version["document_id"], document.to_string());
    assert_eq!(version["number"], 1);
    assert_eq!(version["status"], "draft");
    assert_eq!(version["markdown"], markdown);
    assert_eq!(
        version["manifest"],
        json!({"facts": [{"fact_id": fact, "version": 1}], "sources": []})
    );

    // A version that the member cannot read is not found, and the agent sees the refusal as a tool result.
    let body = mcp
        .call(
            "get_document_version",
            json!({"version_id": Uuid::now_v7()}),
        )
        .await;
    assert_eq!(problem(&body)["code"], "not-found", "{body}");
    let body = mcp
        .call("list_documents", json!({"event_key": "FLY31"}))
        .await;
    assert_eq!(problem(&body)["code"], "not-found", "{body}");
}

impl Mcp {
    /// The owner creates the institution "Gemeinde Testwil" and returns it.
    async fn institution(&self) -> InstitutionId {
        let database = &self.test.database;
        let input = NewInstitution {
            id: None,
            name: "Gemeinde Testwil".to_owned(),
            kind: "authority".to_owned(),
            email: Some("bauamt@example.org".to_owned()),
            phone: None,
        };
        let shown = create_institution(&self.owner, input, database, database, &*self.clock)
            .await
            .unwrap();
        shown.record.id
    }

    /// The owner creates a commitment of `promisor` in `event`. A `condition` makes it conditional.
    async fn commitment(
        &self,
        event: EventId,
        promisor: Party,
        text: &str,
        condition: Option<&str>,
    ) {
        let database = &self.test.database;
        let ports = WorkPorts {
            identity: database,
            work: database,
            workstreams: database,
            parties: database,
            clock: &*self.clock,
        };
        let input = NewCommitment {
            id: None,
            text: text.to_owned(),
            condition: condition.map(str::to_owned),
            promisor,
            owner: self.owner.user_id(),
            workstream: None,
            due_date: None,
        };
        create_commitment(&self.owner, event, input, ports)
            .await
            .unwrap();
    }
}

/// The member reads the commitments of an event through MCP, and the status filter and the event role count.
#[tokio::test]
async fn mcp_lists_the_commitments_of_an_event() {
    let mcp = Mcp::start().await;
    let institution = Party::Institution(mcp.institution().await);
    mcp.commitment(
        mcp.open_day,
        institution,
        "Sperrt die Zufahrt",
        Some("wenn der Ort feststeht"),
    )
    .await;
    mcp.commitment(mcp.open_day, institution, "Stellt Tische", None)
        .await;
    mcp.commitment(mcp.secret, institution, "Geheim", None)
        .await;

    // Anna is a viewer of OPEN30: a `read` token reads.
    let all = mcp
        .result("list_commitments", json!({"event_key": "OPEN30"}))
        .await;
    let items = all["commitments"].as_array().unwrap();
    assert_eq!(items.len(), 2, "{all}");
    assert_eq!(all["more"], false);
    assert_eq!(items[0]["local_id"], "COM-001", "{all}");
    assert_eq!(items[0]["status"], "conditional");
    assert_eq!(items[0]["condition"], "wenn der Ort feststeht");
    assert_eq!(items[0]["promisor"]["local_id"], "INS-001");
    assert_eq!(items[0]["promisor"]["name"], "Gemeinde Testwil");
    assert_eq!(items[1]["status"], "firm");
    assert!(items[1]["condition"].is_null());
    // The rights of the member are not for the agent: it cannot change a record directly.
    assert!(items[0].get("can_change").is_none(), "{all}");

    let firm = mcp
        .result(
            "list_commitments",
            json!({"event_key": "OPEN30", "status": "firm"}),
        )
        .await;
    assert_eq!(firm["commitments"].as_array().unwrap().len(), 1, "{firm}");
    assert_eq!(firm["commitments"][0]["text"], "Stellt Tische");

    // Anna has no role in SECRET30.
    let body = mcp
        .call("list_commitments", json!({"event_key": "SECRET30"}))
        .await;
    assert_eq!(problem(&body)["code"], "not-found", "{body}");
}

#[tokio::test]
async fn mcp_lists_workstreams_and_actions() {
    let mcp = Mcp::start().await;
    let database = &mcp.test.database;
    let workstream = create_workstream(
        &mcp.owner,
        mcp.open_day,
        NewWorkstream {
            id: None,
            name: "Aufbau".to_owned(),
            lead: mcp.owner.user_id(),
        },
        database,
        database,
        &*mcp.clock,
    )
    .await
    .unwrap();
    let ports = WorkPorts {
        identity: database,
        work: database,
        workstreams: database,
        parties: database,
        clock: &*mcp.clock,
    };
    let action = NewAction {
        id: None,
        title: "Zelt bestellen".to_owned(),
        description: None,
        owner: mcp.owner.user_id(),
        workstream: Some(workstream.id),
        due_date: None,
    };
    create_action(&mcp.owner, mcp.open_day, action, ports)
        .await
        .unwrap();

    let streams = mcp
        .result("list_workstreams", json!({"event_key": "OPEN30"}))
        .await;
    assert_eq!(streams["workstreams"][0]["name"], "Aufbau", "{streams}");
    assert_eq!(streams["workstreams"][0]["status"], "active");
    let actions = mcp
        .result("list_actions", json!({"event_key": "OPEN30"}))
        .await;
    assert_eq!(actions["actions"][0]["local_id"], "ACT-001", "{actions}");
    assert_eq!(
        actions["actions"][0]["workstream_id"],
        json!(workstream.id.as_uuid())
    );
    let done = mcp
        .result(
            "list_actions",
            json!({"event_key": "OPEN30", "status": "done"}),
        )
        .await;
    assert!(done["actions"].as_array().unwrap().is_empty(), "{done}");
    let body = mcp
        .call(
            "list_actions",
            json!({"event_key": "OPEN30", "status": "later"}),
        )
        .await;
    assert_eq!(body["result"]["isError"], true, "{body}");
}

#[tokio::test]
async fn mcp_search_parties_finds_an_institution() {
    let mcp = Mcp::start().await;
    mcp.institution().await;
    let database = &mcp.test.database;
    let person = NewPerson {
        id: None,
        name: "Gerda Gemeinde".to_owned(),
        email: None,
        phone: None,
        user_id: None,
    };
    create_person(&mcp.owner, person, database, database, &*mcp.clock)
        .await
        .unwrap();

    let found = mcp
        .result("search_parties", json!({"q": "gemeinde testwil"}))
        .await;
    assert_eq!(
        found["institutions"].as_array().unwrap().len(),
        1,
        "{found}"
    );
    assert_eq!(found["institutions"][0]["local_id"], "INS-001");
    assert_eq!(found["institutions"][0]["kind"], "authority");
    assert_eq!(found["institutions"][0]["email"], "bauamt@example.org");
    assert!(found["persons"].as_array().unwrap().is_empty(), "{found}");

    let both = mcp.result("search_parties", json!({"q": "gemeinde"})).await;
    assert_eq!(both["persons"][0]["local_id"], "PER-001", "{both}");
    assert_eq!(both["institutions"].as_array().unwrap().len(), 1);
    assert_eq!(both["more"], false);

    // Another organization sees none of them.
    let none = mcp.result("search_parties", json!({"q": "zzz"})).await;
    assert!(none["persons"].as_array().unwrap().is_empty());
}

/// A viewer reads work through a `read` token; only a `propose` token proposes a commitment (ADR 0052).
#[tokio::test]
async fn a_viewer_token_reads_but_a_read_token_cannot_propose_a_commitment() {
    let mcp = Mcp::start().await;
    let institution = mcp.institution().await;
    mcp.commitment(
        mcp.open_day,
        Party::Institution(institution),
        "Stellt Tische",
        None,
    )
    .await;
    let listed = mcp
        .result("list_commitments", json!({"event_key": "OPEN30"}))
        .await;
    assert_eq!(
        listed["commitments"].as_array().unwrap().len(),
        1,
        "{listed}"
    );

    let (_, propose) = mcp.contributor_token().await;
    let arguments = changeset(
        mcp.secret,
        SOURCE,
        json!([proposal(
            Uuid::now_v7(),
            json!({
                "kind": "create-commitment", "id": Uuid::now_v7(), "event_id": mcp.secret.as_uuid(),
                "text": "Klärt die Bewilligung", "promisor": {"institution": institution.as_uuid()},
                "owner": mcp.anna.as_uuid(), "condition": "wenn der Ort feststeht",
            }),
            &[],
            SOURCE,
            "Der Ort ist noch offen"
        )]),
    );
    let body = mcp.propose_with(&mcp.token, arguments.clone()).await;
    assert_eq!(problem(&body)["code"], "forbidden", "{body}");
    let body = mcp.propose_with(&propose, arguments).await;
    assert_eq!(body["result"]["isError"], false, "{body}");
}
