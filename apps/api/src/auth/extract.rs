//! Extractor de axum: `AuthUser` en la firma de un handler exige un JWT válido de syntroAuth.
//! Sin header, mal formado o inválido → 401 con cuerpo fijo; JWKS nunca cargado → 503.

use axum::{
    extract::FromRequestParts,
    http::{StatusCode, header::AUTHORIZATION, request::Parts},
    response::{IntoResponse, Response},
};
use serde_json::json;

use super::{AuthError, Claims};
use crate::state::AppState;

#[derive(Debug, Clone)]
pub struct AuthUser(pub Claims);

impl IntoResponse for AuthError {
    fn into_response(self) -> Response {
        match self {
            AuthError::Unauthorized(_) => (
                StatusCode::UNAUTHORIZED,
                [("www-authenticate", "Bearer")],
                axum::Json(json!({ "error": "UNAUTHORIZED" })),
            )
                .into_response(),
            AuthError::JwksUnavailable => (
                StatusCode::SERVICE_UNAVAILABLE,
                axum::Json(json!({ "error": "AUTH_UNAVAILABLE" })),
            )
                .into_response(),
        }
    }
}

impl FromRequestParts<AppState> for AuthUser {
    type Rejection = AuthError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, AuthError> {
        let raw = parts
            .headers
            .get(AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .ok_or(AuthError::Unauthorized("sin Authorization"))?;
        let token = raw
            .strip_prefix("Bearer ")
            .map(str::trim)
            .filter(|t| !t.is_empty())
            .ok_or(AuthError::Unauthorized("Authorization no es Bearer"))?;
        state.auth.validate(token).await.map(AuthUser)
    }
}
