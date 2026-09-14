//! Seed del contador de IDs por (proyecto, prefijo). Es el paso 8 del plan: `next` = máximo ID
//! real del repo + 1. **Nunca baja**: un contador que retrocede reparte IDs ya usados, que es el
//! bug que este servicio existe para matar.

use axum::{
    Json,
    extract::{Path, State},
};
use serde::{Deserialize, Serialize};

use super::{ApiError, membership};
use crate::{auth::AuthUser, state::AppState};

#[derive(Deserialize)]
pub struct Seed {
    pub next: i32,
}

#[derive(Serialize)]
pub struct Sequence {
    pub prefix: String,
    pub next: i32,
}

pub fn prefijo_valido(p: &str) -> bool {
    (1..=16).contains(&p.len())
        && p.bytes()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
}

pub async fn seed(
    State(st): State<AppState>,
    user: AuthUser,
    Path((slug, prefix)): Path<(String, String)>,
    Json(body): Json<Seed>,
) -> Result<Json<Sequence>, ApiError> {
    let m = membership::require_owner(&st.pool, &slug, &user.claims.sub).await?;
    if !prefijo_valido(&prefix) {
        return Err(ApiError::Validation("PREFIJO_INVALIDO"));
    }
    if body.next < 1 {
        return Err(ApiError::Validation("NEXT_INVALIDO"));
    }
    // Un solo statement: inserta o sube; si el nuevo valor es menor, no toca nada y devuelve 0
    // filas → 409. Sin ventana entre leer y escribir.
    let row: Option<(i32,)> = sqlx::query_as(
        "insert into id_sequences (project_id, prefix, next) values ($1, $2, $3) \
         on conflict (project_id, prefix) do update set next = excluded.next \
         where id_sequences.next <= excluded.next \
         returning next",
    )
    .bind(m.project_id)
    .bind(&prefix)
    .bind(body.next)
    .fetch_optional(&st.pool)
    .await?;
    match row {
        Some((next,)) => Ok(Json(Sequence { prefix, next })),
        None => Err(ApiError::Conflict("SEED_POR_DEBAJO_DEL_CONTADOR")),
    }
}
