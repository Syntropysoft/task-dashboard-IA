//! `/mcp`: lo que ve un agente con un PAT. Las herramientas están expuestas como rutas HTTP
//! planas (mismo contrato que tendrán en rmcp, ítem 7) para poder probarlas con curl y tests.
//! El proyecto y el usuario salen SIEMPRE del PAT: ninguna ruta los acepta en el cuerpo.

use axum::{
    Json, Router, middleware,
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    api::{ApiError, sequences::prefijo_valido},
    auth::pat::{self, PatUser},
    ids,
    state::AppState,
};

#[derive(Serialize)]
pub struct WhoAmI {
    pub project_id: Uuid,
    pub user_sub: String,
}

pub fn router(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/whoami", get(whoami))
        .route("/reservar_id", post(reservar_id))
        .route_layer(middleware::from_fn_with_state(state, pat::require_pat))
}

async fn whoami(PatUser(ctx): PatUser) -> Json<WhoAmI> {
    Json(WhoAmI {
        project_id: ctx.project_id,
        user_sub: ctx.user_sub,
    })
}

#[derive(Deserialize)]
pub struct ReservarId {
    pub prefijo: String,
}

/// Contrato: si esto falla, el agente NO inventa un ID (regla 1 del plan). Por eso el error es
/// explícito y distinto: `PREFIJO_DESCONOCIDO` (falta el seed) vs 5xx (base).
async fn reservar_id(
    axum::extract::State(st): axum::extract::State<AppState>,
    PatUser(ctx): PatUser,
    Json(body): Json<ReservarId>,
) -> Result<Json<ids::Reserved>, ApiError> {
    let prefijo = body.prefijo.trim();
    if !prefijo_valido(prefijo) {
        return Err(ApiError::Validation("PREFIJO_INVALIDO"));
    }
    let r = ids::reserve(&st.pool, ctx.project_id, prefijo, &ctx.user_sub).await?;
    Ok(Json(r))
}
