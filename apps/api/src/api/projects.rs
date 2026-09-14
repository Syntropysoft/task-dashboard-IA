use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

use super::{ApiError, membership};
use crate::{auth::AuthUser, state::AppState};

#[derive(Deserialize)]
pub struct CreateProject {
    pub slug: String,
    pub name: String,
    pub repo_url: String,
    #[serde(default)]
    pub branch: Option<String>,
}

#[derive(Serialize, FromRow)]
pub struct Project {
    pub id: Uuid,
    pub slug: String,
    pub name: String,
    pub repo_url: String,
    pub branch: String,
    pub created_at: DateTime<Utc>,
}

/// Mismo formato que el CHECK de la migración: la validación en código da un 400 con código
/// en vez de un 500 por constraint.
fn slug_valido(s: &str) -> bool {
    let b = s.as_bytes();
    (2..=63).contains(&b.len())
        && b[0].is_ascii_lowercase() | b[0].is_ascii_digit()
        && b.iter()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == b'-')
}

pub async fn list(
    State(st): State<AppState>,
    user: AuthUser,
) -> Result<Json<Vec<Project>>, ApiError> {
    let rows = sqlx::query_as::<_, Project>(
        "select p.id, p.slug, p.name, p.repo_url, p.branch, p.created_at from projects p \
         join project_members m on m.project_id = p.id where m.user_sub = $1 order by p.slug",
    )
    .bind(&user.claims.sub)
    .fetch_all(&st.pool)
    .await?;
    Ok(Json(rows))
}

/// Crear proyecto y quedar como owner es una sola transacción: un proyecto sin owner no puede
/// existir ni un instante.
pub async fn create(
    State(st): State<AppState>,
    user: AuthUser,
    Json(body): Json<CreateProject>,
) -> Result<(StatusCode, Json<Project>), ApiError> {
    if !slug_valido(&body.slug) {
        return Err(ApiError::Validation("SLUG_INVALIDO"));
    }
    if body.name.trim().is_empty() || body.repo_url.trim().is_empty() {
        return Err(ApiError::Validation("CAMPO_VACIO"));
    }
    let branch = body
        .branch
        .as_deref()
        .map(str::trim)
        .filter(|b| !b.is_empty())
        .unwrap_or("main");

    let mut tx = st.pool.begin().await?;
    let inserted = sqlx::query_as::<_, Project>(
        "insert into projects (slug, name, repo_url, branch, created_by) values ($1, $2, $3, $4, $5) \
         on conflict (slug) do nothing \
         returning id, slug, name, repo_url, branch, created_at",
    )
    .bind(&body.slug)
    .bind(body.name.trim())
    .bind(body.repo_url.trim())
    .bind(branch)
    .bind(&user.claims.sub)
    .fetch_optional(&mut *tx)
    .await?;
    let Some(project) = inserted else {
        return Err(ApiError::Conflict("SLUG_EN_USO"));
    };
    sqlx::query(
        "insert into project_members (project_id, user_sub, role) values ($1, $2, 'owner')",
    )
    .bind(project.id)
    .bind(&user.claims.sub)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok((StatusCode::CREATED, Json(project)))
}

#[derive(Deserialize)]
pub struct AddMember {
    pub user_sub: String,
    #[serde(default)]
    pub role: Option<String>,
}

#[derive(Serialize)]
pub struct Member {
    pub user_sub: String,
    pub role: String,
}

pub async fn add_member(
    State(st): State<AppState>,
    user: AuthUser,
    Path(slug): Path<String>,
    Json(body): Json<AddMember>,
) -> Result<(StatusCode, Json<Member>), ApiError> {
    let m = membership::require_owner(&st.pool, &slug, &user.claims.sub).await?;
    let sub = body.user_sub.trim();
    if sub.is_empty() {
        return Err(ApiError::Validation("USER_SUB_VACIO"));
    }
    let role = match body.role.as_deref().unwrap_or("member") {
        "owner" => "owner",
        "member" => "member",
        _ => return Err(ApiError::Validation("ROL_INVALIDO")),
    };
    // Idempotente sobre el mismo (proyecto, usuario): repetir el alta actualiza el rol.
    sqlx::query(
        "insert into project_members (project_id, user_sub, role) values ($1, $2, $3) \
         on conflict (project_id, user_sub) do update set role = excluded.role",
    )
    .bind(m.project_id)
    .bind(sub)
    .bind(role)
    .execute(&st.pool)
    .await?;
    Ok((
        StatusCode::CREATED,
        Json(Member {
            user_sub: sub.to_string(),
            role: role.to_string(),
        }),
    ))
}
