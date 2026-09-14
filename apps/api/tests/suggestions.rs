//! Spec del ítem 6: `sugerir` cae en el proyecto del PAT con el autor del PAT; la lista muestra
//! las abiertas del proyecto y nada de otro; texto vacío o desmedido se rechaza sin rastro.

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
use task_dashboard_api::{app, state::AppState, suggestions};
use tower::ServiceExt;

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

async fn world() -> (AppState, TestDb, String) {
    let tdb = TestDb::create().await;
    let pool = tdb.pool().await;
    task_dashboard_api::db::migrate(&pool).await.unwrap();
    let signer = Signer::generate("k1");
    let jwks = FakeJwks::serve(&[&signer]).await;
    let state = state_with_pool(jwks.config(), pool);
    state.auth.refresh().await.unwrap();
    (
        state,
        tdb,
        format!("Bearer {}", signer.token(|t| t.sub = GABRIEL.into())),
    )
}

async fn pat(state: &AppState, jwt: &str, slug: &str) -> String {
    let (s, _) = req(
        state,
        Method::POST,
        "/api/projects",
        jwt,
        Some(json!({"slug": slug, "name": slug, "repo_url": "https://x"})),
    )
    .await;
    assert_eq!(s, StatusCode::CREATED);
    let (_, t) = req(
        state,
        Method::POST,
        &format!("/api/projects/{slug}/tokens"),
        jwt,
        Some(json!({"name": "a"})),
    )
    .await;
    format!("Bearer {}", t["token"].as_str().unwrap())
}

#[tokio::test]
async fn sugerir_cae_en_el_proyecto_del_pat_y_se_lista_abierta() {
    let (state, tdb, jwt) = world().await;
    let a = pat(&state, &jwt, "convertix").await;
    let b = pat(&state, &jwt, "otro").await;

    let (s, r) = req(&state, Method::POST, "/mcp/sugerir", &a, Some(json!({"texto": "  el retry de Graph API no loguea el status  ", "contexto": "MVC-0393"}))).await;
    assert_eq!(s, StatusCode::CREATED, "{r}");
    let id = r["id"].as_i64().unwrap();
    let (s, r2) = req(
        &state,
        Method::POST,
        "/mcp/sugerir",
        &a,
        Some(json!({"texto": "segunda"})),
    )
    .await;
    assert_eq!(s, StatusCode::CREATED);
    assert!(r2["id"].as_i64().unwrap() > id);

    let (s, list) = req(&state, Method::GET, "/mcp/sugerencias", &a, None).await;
    assert_eq!(s, StatusCode::OK);
    let list = list.as_array().unwrap();
    assert_eq!(list.len(), 2);
    assert_eq!(list[0]["id"], id, "más vieja primero");
    assert_eq!(
        list[0]["text"], "el retry de Graph API no loguea el status",
        "recortado"
    );
    assert_eq!(list[0]["context"], "MVC-0393");
    assert_eq!(list[0]["author"], GABRIEL);
    assert_eq!(list[0]["status"], "open");
    assert!(list[1]["context"].is_null());

    // Otro proyecto, mismo usuario: nada.
    let (_, list_b) = req(&state, Method::GET, "/mcp/sugerencias", &b, None).await;
    assert!(list_b.as_array().unwrap().is_empty());

    // Una promovida deja de listarse como abierta.
    sqlx::query("update suggestions set status = 'promoted', ficha_id = 'MVC-0410' where id = $1")
        .bind(id)
        .execute(&state.pool)
        .await
        .unwrap();
    let (_, list) = req(&state, Method::GET, "/mcp/sugerencias", &a, None).await;
    assert_eq!(list.as_array().unwrap().len(), 1);
    state.pool.close().await;
    tdb.drop().await;
}

#[tokio::test]
async fn texto_vacio_o_desmedido_se_rechaza_sin_rastro() {
    let (state, tdb, jwt) = world().await;
    let a = pat(&state, &jwt, "convertix").await;
    let casos = vec![
        (json!({"texto": ""}), "TEXTO_VACIO"),
        (json!({"texto": "   \n "}), "TEXTO_VACIO"),
        (
            json!({"texto": "x".repeat(suggestions::MAX_TEXT + 1)}),
            "TEXTO_DEMASIADO_LARGO",
        ),
        (
            json!({"texto": "ok", "contexto": "c".repeat(suggestions::MAX_CONTEXT + 1)}),
            "TEXTO_DEMASIADO_LARGO",
        ),
    ];
    for (body, code) in casos {
        let (s, r) = req(&state, Method::POST, "/mcp/sugerir", &a, Some(body)).await;
        assert_eq!(s, StatusCode::BAD_REQUEST, "{code}");
        assert_eq!(r["error"], code);
    }
    // El límite exacto entra.
    let (s, _) = req(
        &state,
        Method::POST,
        "/mcp/sugerir",
        &a,
        Some(json!({"texto": "y".repeat(suggestions::MAX_TEXT)})),
    )
    .await;
    assert_eq!(s, StatusCode::CREATED);
    let n: i64 = sqlx::query("select count(*) from suggestions")
        .fetch_one(&state.pool)
        .await
        .unwrap()
        .get(0);
    assert_eq!(n, 1);
    state.pool.close().await;
    tdb.drop().await;
}
