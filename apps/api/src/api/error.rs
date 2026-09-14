//! Un solo tipo de error para `/api`: código estable en el cuerpo, detalle solo en el log.

use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde_json::json;
use tracing::error;

use crate::auth::AuthError;

#[derive(Debug)]
pub enum ApiError {
    /// 400 — entrada inválida; el `&str` es un código, no prosa.
    Validation(&'static str),
    /// 404 — no existe o no sos miembro: no se distingue a propósito.
    NotFound,
    /// 403 — sos miembro pero no owner.
    Forbidden,
    /// 409 — conflicto con el estado actual (slug repetido, seed por debajo del contador).
    Conflict(&'static str),
    /// 503 — una dependencia (syntroAuth) no pudo confirmar; fail-closed.
    Unavailable(&'static str),
    Auth(AuthError),
    Db(sqlx::Error),
}

impl From<sqlx::Error> for ApiError {
    fn from(e: sqlx::Error) -> Self {
        ApiError::Db(e)
    }
}

impl From<AuthError> for ApiError {
    fn from(e: AuthError) -> Self {
        ApiError::Auth(e)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, code) = match self {
            ApiError::Validation(c) => (StatusCode::BAD_REQUEST, c),
            ApiError::NotFound => (StatusCode::NOT_FOUND, "NOT_FOUND"),
            ApiError::Forbidden => (StatusCode::FORBIDDEN, "FORBIDDEN"),
            ApiError::Conflict(c) => (StatusCode::CONFLICT, c),
            ApiError::Unavailable(c) => (StatusCode::SERVICE_UNAVAILABLE, c),
            ApiError::Auth(e) => return e.into_response(),
            ApiError::Db(e) => {
                error!(error = %e, "error de base en /api");
                (StatusCode::INTERNAL_SERVER_ERROR, "DB_ERROR")
            }
        };
        (status, Json(json!({ "error": code }))).into_response()
    }
}
