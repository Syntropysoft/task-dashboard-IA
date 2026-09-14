use std::sync::Arc;

use sqlx::PgPool;

use crate::auth::Validator;

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub auth: Arc<Validator>,
    /// Hosts que el servidor MCP acepta en `Host:` (rmcp rechaza el resto: anti DNS-rebinding).
    pub mcp_allowed_hosts: Vec<String>,
}
