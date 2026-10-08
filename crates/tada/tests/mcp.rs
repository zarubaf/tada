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
use tada_app::domain::facts::core_catalog;
use tada_app::domain::identity::{DisplayName, Email, EventRole};
use tada_app::domain::ids::{EventId, OrganizationId, ProposalId, UserId};
use tada_app::domain::sources::SourceText;
use tada_app::event_members::add_event_member;
use tada_app::proposals::{Changeset, Created, NewChangeset, ProposeStores, create_changeset};
use tada_app::review::{ApplyInput, ReviewStores, apply_changeset};
use tada_app::search::{SearchRequest, search_sources};
use tada_app::sources::SourceStore;
use tada_app::tokens::{
    NOTICE_VERSION, TokenAuthenticator, TokenRequest, TokenScope, create_token,
};
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
    owner: MemberCaller,
    /// An owner of Musterhausen.
    other_owner: MemberCaller,
    anna: UserId,
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
        let anna_caller = MemberCaller::new(anna, testwil, OrganizationRole::Member);
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
            sources: database,
            public_url: support::public_url(),
        };
        let router = tada::serve::routes(api, mcp, None);
        let mcp = Self {
            router,
            test,
            clock,
            owner,
            other_owner,
            anna,
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
        send(&self.router, request).await
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
        assert!(body["error"].is_null(), "{body}");
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

/// Sends a request and returns the status and the JSON body. Each response must forbid the referrer (ADR 0008).
async fn send(router: &Router, request: Request<Body>) -> (StatusCode, Value) {
    let response = router.clone().oneshot(request).await.unwrap();
    assert_eq!(response.headers()[header::REFERRER_POLICY], "no-referrer");
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    if bytes.is_empty() {
        return (status, Value::Null);
    }
    let body = serde_json::from_slice(&bytes)
        .unwrap_or_else(|_| panic!("{status}: not JSON: {}", String::from_utf8_lossy(&bytes)));
    (status, body)
}

fn set_fact(event: EventId, field: &str, value: Value) -> Value {
    json!({
        "kind": "set_fact", "event_id": event.as_uuid(), "field_id": core_field(field),
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
    json!({"type": "date_window", "start": "2030-05-01", "end": "2030-05-31", "granularity": "month"})
}

#[tokio::test]
async fn a_request_without_a_valid_token_or_with_a_foreign_origin_is_rejected() {
    let mcp = Mcp::start().await;
    let list = json!({"jsonrpc": "2.0", "id": 1, "method": "tools/list"});

    let (status, body) = mcp.rpc(None, None, &list).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body["code"], "unauthenticated");
    let (status, _) = mcp.rpc(Some("tada_pat_unknown"), None, &list).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    let (status, body) = mcp
        .rpc(Some(&mcp.token), Some("https://evil.example"), &list)
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(body["code"], "forbidden");

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
    assert_eq!(names.len(), 5, "{names:?}");
    for name in [
        "list_events",
        "get_event_schema",
        "get_event_profile",
        "search_sources",
        "get_source_passage",
    ] {
        assert!(names.contains(&name), "{names:?}");
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
    assert_eq!(body["error"]["data"]["code"], "not-found", "{body}");
    let body = mcp
        .call("get_event_profile", json!({"event_key": "NONE30"}))
        .await;
    assert_eq!(body["error"]["data"]["code"], "not-found", "{body}");
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
        assert_eq!(body["error"]["data"]["code"], "not-found", "{body}");
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
    assert_eq!(body["error"]["data"]["code"], "not-found", "{body}");
    let body = mcp.call("search_sources", json!({"query": " "})).await;
    assert_eq!(body["error"]["data"]["code"], "validation-failed", "{body}");
}

#[tokio::test]
async fn a_member_reads_the_organization_source_that_the_facts_of_its_event_cite() {
    let mcp = Mcp::start().await;
    let (event, create_id, window) = (Uuid::now_v7(), Uuid::now_v7(), Uuid::now_v7());
    let june = json!({"type": "date_window", "start": "2030-06-01", "end": "2030-06-30", "granularity": "month"});
    let create =
        json!({"kind": "create_event", "id": event, "key": "FEST30", "name": "Hangarfest"});
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
    assert_eq!(body["error"]["data"]["code"], "not-found", "{body}");

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

    let initialized = json!({"jsonrpc": "2.0", "method": "notifications/initialized"});
    let (status, body) = client_post(&mcp, Some(PROTOCOL_VERSION), &initialized).await;
    assert_eq!(status, StatusCode::ACCEPTED, "{body}");

    let list = json!({"jsonrpc": "2.0", "id": 1, "method": "tools/list"});
    let (status, body) = client_post(&mcp, Some(PROTOCOL_VERSION), &list).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let tools = body["result"]["tools"].as_array().unwrap();
    assert_eq!(tools.len(), 5);
    for tool in tools {
        assert_eq!(tool["inputSchema"]["type"], "object", "{tool}");
        assert_eq!(tool["outputSchema"]["type"], "object", "{tool}");
        assert_eq!(tool["annotations"]["readOnlyHint"], true, "{tool}");
    }

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
    // A rejected input repeats the value in its serde message; rmcp would log it below `error`.
    let wrong =
        json!({"source_version_id": hit["source_version_id"], "start": "Flugfeld", "end": 1});
    let body = mcp.call("get_source_passage", wrong).await;
    assert_eq!(body["result"]["isError"], true, "{body}");
    assert!(
        body.to_string().contains("Flugfeld"),
        "the client sees its input: {body}"
    );

    logs::assert_clean(&["Flugfeld", "Das Ope", "text-search query"]);
}
