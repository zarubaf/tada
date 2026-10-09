//! `/api/v1/tokens` and `/api/v1/organization/features`: personal API tokens and the MCP switch (ADR 0039, ADR 0045).
//!
//! The HTTP API accepts only sessions: the `Caller` extractor rejects a token caller.
//! A token opens `/mcp` only, which keeps the surface of the AI small.

use axum::extract::State;
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use jiff::Timestamp;
use secrecy::ExposeSecret;
use serde::{Deserialize, Serialize};
use tada_app::domain::ids::ApiTokenId;
use tada_app::domain::name::NAME_MAX_CHARS;
use tada_app::problem::ProblemCode;
use tada_app::tokens::{
    self as app, ApiToken as AppApiToken, Feature, FeatureState, NOTICE_VERSION, TokenError,
    TokenRequest, TokenScope,
};
use utoipa::ToSchema;
use utoipa::openapi::schema::{Object, ObjectBuilder, Type};
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

use crate::ApiState;
use crate::contract::{AUTHENTICATED, JSON_BODY, PATH, codes};
use crate::extract::{Caller, Json, Path, record_version};
use crate::problem::{ApiError, Problem};

pub(crate) fn routes() -> OpenApiRouter<ApiState> {
    OpenApiRouter::new()
        .routes(routes!(list_tokens, create_token))
        .routes(routes!(revoke_token))
        .routes(routes!(get_token_notice))
        .routes(routes!(list_organization_features))
        .routes(routes!(set_organization_feature))
}

/// The problem codes of each operation (ADR 0037).
pub(crate) fn problem_codes() -> Vec<(&'static str, Vec<ProblemCode>)> {
    let tokens = |extra: &[&[ProblemCode]]| {
        let mut lists = vec![AUTHENTICATED, TokenError::CODES];
        lists.extend_from_slice(extra);
        codes(&lists)
    };
    vec![
        ("list_tokens", tokens(&[])),
        ("create_token", tokens(&[JSON_BODY])),
        ("revoke_token", tokens(&[PATH])),
        ("get_token_notice", tokens(&[])),
        ("list_organization_features", tokens(&[])),
        ("set_organization_feature", tokens(&[PATH, JSON_BODY])),
    ]
}

/// What a token allows (ADR 0039).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "kebab-case")]
pub enum ApiTokenScope {
    /// Queries only.
    Read,
    /// Queries and proposals. The member needs the right to propose in an event.
    Propose,
}

impl From<TokenScope> for ApiTokenScope {
    fn from(scope: TokenScope) -> Self {
        match scope {
            TokenScope::Read => Self::Read,
            TokenScope::Propose => Self::Propose,
        }
    }
}

impl From<ApiTokenScope> for TokenScope {
    fn from(scope: ApiTokenScope) -> Self {
        match scope {
            ApiTokenScope::Read => Self::Read,
            ApiTokenScope::Propose => Self::Propose,
        }
    }
}

/// A switch of the organization.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "kebab-case")]
pub enum OrganizationFeatureName {
    /// Personal API tokens for MCP clients. It is on until an owner switches it off.
    McpTokens,
}

impl From<Feature> for OrganizationFeatureName {
    fn from(feature: Feature) -> Self {
        match feature {
            Feature::McpTokens => Self::McpTokens,
        }
    }
}

impl From<OrganizationFeatureName> for Feature {
    fn from(feature: OrganizationFeatureName) -> Self {
        match feature {
            OrganizationFeatureName::McpTokens => Self::McpTokens,
        }
    }
}

/// A token of the member. It never holds the secret.
#[derive(Debug, Serialize, ToSchema)]
pub struct ApiToken {
    pub id: Uuid,
    pub name: String,
    pub scope: ApiTokenScope,
    pub expires_at: Timestamp,
    /// The version of the notice that the member confirmed when the member created the token.
    pub notice_version: u32,
    pub created_at: Timestamp,
    /// The last time a client used the token. It is absent if no client used it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_used_at: Option<Timestamp>,
    /// The time of the revocation. It is absent for a token that is not revoked.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub revoked_at: Option<Timestamp>,
}

impl From<AppApiToken> for ApiToken {
    fn from(token: AppApiToken) -> Self {
        Self {
            id: token.id.as_uuid(),
            name: token.name.as_str().to_owned(),
            scope: token.scope.into(),
            expires_at: token.expires_at,
            notice_version: token.notice_version,
            created_at: token.created_at,
            last_used_at: token.last_used_at,
            revoked_at: token.revoked_at,
        }
    }
}

/// The tokens of the member. A member has few, so the list has one page.
#[derive(Debug, Serialize, ToSchema)]
pub struct ApiTokenList {
    pub items: Vec<ApiToken>,
}

/// A new token with its secret.
#[derive(Serialize, ToSchema)]
pub struct CreatedApiToken {
    pub token: ApiToken,
    /// The secret of the token. It shows once, in this response.
    pub secret: String,
}

/// The secret must not reach a log line (ADR 0035), so `Debug` leaves it out.
impl std::fmt::Debug for CreatedApiToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CreatedApiToken")
            .field("token", &self.token)
            .finish_non_exhaustive()
    }
}

/// The input of `CreateToken`.
#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateTokenRequest {
    #[schema(schema_with = token_name_schema)]
    pub name: String,
    pub scope: ApiTokenScope,
    /// The expiry. It is in the future and at most one year away.
    pub expires_at: Timestamp,
    /// The version of the notice that the member confirmed: `GetTokenNotice` gives it.
    /// Any other version than the current one, also a missing one, gives `validation-failed` with the code `not-confirmed`.
    pub notice_version_confirmed: Option<u32>,
}

/// The schema of a token name. The length limit is the one of each record name (`tada_domain::name`).
fn token_name_schema() -> Object {
    ObjectBuilder::new()
        .schema_type(Type::String)
        .description(Some(
            "A name that helps the member to find the token again. \
             tada removes the spaces at the ends and counts the characters (Unicode scalar values).",
        ))
        .min_length(Some(1))
        .max_length(Some(NAME_MAX_CHARS))
        .examples(["Claude Code"])
        .build()
}

/// The version of the token notice. The text of the notice is part of the web client.
#[derive(Debug, Serialize, ToSchema)]
pub struct TokenNotice {
    pub version: u32,
}

/// A switch with its value.
#[derive(Debug, Serialize, ToSchema)]
pub struct OrganizationFeature {
    pub feature: OrganizationFeatureName,
    pub enabled: bool,
    /// The record version. `SetOrganizationFeature` needs it.
    pub version: i64,
}

impl From<FeatureState> for OrganizationFeature {
    fn from(state: FeatureState) -> Self {
        Self {
            feature: state.feature.into(),
            enabled: state.enabled,
            version: state.version.get(),
        }
    }
}

/// The switches of the organization.
#[derive(Debug, Serialize, ToSchema)]
pub struct OrganizationFeatureList {
    pub items: Vec<OrganizationFeature>,
}

/// The input of `SetOrganizationFeature`.
#[derive(Debug, Deserialize, ToSchema)]
pub struct SetFeatureRequest {
    pub enabled: bool,
    #[schema(minimum = 1)]
    pub expected_version: i64,
}

/// Lists the API tokens of the member in this organization, the newest first.
/// The list holds the revoked and expired tokens, and never a secret.
#[utoipa::path(
    get,
    path = "/tokens",
    operation_id = "list_tokens",
    tag = "tokens",
    responses(
        (status = OK, description = "The tokens of the member.", body = ApiTokenList),
        (status = "default", description = "A problem (ADR 0037).", body = Problem, content_type = "application/problem+json"),
    ),
)]
async fn list_tokens(
    State(state): State<ApiState>,
    Caller(caller): Caller,
) -> Result<axum::Json<ApiTokenList>, ApiError> {
    let tokens = app::list_tokens(&caller, state.tokens.as_ref()).await?;
    Ok(axum::Json(ApiTokenList {
        items: tokens.into_iter().map(ApiToken::from).collect(),
    }))
}

/// Creates an API token for an MCP client of the member (ADR 0045).
///
/// The member must confirm the current notice with `notice_version_confirmed`.
/// A `propose` token needs the right to propose in an event.
/// If an owner switched off MCP tokens, or the member lacks the right to propose, the response is `forbidden`.
/// A client reads `GET /organization/features` to tell a switched-off MCP switch from a missing right.
/// The response holds the secret; the member sees it once.
#[utoipa::path(
    post,
    path = "/tokens",
    operation_id = "create_token",
    tag = "tokens",
    request_body = CreateTokenRequest,
    responses(
        (status = CREATED, description = "The new token with its secret.", body = CreatedApiToken),
        (status = "default", description = "A problem (ADR 0037).", body = Problem, content_type = "application/problem+json"),
    ),
)]
async fn create_token(
    State(state): State<ApiState>,
    Caller(caller): Caller,
    Json(request): Json<CreateTokenRequest>,
) -> Result<Response, ApiError> {
    let input = TokenRequest {
        name: request.name,
        scope: request.scope.into(),
        expires_at: request.expires_at,
        // Version 0 never matches: a missing confirmation is a validation problem, not a malformed body.
        notice_version_confirmed: request.notice_version_confirmed.unwrap_or(0),
    };
    let created = app::create_token(
        &caller,
        input,
        state.tokens.as_ref(),
        state.identity.as_ref(),
        state.clock.as_ref(),
    )
    .await?;
    let body = CreatedApiToken {
        token: created.token.into(),
        secret: created.secret.expose_secret().to_owned(),
    };
    let mut response = (StatusCode::CREATED, axum::Json(body)).into_response();
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    Ok(response)
}

/// Revokes a token of the member. The token stops working with the next request.
/// A second revocation changes nothing.
#[utoipa::path(
    post,
    path = "/tokens/{token_id}/revoke",
    operation_id = "revoke_token",
    tag = "tokens",
    params(("token_id" = Uuid, Path, description = "The ID of the token.")),
    responses(
        (status = NO_CONTENT, description = "The token is revoked."),
        (status = "default", description = "A problem (ADR 0037).", body = Problem, content_type = "application/problem+json"),
    ),
)]
async fn revoke_token(
    State(state): State<ApiState>,
    Caller(caller): Caller,
    Path(token_id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    app::revoke_token(
        &caller,
        ApiTokenId::from_uuid(token_id),
        state.tokens.as_ref(),
        state.clock.as_ref(),
    )
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Gives the version of the token notice (ADR 0045). The web client shows the text and sends the version
/// with `CreateToken` when the member confirms.
#[utoipa::path(
    get,
    path = "/token-notice",
    operation_id = "get_token_notice",
    tag = "tokens",
    responses(
        (status = OK, description = "The version of the notice.", body = TokenNotice),
        (status = "default", description = "A problem (ADR 0037).", body = Problem, content_type = "application/problem+json"),
    ),
)]
async fn get_token_notice(Caller(_): Caller) -> axum::Json<TokenNotice> {
    axum::Json(TokenNotice {
        version: NOTICE_VERSION,
    })
}

/// Lists the switches of the organization with their values. Each member can read them.
#[utoipa::path(
    get,
    path = "/organization/features",
    operation_id = "list_organization_features",
    tag = "organization",
    responses(
        (status = OK, description = "The switches.", body = OrganizationFeatureList),
        (status = "default", description = "A problem (ADR 0037).", body = Problem, content_type = "application/problem+json"),
    ),
)]
async fn list_organization_features(
    State(state): State<ApiState>,
    Caller(caller): Caller,
) -> Result<axum::Json<OrganizationFeatureList>, ApiError> {
    let features = app::get_features(&caller, state.tokens.as_ref()).await?;
    Ok(axum::Json(OrganizationFeatureList {
        items: features
            .into_iter()
            .map(OrganizationFeature::from)
            .collect(),
    }))
}

/// Switches a feature of the organization on or off. Only an owner can do this.
#[utoipa::path(
    post,
    path = "/organization/features/{feature}/set",
    operation_id = "set_organization_feature",
    tag = "organization",
    params(("feature" = OrganizationFeatureName, Path, description = "The switch.")),
    request_body = SetFeatureRequest,
    responses(
        (status = OK, description = "The new value of the switch.", body = OrganizationFeature),
        (status = "default", description = "A problem (ADR 0037).", body = Problem, content_type = "application/problem+json"),
    ),
)]
async fn set_organization_feature(
    State(state): State<ApiState>,
    Caller(caller): Caller,
    Path(feature): Path<OrganizationFeatureName>,
    Json(request): Json<SetFeatureRequest>,
) -> Result<axum::Json<OrganizationFeature>, ApiError> {
    let changed = app::set_feature(
        &caller,
        feature.into(),
        request.enabled,
        record_version(request.expected_version)?,
        state.tokens.as_ref(),
    )
    .await?;
    Ok(axum::Json(changed.into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_debug_output_has_no_secret() {
        let created = CreatedApiToken {
            token: ApiToken {
                id: Uuid::nil(),
                name: "Claude Code".to_owned(),
                scope: ApiTokenScope::Read,
                expires_at: Timestamp::UNIX_EPOCH,
                notice_version: 1,
                created_at: Timestamp::UNIX_EPOCH,
                last_used_at: None,
                revoked_at: None,
            },
            secret: "tada_pat_secret".to_owned(),
        };
        assert!(!format!("{created:?}").contains("tada_pat_secret"));
    }

    #[test]
    fn the_contract_takes_the_name_limit_from_the_domain() {
        use utoipa::PartialSchema;
        let schema = serde_json::to_value(CreateTokenRequest::schema()).unwrap();
        let name = &schema["properties"]["name"];
        assert_eq!(name["minLength"], 1);
        assert_eq!(name["maxLength"], NAME_MAX_CHARS);
    }
}
