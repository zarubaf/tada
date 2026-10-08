//! HTTP handlers, DTOs and the OpenAPI document.

mod client_ip;
mod contract;
mod documents;
mod event_members;
mod events;
mod extract;
mod facts;
mod health;
mod json_schema;
mod members;
mod origin;
mod problem;
mod request_id;
mod review;
mod roles;
mod sign_in;
mod telegram;
mod values;

use std::net::SocketAddr;
use std::num::NonZeroU64;
use std::path::Path;
use std::sync::Arc;

use axum::Router;
use axum::http::{HeaderValue, header};
use axum::middleware;
use axum::response::Response;
use axum::routing::any;
use ipnet::IpNet;
use tada_app::auth::Authenticator;
use tada_app::blobs::BlobStore;
use tada_app::clock::Clock;
use tada_app::documents::DocumentStore;
use tada_app::event_members::EventMemberStore;
use tada_app::events::EventStore;
use tada_app::facts::FactStore;
use tada_app::health::DependencyCheck;
use tada_app::identity::IdentityStore;
use tada_app::members::MemberStore;
use tada_app::problem::ProblemCode;
use tada_app::proposals::ProposalStore;
use tada_app::public_url::PublicUrl;
use tada_app::review::ReviewStore;
use tada_app::session::SessionStore;
use tada_app::sign_in::{SignInRequestStore, SignInStore};
use tada_app::sources::SourceStore;
use tada_app::telegram::TelegramLinks;
use tower_http::services::{ServeDir, ServeFile};
use utoipa::openapi::OpenApi;
use utoipa_axum::router::OpenApiRouter;

use crate::problem::ApiError;

/// All routes of the versioned API have this prefix (ADR 0017).
pub const API_PREFIX: &str = "/api/v1";

/// What the HTTP handlers need from the composition root.
#[derive(Debug, Clone)]
pub struct ApiState {
    /// The dependencies that `GET /readyz` checks.
    pub dependencies: Vec<Arc<dyn DependencyCheck>>,
    pub authenticator: Arc<dyn Authenticator>,
    pub events: Arc<dyn EventStore>,
    pub telegram: Arc<dyn TelegramLinks>,
    pub identity: Arc<dyn IdentityStore>,
    pub sessions: Arc<dyn SessionStore>,
    pub sign_in: Arc<dyn SignInStore>,
    pub sign_in_requests: Arc<dyn SignInRequestStore>,
    pub clock: Arc<dyn Clock>,
    /// The proxies whose `X-Request-Id` and `X-Forwarded-For` the server accepts (ADR 0008, ADR 0035).
    pub trusted_proxies: Vec<IpNet>,
    pub event_members: Arc<dyn EventMemberStore>,
    pub members: Arc<dyn MemberStore>,
    /// `TADA_PUBLIC_URL`. Its origin is the only `Origin` of a state-changing request (ADR 0008).
    pub public_url: PublicUrl,
    pub documents: Arc<dyn DocumentStore>,
    /// The object storage of the document files (ADR 0009).
    pub blobs: Arc<dyn BlobStore>,
    /// `TADA_UPLOAD_MAX_BYTES`: the largest file of one upload (ADR 0043).
    pub upload_max_bytes: NonZeroU64,
    // Facts and review (ADR 0049, ADR 0050).
    pub facts: Arc<dyn FactStore>,
    pub proposals: Arc<dyn ProposalStore>,
    pub review: Arc<dyn ReviewStore>,
    /// The source versions that drafts cite.
    pub sources: Arc<dyn SourceStore>,
}

pub use contract::{PROBLEM_CODES_EXTENSION, problem_catalog};
pub use extract::SESSION_COOKIE;

/// The versioned API: its routes and its OpenAPI document.
fn api() -> (Router<ApiState>, OpenApi) {
    let (router, document) = OpenApiRouter::<ApiState>::new()
        .nest(
            API_PREFIX,
            events::routes()
                .merge(telegram::routes())
                .merge(sign_in::routes())
                .merge(event_members::routes())
                .merge(members::routes())
                .merge(documents::routes())
                .merge(facts::routes())
                .merge(review::routes()),
        )
        .split_for_parts();
    let problem_codes = problem_codes().into_iter().collect();
    (router, contract::complete(document, &problem_codes))
}

/// The problem codes of each operation of the versioned API (ADR 0037).
fn problem_codes() -> Vec<(&'static str, Vec<ProblemCode>)> {
    events::problem_codes()
        .into_iter()
        .chain(telegram::problem_codes())
        .chain(sign_in::problem_codes())
        .chain(event_members::problem_codes())
        .chain(members::problem_codes())
        .chain(documents::problem_codes())
        .chain(facts::problem_codes())
        .chain(review::problem_codes())
        .collect()
}

/// All routes of the `serve` process role.
///
/// With `web_root`, the server also delivers the built web client from this folder (ADR 0005).
/// A path outside `/api` and `/.well-known` that is not a file gets `index.html`, so that the client handles its own routes.
pub fn router(state: ApiState, web_root: Option<&Path>) -> Router {
    router_with(state, web_root, Router::new())
}

/// Like `router`, with the routes of other driving adapters, for example the MCP server (ADR 0040).
/// They get the request ID, the request log and the referrer policy of the API.
pub fn router_with(state: ApiState, web_root: Option<&Path>, adapters: Router) -> Router {
    let (api, _) = api();
    let router = Router::new()
        .merge(api)
        .route("/api/{*path}", any(not_found))
        // tada serves no well-known resource. A client that probes one, for example the OAuth
        // discovery of an MCP client after a 401, must get 404, not the web client (ADR 0040).
        .route("/.well-known", any(not_found))
        .route("/.well-known/", any(not_found))
        .route("/.well-known/{*path}", any(not_found));
    let router = match web_root {
        Some(root) => router.fallback_service(
            ServeDir::new(root).fallback(ServeFile::new(root.join("index.html"))),
        ),
        None => router.fallback(not_found),
    };
    router
        .with_state(state.clone())
        .merge(adapters)
        .layer(middleware::from_fn_with_state(state.clone(), origin::check))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            request_id::track,
        ))
        .merge(health::routes().with_state(state))
        .layer(middleware::map_response(no_referrer))
}

/// No response sends its URL as the referrer of the next request (ADR 0008).
/// The sign-in pages of the web client then never leak a token or a path to another site.
async fn no_referrer(mut response: Response) -> Response {
    response.headers_mut().insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("no-referrer"),
    );
    response
}

/// The OpenAPI document of the versioned API.
pub fn openapi() -> OpenApi {
    api().1
}

async fn not_found() -> ApiError {
    ApiError::new(ProblemCode::NotFound)
}

/// Serves `router` on `listener` until `shutdown` completes, then waits for the open requests.
pub async fn serve(
    listener: tokio::net::TcpListener,
    router: Router,
    shutdown: impl Future<Output = ()> + Send + 'static,
) -> std::io::Result<()> {
    axum::serve(
        listener,
        router.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown)
    .await
}

#[cfg(test)]
mod tests {
    use std::fs;

    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tada_app::auth::{Authenticated, AuthenticationError, Credential};
    use tada_app::caller::{AiCaller, MemberCaller, OrganizationRole};
    use tada_app::domain::ids::{ApiTokenId, OrganizationId, UserId};
    use tada_app::tokens::TokenScope;
    use tower::ServiceExt;

    use super::*;

    #[derive(Debug)]
    struct NoCaller;

    #[async_trait::async_trait]
    impl Authenticator for NoCaller {
        async fn authenticate(
            &self,
            _credential: Option<Credential<'_>>,
        ) -> Result<Authenticated, AuthenticationError> {
            Err(AuthenticationError::Unauthenticated)
        }
    }

    /// An authenticator that finds an AI client of an owner for each request.
    #[derive(Debug)]
    struct AiClient;

    #[async_trait::async_trait]
    impl Authenticator for AiClient {
        async fn authenticate(
            &self,
            _credential: Option<Credential<'_>>,
        ) -> Result<Authenticated, AuthenticationError> {
            let owner = MemberCaller::new(
                UserId::from_uuid(uuid::Uuid::now_v7()),
                OrganizationId::from_uuid(uuid::Uuid::now_v7()),
                OrganizationRole::Owner,
            );
            let token = ApiTokenId::from_uuid(uuid::Uuid::now_v7());
            Ok(Authenticated::Ai(AiCaller::new(
                owner,
                token,
                TokenScope::Propose,
            )))
        }
    }

    #[derive(Debug)]
    struct NoClock;

    impl Clock for NoClock {
        fn now(&self) -> jiff::Timestamp {
            jiff::Timestamp::UNIX_EPOCH
        }
    }

    #[derive(Debug)]
    struct NoEvents;

    #[async_trait::async_trait]
    impl EventStore for NoEvents {
        async fn insert(
            &self,
            _: tada_app::caller::OrgScope,
            _: &tada_app::domain::events::Event,
            _: tada_app::domain::ids::UserId,
            _: &[tada_app::audit::AuditEvent],
        ) -> Result<tada_app::events::Inserted, tada_app::store::StoreError> {
            unreachable!()
        }

        async fn get(
            &self,
            _: tada_app::caller::OrgScope,
            _: tada_app::domain::ids::EventId,
        ) -> Result<Option<tada_app::domain::events::Event>, tada_app::store::StoreError> {
            unreachable!()
        }

        async fn find_by_key(
            &self,
            _: tada_app::caller::OrgScope,
            _: &tada_app::domain::events::EventKey,
        ) -> Result<Option<tada_app::domain::events::Event>, tada_app::store::StoreError> {
            unreachable!()
        }

        async fn list(
            &self,
            _: tada_app::caller::OrgScope,
            _: Option<&tada_app::events::EventCursor>,
            _: u32,
        ) -> Result<Vec<tada_app::domain::events::Event>, tada_app::store::StoreError> {
            unreachable!()
        }

        async fn list_of_member(
            &self,
            _: tada_app::caller::OrgScope,
            _: tada_app::domain::ids::UserId,
            _: Option<&tada_app::events::EventCursor>,
            _: u32,
        ) -> Result<Vec<tada_app::domain::events::Event>, tada_app::store::StoreError> {
            unreachable!()
        }
    }

    #[derive(Debug)]
    struct NoTelegram;

    #[async_trait::async_trait]
    impl TelegramLinks for NoTelegram {
        async fn create_code(
            &self,
            _: tada_app::caller::OrgScope,
            _: tada_app::domain::ids::UserId,
            _: jiff::Timestamp,
        ) -> Result<String, tada_app::store::StoreError> {
            unreachable!()
        }

        async fn claim(
            &self,
            _: &str,
            _: tada_app::telegram::TelegramUserId,
            _: &tada_app::telegram::TelegramName,
            _: jiff::Timestamp,
        ) -> Result<bool, tada_app::store::StoreError> {
            unreachable!()
        }

        async fn requests(
            &self,
            _: tada_app::caller::OrgScope,
            _: tada_app::domain::ids::UserId,
            _: jiff::Timestamp,
        ) -> Result<Vec<tada_app::telegram::LinkRequest>, tada_app::store::StoreError> {
            unreachable!()
        }

        async fn confirm(
            &self,
            _: tada_app::caller::OrgScope,
            _: tada_app::domain::ids::UserId,
            _: uuid::Uuid,
            _: jiff::Timestamp,
        ) -> Result<tada_app::telegram::Confirmed, tada_app::store::StoreError> {
            unreachable!()
        }

        async fn record_update(&self, _: i64) -> Result<bool, tada_app::store::StoreError> {
            unreachable!()
        }
    }

    /// The stores of sign-in. The tests of this module never sign in.
    #[derive(Debug)]
    struct NoSignIn;

    #[async_trait::async_trait]
    impl IdentityStore for NoSignIn {
        async fn user(
            &self,
            _: tada_app::domain::ids::UserId,
        ) -> Result<Option<tada_app::identity::UserRef>, tada_app::store::StoreError> {
            unreachable!()
        }

        async fn memberships_of(
            &self,
            _: tada_app::domain::ids::UserId,
        ) -> Result<Vec<tada_app::identity::Membership>, tada_app::store::StoreError> {
            unreachable!()
        }

        async fn membership(
            &self,
            _: tada_app::caller::OrgScope,
            _: tada_app::domain::ids::UserId,
        ) -> Result<Option<tada_app::caller::OrganizationRole>, tada_app::store::StoreError>
        {
            unreachable!()
        }

        async fn event_exists(
            &self,
            _: tada_app::caller::OrgScope,
            _: tada_app::domain::ids::EventId,
        ) -> Result<bool, tada_app::store::StoreError> {
            unreachable!()
        }

        async fn event_role(
            &self,
            _: tada_app::caller::OrgScope,
            _: tada_app::domain::ids::EventId,
            _: tada_app::domain::ids::UserId,
        ) -> Result<Option<tada_app::domain::identity::EventRole>, tada_app::store::StoreError>
        {
            unreachable!()
        }

        async fn event_roles_of(
            &self,
            _: tada_app::caller::OrgScope,
            _: tada_app::domain::ids::UserId,
        ) -> Result<
            Vec<(
                tada_app::domain::ids::EventId,
                tada_app::domain::identity::EventRole,
            )>,
            tada_app::store::StoreError,
        > {
            unreachable!()
        }
    }

    #[async_trait::async_trait]
    impl SessionStore for NoSignIn {
        async fn find(
            &self,
            _: &str,
            _: jiff::Timestamp,
        ) -> Result<Option<tada_app::session::SessionRow>, tada_app::store::StoreError> {
            unreachable!()
        }

        async fn touch(
            &self,
            _: &str,
            _: jiff::Timestamp,
        ) -> Result<(), tada_app::store::StoreError> {
            unreachable!()
        }

        async fn set_organization(
            &self,
            _: &str,
            _: Option<tada_app::domain::ids::OrganizationId>,
        ) -> Result<(), tada_app::store::StoreError> {
            unreachable!()
        }

        async fn delete(&self, _: &str) -> Result<(), tada_app::store::StoreError> {
            unreachable!()
        }
    }

    #[async_trait::async_trait]
    impl SignInStore for NoSignIn {
        async fn redeem_magic_link(
            &self,
            _: &str,
            _: Option<&str>,
            _: jiff::Timestamp,
        ) -> Result<Option<secrecy::SecretString>, tada_app::store::StoreError> {
            unreachable!()
        }

        async fn preview_invitation(
            &self,
            _: &str,
            _: jiff::Timestamp,
        ) -> Result<Option<tada_app::sign_in::InvitationPreview>, tada_app::store::StoreError>
        {
            unreachable!()
        }

        async fn accept_invitation(
            &self,
            _: &str,
            _: Option<&str>,
            _: Option<uuid::Uuid>,
            _: jiff::Timestamp,
        ) -> Result<Option<secrecy::SecretString>, tada_app::store::StoreError> {
            unreachable!()
        }
    }

    #[derive(Debug)]
    struct NoEventMembers;

    #[async_trait::async_trait]
    impl EventMemberStore for NoEventMembers {
        async fn list(
            &self,
            _: tada_app::caller::OrgScope,
            _: tada_app::domain::ids::EventId,
        ) -> Result<Vec<tada_app::event_members::EventMember>, tada_app::store::StoreError>
        {
            unreachable!()
        }

        async fn add(
            &self,
            _: tada_app::caller::OrgScope,
            _: tada_app::domain::ids::EventId,
            _: tada_app::domain::ids::UserId,
            _: tada_app::domain::identity::EventRole,
            _: jiff::Timestamp,
            _: &tada_app::audit::AuditEvent,
        ) -> Result<tada_app::event_members::Added, tada_app::store::StoreError> {
            unreachable!()
        }

        async fn change_role(
            &self,
            _: tada_app::caller::OrgScope,
            _: tada_app::domain::ids::EventId,
            _: tada_app::domain::ids::UserId,
            _: tada_app::domain::identity::EventRole,
            _: tada_app::domain::RecordVersion,
            _: &tada_app::audit::AuditEvent,
        ) -> Result<
            tada_app::event_members::Changed<tada_app::event_members::EventMember>,
            tada_app::store::StoreError,
        > {
            unreachable!()
        }

        async fn remove(
            &self,
            _: tada_app::caller::OrgScope,
            _: tada_app::domain::ids::EventId,
            _: tada_app::domain::ids::UserId,
            _: tada_app::domain::RecordVersion,
            _: &tada_app::audit::AuditEvent,
        ) -> Result<tada_app::event_members::Changed<()>, tada_app::store::StoreError> {
            unreachable!()
        }
    }

    #[derive(Debug)]
    struct NoMembers;

    #[async_trait::async_trait]
    impl MemberStore for NoMembers {
        async fn list(
            &self,
            _: tada_app::caller::OrgScope,
            _: Option<tada_app::members::MemberCursor>,
            _: u32,
        ) -> Result<Vec<tada_app::members::OrganizationMember>, tada_app::store::StoreError>
        {
            unreachable!()
        }

        async fn invite(
            &self,
            _: tada_app::caller::OrgScope,
            _: &tada_app::members::Invitation,
            _: tada_app::domain::ids::UserId,
            _: &tada_app::audit::AuditEvent,
        ) -> Result<tada_app::members::InvitationInsert, tada_app::store::StoreError> {
            unreachable!()
        }

        async fn invitation(
            &self,
            _: tada_app::caller::OrgScope,
            _: tada_app::domain::ids::InvitationId,
        ) -> Result<Option<tada_app::members::Invitation>, tada_app::store::StoreError> {
            unreachable!()
        }

        async fn pending_invitations(
            &self,
            _: tada_app::caller::OrgScope,
        ) -> Result<Vec<tada_app::members::Invitation>, tada_app::store::StoreError> {
            unreachable!()
        }

        async fn revoke(
            &self,
            _: tada_app::caller::OrgScope,
            _: tada_app::domain::ids::InvitationId,
            _: jiff::Timestamp,
            _: &tada_app::audit::AuditEvent,
        ) -> Result<bool, tada_app::store::StoreError> {
            unreachable!()
        }

        async fn remove(
            &self,
            _: tada_app::caller::OrgScope,
            _: tada_app::members::Remover,
            _: tada_app::domain::ids::UserId,
            _: tada_app::domain::RecordVersion,
            _: &tada_app::audit::AuditEvent,
        ) -> Result<Option<tada_app::members::Refusal>, tada_app::store::StoreError> {
            unreachable!()
        }
    }

    #[async_trait::async_trait]
    impl SignInRequestStore for NoSignIn {
        async fn queue_magic_link(
            &self,
            _: &tada_app::domain::identity::Email,
            _: &[tada_app::rate_limit::RateLimit<'_>],
            _: Option<uuid::Uuid>,
            _: jiff::Timestamp,
        ) -> Result<tada_app::rate_limit::RateDecision, tada_app::store::StoreError> {
            unreachable!()
        }
    }

    #[derive(Debug)]
    struct NoDocuments;

    #[async_trait::async_trait]
    impl DocumentStore for NoDocuments {
        async fn publish(
            &self,
            _: tada_app::caller::OrgScope,
            _: &tada_app::documents::NewUpload,
            _: &tada_app::audit::AuditEvent,
        ) -> Result<tada_app::documents::Published, tada_app::store::StoreError> {
            unreachable!()
        }

        async fn list(
            &self,
            _: tada_app::caller::OrgScope,
            _: tada_app::domain::ids::EventId,
            _: Option<&str>,
            _: Option<tada_app::documents::DocumentCursor>,
            _: u32,
        ) -> Result<Vec<tada_app::documents::DocumentView>, tada_app::store::StoreError> {
            unreachable!()
        }

        async fn get(
            &self,
            _: tada_app::caller::OrgScope,
            _: tada_app::domain::ids::DocumentId,
        ) -> Result<Option<tada_app::documents::DocumentView>, tada_app::store::StoreError>
        {
            unreachable!()
        }

        async fn versions(
            &self,
            _: tada_app::caller::OrgScope,
            _: tada_app::domain::ids::DocumentId,
        ) -> Result<Vec<tada_app::documents::VersionView>, tada_app::store::StoreError> {
            unreachable!()
        }

        async fn version(
            &self,
            _: tada_app::caller::OrgScope,
            _: tada_app::domain::ids::DocumentVersionId,
        ) -> Result<Option<tada_app::documents::StoredVersion>, tada_app::store::StoreError>
        {
            unreachable!()
        }
    }

    #[derive(Debug)]
    struct NoBlobs;

    #[async_trait::async_trait]
    impl BlobStore for NoBlobs {
        async fn put(
            &self,
            _: &tada_app::blobs::BlobKey,
            _: tada_app::blobs::ByteStream,
            _: u64,
        ) -> Result<u64, tada_app::blobs::BlobError> {
            unreachable!()
        }

        async fn get(
            &self,
            _: &tada_app::blobs::BlobKey,
        ) -> Result<Option<tada_app::blobs::ByteStream>, tada_app::blobs::BlobError> {
            unreachable!()
        }

        async fn head(
            &self,
            _: &tada_app::blobs::BlobKey,
        ) -> Result<Option<u64>, tada_app::blobs::BlobError> {
            unreachable!()
        }

        async fn delete(
            &self,
            _: &tada_app::blobs::BlobKey,
        ) -> Result<(), tada_app::blobs::BlobError> {
            unreachable!()
        }
    }

    /// The stores of facts and review. The tests of this module never read facts.
    #[derive(Debug)]
    struct NoReview;

    #[async_trait::async_trait]
    impl FactStore for NoReview {
        async fn catalog(
            &self,
            _: tada_app::caller::OrgScope,
            _: tada_app::domain::ids::EventId,
        ) -> Result<Vec<tada_app::domain::facts::FieldDefinition>, tada_app::store::StoreError>
        {
            unreachable!()
        }

        async fn profile(
            &self,
            _: tada_app::caller::OrgScope,
            _: tada_app::domain::ids::EventId,
        ) -> Result<tada_app::facts::EventProfile, tada_app::store::StoreError> {
            unreachable!()
        }

        async fn current_version(
            &self,
            _: tada_app::caller::OrgScope,
            _: tada_app::domain::ids::EventId,
            _: tada_app::domain::ids::FieldDefinitionId,
        ) -> Result<Option<tada_app::facts::FactVersionRef>, tada_app::store::StoreError> {
            unreachable!()
        }

        async fn existing_versions(
            &self,
            _: tada_app::caller::OrgScope,
            _: tada_app::domain::ids::EventId,
            _: &[(
                tada_app::domain::ids::FactId,
                tada_app::domain::RecordVersion,
            )],
        ) -> Result<
            Vec<(
                tada_app::domain::ids::FactId,
                tada_app::domain::RecordVersion,
            )>,
            tada_app::store::StoreError,
        > {
            unreachable!()
        }
    }

    #[async_trait::async_trait]
    impl SourceStore for NoReview {
        async fn add_member_text(
            &self,
            _: tada_app::caller::OrgScope,
            _: tada_app::domain::ids::EventId,
            _: &tada_app::domain::sources::SourceText,
            _: &tada_app::caller::Actor,
            _: jiff::Timestamp,
        ) -> Result<tada_app::sources::SourceVersionRef, tada_app::store::StoreError> {
            unreachable!()
        }

        async fn search(
            &self,
            _: tada_app::caller::OrgScope,
            _: &tada_app::access::SourceReach,
            _: &str,
            _: u32,
        ) -> Result<Vec<tada_app::sources::SourceHit>, tada_app::store::StoreError> {
            unreachable!()
        }

        async fn readable_text(
            &self,
            _: tada_app::caller::OrgScope,
            _: &tada_app::access::SourceReach,
            _: tada_app::domain::ids::SourceVersionId,
        ) -> Result<Option<String>, tada_app::store::StoreError> {
            unreachable!()
        }

        async fn texts(
            &self,
            _: tada_app::caller::OrgScope,
            _: &tada_app::access::SourceReach,
            _: &[tada_app::domain::ids::SourceVersionId],
        ) -> Result<Vec<tada_app::sources::SourceVersionText>, tada_app::store::StoreError>
        {
            unreachable!()
        }
    }

    #[async_trait::async_trait]
    impl ProposalStore for NoReview {
        async fn taken_ids(
            &self,
            _: &[uuid::Uuid],
        ) -> Result<Vec<uuid::Uuid>, tada_app::store::StoreError> {
            unreachable!()
        }

        async fn insert(
            &self,
            _: tada_app::caller::OrgScope,
            _: &tada_app::proposals::Changeset,
            _: &tada_app::domain::sources::SourceText,
            _: &tada_app::audit::AuditEvent,
        ) -> Result<tada_app::proposals::Inserted, tada_app::store::StoreError> {
            unreachable!()
        }

        async fn get(
            &self,
            _: tada_app::caller::OrgScope,
            _: tada_app::domain::ids::ChangesetId,
        ) -> Result<
            Option<(
                tada_app::proposals::Changeset,
                tada_app::domain::sources::SourceText,
            )>,
            tada_app::store::StoreError,
        > {
            unreachable!()
        }
    }

    #[async_trait::async_trait]
    impl ReviewStore for NoReview {
        async fn results(
            &self,
            _: tada_app::caller::OrgScope,
            _: tada_app::domain::ids::ChangesetId,
        ) -> Result<Vec<tada_app::review::ReviewRecord>, tada_app::store::StoreError> {
            unreachable!()
        }

        async fn apply(
            &self,
            _: tada_app::caller::OrgScope,
            _: &tada_app::review::ApplyPlan,
        ) -> Result<tada_app::review::ApplyOutcome, tada_app::store::StoreError> {
            unreachable!()
        }

        async fn record(
            &self,
            _: tada_app::caller::OrgScope,
            _: &tada_app::review::ReviewBatch,
        ) -> Result<tada_app::review::Recorded, tada_app::store::StoreError> {
            unreachable!()
        }

        async fn open_changesets(
            &self,
            _: tada_app::caller::OrgScope,
            _: Option<tada_app::domain::ids::EventId>,
        ) -> Result<Vec<tada_app::review::OpenChangeset>, tada_app::store::StoreError> {
            unreachable!()
        }
    }

    fn state() -> ApiState {
        ApiState {
            dependencies: Vec::new(),
            authenticator: Arc::new(NoCaller),
            events: Arc::new(NoEvents),
            telegram: Arc::new(NoTelegram),
            identity: Arc::new(NoSignIn),
            sessions: Arc::new(NoSignIn),
            sign_in: Arc::new(NoSignIn),
            sign_in_requests: Arc::new(NoSignIn),
            clock: Arc::new(NoClock),
            trusted_proxies: Vec::new(),
            event_members: Arc::new(NoEventMembers),
            members: Arc::new(NoMembers),
            public_url: PublicUrl::parse("https://tada.example.org").unwrap(),
            documents: Arc::new(NoDocuments),
            blobs: Arc::new(NoBlobs),
            upload_max_bytes: NonZeroU64::MIN,
            facts: Arc::new(NoReview),
            proposals: Arc::new(NoReview),
            review: Arc::new(NoReview),
            sources: Arc::new(NoReview),
        }
    }

    async fn get(router: &Router, path: &str) -> (StatusCode, String) {
        let response = router
            .clone()
            .oneshot(Request::get(path).body(Body::empty()).unwrap())
            .await
            .unwrap();
        let status = response.status();
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        (status, String::from_utf8(body.to_vec()).unwrap())
    }

    #[tokio::test]
    async fn serves_the_web_client_and_keeps_problems_for_the_api() {
        let web = tempfile::tempdir().unwrap();
        fs::write(web.path().join("index.html"), "<html>tada</html>").unwrap();
        fs::create_dir(web.path().join("assets")).unwrap();
        fs::write(web.path().join("assets/app.js"), "console.log(1)").unwrap();
        let router = router(state(), Some(web.path()));

        assert_eq!(
            get(&router, "/").await,
            (StatusCode::OK, "<html>tada</html>".to_owned())
        );
        assert_eq!(
            get(&router, "/assets/app.js").await,
            (StatusCode::OK, "console.log(1)".to_owned())
        );
        assert_eq!(
            get(&router, "/events/FLY28").await,
            (StatusCode::OK, "<html>tada</html>".to_owned())
        );

        let (status, body) = get(&router, "/api/v1/nothing").await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert!(body.contains("\"code\":\"not-found\""));
        let (status, _) = get(&router, "/api/v1/events").await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn the_rest_api_rejects_an_ai_client() {
        let router = router(
            ApiState {
                authenticator: Arc::new(AiClient),
                ..state()
            },
            None,
        );
        let (status, body) = get(&router, "/api/v1/events").await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert!(body.contains("\"code\":\"unauthenticated\""));
    }

    #[tokio::test]
    async fn without_the_web_client_each_unknown_path_is_a_problem() {
        let router = router(state(), None);
        let (status, body) = get(&router, "/events").await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert!(body.contains("\"code\":\"not-found\""));
    }

    #[tokio::test]
    async fn the_routes_of_another_adapter_get_the_request_id_and_the_referrer_policy() {
        let web = tempfile::tempdir().unwrap();
        fs::write(web.path().join("index.html"), "<html>tada</html>").unwrap();
        let adapter = Router::new().nest_service("/mcp", any(|| async { "mcp" }));
        let router = router_with(state(), Some(web.path()), adapter);

        let response = router
            .clone()
            .oneshot(Request::get("/mcp").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.headers()[header::REFERRER_POLICY], "no-referrer");
        assert!(response.headers().contains_key(request_id::HEADER));
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        assert_eq!(&body[..], b"mcp");
        assert_eq!(
            get(&router, "/").await,
            (StatusCode::OK, "<html>tada</html>".to_owned())
        );
    }

    #[tokio::test]
    async fn a_well_known_path_is_not_found_and_never_the_web_client() {
        let web = tempfile::tempdir().unwrap();
        fs::write(web.path().join("index.html"), "<html>tada</html>").unwrap();
        let router = router(state(), Some(web.path()));
        for path in [
            "/.well-known/oauth-protected-resource",
            "/.well-known/oauth-protected-resource/mcp",
            "/.well-known/oauth-authorization-server",
            "/.well-known/openid-configuration",
            "/.well-known/",
            "/.well-known",
        ] {
            let (status, body) = get(&router, path).await;
            assert_eq!(status, StatusCode::NOT_FOUND, "{path}");
            assert!(body.contains("\"code\":\"not-found\""), "{path}: {body}");
        }
    }
}
