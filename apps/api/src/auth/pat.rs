//! PAT para `/mcp`: `Authorization: Bearer tdp_…` → hash → (proyecto, usuario). Es la ÚNICA
//! forma de resolver el proyecto de una llamada MCP: ninguna herramienta lo recibe por parámetro.

use axum::{
    extract::{FromRequestParts, Request, State},
    http::{header::AUTHORIZATION, request::Parts},
    middleware::Next,
    response::Response,
};
use sqlx::PgPool;
use tracing::warn;
use uuid::Uuid;

use super::AuthError;
use crate::{api::tokens, state::AppState};

/// Quién y en qué proyecto. Se inserta en las extensions de la request por el middleware.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PatContext {
    pub token_id: Uuid,
    pub project_id: Uuid,
    pub user_sub: String,
}

/// Inexistente, revocado o con prefijo ajeno dan el mismo 401: un atacante no aprende nada
/// de la diferencia. El detalle va al log.
pub async fn resolve(pool: &PgPool, bearer: &str) -> Result<PatContext, AuthError> {
    if !bearer.starts_with(tokens::PREFIX) {
        return Err(AuthError::Unauthorized("no es un PAT"));
    }
    let row: Option<(Uuid, Uuid, String)> = sqlx::query_as(
        "select id, project_id, user_sub from access_tokens \
         where token_hash = $1 and revoked_at is null",
    )
    .bind(tokens::hash(bearer))
    .fetch_optional(pool)
    .await
    .map_err(|e| {
        warn!(error = %e, "base caída al resolver un PAT");
        AuthError::JwksUnavailable
    })?;
    let (token_id, project_id, user_sub) =
        row.ok_or(AuthError::Unauthorized("PAT desconocido o revocado"))?;

    // Como mucho un UPDATE por minuto por token: es telemetría, no puede costar una escritura
    // por cada llamada del agente. Si falla, no bloquea la request.
    let _ = sqlx::query(
        "update access_tokens set last_used_at = now() \
         where id = $1 and (last_used_at is null or last_used_at < now() - interval '60 seconds')",
    )
    .bind(token_id)
    .execute(pool)
    .await
    .map_err(|e| warn!(error = %e, "no se pudo marcar last_used_at"));

    Ok(PatContext {
        token_id,
        project_id,
        user_sub,
    })
}

/// Middleware para todo `/mcp`: sin PAT válido la request no llega a ningún handler.
pub async fn require_pat(
    State(st): State<AppState>,
    mut req: Request,
    next: Next,
) -> Result<Response, AuthError> {
    let raw = req
        .headers()
        .get(AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .ok_or(AuthError::Unauthorized("sin Authorization"))?;
    let bearer = raw
        .strip_prefix("Bearer ")
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .ok_or(AuthError::Unauthorized("Authorization no es Bearer"))?;
    let ctx = resolve(&st.pool, bearer).await?;
    req.extensions_mut().insert(ctx);
    Ok(next.run(req).await)
}

/// Extractor para handlers detrás de `require_pat`. Si falta la extension es un bug de montaje
/// (ruta sin el middleware), y se responde 401 igual: fail-closed antes que un handler sin proyecto.
#[derive(Debug, Clone)]
pub struct PatUser(pub PatContext);

impl FromRequestParts<AppState> for PatUser {
    type Rejection = AuthError;

    async fn from_request_parts(parts: &mut Parts, _: &AppState) -> Result<Self, AuthError> {
        parts
            .extensions
            .get::<PatContext>()
            .cloned()
            .map(PatUser)
            .ok_or(AuthError::Unauthorized("ruta /mcp sin middleware de PAT"))
    }
}
