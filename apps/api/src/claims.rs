//! Claims: quién tiene qué ficha AHORA. Una fila por (proyecto, ficha); sin historial ni TTL —
//! con `fichas_tomadas` + `force` alcanza para un equipo chico (decisión del plan).

use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::PgPool;
use tracing::warn;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Claim {
    pub ficha_id: String,
    pub held_by: String,
    pub held_since: DateTime<Utc>,
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Taken {
    pub ficha_id: String,
    pub held_by: String,
    pub held_since: DateTime<Utc>,
    pub note: Option<String>,
    /// Solo con `force` sobre una ficha ajena: a quién se le sacó. Quien fuerza asume el choque.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub previous_holder: Option<String>,
}

#[derive(Debug)]
pub enum ClaimError {
    /// Otro la tiene. Va con quién y desde cuándo para que el agente decida (o hable).
    AlreadyTaken {
        held_by: String,
        held_since: DateTime<Utc>,
    },
    NotYours {
        held_by: String,
    },
    NotTaken,
    Db(sqlx::Error),
}

impl From<sqlx::Error> for ClaimError {
    fn from(e: sqlx::Error) -> Self {
        ClaimError::Db(e)
    }
}

/// `MVC-0385`, `FE-12`, `bug/login`… Lo que un repo use como identificador, sin espacios ni
/// control chars, hasta 64.
pub fn ficha_id_valido(s: &str) -> bool {
    (1..=64).contains(&s.len())
        && s.bytes()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'-' | b'_' | b'.' | b'/' | b'#'))
}

pub async fn take(
    pool: &PgPool,
    project_id: Uuid,
    ficha_id: &str,
    user_sub: &str,
    note: Option<&str>,
    force: bool,
) -> Result<Taken, ClaimError> {
    let mut tx = pool.begin().await?;
    // FOR UPDATE: si existe, nadie más la toca hasta que decidamos. Si no existe, el INSERT de
    // abajo compite con otro INSERT igual y uno de los dos cae en el ON CONFLICT (ver más abajo).
    let current: Option<(String, DateTime<Utc>)> = sqlx::query_as(
        "select held_by, held_since from claims where project_id = $1 and ficha_id = $2 for update",
    )
    .bind(project_id)
    .bind(ficha_id)
    .fetch_optional(&mut *tx)
    .await?;

    let previous_holder = match &current {
        Some((holder, _)) if holder == user_sub => None, // retomar la propia: idempotente
        Some((holder, since)) if !force => {
            return Err(ClaimError::AlreadyTaken {
                held_by: holder.clone(),
                held_since: *since,
            });
        }
        Some((holder, _)) => {
            warn!(project = %project_id, ficha = ficha_id, de = %holder, a = %user_sub, "claim FORZADO");
            Some(holder.clone())
        }
        None => None,
    };

    let row: Option<Claim> = sqlx::query_as(
        "insert into claims (project_id, ficha_id, held_by, note) values ($1, $2, $3, $4) \
         on conflict (project_id, ficha_id) do update \
           set held_by = excluded.held_by, held_since = now(), note = excluded.note \
           where claims.held_by = excluded.held_by or $5 \
         returning ficha_id, held_by, held_since, note",
    )
    .bind(project_id)
    .bind(ficha_id)
    .bind(user_sub)
    .bind(note)
    .bind(force)
    .fetch_optional(&mut *tx)
    .await?;
    tx.commit().await?;

    match row {
        Some(c) => Ok(Taken {
            ficha_id: c.ficha_id,
            held_by: c.held_by,
            held_since: c.held_since,
            note: c.note,
            previous_holder,
        }),
        // Carrera entre dos INSERT de una ficha nueva: el que perdió el conflicto llega acá.
        // Se responde con el estado real, no con un error genérico.
        None => {
            let (held_by, held_since): (String, DateTime<Utc>) = sqlx::query_as(
                "select held_by, held_since from claims where project_id = $1 and ficha_id = $2",
            )
            .bind(project_id)
            .bind(ficha_id)
            .fetch_one(pool)
            .await?;
            Err(ClaimError::AlreadyTaken {
                held_by,
                held_since,
            })
        }
    }
}

pub async fn release(
    pool: &PgPool,
    project_id: Uuid,
    ficha_id: &str,
    user_sub: &str,
    force: bool,
) -> Result<(), ClaimError> {
    let mut tx = pool.begin().await?;
    let current: Option<(String,)> = sqlx::query_as(
        "select held_by from claims where project_id = $1 and ficha_id = $2 for update",
    )
    .bind(project_id)
    .bind(ficha_id)
    .fetch_optional(&mut *tx)
    .await?;
    let Some((holder,)) = current else {
        return Err(ClaimError::NotTaken);
    };
    if holder != user_sub {
        if !force {
            return Err(ClaimError::NotYours { held_by: holder });
        }
        warn!(project = %project_id, ficha = ficha_id, de = %holder, por = %user_sub, "liberación FORZADA");
    }
    sqlx::query("delete from claims where project_id = $1 and ficha_id = $2")
        .bind(project_id)
        .bind(ficha_id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(())
}

pub async fn list(pool: &PgPool, project_id: Uuid) -> Result<Vec<Claim>, ClaimError> {
    Ok(sqlx::query_as::<_, Claim>(
        "select ficha_id, held_by, held_since, note from claims where project_id = $1 order by held_since",
    )
    .bind(project_id)
    .fetch_all(pool)
    .await?)
}
