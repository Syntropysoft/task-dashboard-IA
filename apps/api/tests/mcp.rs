//! Spec del ítem 7: el servidor MCP real (rmcp, Streamable HTTP, sesiones en memoria) detrás del
//! PAT. Se habla JSON-RPC por HTTP contra un servidor levantado en 127.0.0.1:0, como lo haría
//! Claude Code: initialize → notifications/initialized → tools/list → tools/call, con
//! `Mcp-Session-Id`. Sin PAT: 401 antes de llegar al MCP.

mod common;

use std::sync::Mutex;

use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use common::TestDb;
use common::jwks::{FakeJwks, Signer};
use common::state::state_with_pool;
use http_body_util::BodyExt;
use serde_json::{Value, json};
use task_dashboard_api::{app, state::AppState};
use tower::ServiceExt;

const GABRIEL: &str = "8d5a1f2e-0000-4000-8000-000000000001";

async fn api(
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

fn init_params() -> Value {
    json!({ "protocolVersion": "2025-03-26", "capabilities": {}, "clientInfo": { "name": "test", "version": "0" } })
}

struct Mcp {
    url: String,
    http: reqwest::Client,
    pat: String,
    session: Mutex<Option<String>>,
}

impl Mcp {
    /// Un POST JSON-RPC como lo hace un cliente con handshake: si ya hay sesión, viaja en
    /// `Mcp-Session-Id`; si el servidor devuelve una, se guarda. (status, body JSON o texto).
    async fn rpc(&self, auth: Option<&str>, method: &str, params: Value) -> (u16, Value) {
        let mut body = json!({ "jsonrpc": "2.0", "method": method, "params": params });
        if !method.starts_with("notifications/") {
            body["id"] = json!(1);
        }
        let mut req = self
            .http
            .post(&self.url)
            .header("content-type", "application/json")
            .header("accept", "application/json, text/event-stream")
            .json(&body);
        if let Some(a) = auth {
            req = req.header("authorization", a);
        }
        if let Some(sid) = self.session.lock().unwrap().clone() {
            req = req.header("mcp-session-id", sid);
        }
        let res = req.send().await.unwrap();
        if let Some(sid) = res
            .headers()
            .get("mcp-session-id")
            .and_then(|v| v.to_str().ok())
        {
            *self.session.lock().unwrap() = Some(sid.to_string());
        }
        let status = res.status().as_u16();
        let text = res.text().await.unwrap_or_default();
        (status, parse_body(&text))
    }

    /// initialize + notifications/initialized, como cualquier cliente MCP.
    async fn handshake(&self) -> Value {
        let (status, init) = self.rpc(Some(&self.pat), "initialize", init_params()).await;
        assert_eq!(status, 200, "{init}");
        assert!(
            self.session.lock().unwrap().is_some(),
            "el servidor tiene que devolver Mcp-Session-Id"
        );
        let (status, _) = self
            .rpc(Some(&self.pat), "notifications/initialized", json!({}))
            .await;
        assert!(status == 200 || status == 202, "initialized: {status}");
        init
    }

    async fn call(&self, tool: &str, args: Value) -> Value {
        let (status, body) = self
            .rpc(
                Some(&self.pat),
                "tools/call",
                json!({ "name": tool, "arguments": args }),
            )
            .await;
        assert_eq!(status, 200, "{body}");
        body["result"].clone()
    }
}

/// Servidor real en un puerto efímero + proyecto sembrado + PAT emitido por la API.
async fn world() -> (AppState, TestDb, Mcp) {
    let tdb = TestDb::create().await;
    let pool = tdb.pool().await;
    task_dashboard_api::db::migrate(&pool).await.unwrap();
    let signer = Signer::generate("k1");
    let jwks = FakeJwks::serve(&[&signer]).await;
    let state = state_with_pool(jwks.config(), pool);
    state.auth.refresh().await.unwrap();
    let jwt = format!("Bearer {}", signer.token(|t| t.sub = GABRIEL.into()));
    api(
        &state,
        Method::POST,
        "/api/projects",
        &jwt,
        Some(json!({"slug": "convertix", "name": "c", "repo_url": "https://x"})),
    )
    .await;
    api(
        &state,
        Method::PUT,
        "/api/projects/convertix/sequences/MVC",
        &jwt,
        Some(json!({"next": 407})),
    )
    .await;
    let (_, t) = api(
        &state,
        Method::POST,
        "/api/projects/convertix/tokens",
        &jwt,
        Some(json!({"name": "claude"})),
    )
    .await;
    let pat = format!("Bearer {}", t["token"].as_str().unwrap());

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let router = app(state.clone());
    tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    let mcp = Mcp {
        url: format!("http://{addr}/mcp"),
        http: reqwest::Client::new(),
        pat,
        session: Mutex::new(None),
    };
    (state, tdb, mcp)
}

/// rmcp contesta JSON cuando puede y SSE (`event: message` / `data: {...}`) cuando el handler
/// emitió algo antes de la respuesta. El cliente de prueba entiende las dos formas.
fn parse_body(text: &str) -> Value {
    if let Ok(v) = serde_json::from_str::<Value>(text) {
        return v;
    }
    let last = text
        .lines()
        .filter_map(|l| l.strip_prefix("data:"))
        .filter_map(|d| serde_json::from_str::<Value>(d.trim()).ok())
        .next_back();
    last.unwrap_or_else(|| Value::String(text.to_string()))
}

fn texto(result: &Value) -> String {
    result["content"][0]["text"]
        .as_str()
        .unwrap_or("")
        .to_string()
}

#[tokio::test]
async fn initialize_y_tools_list_exponen_las_cinco_herramientas_con_las_instrucciones() {
    let (state, tdb, mcp) = world().await;
    let init = mcp.handshake().await;
    let instr = init["result"]["instructions"].as_str().unwrap();
    assert!(
        instr.contains("NO inventes"),
        "las reglas del contrato viajan en initialize: {instr}"
    );

    let (status, list) = mcp.rpc(Some(&mcp.pat), "tools/list", json!({})).await;
    assert_eq!(status, 200, "{list}");
    let tools = list["result"]["tools"].as_array().unwrap();
    let mut names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
    names.sort();
    assert_eq!(
        names,
        [
            "fichas_tomadas",
            "liberar_ficha",
            "reservar_id",
            "sugerir",
            "tomar_ficha"
        ]
    );
    // Ninguna tool acepta proyecto ni quién: salen del PAT.
    for t in tools {
        let props = t["inputSchema"]["properties"]
            .as_object()
            .cloned()
            .unwrap_or_default();
        for prohibido in [
            "proyecto",
            "project",
            "project_id",
            "quien",
            "user",
            "user_sub",
        ] {
            assert!(
                !props.contains_key(prohibido),
                "{}: acepta `{prohibido}`",
                t["name"]
            );
        }
    }
    state.pool.close().await;
    tdb.drop().await;
}

#[tokio::test]
async fn flujo_completo_por_mcp_reservar_tomar_listar_sugerir_liberar() {
    let (state, tdb, mcp) = world().await;
    mcp.handshake().await;

    let r = mcp.call("reservar_id", json!({ "prefijo": "MVC" })).await;
    assert_ne!(r["isError"], true, "{r}");
    assert_eq!(r["structuredContent"]["id"], "MVC-0407", "{r}");
    let r = mcp.call("reservar_id", json!({ "prefijo": "MVC" })).await;
    assert_eq!(r["structuredContent"]["id"], "MVC-0408");

    let r = mcp
        .call(
            "tomar_ficha",
            json!({ "ficha_id": "MVC-0407", "nota": "arrancando" }),
        )
        .await;
    assert_eq!(r["structuredContent"]["held_by"], GABRIEL, "{r}");

    let r = mcp.call("fichas_tomadas", json!({})).await;
    let fichas = r["structuredContent"].as_array().unwrap();
    assert_eq!(fichas.len(), 1);
    assert_eq!(fichas[0]["ficha_id"], "MVC-0407");

    let r = mcp
        .call(
            "sugerir",
            json!({ "texto": "el retry no loguea el status", "contexto": "MVC-0407" }),
        )
        .await;
    assert_ne!(r["isError"], true, "{r}");
    assert!(texto(&r).contains("sugerencia #"), "{r}");

    let r = mcp
        .call("liberar_ficha", json!({ "ficha_id": "MVC-0407" }))
        .await;
    assert_ne!(r["isError"], true, "{r}");
    let r = mcp.call("fichas_tomadas", json!({})).await;
    assert!(r["structuredContent"].as_array().unwrap().is_empty());
    state.pool.close().await;
    tdb.drop().await;
}

#[tokio::test]
async fn los_errores_de_dominio_llegan_al_agente_como_is_error_con_codigo() {
    let (state, tdb, mcp) = world().await;
    mcp.handshake().await;

    let r = mcp.call("reservar_id", json!({ "prefijo": "FE" })).await;
    assert_eq!(r["isError"], true, "{r}");
    assert!(texto(&r).starts_with("PREFIJO_DESCONOCIDO:"), "{r}");
    assert!(
        texto(&r).contains("No inventes"),
        "el mensaje repite la regla: {r}"
    );

    let r = mcp
        .call("liberar_ficha", json!({ "ficha_id": "MVC-0999" }))
        .await;
    assert_eq!(r["isError"], true);
    assert!(texto(&r).starts_with("NO_TOMADA:"), "{r}");

    let r = mcp
        .call("tomar_ficha", json!({ "ficha_id": "con espacio" }))
        .await;
    assert_eq!(r["isError"], true);
    assert!(texto(&r).starts_with("FICHA_ID_INVALIDO:"), "{r}");

    // Argumento faltante: error de protocolo o is_error, nunca un 500 ni un panic.
    let (status, body) = mcp
        .rpc(
            Some(&mcp.pat),
            "tools/call",
            json!({ "name": "reservar_id", "arguments": {} }),
        )
        .await;
    assert_eq!(status, 200, "{body}");
    assert!(
        body["error"].is_object() || body["result"]["isError"] == true,
        "{body}"
    );
    state.pool.close().await;
    tdb.drop().await;
}

#[tokio::test]
async fn sin_pat_el_mcp_no_responde_nada_y_host_ajeno_se_rechaza() {
    let (state, tdb, mcp) = world().await;
    let (status, body) = mcp.rpc(None, "initialize", init_params()).await;
    assert_eq!(status, 401, "{body}");
    assert_eq!(body, json!({ "error": "UNAUTHORIZED" }));
    let (status, _) = mcp
        .rpc(Some("Bearer tdp_no-existe"), "initialize", init_params())
        .await;
    assert_eq!(status, 401);

    // Anti DNS-rebinding de rmcp: un Host fuera de la lista se rechaza aunque el PAT sea válido.
    let res = mcp
        .http
        .post(&mcp.url)
        .header("authorization", &mcp.pat)
        .header("host", "evil.example")
        .header("content-type", "application/json")
        .header("accept", "application/json, text/event-stream")
        .json(
            &json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": init_params() }),
        )
        .send()
        .await
        .unwrap();
    assert!(
        res.status().is_client_error(),
        "host ajeno: {}",
        res.status()
    );
    state.pool.close().await;
    tdb.drop().await;
}

#[tokio::test]
async fn una_sesion_desconocida_es_404_y_el_cliente_puede_reinicializar() {
    // Lo que pasa después de un app sleeping / redeploy: la sesión en memoria ya no existe.
    let (state, tdb, mcp) = world().await;
    *mcp.session.lock().unwrap() = Some("sesion-de-antes-del-redeploy".into());
    let (status, body) = mcp.rpc(Some(&mcp.pat), "tools/list", json!({})).await;
    assert_eq!(status, 404, "{body}");
    *mcp.session.lock().unwrap() = None;
    mcp.handshake().await;
    let (status, list) = mcp.rpc(Some(&mcp.pat), "tools/list", json!({})).await;
    assert_eq!(status, 200, "{list}");
    assert_eq!(list["result"]["tools"].as_array().unwrap().len(), 5);
    state.pool.close().await;
    tdb.drop().await;
}
