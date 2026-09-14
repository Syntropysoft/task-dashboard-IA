//! El router es una función pura sobre su configuración para poder testearlo sin abrir un puerto.

use axum::{Json, Router, routing::get};
use serde::Serialize;

pub mod config;

/// Respuesta de `/health`. Railway la usa como healthcheck del deploy; no dice nada de la base
/// a propósito: un healthcheck que depende de Postgres tumba el servicio cuando la base parpadea.
#[derive(Serialize)]
pub struct Health {
    pub status: &'static str,
    pub version: &'static str,
}

pub fn app() -> Router {
    Router::new().route("/health", get(health))
}

async fn health() -> Json<Health> {
    Json(Health {
        status: "ok",
        version: env!("CARGO_PKG_VERSION"),
    })
}
