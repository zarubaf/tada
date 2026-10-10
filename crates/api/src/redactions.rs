//! `/api/v1/redactions`: a legal redaction on the request of a person (ADR 0072).

use axum::extract::State;
use serde::{Deserialize, Serialize};
use tada_app::domain::redaction::{RedactionTarget, RedactionTargetKind};
use tada_app::problem::{FieldError, ProblemCode};
use tada_app::redaction::{self as app, RedactionError};
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

use crate::ApiState;
use crate::contract::{AUTHENTICATED, JSON_BODY, codes};
use crate::extract::{Caller, Json};
use crate::problem::{ApiError, Problem};

pub(crate) fn routes() -> OpenApiRouter<ApiState> {
    OpenApiRouter::new().routes(routes!(create_redaction))
}

/// The problem codes of each operation (ADR 0037).
pub(crate) fn problem_codes() -> Vec<(&'static str, Vec<ProblemCode>)> {
    vec![(
        "create_redaction",
        codes(&[AUTHENTICATED, JSON_BODY, RedactionError::CODES]),
    )]
}

/// One record to redact.
#[derive(Debug, Deserialize, ToSchema)]
pub struct RedactionTargetInput {
    /// `source-item` (all its versions), `fact-version`, `document-version`, `proposal`, `person` or `institution`.
    /// The kind `event` is for the retention job only and gets `validation-failed`.
    #[schema(example = "source-item")]
    pub kind: String,
    pub id: Uuid,
}

/// The input of `CreateRedaction`.
#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateRedactionRequest {
    /// At least one target. All targets change in one transaction.
    pub targets: Vec<RedactionTargetInput>,
    /// Always `request`: the redaction answers the request of a person.
    #[schema(example = "request")]
    pub reason: String,
}

/// What a redaction did.
#[derive(Debug, Serialize, ToSchema)]
pub struct RedactionResult {
    /// The IDs of the new redactions: one for each target that had content.
    pub applied: Vec<Uuid>,
    /// The number of targets that were redacted already.
    pub already: usize,
}

/// The targets of the request. An unknown kind or a reason other than `request` is invalid.
fn targets(request: &CreateRedactionRequest) -> Result<Vec<RedactionTarget>, RedactionError> {
    let mut errors = Vec::new();
    if request.reason != "request" {
        errors.push(FieldError::new("reason", "invalid"));
    }
    let targets = request
        .targets
        .iter()
        .enumerate()
        .filter_map(
            |(index, target)| match target.kind.parse::<RedactionTargetKind>() {
                Ok(kind) => Some(RedactionTarget {
                    kind,
                    id: target.id,
                }),
                Err(_) => {
                    errors.push(FieldError::new(format!("targets/{index}/kind"), "unknown"));
                    None
                }
            },
        )
        .collect();
    if errors.is_empty() {
        Ok(targets)
    } else {
        Err(RedactionError::Invalid(errors))
    }
}

/// Redacts records on the request of a person (ADR 0072): their content becomes a tombstone, their IDs stay.
/// It cannot be undone. A redacted target stays as it is and counts in `already`.
/// Only an owner can do this; a caller of another organization does not find the targets.
#[utoipa::path(
    post,
    path = "/redactions",
    operation_id = "create_redaction",
    tag = "parties",
    request_body = CreateRedactionRequest,
    responses(
        (status = OK, description = "What the redaction did.", body = RedactionResult),
        (status = "default", description = "A problem (ADR 0037).", body = Problem, content_type = "application/problem+json"),
    ),
)]
async fn create_redaction(
    State(state): State<ApiState>,
    Caller(caller): Caller,
    Json(request): Json<CreateRedactionRequest>,
) -> Result<axum::Json<RedactionResult>, ApiError> {
    let targets = targets(&request)?;
    let redacted = app::redact(
        &caller,
        &targets,
        state.redaction.as_ref(),
        state.blobs.as_ref(),
    )
    .await?;
    if redacted.undeleted_objects > 0 {
        // The sweep of orphan objects deletes them later (ADR 0074).
        tracing::warn!(
            undeleted_objects = redacted.undeleted_objects,
            "the objects of redacted versions stay until the orphan sweep"
        );
    }
    Ok(axum::Json(RedactionResult {
        applied: redacted
            .applied
            .into_iter()
            .map(|id| id.as_uuid())
            .collect(),
        already: redacted.already,
    }))
}
