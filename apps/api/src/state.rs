use std::sync::Arc;

use sqlx::PgPool;

use crate::auth::Validator;

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub auth: Arc<Validator>,
}
