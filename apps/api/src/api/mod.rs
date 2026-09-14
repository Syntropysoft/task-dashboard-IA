//! `/api/*`: lo que un humano (o el frontend) hace con un JWT de syntroAuth. Sin pantalla
//! todavía: es lo justo para operar con curl (plan, ítem 3b).

use axum::{
    Router,
    routing::{delete, get, post, put},
};

use crate::state::AppState;

pub mod error;
pub mod membership;
pub mod projects;
pub mod sequences;
pub mod tokens;

pub use error::ApiError;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/projects", get(projects::list).post(projects::create))
        .route("/projects/{slug}/members", post(projects::add_member))
        .route(
            "/projects/{slug}/tokens",
            get(tokens::list).post(tokens::create),
        )
        .route("/projects/{slug}/tokens/{id}", delete(tokens::revoke))
        .route("/projects/{slug}/sequences/{prefix}", put(sequences::seed))
}
