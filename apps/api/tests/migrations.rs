//! Spec del ítem 2: las migraciones aplican en una base vacía, son idempotentes, y dejan el
//! esquema del plan. Fail-path: una URL a una base inexistente falla rápido, sin colgarse.

mod common;

use std::time::{Duration, Instant};

use common::TestDb;
use sqlx::Row;
use task_dashboard_api::db;

#[tokio::test]
async fn migraciones_aplican_y_son_idempotentes() {
    let tdb = TestDb::create().await;
    let pool = tdb.pool().await;

    db::migrate(&pool).await.expect("primera corrida");
    db::migrate(&pool)
        .await
        .expect("segunda corrida: no debe re-aplicar nada");

    let applied: i64 = sqlx::query("select count(*) from _sqlx_migrations")
        .fetch_one(&pool)
        .await
        .unwrap()
        .get(0);
    assert_eq!(applied, db::MIGRATOR.iter().count() as i64);

    let tables: Vec<String> = sqlx::query(
        "select table_name from information_schema.tables \
         where table_schema = 'public' and table_name <> '_sqlx_migrations' order by 1",
    )
    .fetch_all(&pool)
    .await
    .unwrap()
    .into_iter()
    .map(|r| r.get::<String, _>(0))
    .collect();
    assert_eq!(
        tables,
        [
            "access_tokens",
            "claims",
            "id_reservations",
            "id_sequences",
            "project_members",
            "projects",
            "suggestions",
        ]
    );

    pool.close().await;
    tdb.drop().await;
}

#[tokio::test]
async fn toda_tabla_de_datos_lleva_project_id() {
    // Invariante §4 del chasis: multi-proyecto fail-closed. Que el esquema no pueda olvidarlo.
    let tdb = TestDb::create().await;
    let pool = tdb.pool().await;
    db::migrate(&pool).await.unwrap();

    let sin_project_id: Vec<String> = sqlx::query(
        "select t.table_name from information_schema.tables t \
         where t.table_schema = 'public' and t.table_name not in ('_sqlx_migrations', 'projects') \
         and not exists (select 1 from information_schema.columns c \
           where c.table_schema = t.table_schema and c.table_name = t.table_name \
           and c.column_name = 'project_id' and c.is_nullable = 'NO')",
    )
    .fetch_all(&pool)
    .await
    .unwrap()
    .into_iter()
    .map(|r| r.get::<String, _>(0))
    .collect();
    assert!(
        sin_project_id.is_empty(),
        "sin project_id NOT NULL: {sin_project_id:?}"
    );

    pool.close().await;
    tdb.drop().await;
}

#[tokio::test]
async fn base_inexistente_falla_rapido_no_cuelga() {
    let admin = std::env::var("TEST_DATABASE_URL").expect("TEST_DATABASE_URL");
    let bad = admin
        .rsplit_once('/')
        .map(|(b, _)| format!("{b}/no_existe_td"))
        .unwrap();
    let t0 = Instant::now();
    let r = db::connect_and_migrate(&bad).await;
    assert!(matches!(r, Err(db::DbError::Connect(_))), "{r:?}");
    assert!(
        t0.elapsed() < Duration::from_secs(10),
        "tardó {:?}",
        t0.elapsed()
    );
}
