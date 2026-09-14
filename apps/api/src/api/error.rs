//! Un solo tipo de error para `/api`: código estable en el cuerpo, detalle solo en el log.

use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde_json::json;
use tracing::error;

use crate::auth::AuthError;
use crate::claims::ClaimError;
use crate::ids::IdError;

#[derive(Debug)]
pub enum ApiError {
    /// 400 — entrada inválida; el `&str` es un código, no prosa.
    Validation(&'static str),
    /// 404 — no existe o no sos miembro: no se distingue a propósito.
    NotFound,
    /// 403 — sos miembro pero no owner.
    Forbidden,
    /// 404 — el proyecto no tiene contador para ese prefijo: falta el seed.
    UnknownPrefix,
    /// 409 — otro tiene la ficha; el cuerpo dice quién y desde cuándo.
    AlreadyTaken {
        held_by: String,
        held_since: chrono::DateTime<chrono::Utc>,
    },
    /// 403 — la ficha es de otro (sin `force`); el cuerpo dice de quién.
    NotYours {
        held_by: String,
    },
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

impl From<IdError> for ApiError {
    fn from(e: IdError) -> Self {
        match e {
            IdError::UnknownPrefix => ApiError::UnknownPrefix,
            IdError::Db(e) => ApiError::Db(e),
        }
    }
}

impl From<ClaimError> for ApiError {
    fn from(e: ClaimError) -> Self {
        match e {
            ClaimError::AlreadyTaken {
                held_by,
                held_since,
            } => ApiError::AlreadyTaken {
                held_by,
                held_since,
            },
            ClaimError::NotYours { held_by } => ApiError::NotYours { held_by },
            ClaimError::NotTaken => ApiError::Conflict("NO_TOMADA"),
            ClaimError::Db(e) => ApiError::Db(e),
        }
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
            ApiError::UnknownPrefix => (StatusCode::NOT_FOUND, "PREFIJO_DESCONOCIDO"),
            ApiError::AlreadyTaken {
                held_by,
                held_since,
            } => {
                return (
                    StatusCode::CONFLICT,
                    Json(json!({ "error": "YA_TOMADA", "held_by": held_by, "held_since": held_since })),
                )
                    .into_response();
            }
            ApiError::NotYours { held_by } => {
                return (
                    StatusCode::FORBIDDEN,
                    Json(json!({ "error": "NO_ES_TUYA", "held_by": held_by })),
                )
                    .into_response();
            }
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
