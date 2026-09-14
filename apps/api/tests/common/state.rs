//! Estado de prueba sin base real: `connect_lazy` no abre conexión hasta el primer uso.

use std::sync::Arc;

use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;
use task_dashboard_api::auth::{JwtConfig, Validator};
use task_dashboard_api::state::AppState;

pub fn state_with(jwt: JwtConfig) -> AppState {
    AppState {
        pool: PgPoolOptions::new()
            .connect_lazy("postgres://nadie:nada@127.0.0.1:1/no_se_usa")
            .expect("pool lazy"),
        auth: Arc::new(Validator::new(jwt)),
    }
}

pub fn state_sin_auth() -> AppState {
    state_with(JwtConfig {
        issuer: "SyntroAuth".into(),
        audience: "SyntroAuth".into(),
        jwks_url: "http://127.0.0.1:1/jwks".into(), // puerto cerrado: nunca responde
    })
}

pub fn state_with_pool(jwt: JwtConfig, pool: PgPool) -> AppState {
    AppState {
        pool,
        auth: Arc::new(Validator::new(jwt)),
    }
}
