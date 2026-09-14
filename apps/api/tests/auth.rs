//! Spec del ítem 3a: `/api/me` acepta solo un JWT RS256 de syntroAuth con `iss`, `aud`, `exp`
//! y firma válidos; todo lo demás es 401 sin distinguir por qué; JWKS nunca cargado es 503.

mod common;

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use common::jwks::{FakeJwks, Signer};
use common::state::state_with;
use http_body_util::BodyExt;
use jsonwebtoken::Algorithm;
use task_dashboard_api::app;
use task_dashboard_api::state::AppState;
use tower::ServiceExt;

async fn me(state: &AppState, auth_header: Option<&str>) -> (StatusCode, serde_json::Value) {
    let mut req = Request::get("/api/me");
    if let Some(h) = auth_header {
        req = req.header("authorization", h);
    }
    let res = app(state.clone())
        .oneshot(req.body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = res.status();
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let json = serde_json::from_slice(&body).unwrap_or(serde_json::Value::Null);
    (status, json)
}

async fn ready_state(jwks: &FakeJwks) -> AppState {
    let state = state_with(jwks.config());
    state.auth.refresh().await.expect("JWKS inicial");
    state
}

#[tokio::test]
async fn token_valido_devuelve_el_sub() {
    let signer = Signer::generate("k1");
    let jwks = FakeJwks::serve(&[&signer]).await;
    let state = ready_state(&jwks).await;

    let (status, json) = me(&state, Some(&format!("Bearer {}", signer.token(|_| {})))).await;
    assert_eq!(status, StatusCode::OK, "{json}");
    assert_eq!(json["sub"], "8d5a1f2e-0000-4000-8000-000000000001");
    assert_eq!(json["email"], "gabriel@example.test");
    assert_eq!(json["tenant_id"], "t-1");
}

#[tokio::test]
async fn sin_header_o_mal_formado_es_401() {
    let signer = Signer::generate("k1");
    let jwks = FakeJwks::serve(&[&signer]).await;
    let state = ready_state(&jwks).await;

    for h in [
        None,
        Some(""),
        Some("Bearer"),
        Some("Bearer "),
        Some("Basic abc"),
        Some("no-es-jwt"),
    ] {
        let (status, json) = me(&state, h).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "header {h:?}");
        assert_eq!(json["error"], "UNAUTHORIZED");
    }
}

#[tokio::test]
async fn vencido_otro_iss_otro_aud_o_sin_aud_es_401() {
    let signer = Signer::generate("k1");
    let jwks = FakeJwks::serve(&[&signer]).await;
    let state = ready_state(&jwks).await;

    let casos: Vec<(&str, String)> = vec![
        ("vencido", signer.token(|t| t.exp_offset_secs = -120)),
        ("otro iss", signer.token(|t| t.iss = Some("otro".into()))),
        (
            "otro aud",
            signer.token(|t| t.aud = Some("otra-app".into())),
        ),
        ("sin aud", signer.token(|t| t.aud = None)),
        ("sin iss", signer.token(|t| t.iss = None)),
    ];
    for (nombre, tok) in casos {
        let (status, json) = me(&state, Some(&format!("Bearer {tok}"))).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{nombre}");
        // El motivo no viaja al cliente: mismo cuerpo para todos.
        assert_eq!(
            json,
            serde_json::json!({ "error": "UNAUTHORIZED" }),
            "{nombre}"
        );
    }
}

#[tokio::test]
async fn firmado_por_otra_clave_es_401_aunque_el_kid_coincida() {
    let real = Signer::generate("k1");
    let impostor = Signer::generate_fresh("k1"); // mismo kid, otra clave privada, fuera del depósito
    let jwks = FakeJwks::serve(&[&real]).await;
    let state = ready_state(&jwks).await;

    let (status, _) = me(&state, Some(&format!("Bearer {}", impostor.token(|_| {})))).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn hs256_se_rechaza_sin_intentar_validar() {
    let signer = Signer::generate("k1");
    let jwks = FakeJwks::serve(&[&signer]).await;
    let state = ready_state(&jwks).await;
    let hits_antes = jwks.hits();

    // Header dice HS256 (aunque la firma sea RSA): se corta por el alg, sin refrescar JWKS.
    let tok = signer.token(|t| t.alg = Algorithm::HS256);
    let (status, _) = me(&state, Some(&format!("Bearer {tok}"))).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(
        jwks.hits(),
        hits_antes,
        "no debe ir al JWKS por un alg inválido"
    );
}

#[tokio::test]
async fn rotacion_de_clave_se_resuelve_con_un_refresh() {
    let k1 = Signer::generate("k1");
    let k2 = Signer::generate("k2");
    let jwks = FakeJwks::serve(&[&k1]).await;
    let state = ready_state(&jwks).await;

    jwks.rotate_to(&k2); // syntroAuth rotó; nosotros todavía tenemos k1
    let hits_antes = jwks.hits();
    let (status, json) = me(&state, Some(&format!("Bearer {}", k2.token(|_| {})))).await;
    assert_eq!(status, StatusCode::OK, "{json}");
    assert_eq!(jwks.hits(), hits_antes + 1, "exactamente un refresh");
}

#[tokio::test]
async fn kid_desconocido_no_martilla_el_jwks() {
    let k1 = Signer::generate("k1");
    let otro = Signer::generate("desconocido");
    let jwks = FakeJwks::serve(&[&k1]).await;
    let state = ready_state(&jwks).await;

    let hits_antes = jwks.hits();
    for _ in 0..5 {
        let (status, _) = me(&state, Some(&format!("Bearer {}", otro.token(|_| {})))).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }
    // El primer kid desconocido puede refrescar UNA vez; los siguientes esperan el cooldown.
    assert_eq!(jwks.hits(), hits_antes + 1, "cooldown de refresh violado");
}

#[tokio::test]
async fn sin_jwks_nunca_cargado_es_503_y_nunca_200() {
    let signer = Signer::generate("k1");
    // Apunta a un puerto cerrado: el refresh inicial falla y no hay claves.
    let state = state_with(task_dashboard_api::auth::JwtConfig {
        issuer: "SyntroAuth".into(),
        audience: "SyntroAuth".into(),
        jwks_url: "http://127.0.0.1:1/.well-known/jwks.json".into(),
    });
    assert!(state.auth.refresh().await.is_err());

    let (status, json) = me(&state, Some(&format!("Bearer {}", signer.token(|_| {})))).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(json["error"], "AUTH_UNAVAILABLE");
    // Y /health sigue vivo: la auth caída no tumba el servicio.
    let res = app(state.clone())
        .oneshot(Request::get("/health").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}

#[tokio::test]
async fn jwks_sin_claves_rsa_se_rechaza() {
    // Un JWKS que solo trae claves EC (o vacío) no puede dejar al servicio "listo" sin poder validar.
    let jwks = FakeJwks::serve(&[]).await;
    let state = state_with(jwks.config());
    assert!(state.auth.refresh().await.is_err());
    assert_eq!(state.auth.key_count(), 0);
}
