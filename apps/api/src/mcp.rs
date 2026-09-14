//! `/mcp`: lo que ve un agente con un PAT. Hoy solo la sonda `whoami`; el servidor MCP (rmcp)
//! se monta acá mismo en el ítem 7, detrás del mismo middleware.

use axum::{Json, Router, middleware, routing::get};
use serde::Serialize;
use uuid::Uuid;

use crate::{
    auth::pat::{self, PatUser},
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
        .route_layer(middleware::from_fn_with_state(state, pat::require_pat))
}

async fn whoami(PatUser(ctx): PatUser) -> Json<WhoAmI> {
    Json(WhoAmI {
        project_id: ctx.project_id,
        user_sub: ctx.user_sub,
    })
}
