//! El servidor MCP: las cinco herramientas del contrato (`docs/PLAN-PASO-1.md`) sobre los
//! módulos de dominio. Proyecto y usuario salen del `PatContext` que dejó el middleware en las
//! `Parts` HTTP — rmcp las inyecta en cada tool (`Extension<Parts>`, verificado en
//! `rmcp-3.3.0/src/transport/streamable_http_server/tower.rs`). Ninguna tool acepta `proyecto`
//! ni `quien`.
//!
//! Errores de dominio salen como resultado `is_error` con `CODIGO: explicación`, no como error
//! JSON-RPC: es lo que el agente lee y por lo que decide (p. ej. `PREFIJO_DESCONOCIDO` → pedir
//! el seed, nunca inventar un ID).

use http::request::Parts;
use rmcp::{
    ServerHandler,
    handler::server::{router::tool::ToolRouter, tool::Extension, wrapper::Parameters},
    model::{ServerCapabilities, ServerInfo},
    tool, tool_handler, tool_router,
};
use schemars::JsonSchema;
use serde::Deserialize;
use sqlx::PgPool;

use crate::{
    api::sequences::prefijo_valido,
    auth::pat::PatContext,
    claims::{self, ClaimError},
    ids::{self, IdError},
    suggestions::{self, SuggestError},
};

pub const INSTRUCTIONS: &str = "\
Coordinación de fichas e IDs entre agentes de un mismo proyecto. Reglas del contrato:
1. Si `reservar_id` falla o no responde, NO inventes ni completes un ID: avisá y esperá. \
Un reintento con espera es aceptable; inventar, nunca.
2. Antes de trabajar una ficha, `tomar_ficha`. Al cerrarla por commit, `liberar_ficha`.
3. `force` es solo para claims huérfanos (sesión muerta). Quien fuerza asume el choque; queda en el log.
Los errores vienen como `CODIGO: explicación` (YA_TOMADA, NO_ES_TUYA, NO_TOMADA, PREFIJO_DESCONOCIDO…).";

#[derive(Clone)]
pub struct TaskDashboardMcp {
    pool: PgPool,
    tool_router: ToolRouter<Self>,
}

#[derive(Deserialize, JsonSchema)]
pub struct ReservarIdParams {
    /// Prefijo del contador, en mayúsculas (ej. `MVC`). Tiene que estar sembrado en el proyecto.
    pub prefijo: String,
}

#[derive(Deserialize, JsonSchema)]
pub struct TomarFichaParams {
    /// Identificador de la ficha tal como aparece en el repo (ej. `MVC-0385`).
    pub ficha_id: String,
    /// Opcional: dónde la estás trabajando (rama, contexto).
    #[serde(default)]
    pub nota: Option<String>,
    /// Pisar un claim ajeno. Solo para claims huérfanos; queda registrado con ambos nombres.
    #[serde(default)]
    pub force: bool,
}

#[derive(Deserialize, JsonSchema)]
pub struct LiberarFichaParams {
    pub ficha_id: String,
    /// Liberar un claim ajeno (huérfano). Queda registrado.
    #[serde(default)]
    pub force: bool,
}

#[derive(Deserialize, JsonSchema)]
pub struct SugerirParams {
    /// El hallazgo, en una o pocas frases. Todavía no es una ficha.
    pub texto: String,
    /// Opcional: ficha, archivo o tema desde donde surgió.
    #[serde(default)]
    pub contexto: Option<String>,
}

fn ctx(parts: &Parts) -> Result<PatContext, String> {
    parts
        .extensions
        .get::<PatContext>()
        .cloned()
        .ok_or_else(|| "SIN_PAT: la request llegó sin contexto de PAT (bug de montaje)".to_string())
}

fn db_err(e: sqlx::Error) -> String {
    tracing::error!(error = %e, "error de base en una tool MCP");
    "DB_ERROR: la base no respondió; reintentá en unos segundos, no inventes nada".to_string()
}

#[tool_router]
impl TaskDashboardMcp {
    pub fn new(pool: PgPool) -> Self {
        Self {
            pool,
            tool_router: Self::tool_router(),
        }
    }

    #[tool(
        name = "reservar_id",
        description = "Reserva atómicamente el próximo ID del prefijo en este proyecto (ej. MVC → MVC-0407). Dos sesiones nunca reciben el mismo. Si falla, NO inventes un ID."
    )]
    async fn reservar_id(
        &self,
        Extension(parts): Extension<Parts>,
        Parameters(p): Parameters<ReservarIdParams>,
    ) -> Result<rmcp::handler::server::wrapper::Json<ids::Reserved>, String> {
        let c = ctx(&parts)?;
        let prefijo = p.prefijo.trim();
        if !prefijo_valido(prefijo) {
            return Err(
                "PREFIJO_INVALIDO: mayúsculas y dígitos, hasta 16 caracteres (ej. MVC)".into(),
            );
        }
        match ids::reserve(&self.pool, c.project_id, prefijo, &c.user_sub).await {
            Ok(r) => Ok(rmcp::handler::server::wrapper::Json(r)),
            Err(IdError::UnknownPrefix) => Err(format!(
                "PREFIJO_DESCONOCIDO: este proyecto no tiene contador para `{prefijo}`. \
                 Un owner tiene que sembrarlo (PUT /api/projects/{{slug}}/sequences/{prefijo}). No inventes un ID."
            )),
            Err(IdError::Db(e)) => Err(db_err(e)),
        }
    }

    #[tool(
        name = "tomar_ficha",
        description = "Toma una ficha para trabajarla. Si otro la tiene, devuelve YA_TOMADA con quién y desde cuándo. Retomar la propia actualiza la nota."
    )]
    async fn tomar_ficha(
        &self,
        Extension(parts): Extension<Parts>,
        Parameters(p): Parameters<TomarFichaParams>,
    ) -> Result<rmcp::handler::server::wrapper::Json<claims::Taken>, String> {
        let c = ctx(&parts)?;
        let id = p.ficha_id.trim();
        if !claims::ficha_id_valido(id) {
            return Err(
                "FICHA_ID_INVALIDO: sin espacios, hasta 64 caracteres (ej. MVC-0385)".into(),
            );
        }
        let nota = p.nota.as_deref().map(str::trim).filter(|n| !n.is_empty());
        match claims::take(&self.pool, c.project_id, id, &c.user_sub, nota, p.force).await {
            Ok(t) => Ok(rmcp::handler::server::wrapper::Json(t)),
            Err(ClaimError::AlreadyTaken {
                held_by,
                held_since,
            }) => Err(format!(
                "YA_TOMADA: `{id}` la tiene {held_by} desde {held_since}. Hablá con esa persona o, si la sesión murió, usá force=true."
            )),
            Err(ClaimError::NotYours { held_by }) => {
                Err(format!("NO_ES_TUYA: `{id}` es de {held_by}"))
            }
            Err(ClaimError::NotTaken) => Err(format!("NO_TOMADA: `{id}` no está tomada")),
            Err(ClaimError::Db(e)) => Err(db_err(e)),
        }
    }

    #[tool(
        name = "liberar_ficha",
        description = "Libera una ficha que tenías tomada (al cerrarla por commit). force=true libera un claim ajeno huérfano."
    )]
    async fn liberar_ficha(
        &self,
        Extension(parts): Extension<Parts>,
        Parameters(p): Parameters<LiberarFichaParams>,
    ) -> Result<String, String> {
        let c = ctx(&parts)?;
        let id = p.ficha_id.trim();
        if !claims::ficha_id_valido(id) {
            return Err(
                "FICHA_ID_INVALIDO: sin espacios, hasta 64 caracteres (ej. MVC-0385)".into(),
            );
        }
        match claims::release(&self.pool, c.project_id, id, &c.user_sub, p.force).await {
            Ok(()) => Ok(format!("liberada `{id}`")),
            Err(ClaimError::NotYours { held_by }) => Err(format!(
                "NO_ES_TUYA: `{id}` es de {held_by}. Si su sesión murió, force=true."
            )),
            Err(ClaimError::NotTaken) => Err(format!("NO_TOMADA: `{id}` no estaba tomada")),
            Err(ClaimError::AlreadyTaken { .. }) => {
                unreachable!("release no devuelve AlreadyTaken")
            }
            Err(ClaimError::Db(e)) => Err(db_err(e)),
        }
    }

    #[tool(
        name = "fichas_tomadas",
        description = "Lista qué fichas están tomadas en este proyecto, por quién y desde cuándo."
    )]
    async fn fichas_tomadas(
        &self,
        Extension(parts): Extension<Parts>,
    ) -> Result<rmcp::handler::server::wrapper::Json<Vec<claims::Claim>>, String> {
        let c = ctx(&parts)?;
        claims::list(&self.pool, c.project_id)
            .await
            .map(rmcp::handler::server::wrapper::Json)
            .map_err(|e| match e {
                ClaimError::Db(e) => db_err(e),
                other => format!("ERROR: {other:?}"),
            })
    }

    #[tool(
        name = "sugerir",
        description = "Registra un hallazgo que todavía no es ficha (mejora, riesgo, deuda) para que no se pierda."
    )]
    async fn sugerir(
        &self,
        Extension(parts): Extension<Parts>,
        Parameters(p): Parameters<SugerirParams>,
    ) -> Result<String, String> {
        let c = ctx(&parts)?;
        match suggestions::create(
            &self.pool,
            c.project_id,
            &c.user_sub,
            &p.texto,
            p.contexto.as_deref(),
        )
        .await
        {
            Ok(id) => Ok(format!("sugerencia #{id} registrada")),
            Err(SuggestError::EmptyText) => Err("TEXTO_VACIO: escribí el hallazgo".into()),
            Err(SuggestError::TooLong) => Err(format!(
                "TEXTO_DEMASIADO_LARGO: hasta {} caracteres (contexto hasta {})",
                suggestions::MAX_TEXT,
                suggestions::MAX_CONTEXT
            )),
            Err(SuggestError::Db(e)) => Err(db_err(e)),
        }
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for TaskDashboardMcp {
    fn get_info(&self) -> ServerInfo {
        let mut info = ServerInfo::default().with_instructions(INSTRUCTIONS);
        info.capabilities = ServerCapabilities::builder().enable_tools().build();
        info
    }
}
