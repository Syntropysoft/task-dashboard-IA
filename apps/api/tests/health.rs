//! Spec del ítem 1: `/health` responde 200 con JSON, y lo que no existe es 404 — sin abrir un
//! puerto ni tocar la base ni syntroAuth.

mod common;

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use common::state::state_sin_auth;
use http_body_util::BodyExt;
use task_dashboard_api::app;
use tower::ServiceExt;

#[tokio::test]
async fn health_responde_200_con_status_ok() {
    let res = app(state_sin_auth())
        .oneshot(Request::get("/health").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["status"], "ok");
    assert_eq!(json["version"], env!("CARGO_PKG_VERSION"));
}

#[tokio::test]
async fn ruta_inexistente_es_404() {
    let res = app(state_sin_auth())
        .oneshot(Request::get("/no-existe").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}
