//! Spec del ítem 3c: `/mcp` solo entra con un PAT vivo; inexistente, revocado, con otro prefijo
//! o un JWT dan el mismo 401. El PAT resuelve SU proyecto, y no sirve en `/api`.

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
use sqlx::Row;
use task_dashboard_api::app;
use task_dashboard_api::state::AppState;
use tower::ServiceExt;

const GABRIEL: &str = "8d5a1f2e-0000-4000-8000-000000000001";

async fn req(
    state: &AppState,
    method: Method,
    path: &str,
    auth: Option<&str>,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let mut b = Request::builder().method(method).uri(path);
    if let Some(a) = auth {
        b = b.header("authorization", a);
    }
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

/// Proyecto + PAT emitidos por la API real (ítem 3b), no insertados a mano: si cambia cómo se
/// hashea, este test lo ve.
async fn world() -> (AppState, TestDb, String, String, Value) {
    let tdb = TestDb::create().await;
    let pool = tdb.pool().await;
    task_dashboard_api::db::migrate(&pool).await.unwrap();
    let signer = Signer::generate("k1");
    let jwks = FakeJwks::serve(&[&signer]).await;
    let state = state_with_pool(jwks.config(), pool);
    state.auth.refresh().await.unwrap();
    let jwt = format!("Bearer {}", signer.token(|t| t.sub = GABRIEL.into()));

    let (s, project) = req(
        &state,
        Method::POST,
        "/api/projects",
        Some(&jwt),
        Some(json!({"slug": "convertix", "name": "Convertix", "repo_url": "https://x"})),
    )
    .await;
    assert_eq!(s, StatusCode::CREATED);
    let (s, issued) = req(
        &state,
        Method::POST,
        "/api/projects/convertix/tokens",
        Some(&jwt),
        Some(json!({"name": "claude"})),
    )
    .await;
    assert_eq!(s, StatusCode::CREATED);
    let pat = issued["token"].as_str().unwrap().to_string();
    (state, tdb, jwt, pat, project)
}

#[tokio::test]
async fn pat_valido_resuelve_su_proyecto_y_usuario_y_marca_last_used() {
    let (state, tdb, _jwt, pat, project) = world().await;
    let (s, who) = req(
        &state,
        Method::GET,
        "/mcp/whoami",
        Some(&format!("Bearer {pat}")),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{who}");
    assert_eq!(who["project_id"], project["id"]);
    assert_eq!(who["user_sub"], GABRIEL);

    let last_used: Option<chrono::DateTime<chrono::Utc>> =
        sqlx::query("select last_used_at from access_tokens")
            .fetch_one(&state.pool)
            .await
            .unwrap()
            .get(0);
    assert!(last_used.is_some(), "last_used_at tiene que quedar marcado");
    state.pool.close().await;
    tdb.drop().await;
}

#[tokio::test]
async fn sin_pat_invalido_o_jwt_en_mcp_es_401_identico() {
    let (state, tdb, jwt, pat, _) = world().await;
    let ajeno = task_dashboard_api::api::tokens::generate_secret(); // bien formado, nunca emitido
    let casos: Vec<(&str, Option<String>)> = vec![
        ("sin header", None),
        ("bearer vacío", Some("Bearer ".into())),
        ("basic", Some("Basic abc".into())),
        ("jwt válido de syntroAuth en /mcp", Some(jwt.clone())),
        ("prefijo ajeno", Some("Bearer xyz_123".into())),
        (
            "PAT bien formado nunca emitido",
            Some(format!("Bearer {ajeno}")),
        ),
        (
            "PAT con un carácter cambiado",
            Some(format!("Bearer {}x", &pat[..pat.len() - 1])),
        ),
    ];
    for (nombre, auth) in casos {
        let (s, body) = req(&state, Method::GET, "/mcp/whoami", auth.as_deref(), None).await;
        assert_eq!(s, StatusCode::UNAUTHORIZED, "{nombre}");
        assert_eq!(body, json!({"error": "UNAUTHORIZED"}), "{nombre}");
    }
    state.pool.close().await;
    tdb.drop().await;
}

#[tokio::test]
async fn pat_revocado_deja_de_entrar_y_pat_no_sirve_en_api() {
    let (state, tdb, jwt, pat, _) = world().await;
    let auth = format!("Bearer {pat}");
    // Un PAT no es un JWT: /api lo rechaza.
    let (s, _) = req(&state, Method::GET, "/api/me", Some(&auth), None).await;
    assert_eq!(s, StatusCode::UNAUTHORIZED);

    let (s, _) = req(&state, Method::GET, "/mcp/whoami", Some(&auth), None).await;
    assert_eq!(s, StatusCode::OK);

    let (_, list) = req(
        &state,
        Method::GET,
        "/api/projects/convertix/tokens",
        Some(&jwt),
        None,
    )
    .await;
    let id = list[0]["id"].as_str().unwrap();
    let (s, _) = req(
        &state,
        Method::DELETE,
        &format!("/api/projects/convertix/tokens/{id}"),
        Some(&jwt),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::NO_CONTENT);

    let (s, body) = req(&state, Method::GET, "/mcp/whoami", Some(&auth), None).await;
    assert_eq!(s, StatusCode::UNAUTHORIZED, "revocado");
    assert_eq!(body, json!({"error": "UNAUTHORIZED"}));
    state.pool.close().await;
    tdb.drop().await;
}

#[tokio::test]
async fn el_pat_de_un_proyecto_no_ve_otro() {
    let (state, tdb, jwt, pat_a, project_a) = world().await;
    let (s, project_b) = req(
        &state,
        Method::POST,
        "/api/projects",
        Some(&jwt),
        Some(json!({"slug": "otro", "name": "Otro", "repo_url": "https://y"})),
    )
    .await;
    assert_eq!(s, StatusCode::CREATED);
    let (_, issued_b) = req(
        &state,
        Method::POST,
        "/api/projects/otro/tokens",
        Some(&jwt),
        Some(json!({"name": "b"})),
    )
    .await;
    let pat_b = issued_b["token"].as_str().unwrap();

    // Mismo usuario, dos PATs: cada uno resuelve exactamente su proyecto.
    let (_, who_a) = req(
        &state,
        Method::GET,
        "/mcp/whoami",
        Some(&format!("Bearer {pat_a}")),
        None,
    )
    .await;
    let (_, who_b) = req(
        &state,
        Method::GET,
        "/mcp/whoami",
        Some(&format!("Bearer {pat_b}")),
        None,
    )
    .await;
    assert_eq!(who_a["project_id"], project_a["id"]);
    assert_eq!(who_b["project_id"], project_b["id"]);
    assert_ne!(who_a["project_id"], who_b["project_id"]);
    state.pool.close().await;
    tdb.drop().await;
}
