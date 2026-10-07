//! Request extractors whose rejections are problem details (ADR 0037).

use axum::extract::rejection::{JsonRejection, PathRejection, QueryRejection};
use axum::extract::{FromRequest, FromRequestParts, Request};
use axum::http::header;
use axum::http::request::Parts;
use serde::de::DeserializeOwned;
use tada_app::auth::{AuthenticationError, Credential};
use tada_app::caller::{Channel, MemberCaller};
use tada_app::problem::ProblemCode;

use crate::ApiState;
use crate::problem::ApiError;
use crate::request_id;

/// The session cookie (ADR 0008).
const SESSION_COOKIE: &str = "__Host-tada-session";

/// A JSON request body.
#[derive(Debug)]
pub struct Json<T>(pub T);

impl<S, T> FromRequest<S> for Json<T>
where
    S: Send + Sync,
    T: DeserializeOwned,
{
    type Rejection = ApiError;

    async fn from_request(request: Request, state: &S) -> Result<Self, ApiError> {
        match axum::Json::<T>::from_request(request, state).await {
            Ok(axum::Json(value)) => Ok(Self(value)),
            // The serde message can repeat input values, so the detail is fixed text.
            Err(JsonRejection::MissingJsonContentType(_)) => {
                Err(ApiError::new(ProblemCode::UnsupportedMediaType)
                    .with_detail("The body must have the media type application/json."))
            }
            Err(JsonRejection::BytesRejection(_)) => {
                Err(ApiError::new(ProblemCode::PayloadTooLarge))
            }
            Err(_) => Err(ApiError::new(ProblemCode::MalformedRequest)
                .with_detail("The body is not valid JSON or does not match the schema.")),
        }
    }
}

/// The query parameters of a request.
#[derive(Debug)]
pub struct Query<T>(pub T);

impl<S, T> FromRequestParts<S> for Query<T>
where
    S: Send + Sync,
    T: DeserializeOwned,
{
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, ApiError> {
        axum::extract::Query::<T>::from_request_parts(parts, state)
            .await
            .map(|axum::extract::Query(value)| Self(value))
            .map_err(|_: QueryRejection| {
                ApiError::new(ProblemCode::MalformedRequest)
                    .with_detail("The query parameters do not match the schema.")
            })
    }
}

/// The path parameters of a request.
#[derive(Debug)]
pub struct Path<T>(pub T);

impl<S, T> FromRequestParts<S> for Path<T>
where
    S: Send + Sync,
    T: DeserializeOwned + Send,
{
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, ApiError> {
        axum::extract::Path::<T>::from_request_parts(parts, state)
            .await
            .map(|axum::extract::Path(value)| Self(value))
            .map_err(|_: PathRejection| {
                ApiError::new(ProblemCode::MalformedRequest)
                    .with_detail("The path parameters do not match the schema.")
            })
    }
}

/// The member who sends the request (ADR 0039).
#[derive(Debug)]
pub struct Caller(pub MemberCaller);

impl FromRequestParts<ApiState> for Caller {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &ApiState) -> Result<Self, ApiError> {
        // The REST API accepts only the session cookie (ADR 0039).
        let credential = session_token(parts).map(Credential::Session);
        match state.authenticator.authenticate(credential).await {
            Ok(caller) => Ok(Self(caller.with_request(Channel::Web, request_id()))),
            Err(AuthenticationError::Unauthenticated) => {
                Err(ApiError::new(ProblemCode::Unauthenticated))
            }
            Err(AuthenticationError::OrganizationRequired) => {
                Err(ApiError::new(ProblemCode::OrganizationRequired))
            }
            Err(AuthenticationError::Store(error)) => Err(ApiError::store(&error)),
        }
    }
}

/// The ID of the current request. Outside a request it is the nil UUID, which is no ID.
fn request_id() -> Option<uuid::Uuid> {
    Some(request_id::current()).filter(|id| !id.is_nil())
}

fn session_token(parts: &Parts) -> Option<&str> {
    parts
        .headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(';'))
        .filter_map(|pair| pair.trim().split_once('='))
        .find_map(|(name, value)| (name == SESSION_COOKIE).then_some(value))
}
