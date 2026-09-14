//! Resolución proyecto + rol del usuario. Es la única puerta a un `project_id`: toda query de
//! datos parte de acá, así el `WHERE project_id` no puede faltar (invariante §4).

use sqlx::PgPool;
use uuid::Uuid;

use super::ApiError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Owner,
    Member,
}

#[derive(Debug, Clone)]
pub struct Membership {
    pub project_id: Uuid,
    pub role: Role,
}

/// No miembro y proyecto inexistente dan lo mismo (`NotFound`): un slug no es información
/// que se regale a quien no pertenece.
pub async fn require(pool: &PgPool, slug: &str, user_sub: &str) -> Result<Membership, ApiError> {
    let row: Option<(Uuid, String)> = sqlx::query_as(
        "select p.id, m.role from projects p \
         join project_members m on m.project_id = p.id \
         where p.slug = $1 and m.user_sub = $2",
    )
    .bind(slug)
    .bind(user_sub)
    .fetch_optional(pool)
    .await?;
    let (project_id, role) = row.ok_or(ApiError::NotFound)?;
    let role = match role.as_str() {
        "owner" => Role::Owner,
        _ => Role::Member,
    };
    Ok(Membership { project_id, role })
}

pub async fn require_owner(
    pool: &PgPool,
    slug: &str,
    user_sub: &str,
) -> Result<Membership, ApiError> {
    let m = require(pool, slug, user_sub).await?;
    if m.role != Role::Owner {
        return Err(ApiError::Forbidden);
    }
    Ok(m)
}
