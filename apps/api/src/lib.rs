//! El router es una función de su estado para poder testearlo sin abrir un puerto.

use axum::{Json, Router, routing::get};
use serde::Serialize;

pub mod api;
pub mod auth;
pub mod config;
pub mod db;
pub mod state;

use auth::AuthUser;
use state::AppState;

/// Respuesta de `/health`. Railway la usa como healthcheck del deploy; no dice nada de la base
/// a propósito: un healthcheck que depende de Postgres tumba el servicio cuando la base parpadea.
#[derive(Serialize)]
pub struct Health {
    pub status: &'static str,
    pub version: &'static str,
}

#[derive(Serialize)]
pub struct Me {
    pub sub: String,
    pub email: Option<String>,
    pub tenant_id: Option<String>,
}

pub fn app(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/api/me", get(me))
        .nest("/api", api::router())
        .with_state(state)
}

async fn health() -> Json<Health> {
    Json(Health {
        status: "ok",
        version: env!("CARGO_PKG_VERSION"),
    })
}

/// Prueba de vida de la identidad: devuelve lo que el JWT dice que sos. Sin autorización
/// todavía — cualquier usuario válido de la suite puede llamarlo.
async fn me(user: AuthUser) -> Json<Me> {
    Json(Me {
        sub: user.claims.sub,
        email: user.claims.email,
        tenant_id: user.claims.tenant_id,
    })
}
