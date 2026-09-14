// Cada binario de tests usa una parte de este módulo; lo que no usa uno lo usa otro.
#![allow(dead_code)]

//! Base de prueba efímera por test: `TEST_DATABASE_URL` apunta a un Postgres con permiso de
//! CREATE DATABASE (el de `docker-compose.yml`). Sin la variable el test FALLA con mensaje
//! claro — no se saltea: un gate que se saltea en silencio no es un gate.

use sqlx::postgres::PgPool;
use sqlx::{AssertSqlSafe, Connection, PgConnection};

pub struct TestDb {
    pub url: String,
    admin_url: String,
    name: String,
}

impl TestDb {
    pub async fn create() -> TestDb {
        let admin_url = std::env::var("TEST_DATABASE_URL").expect(
            "TEST_DATABASE_URL no definida: levantá el Postgres de pruebas con `make db-up` \
             (o exportá la URL de uno propio con permiso de CREATE DATABASE)",
        );
        let name = format!("td_test_{}", uuid::Uuid::new_v4().simple());
        let mut admin = PgConnection::connect(&admin_url)
            .await
            .expect("conectar admin");
        // DDL con nombre generado por nosotros (uuid), no por un usuario: sin inyección posible.
        let sql = format!("create database {name}");
        sqlx::query(AssertSqlSafe(sql))
            .execute(&mut admin)
            .await
            .expect("create database");
        let url = replace_db_name(&admin_url, &name);
        TestDb {
            url,
            admin_url,
            name,
        }
    }

    pub async fn pool(&self) -> PgPool {
        task_dashboard_api::db::connect(&self.url)
            .await
            .expect("pool")
    }

    /// Borrado explícito al final del test (Drop no puede ser async). Un test que falla deja la
    /// base para inspeccionarla; se limpian con `make db-reset`.
    pub async fn drop(self) {
        let mut admin = PgConnection::connect(&self.admin_url)
            .await
            .expect("conectar admin");
        let sql = format!("drop database {} with (force)", self.name);
        sqlx::query(AssertSqlSafe(sql))
            .execute(&mut admin)
            .await
            .expect("drop database");
    }
}

fn replace_db_name(url: &str, name: &str) -> String {
    // postgres://user:pass@host:port/dbname?params → mismo prefijo, otro dbname
    let (base, query) = match url.split_once('?') {
        Some((b, q)) => (b, Some(q)),
        None => (url, None),
    };
    let cut = base.rfind('/').expect("url sin '/'");
    let mut out = format!("{}/{}", &base[..cut], name);
    if let Some(q) = query {
        out.push('?');
        out.push_str(q);
    }
    out
}

pub mod jwks;
pub mod state;
