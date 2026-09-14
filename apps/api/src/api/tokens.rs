//! PAT para el MCP: un usuario en un proyecto. El secreto se devuelve UNA vez; en la base
//! queda solo su sha256. Revocar es destructivo → `/api/auth/validate` en syntroAuth antes.

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use chrono::{DateTime, Utc};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::FromRow;
use uuid::Uuid;

use super::{
    ApiError,
    membership::{self, Role},
};
use crate::{auth::AuthUser, state::AppState};

pub const PREFIX: &str = "tdp_";

/// 32 bytes de entropía, base64url sin padding, con prefijo reconocible para que un secret
/// scanner lo detecte si alguien lo commitea.
pub fn generate_secret() -> String {
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    use base64::Engine;
    format!(
        "{PREFIX}{}",
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
    )
}

pub fn hash(secret: &str) -> String {
    hex::encode(Sha256::digest(secret.as_bytes()))
}

#[derive(Deserialize)]
pub struct CreateToken {
    pub name: String,
}

#[derive(Serialize)]
pub struct IssuedToken {
    pub id: Uuid,
    pub name: String,
    /// Solo acá. No vuelve a mostrarse ni a guardarse.
    pub token: String,
}

#[derive(Serialize, FromRow)]
pub struct TokenInfo {
    pub id: Uuid,
    pub name: String,
    pub created_at: DateTime<Utc>,
    pub last_used_at: Option<DateTime<Utc>>,
    pub revoked_at: Option<DateTime<Utc>>,
}

pub async fn create(
    State(st): State<AppState>,
    user: AuthUser,
    Path(slug): Path<String>,
    Json(body): Json<CreateToken>,
) -> Result<(StatusCode, Json<IssuedToken>), ApiError> {
    let m = membership::require(&st.pool, &slug, &user.claims.sub).await?;
    let name = body.name.trim();
    if name.is_empty() {
        return Err(ApiError::Validation("NOMBRE_VACIO"));
    }
    let secret = generate_secret();
    let id: (Uuid,) = sqlx::query_as(
        "insert into access_tokens (project_id, user_sub, name, token_hash) values ($1, $2, $3, $4) \
         returning id",
    )
    .bind(m.project_id)
    .bind(&user.claims.sub)
    .bind(name)
    .bind(hash(&secret))
    .fetch_one(&st.pool)
    .await?;
    Ok((
        StatusCode::CREATED,
        Json(IssuedToken {
            id: id.0,
            name: name.to_string(),
            token: secret,
        }),
    ))
}

/// Solo los tokens propios: un owner ve los de los demás recién cuando exista pantalla y una
/// decisión sobre eso.
pub async fn list(
    State(st): State<AppState>,
    user: AuthUser,
    Path(slug): Path<String>,
) -> Result<Json<Vec<TokenInfo>>, ApiError> {
    let m = membership::require(&st.pool, &slug, &user.claims.sub).await?;
    let rows = sqlx::query_as::<_, TokenInfo>(
        "select id, name, created_at, last_used_at, revoked_at from access_tokens \
         where project_id = $1 and user_sub = $2 order by created_at",
    )
    .bind(m.project_id)
    .bind(&user.claims.sub)
    .fetch_all(&st.pool)
    .await?;
    Ok(Json(rows))
}

/// Propio, o cualquiera del proyecto si sos owner. Antes, syntroAuth tiene que confirmar que
/// la sesión sigue viva (`/validate`): un JWT robado con 20 minutos de vida no alcanza para
/// revocarle el acceso al otro.
pub async fn revoke(
    State(st): State<AppState>,
    user: AuthUser,
    Path((slug, id)): Path<(String, Uuid)>,
) -> Result<StatusCode, ApiError> {
    let m = membership::require(&st.pool, &slug, &user.claims.sub).await?;
    st.auth.validate_online(&user.token).await?;

    let owner_filter = if m.role == Role::Owner {
        ""
    } else {
        " and user_sub = $3"
    };
    let sql = format!(
        "update access_tokens set revoked_at = now() \
         where project_id = $1 and id = $2 and revoked_at is null{owner_filter}"
    );
    let mut q = sqlx::query(sqlx::AssertSqlSafe(sql))
        .bind(m.project_id)
        .bind(id);
    if m.role != Role::Owner {
        q = q.bind(&user.claims.sub);
    }
    let done = q.execute(&st.pool).await?;
    if done.rows_affected() == 0 {
        return Err(ApiError::NotFound);
    }
    Ok(StatusCode::NO_CONTENT)
}
