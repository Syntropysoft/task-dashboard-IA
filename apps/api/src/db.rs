//! Conexión y migraciones. Las migraciones viven en `db/migrations` (raíz del repo) y se
//! embeben en el binario: en Railway no hay árbol de fuentes, solo el ejecutable.

use std::time::Duration;

use sqlx::postgres::{PgPool, PgPoolOptions};

/// Migraciones embebidas. sqlx toma un advisory lock al aplicarlas, así dos réplicas (o un
/// redeploy que solapa con la instancia vieja) no corren la misma migración dos veces.
pub static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("../../db/migrations");

#[derive(Debug)]
pub enum DbError {
    Connect(sqlx::Error),
    Migrate(sqlx::migrate::MigrateError),
}

impl std::fmt::Display for DbError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DbError::Connect(e) => write!(f, "no se pudo conectar a la base: {e}"),
            DbError::Migrate(e) => write!(f, "migración fallida: {e}"),
        }
    }
}

impl std::error::Error for DbError {}

/// Pool chico a propósito: el Postgres es de 1 GB y el servicio duerme entre ráfagas.
/// `acquire_timeout` corto: una base caída tiene que fallar rápido, no colgar la request.
pub async fn connect(database_url: &str) -> Result<PgPool, DbError> {
    PgPoolOptions::new()
        .max_connections(5)
        .acquire_timeout(Duration::from_secs(5))
        .connect(database_url)
        .await
        .map_err(DbError::Connect)
}

/// Idempotente: lo ya aplicado se salta; una migración aplicada cuyo contenido cambió es error
/// (checksum) — el servicio no arranca, que es lo correcto.
pub async fn migrate(pool: &PgPool) -> Result<(), DbError> {
    MIGRATOR.run(pool).await.map_err(DbError::Migrate)
}

/// Conectar + migrar, que es lo que hace el arranque.
pub async fn connect_and_migrate(database_url: &str) -> Result<PgPool, DbError> {
    let pool = connect(database_url).await?;
    migrate(&pool).await?;
    Ok(pool)
}
