//! `/api/v1/system/version`: the versions of the components (ADR 0075).

use axum::extract::State;
use jiff::Timestamp;
use serde::Serialize;
use tada_app::build_info::BuildInfo;
use tada_app::problem::ProblemCode;
use tada_app::store::StoreError;
use tada_app::system::{self as app, SystemVersion as AppSystemVersion};
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use crate::ApiState;
use crate::contract::{AUTHENTICATED, codes};
use crate::extract::Caller;
use crate::problem::{ApiError, Problem};

pub(crate) fn routes() -> OpenApiRouter<ApiState> {
    OpenApiRouter::new().routes(routes!(get_system_version))
}

/// The problem codes of each operation (ADR 0037).
pub(crate) fn problem_codes() -> Vec<(&'static str, Vec<ProblemCode>)> {
    vec![(
        "get_system_version",
        codes(&[AUTHENTICATED, StoreError::CODES]),
    )]
}

/// The versions of tada and of its components.
#[derive(Debug, Serialize, ToSchema)]
pub struct SystemVersion {
    /// The product version, for example `0.3.0`.
    pub product: String,
    /// The short Git commit of the build.
    pub commit: String,
    /// The number of the last applied migration.
    pub schema: i64,
    /// The version of the HTTP API.
    #[schema(example = "v1")]
    pub api: String,
    /// The version of the MCP server.
    pub mcp: String,
    /// The process roles that run now.
    pub roles: Vec<RoleVersion>,
    /// True if a running role has another version or commit than the process that answers.
    pub mismatch: bool,
}

/// One running process role.
#[derive(Debug, Serialize, ToSchema)]
pub struct RoleVersion {
    /// `serve`, `worker` or `telegram`.
    pub role: String,
    /// The product version of the role, or `unknown` for a role of a previous release.
    pub version: String,
    pub commit: String,
    /// The time of the last heartbeat.
    pub last_seen: Timestamp,
}

impl From<AppSystemVersion> for SystemVersion {
    fn from(version: AppSystemVersion) -> Self {
        Self {
            product: version.product,
            commit: version.commit,
            schema: version.schema,
            api: version.api.to_owned(),
            mcp: version.mcp,
            roles: version
                .roles
                .into_iter()
                .map(|role| RoleVersion {
                    role: role.role,
                    version: role.version,
                    commit: role.commit,
                    last_seen: role.last_seen,
                })
                .collect(),
            mismatch: version.mismatch,
        }
    }
}

/// Shows the versions of the product and of the running roles. Only a signed-in member can read them.
#[utoipa::path(
    get,
    path = "/system/version",
    operation_id = "get_system_version",
    tag = "system",
    responses(
        (status = OK, description = "The versions.", body = SystemVersion),
        (status = "default", description = "A problem (ADR 0037).", body = Problem, content_type = "application/problem+json"),
    ),
)]
async fn get_system_version(
    State(state): State<ApiState>,
    Caller(caller): Caller,
) -> Result<axum::Json<SystemVersion>, ApiError> {
    let version =
        app::system_version(&caller, state.versions.as_ref(), &BuildInfo::CURRENT).await?;
    Ok(axum::Json(version.into()))
}
