//! Spec del ítem 3b: proyectos, membresía, PATs y seed — contra Postgres real y un syntroAuth
//! falso. Fail-paths: no miembro → 404 (sin revelar), member sin permiso → 403, slug repetido
//! → 409, seed que baja → 409, revocación sin confirmación de syntroAuth → no revoca.

mod common;

use axum::{
    body::Body,
    http::{Method, Request, StatusCode},
};
use common::TestDb;
use common::jwks::{FakeJwks, Signer, ValidateMode};
use common::state::state_with_pool;
use http_body_util::BodyExt;
use serde_json::{Value, json};
use sqlx::Row;
use task_dashboard_api::app;
use task_dashboard_api::state::AppState;
use tower::ServiceExt;

const GABRIEL: &str = "8d5a1f2e-0000-4000-8000-000000000001";
const ANDRES: &str = "8d5a1f2e-0000-4000-8000-000000000002";

struct World {
    state: AppState,
    signer: Signer,
    jwks: FakeJwks,
    tdb: Option<TestDb>,
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
            signer,
            jwks,
            tdb: Some(tdb),
        }
    }

    fn jwt(&self, sub: &str) -> String {
        let sub = sub.to_string();
        self.signer.token(move |t| t.sub = sub)
    }

    async fn call(
        &self,
        sub: &str,
        method: Method,
        path: &str,
        body: Option<Value>,
    ) -> (StatusCode, Value) {
        let req = Request::builder()
            .method(method)
            .uri(path)
            .header("authorization", format!("Bearer {}", self.jwt(sub)));
        let req = match body {
            Some(b) => req
                .header("content-type", "application/json")
                .body(Body::from(b.to_string()))
                .unwrap(),
            None => req.body(Body::empty()).unwrap(),
        };
        let res = app(self.state.clone()).oneshot(req).await.unwrap();
        let status = res.status();
        let bytes = res.into_body().collect().await.unwrap().to_bytes();
        (
            status,
            serde_json::from_slice(&bytes).unwrap_or(Value::Null),
        )
    }

    async fn crear_convertix(&self) -> Value {
        let (status, json) = self
            .call(
                GABRIEL,
                Method::POST,
                "/api/projects",
                Some(json!({
                    "slug": "convertix", "name": "Convertix",
                    "repo_url": "https://github.com/x/motor-ventas"
                })),
            )
            .await;
        assert_eq!(status, StatusCode::CREATED, "{json}");
        json
    }

    async fn finish(mut self) {
        self.state.pool.close().await;
        self.tdb.take().unwrap().drop().await;
    }
}

#[tokio::test]
async fn crear_proyecto_deja_al_creador_como_owner_y_lo_lista() {
    let w = World::new().await;
    let p = w.crear_convertix().await;
    assert_eq!(p["slug"], "convertix");
    assert_eq!(p["branch"], "main", "branch por defecto");

    let (status, list) = w.call(GABRIEL, Method::GET, "/api/projects", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(list.as_array().unwrap().len(), 1);

    let (_, otro) = w.call(ANDRES, Method::GET, "/api/projects", None).await;
    assert_eq!(
        otro.as_array().unwrap().len(),
        0,
        "Andrés no es miembro todavía"
    );

    let role: String = sqlx::query("select role from project_members where user_sub = $1")
        .bind(GABRIEL)
        .fetch_one(&w.state.pool)
        .await
        .unwrap()
        .get(0);
    assert_eq!(role, "owner");
    w.finish().await;
}

#[tokio::test]
async fn slug_repetido_es_409_y_slug_invalido_400() {
    let w = World::new().await;
    w.crear_convertix().await;
    let (status, json) = w
        .call(
            ANDRES,
            Method::POST,
            "/api/projects",
            Some(json!({ "slug": "convertix", "name": "Otro", "repo_url": "https://x" })),
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(json["error"], "SLUG_EN_USO");

    for bad in ["Convertix", "a", "-x", "con vertix", "x_y"] {
        let (status, json) = w
            .call(
                GABRIEL,
                Method::POST,
                "/api/projects",
                Some(json!({ "slug": bad, "name": "n", "repo_url": "https://x" })),
            )
            .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "slug {bad:?}");
        assert_eq!(json["error"], "SLUG_INVALIDO");
    }
    w.finish().await;
}

#[tokio::test]
async fn no_miembro_ve_404_en_todo_y_member_no_puede_agregar_miembros() {
    let w = World::new().await;
    w.crear_convertix().await;

    // Andrés no es miembro: el proyecto no existe para él, en ninguna ruta.
    for (m, path, body) in [
        (
            Method::POST,
            "/api/projects/convertix/members",
            Some(json!({"user_sub": "z"})),
        ),
        (
            Method::POST,
            "/api/projects/convertix/tokens",
            Some(json!({"name": "x"})),
        ),
        (Method::GET, "/api/projects/convertix/tokens", None),
        (
            Method::PUT,
            "/api/projects/convertix/sequences/MVC",
            Some(json!({"next": 1})),
        ),
    ] {
        let (status, json) = w.call(ANDRES, m.clone(), path, body).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{m} {path}");
        assert_eq!(json["error"], "NOT_FOUND");
    }
    // Un slug inexistente da exactamente lo mismo.
    let (status, _) = w
        .call(GABRIEL, Method::GET, "/api/projects/nope/tokens", None)
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    // Gabriel (owner) agrega a Andrés como member.
    let (status, json) = w
        .call(
            GABRIEL,
            Method::POST,
            "/api/projects/convertix/members",
            Some(json!({ "user_sub": ANDRES })),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{json}");
    assert_eq!(json["role"], "member");

    // Member: puede emitir tokens, no agregar miembros ni sembrar.
    let (status, _) = w
        .call(
            ANDRES,
            Method::POST,
            "/api/projects/convertix/tokens",
            Some(json!({"name": "mac"})),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED);
    let (status, json) = w
        .call(
            ANDRES,
            Method::POST,
            "/api/projects/convertix/members",
            Some(json!({"user_sub": "z"})),
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(json["error"], "FORBIDDEN");
    let (status, _) = w
        .call(
            ANDRES,
            Method::PUT,
            "/api/projects/convertix/sequences/MVC",
            Some(json!({"next": 10})),
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    w.finish().await;
}

#[tokio::test]
async fn pat_se_muestra_una_vez_y_en_la_base_queda_solo_el_hash() {
    let w = World::new().await;
    w.crear_convertix().await;
    let (status, issued) = w
        .call(
            GABRIEL,
            Method::POST,
            "/api/projects/convertix/tokens",
            Some(json!({"name": "claude mac"})),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{issued}");
    let secret = issued["token"].as_str().unwrap().to_string();
    assert!(secret.starts_with("tdp_"), "{secret}");
    assert!(secret.len() > 40);

    let (hash, name): (String, String) =
        sqlx::query_as("select token_hash, name from access_tokens where id = $1")
            .bind(uuid::Uuid::parse_str(issued["id"].as_str().unwrap()).unwrap())
            .fetch_one(&w.state.pool)
            .await
            .unwrap();
    assert_eq!(name, "claude mac");
    assert_ne!(hash, secret);
    assert_eq!(hash, task_dashboard_api::api::tokens::hash(&secret));

    let (_, list) = w
        .call(GABRIEL, Method::GET, "/api/projects/convertix/tokens", None)
        .await;
    let list = list.as_array().unwrap();
    assert_eq!(list.len(), 1);
    assert!(
        list[0].get("token").is_none(),
        "el listado no muestra secretos: {list:?}"
    );
    assert!(list[0]["revoked_at"].is_null());
    w.finish().await;
}

#[tokio::test]
async fn revocar_exige_confirmacion_de_syntroauth_y_respeta_dueno() {
    let w = World::new().await;
    w.crear_convertix().await;
    w.call(
        GABRIEL,
        Method::POST,
        "/api/projects/convertix/members",
        Some(json!({"user_sub": ANDRES})),
    )
    .await;
    let (_, tok_andres) = w
        .call(
            ANDRES,
            Method::POST,
            "/api/projects/convertix/tokens",
            Some(json!({"name": "a"})),
        )
        .await;
    let id = tok_andres["id"].as_str().unwrap().to_string();
    let path = format!("/api/projects/convertix/tokens/{id}");

    // syntroAuth caído → 503 y el token sigue vivo.
    w.jwks.set_validate(ValidateMode::Down);
    let (status, json) = w.call(ANDRES, Method::DELETE, &path, None).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{json}");
    // syntroAuth rechaza la sesión → 401 y el token sigue vivo.
    w.jwks.set_validate(ValidateMode::Reject);
    let (status, _) = w.call(ANDRES, Method::DELETE, &path, None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let vivos: i64 = sqlx::query("select count(*) from access_tokens where revoked_at is null")
        .fetch_one(&w.state.pool)
        .await
        .unwrap()
        .get(0);
    assert_eq!(vivos, 1, "nada se revocó sin confirmación");

    w.jwks.set_validate(ValidateMode::Ok);
    // Un tercer member no puede revocar el de Andrés (404: no es suyo).
    w.call(
        GABRIEL,
        Method::POST,
        "/api/projects/convertix/members",
        Some(json!({"user_sub": "tercero"})),
    )
    .await;
    let (status, _) = w.call("tercero", Method::DELETE, &path, None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    // El owner sí.
    let (status, _) = w.call(GABRIEL, Method::DELETE, &path, None).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    // Revocar dos veces → 404 (ya no está vivo).
    let (status, _) = w.call(GABRIEL, Method::DELETE, &path, None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    w.finish().await;
}

#[tokio::test]
async fn seed_sube_pero_nunca_baja() {
    let w = World::new().await;
    w.crear_convertix().await;
    let path = "/api/projects/convertix/sequences/MVC";

    let (status, json) = w
        .call(GABRIEL, Method::PUT, path, Some(json!({"next": 407})))
        .await;
    assert_eq!(status, StatusCode::OK, "{json}");
    assert_eq!(json["next"], 407);

    let (status, json) = w
        .call(GABRIEL, Method::PUT, path, Some(json!({"next": 500})))
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["next"], 500);

    let (status, json) = w
        .call(GABRIEL, Method::PUT, path, Some(json!({"next": 300})))
        .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(json["error"], "SEED_POR_DEBAJO_DEL_CONTADOR");
    let (next,): (i32,) = sqlx::query_as("select next from id_sequences where prefix = 'MVC'")
        .fetch_one(&w.state.pool)
        .await
        .unwrap();
    assert_eq!(next, 500, "el contador no retrocedió");

    for (p, body) in [("mvc", 1), ("MVC", 0), ("MUY-LARGO-PREFIJO-XXXXX", 1)] {
        let (status, _) = w
            .call(
                GABRIEL,
                Method::PUT,
                &format!("/api/projects/convertix/sequences/{p}"),
                Some(json!({"next": body})),
            )
            .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "prefijo {p:?} next {body}");
    }
    w.finish().await;
}
