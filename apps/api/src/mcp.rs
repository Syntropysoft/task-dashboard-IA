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
    claims, ids,
    state::AppState,
    suggestions,
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
        .route("/tomar_ficha", post(tomar_ficha))
        .route("/liberar_ficha", post(liberar_ficha))
        .route("/fichas_tomadas", get(fichas_tomadas))
        .route("/sugerir", post(sugerir))
        .route("/sugerencias", get(sugerencias))
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

#[derive(Deserialize)]
pub struct TomarFicha {
    pub ficha_id: String,
    #[serde(default)]
    pub nota: Option<String>,
    #[serde(default)]
    pub force: bool,
}

#[derive(Deserialize)]
pub struct LiberarFicha {
    pub ficha_id: String,
    #[serde(default)]
    pub force: bool,
}

fn ficha_id_de(raw: &str) -> Result<&str, ApiError> {
    let id = raw.trim();
    if !claims::ficha_id_valido(id) {
        return Err(ApiError::Validation("FICHA_ID_INVALIDO"));
    }
    Ok(id)
}

async fn tomar_ficha(
    axum::extract::State(st): axum::extract::State<AppState>,
    PatUser(ctx): PatUser,
    Json(body): Json<TomarFicha>,
) -> Result<Json<claims::Taken>, ApiError> {
    let id = ficha_id_de(&body.ficha_id)?;
    let nota = body
        .nota
        .as_deref()
        .map(str::trim)
        .filter(|n| !n.is_empty());
    let t = claims::take(
        &st.pool,
        ctx.project_id,
        id,
        &ctx.user_sub,
        nota,
        body.force,
    )
    .await?;
    Ok(Json(t))
}

async fn liberar_ficha(
    axum::extract::State(st): axum::extract::State<AppState>,
    PatUser(ctx): PatUser,
    Json(body): Json<LiberarFicha>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let id = ficha_id_de(&body.ficha_id)?;
    claims::release(&st.pool, ctx.project_id, id, &ctx.user_sub, body.force).await?;
    Ok(Json(serde_json::json!({ "ok": true, "ficha_id": id })))
}

async fn fichas_tomadas(
    axum::extract::State(st): axum::extract::State<AppState>,
    PatUser(ctx): PatUser,
) -> Result<Json<Vec<claims::Claim>>, ApiError> {
    Ok(Json(claims::list(&st.pool, ctx.project_id).await?))
}

#[derive(Deserialize)]
pub struct Sugerir {
    pub texto: String,
    #[serde(default)]
    pub contexto: Option<String>,
}

async fn sugerir(
    axum::extract::State(st): axum::extract::State<AppState>,
    PatUser(ctx): PatUser,
    Json(body): Json<Sugerir>,
) -> Result<(axum::http::StatusCode, Json<serde_json::Value>), ApiError> {
    let id = suggestions::create(
        &st.pool,
        ctx.project_id,
        &ctx.user_sub,
        &body.texto,
        body.contexto.as_deref(),
    )
    .await?;
    Ok((
        axum::http::StatusCode::CREATED,
        Json(serde_json::json!({ "id": id })),
    ))
}

async fn sugerencias(
    axum::extract::State(st): axum::extract::State<AppState>,
    PatUser(ctx): PatUser,
) -> Result<Json<Vec<suggestions::Suggestion>>, ApiError> {
    Ok(Json(
        suggestions::list_open(&st.pool, ctx.project_id).await?,
    ))
}
