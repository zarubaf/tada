//! The MCP server (ADR 0040): a driving adapter for the AI clients of members.
//! It turns tool calls of the Model Context Protocol into `app` queries and contains no domain rules.
//!
//! Each request shows a personal API token (ADR 0039). The token authenticator gives an `AiCaller`,
//! never a member caller, so no tool can call a command that only a member can call.

mod guard;
mod propose;
mod tools;

use std::sync::Arc;

use axum::Router;
use axum::middleware;
use rmcp::transport::streamable_http_server::session::never::NeverSessionManager;
use rmcp::transport::streamable_http_server::{StreamableHttpServerConfig, StreamableHttpService};
use tada_app::clock::Clock;
use tada_app::documents::DocumentStore;
use tada_app::events::EventStore;
use tada_app::facts::FactStore;
use tada_app::identity::IdentityStore;
use tada_app::proposals::ProposalStore;
use tada_app::public_url::PublicUrl;
use tada_app::sources::SourceStore;
use tada_app::tokens::TokenAuthenticator;

/// The path of the MCP server under the public URL (ADR 0040).
pub const PATH: &str = "/mcp";

/// What the MCP server needs from the composition root.
#[derive(Debug, Clone)]
pub struct McpState {
    /// The only way in: a personal API token (ADR 0039).
    pub authenticator: Arc<TokenAuthenticator>,
    pub events: Arc<dyn EventStore>,
    pub identity: Arc<dyn IdentityStore>,
    pub facts: Arc<dyn FactStore>,
    pub sources: Arc<dyn SourceStore>,
    pub proposals: Arc<dyn ProposalStore>,
    /// The existing documents of draft proposals.
    pub documents: Arc<dyn DocumentStore>,
    pub clock: Arc<dyn Clock>,
    /// `TADA_PUBLIC_URL`. Its origin is the only `Origin` that a request can have.
    pub public_url: PublicUrl,
}

/// The MCP server with the Streamable HTTP transport. Mount it at `PATH`.
///
/// The server keeps no session: each request carries its token and gets its answer as JSON.
/// The guard checks the `Origin` and the token before the transport reads the body.
pub fn router(state: McpState) -> Router {
    let tools = tools::Tools::new(&state);
    // The guard checks the `Origin` against the public URL (DNS rebinding), and a browser never sends
    // the bearer token by itself. A reverse proxy can change the `Host`, so the transport does not check it.
    let config = StreamableHttpServerConfig::default()
        .with_legacy_session_mode(false)
        .with_json_response(true)
        .with_sse_keep_alive(None)
        .disable_allowed_hosts();
    let service = StreamableHttpService::new(
        move || Ok(tools.clone()),
        Arc::new(NeverSessionManager::default()),
        config,
    );
    Router::new()
        .fallback_service(service)
        .layer(middleware::from_fn_with_state(state, guard::check))
}
