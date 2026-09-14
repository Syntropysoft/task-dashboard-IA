//! Spec del ítem 5: una ficha tiene un solo dueño a la vez, incluso con tomas simultáneas;
//! `YA_TOMADA` dice quién y desde cuándo; `force` pisa y lo declara; liberar ajena es
//! `NO_ES_TUYA`; otro proyecto no ve nada.

mod common;

use axum::{
    body::Body,
    http::{Method, Request, StatusCode},
};
use common::TestDb;
use common::jwks::{FakeJwks, Signer};
use common::state::state_with_pool;
use http_body_util::BodyExt;
use serde_json::{Value, json};
use task_dashboard_api::{app, claims, state::AppState};
use tower::ServiceExt;
use uuid::Uuid;

const GABRIEL: &str = "8d5a1f2e-0000-4000-8000-000000000001";
const ANDRES: &str = "8d5a1f2e-0000-4000-8000-000000000002";

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
    signer: Signer,
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
        World {
            state,
            tdb: Some(tdb),
            signer,
        }
    }

    fn jwt(&self, sub: &str) -> String {
        let sub = sub.to_string();
        format!("Bearer {}", self.signer.token(move |t| t.sub = sub))
    }

    /// Proyecto creado por Gabriel (owner) con Andrés como member; un PAT por cada uno.
    async fn project(&self, slug: &str) -> (Uuid, String, String) {
        let g = self.jwt(GABRIEL);
        let (s, p) = req(
            &self.state,
            Method::POST,
            "/api/projects",
            &g,
            Some(json!({"slug": slug, "name": slug, "repo_url": "https://x"})),
        )
        .await;
        assert_eq!(s, StatusCode::CREATED, "{p}");
        req(
            &self.state,
            Method::POST,
            &format!("/api/projects/{slug}/members"),
            &g,
            Some(json!({"user_sub": ANDRES})),
        )
        .await;
        let (_, tg) = req(
            &self.state,
            Method::POST,
            &format!("/api/projects/{slug}/tokens"),
            &g,
            Some(json!({"name": "g"})),
        )
        .await;
        let (_, ta) = req(
            &self.state,
            Method::POST,
            &format!("/api/projects/{slug}/tokens"),
            &self.jwt(ANDRES),
            Some(json!({"name": "a"})),
        )
        .await;
        (
            Uuid::parse_str(p["id"].as_str().unwrap()).unwrap(),
            format!("Bearer {}", tg["token"].as_str().unwrap()),
            format!("Bearer {}", ta["token"].as_str().unwrap()),
        )
    }

    async fn finish(mut self) {
        self.state.pool.close().await;
        self.tdb.take().unwrap().drop().await;
    }
}

#[tokio::test]
async fn tomar_liberar_y_listar_por_http() {
    let w = World::new().await;
    let (_, gabriel, andres) = w.project("convertix").await;

    let (s, t) = req(
        &w.state,
        Method::POST,
        "/mcp/tomar_ficha",
        &gabriel,
        Some(json!({"ficha_id": "MVC-0385", "nota": "en rama feat/x"})),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{t}");
    assert_eq!(t["held_by"], GABRIEL);
    assert_eq!(t["note"], "en rama feat/x");
    assert!(t.get("previous_holder").is_none());

    // Andrés la quiere: YA_TOMADA, con quién y desde cuándo.
    let (s, e) = req(
        &w.state,
        Method::POST,
        "/mcp/tomar_ficha",
        &andres,
        Some(json!({"ficha_id": "MVC-0385"})),
    )
    .await;
    assert_eq!(s, StatusCode::CONFLICT);
    assert_eq!(e["error"], "YA_TOMADA");
    assert_eq!(e["held_by"], GABRIEL);
    assert!(e["held_since"].is_string());

    // Gabriel la retoma: idempotente, y actualiza la nota.
    let (s, t2) = req(
        &w.state,
        Method::POST,
        "/mcp/tomar_ficha",
        &gabriel,
        Some(json!({"ficha_id": "MVC-0385", "nota": "casi"})),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(t2["note"], "casi");

    let (_, list) = req(&w.state, Method::GET, "/mcp/fichas_tomadas", &andres, None).await;
    assert_eq!(list.as_array().unwrap().len(), 1);
    assert_eq!(list[0]["ficha_id"], "MVC-0385");

    // Andrés no puede liberarla; Gabriel sí; después ya no está tomada.
    let (s, e) = req(
        &w.state,
        Method::POST,
        "/mcp/liberar_ficha",
        &andres,
        Some(json!({"ficha_id": "MVC-0385"})),
    )
    .await;
    assert_eq!(s, StatusCode::FORBIDDEN);
    assert_eq!(e["error"], "NO_ES_TUYA");
    assert_eq!(e["held_by"], GABRIEL);
    let (s, _) = req(
        &w.state,
        Method::POST,
        "/mcp/liberar_ficha",
        &gabriel,
        Some(json!({"ficha_id": "MVC-0385"})),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let (s, e) = req(
        &w.state,
        Method::POST,
        "/mcp/liberar_ficha",
        &gabriel,
        Some(json!({"ficha_id": "MVC-0385"})),
    )
    .await;
    assert_eq!(s, StatusCode::CONFLICT);
    assert_eq!(e["error"], "NO_TOMADA");
    let (_, list) = req(&w.state, Method::GET, "/mcp/fichas_tomadas", &gabriel, None).await;
    assert_eq!(list.as_array().unwrap().len(), 0);

    for bad in ["", "   ", "MVC 0385", "a".repeat(65).as_str()] {
        let (s, e) = req(
            &w.state,
            Method::POST,
            "/mcp/tomar_ficha",
            &gabriel,
            Some(json!({"ficha_id": bad})),
        )
        .await;
        assert_eq!(s, StatusCode::BAD_REQUEST, "{bad:?}");
        assert_eq!(e["error"], "FICHA_ID_INVALIDO");
    }
    w.finish().await;
}

#[tokio::test]
async fn force_pisa_y_declara_al_anterior_en_tomar_y_en_liberar() {
    let w = World::new().await;
    let (_, gabriel, andres) = w.project("convertix").await;
    req(
        &w.state,
        Method::POST,
        "/mcp/tomar_ficha",
        &gabriel,
        Some(json!({"ficha_id": "MVC-0390"})),
    )
    .await;

    let (s, t) = req(
        &w.state,
        Method::POST,
        "/mcp/tomar_ficha",
        &andres,
        Some(json!({"ficha_id": "MVC-0390", "force": true})),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{t}");
    assert_eq!(t["held_by"], ANDRES);
    assert_eq!(
        t["previous_holder"], GABRIEL,
        "quien fuerza ve a quién le sacó la ficha"
    );

    // Ahora es de Andrés; Gabriel la libera con force (claim huérfano, por ejemplo).
    let (s, _) = req(
        &w.state,
        Method::POST,
        "/mcp/liberar_ficha",
        &gabriel,
        Some(json!({"ficha_id": "MVC-0390", "force": true})),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let (_, list) = req(&w.state, Method::GET, "/mcp/fichas_tomadas", &gabriel, None).await;
    assert!(list.as_array().unwrap().is_empty());
    w.finish().await;
}

#[tokio::test]
async fn veinte_tomas_simultaneas_de_una_ficha_nueva_ganan_exactamente_una() {
    let w = World::new().await;
    let (project_id, _, _) = w.project("convertix").await;

    let mut handles = Vec::new();
    for i in 0..20 {
        let pool = w.state.pool.clone();
        handles.push(tokio::spawn(async move {
            claims::take(
                &pool,
                project_id,
                "MVC-0400",
                &format!("sesion-{i}"),
                None,
                false,
            )
            .await
        }));
    }
    let mut ok = 0;
    let mut taken = 0;
    let mut holder = None;
    for h in handles {
        match h.await.unwrap() {
            Ok(t) => {
                ok += 1;
                holder = Some(t.held_by);
            }
            Err(claims::ClaimError::AlreadyTaken { held_by, .. }) => {
                taken += 1;
                assert!(held_by.starts_with("sesion-"));
            }
            Err(e) => panic!("error inesperado: {e:?}"),
        }
    }
    assert_eq!((ok, taken), (1, 19));
    let list = claims::list(&w.state.pool, project_id).await.unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(
        Some(list[0].held_by.clone()),
        holder,
        "el dueño es el único que recibió ok"
    );
    w.finish().await;
}

#[tokio::test]
async fn otro_proyecto_no_ve_ni_toca_el_claim() {
    let w = World::new().await;
    let (_, g_a, _) = w.project("convertix").await;
    let (_, g_b, _) = w.project("otro").await;

    req(
        &w.state,
        Method::POST,
        "/mcp/tomar_ficha",
        &g_a,
        Some(json!({"ficha_id": "MVC-0385"})),
    )
    .await;
    let (_, list_b) = req(&w.state, Method::GET, "/mcp/fichas_tomadas", &g_b, None).await;
    assert!(
        list_b.as_array().unwrap().is_empty(),
        "B no ve el claim de A"
    );

    // Mismo usuario, PAT de B: la ficha MVC-0385 en B es otra ficha, libre.
    let (s, t) = req(
        &w.state,
        Method::POST,
        "/mcp/tomar_ficha",
        &g_b,
        Some(json!({"ficha_id": "MVC-0385"})),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{t}");
    // Y liberar con force desde B no toca la de A.
    req(
        &w.state,
        Method::POST,
        "/mcp/liberar_ficha",
        &g_b,
        Some(json!({"ficha_id": "MVC-0385", "force": true})),
    )
    .await;
    let (_, list_a) = req(&w.state, Method::GET, "/mcp/fichas_tomadas", &g_a, None).await;
    assert_eq!(list_a.as_array().unwrap().len(), 1, "el claim de A sigue");
    w.finish().await;
}
