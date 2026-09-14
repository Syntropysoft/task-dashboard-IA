//! Spec del ítem 4: la reserva es atómica bajo concurrencia real, consecutiva, auditada, aislada
//! por proyecto, y sin seed falla explícito (nunca inventa).

mod common;

use std::collections::BTreeSet;

use axum::{
    body::Body,
    http::{Method, Request, StatusCode},
};
use common::TestDb;
use common::jwks::{FakeJwks, Signer};
use common::state::state_with_pool;
use http_body_util::BodyExt;
use serde_json::{Value, json};
use sqlx::Row;
use task_dashboard_api::{app, ids, state::AppState};
use tower::ServiceExt;
use uuid::Uuid;

const GABRIEL: &str = "8d5a1f2e-0000-4000-8000-000000000001";

async fn req(
    state: &AppState,
    method: Method,
    path: &str,
    auth: &str,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let b = Request::builder()
        .method(method)
        .uri(path)
        .header("authorization", auth);
    let r = match body {
        Some(v) => b
            .header("content-type", "application/json")
            .body(Body::from(v.to_string()))
            .unwrap(),
        None => b.body(Body::empty()).unwrap(),
    };
    let res = app(state.clone()).oneshot(r).await.unwrap();
    let status = res.status();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

struct World {
    state: AppState,
    tdb: Option<TestDb>,
    jwt: String,
}

impl World {
    async fn new() -> World {
        let tdb = TestDb::create().await;
        let pool = tdb.pool().await;
        task_dashboard_api::db::migrate(&pool).await.unwrap();
        let signer = Signer::generate("k1");
        let jwks = FakeJwks::serve(&[&signer]).await;
        let state = state_with_pool(jwks.config(), pool);
        state.auth.refresh().await.unwrap();
        let jwt = format!("Bearer {}", signer.token(|t| t.sub = GABRIEL.into()));
        World {
            state,
            tdb: Some(tdb),
            jwt,
        }
    }

    /// Proyecto + PAT + seed, todo por la API real. Devuelve (project_id, "Bearer tdp_…").
    async fn project(&self, slug: &str, seed: Option<(&str, i32)>) -> (Uuid, String) {
        let (s, p) = req(
            &self.state,
            Method::POST,
            "/api/projects",
            &self.jwt,
            Some(json!({"slug": slug, "name": slug, "repo_url": "https://x"})),
        )
        .await;
        assert_eq!(s, StatusCode::CREATED, "{p}");
        if let Some((prefix, next)) = seed {
            let (s, _) = req(
                &self.state,
                Method::PUT,
                &format!("/api/projects/{slug}/sequences/{prefix}"),
                &self.jwt,
                Some(json!({"next": next})),
            )
            .await;
            assert_eq!(s, StatusCode::OK);
        }
        let (_, t) = req(
            &self.state,
            Method::POST,
            &format!("/api/projects/{slug}/tokens"),
            &self.jwt,
            Some(json!({"name": "agente"})),
        )
        .await;
        (
            Uuid::parse_str(p["id"].as_str().unwrap()).unwrap(),
            format!("Bearer {}", t["token"].as_str().unwrap()),
        )
    }

    async fn finish(mut self) {
        self.state.pool.close().await;
        self.tdb.take().unwrap().drop().await;
    }
}

#[tokio::test]
async fn cincuenta_reservas_en_paralelo_dan_cincuenta_numeros_distintos_y_consecutivos() {
    let w = World::new().await;
    let (project_id, _) = w.project("convertix", Some(("MVC", 407))).await;

    // Tareas reales de tokio sobre el mismo pool (5 conexiones): la contención es genuina.
    let mut handles = Vec::new();
    for i in 0..50 {
        let pool = w.state.pool.clone();
        handles.push(tokio::spawn(async move {
            ids::reserve(&pool, project_id, "MVC", &format!("sesion-{i}"))
                .await
                .expect("reserva")
        }));
    }
    let mut numbers = BTreeSet::new();
    let mut ids_vistos = BTreeSet::new();
    for h in handles {
        let r = h.await.unwrap();
        assert!(numbers.insert(r.number), "número repetido: {}", r.number);
        assert!(ids_vistos.insert(r.id.clone()), "id repetido: {}", r.id);
        assert_eq!(r.id, format!("MVC-{:04}", r.number));
    }
    let esperados: BTreeSet<i32> = (407..457).collect();
    assert_eq!(numbers, esperados, "sin huecos ni saltos");

    // El contador quedó exactamente después del último, y la auditoría tiene las 50 filas.
    let (next,): (i32,) =
        sqlx::query_as("select next from id_sequences where project_id = $1 and prefix = 'MVC'")
            .bind(project_id)
            .fetch_one(&w.state.pool)
            .await
            .unwrap();
    assert_eq!(next, 457);
    let audit: i64 = sqlx::query("select count(*) from id_reservations where project_id = $1")
        .bind(project_id)
        .fetch_one(&w.state.pool)
        .await
        .unwrap()
        .get(0);
    assert_eq!(audit, 50);
    w.finish().await;
}

#[tokio::test]
async fn por_http_con_pat_devuelve_el_id_y_sin_seed_es_prefijo_desconocido() {
    let w = World::new().await;
    let (_, pat) = w.project("convertix", Some(("MVC", 407))).await;

    let (s, r) = req(
        &w.state,
        Method::POST,
        "/mcp/reservar_id",
        &pat,
        Some(json!({"prefijo": "MVC"})),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{r}");
    assert_eq!(r["id"], "MVC-0407");
    let (_, r2) = req(
        &w.state,
        Method::POST,
        "/mcp/reservar_id",
        &pat,
        Some(json!({"prefijo": "MVC"})),
    )
    .await;
    assert_eq!(r2["id"], "MVC-0408");

    // Prefijo sin seed en este proyecto: error explícito, y el agente sabe que NO debe inventar.
    let (s, r) = req(
        &w.state,
        Method::POST,
        "/mcp/reservar_id",
        &pat,
        Some(json!({"prefijo": "FE"})),
    )
    .await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    assert_eq!(r["error"], "PREFIJO_DESCONOCIDO");
    // Prefijo mal formado: 400, y tampoco toca la base.
    let (s, r) = req(
        &w.state,
        Method::POST,
        "/mcp/reservar_id",
        &pat,
        Some(json!({"prefijo": "mvc"})),
    )
    .await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    assert_eq!(r["error"], "PREFIJO_INVALIDO");
    // Nada de lo fallido dejó rastro.
    let audit: i64 = sqlx::query("select count(*) from id_reservations")
        .fetch_one(&w.state.pool)
        .await
        .unwrap()
        .get(0);
    assert_eq!(audit, 2);
    w.finish().await;
}

#[tokio::test]
async fn dos_proyectos_con_el_mismo_prefijo_no_se_pisan_y_el_pat_no_cruza() {
    let w = World::new().await;
    let (_, pat_a) = w.project("convertix", Some(("MVC", 407))).await;
    let (_, pat_b) = w.project("otro", Some(("MVC", 1))).await;

    let (_, a1) = req(
        &w.state,
        Method::POST,
        "/mcp/reservar_id",
        &pat_a,
        Some(json!({"prefijo": "MVC"})),
    )
    .await;
    let (_, b1) = req(
        &w.state,
        Method::POST,
        "/mcp/reservar_id",
        &pat_b,
        Some(json!({"prefijo": "MVC"})),
    )
    .await;
    let (_, a2) = req(
        &w.state,
        Method::POST,
        "/mcp/reservar_id",
        &pat_a,
        Some(json!({"prefijo": "MVC"})),
    )
    .await;
    assert_eq!(a1["id"], "MVC-0407");
    assert_eq!(b1["id"], "MVC-0001");
    assert_eq!(
        a2["id"], "MVC-0408",
        "el proyecto B no movió el contador de A"
    );

    // Un proyecto sin seed de FE no ve el seed de FE de otro proyecto.
    let (s, _) = req(
        &w.state,
        Method::PUT,
        "/api/projects/otro/sequences/FE",
        &w.jwt,
        Some(json!({"next": 100})),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let (s, r) = req(
        &w.state,
        Method::POST,
        "/mcp/reservar_id",
        &pat_a,
        Some(json!({"prefijo": "FE"})),
    )
    .await;
    assert_eq!(s, StatusCode::NOT_FOUND, "{r}");
    w.finish().await;
}

#[tokio::test]
async fn el_formato_crece_sin_truncar_pasado_9999() {
    assert_eq!(ids::format_id("MVC", 7), "MVC-0007");
    assert_eq!(ids::format_id("MVC", 9999), "MVC-9999");
    assert_eq!(ids::format_id("MVC", 10000), "MVC-10000");
}
