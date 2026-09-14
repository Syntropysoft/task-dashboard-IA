//! Sugerencias: hallazgos que todavía no son ficha. Caen acá para no perderse; promoverlas a
//! ficha (`status = promoted`, `ficha_id`) es del frontend / paso 2, no de este módulo.

use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

/// Suficiente para un párrafo con contexto; más que eso es un doc, no una sugerencia.
pub const MAX_TEXT: usize = 4000;
pub const MAX_CONTEXT: usize = 500;

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Suggestion {
    pub id: i64,
    pub author: String,
    pub text: String,
    pub context: Option<String>,
    pub status: String,
    pub ficha_id: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug)]
pub enum SuggestError {
    EmptyText,
    TooLong,
    Db(sqlx::Error),
}

impl From<sqlx::Error> for SuggestError {
    fn from(e: sqlx::Error) -> Self {
        SuggestError::Db(e)
    }
}

pub async fn create(
    pool: &PgPool,
    project_id: Uuid,
    author: &str,
    text: &str,
    context: Option<&str>,
) -> Result<i64, SuggestError> {
    let text = text.trim();
    if text.is_empty() {
        return Err(SuggestError::EmptyText);
    }
    let context = context.map(str::trim).filter(|c| !c.is_empty());
    if text.chars().count() > MAX_TEXT || context.is_some_and(|c| c.chars().count() > MAX_CONTEXT) {
        return Err(SuggestError::TooLong);
    }
    let (id,): (i64,) = sqlx::query_as(
        "insert into suggestions (project_id, author, text, context) values ($1, $2, $3, $4) returning id",
    )
    .bind(project_id)
    .bind(author)
    .bind(text)
    .bind(context)
    .fetch_one(pool)
    .await?;
    Ok(id)
}

/// Las abiertas del proyecto, más viejas primero: es la cola de lo que hay que mirar.
pub async fn list_open(pool: &PgPool, project_id: Uuid) -> Result<Vec<Suggestion>, SuggestError> {
    Ok(sqlx::query_as::<_, Suggestion>(
        "select id, author, text, context, status, ficha_id, created_at from suggestions \
         where project_id = $1 and status = 'open' order by created_at, id",
    )
    .bind(project_id)
    .fetch_all(pool)
    .await?)
}
