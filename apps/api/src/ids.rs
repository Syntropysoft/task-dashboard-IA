//! Reserva atómica de IDs por (proyecto, prefijo). La atomicidad ES el producto: dos sesiones
//! nunca reciben el mismo número. Mecanismo: `UPDATE … RETURNING` sobre la fila del contador —
//! Postgres toma el row lock, la segunda transacción espera a la primera y lee el valor ya
//! incrementado. La auditoría en `id_reservations` va en la misma transacción.

use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Reserved {
    pub id: String,
    pub prefix: String,
    pub number: i32,
}

#[derive(Debug)]
pub enum IdError {
    /// No hay contador para ese prefijo en este proyecto: falta el seed (`PUT …/sequences/{p}`).
    UnknownPrefix,
    Db(sqlx::Error),
}

impl From<sqlx::Error> for IdError {
    fn from(e: sqlx::Error) -> Self {
        IdError::Db(e)
    }
}

/// `MVC` + 407 → `MVC-0407`. Cuatro dígitos como mínimo (convención de Convertix); pasado
/// 9999 el número crece sin truncar.
pub fn format_id(prefix: &str, number: i32) -> String {
    format!("{prefix}-{number:04}")
}

pub async fn reserve(
    pool: &PgPool,
    project_id: Uuid,
    prefix: &str,
    user_sub: &str,
) -> Result<Reserved, IdError> {
    let mut tx = pool.begin().await?;
    let taken: Option<(i32,)> = sqlx::query_as(
        "update id_sequences set next = next + 1 \
         where project_id = $1 and prefix = $2 \
         returning next - 1",
    )
    .bind(project_id)
    .bind(prefix)
    .fetch_optional(&mut *tx)
    .await?;
    let Some((number,)) = taken else {
        return Err(IdError::UnknownPrefix);
    };
    let id = format_id(prefix, number);
    sqlx::query(
        "insert into id_reservations (project_id, id, prefix, number, reserved_by) \
         values ($1, $2, $3, $4, $5)",
    )
    .bind(project_id)
    .bind(&id)
    .bind(prefix)
    .bind(number)
    .bind(user_sub)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(Reserved {
        id,
        prefix: prefix.to_string(),
        number,
    })
}
