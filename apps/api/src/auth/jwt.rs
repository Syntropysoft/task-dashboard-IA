//! Validación local del JWT de syntroAuth contra su JWKS (`SSO_SUITE_GUIDE.md` §1.2):
//! firma RS256 + `iss` + `aud` + `exp`. Solo RS256: HS256 obligaría a compartir el secreto
//! entre apps, que es exactamente lo que el modelo de suite evita.

use std::collections::HashMap;
use std::sync::RwLock;
use std::time::{Duration, Instant};

use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode, decode_header};
use serde::Deserialize;
use tokio::sync::Mutex;
use tracing::{info, warn};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JwtConfig {
    pub issuer: String,
    pub audience: String,
    pub jwks_url: String,
}

/// Claims que esta app usa. Lo demás que emite syntroAuth (`sid`, `acr`, `amr`, `pv`, `role`)
/// se ignora a propósito hasta que una decisión lo necesite.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct Claims {
    pub sub: String,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub tenant_id: Option<String>,
    pub exp: u64,
    #[serde(default)]
    pub iat: Option<u64>,
}

#[derive(Debug)]
pub enum AuthError {
    /// HTTP 401 — el motivo va al log, nunca al cliente: no se distingue "vencido" de "firma mala".
    Unauthorized(&'static str),
    /// HTTP 503 — no hay claves para validar (JWKS inaccesible desde el arranque): fail-closed,
    /// sin aceptar nada, pero distinguible de "token malo" para operar.
    JwksUnavailable,
}

impl std::fmt::Display for AuthError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AuthError::Unauthorized(why) => write!(f, "no autorizado: {why}"),
            AuthError::JwksUnavailable => write!(f, "JWKS de syntroAuth no disponible"),
        }
    }
}

impl std::error::Error for AuthError {}

/// Forma mínima del JWKS: solo lo que hace falta para RS256. Parsear a mano evita depender
/// de la representación interna del crate y hace explícito qué se acepta.
#[derive(Deserialize)]
struct JwkSet {
    keys: Vec<Jwk>,
}

#[derive(Deserialize)]
struct Jwk {
    kid: Option<String>,
    kty: String,
    n: Option<String>,
    e: Option<String>,
}

/// Entre refrescos disparados por un `kid` desconocido. Un atacante que mande kids al azar no
/// puede convertir cada request en un GET al JWKS. La carga inicial no cuenta para el cooldown:
/// si syntroAuth rota justo después de nuestro arranque, el primer kid nuevo tiene que poder
/// refrescar (cazado por test el 2026-09-13).
const REFRESH_COOLDOWN: Duration = Duration::from_secs(30);

pub struct Validator {
    cfg: JwtConfig,
    http: reqwest::Client,
    keys: RwLock<HashMap<String, DecodingKey>>,
    /// Último refresh disparado por kid desconocido (no la carga inicial).
    last_kid_refresh: Mutex<Option<Instant>>,
    validation: Validation,
}

impl Validator {
    pub fn new(cfg: JwtConfig) -> Validator {
        let mut validation = Validation::new(Algorithm::RS256);
        validation.set_issuer(&[cfg.issuer.as_str()]);
        validation.set_audience(&[cfg.audience.as_str()]);
        // Por defecto el crate solo exige `exp`: un token SIN `aud` pasaba el set_audience.
        // Cazado por test el 2026-09-13. Sin estos cuatro no hay identidad que valga.
        validation.set_required_spec_claims(&["exp", "iss", "aud", "sub"]);
        validation.leeway = 30; // segundos: reloj de Railway vs syntroAuth
        validation.validate_exp = true;
        Validator {
            cfg,
            http: reqwest::Client::builder()
                .timeout(Duration::from_secs(5))
                .build()
                .expect("reqwest client"),
            keys: RwLock::new(HashMap::new()),
            last_kid_refresh: Mutex::new(None),
            validation,
        }
    }

    pub fn config(&self) -> &JwtConfig {
        &self.cfg
    }

    pub fn key_count(&self) -> usize {
        self.keys.read().expect("keys lock").len()
    }

    /// Baja el JWKS y reemplaza el set completo. Un JWKS vacío o sin claves RSA se rechaza:
    /// dejaría al servicio sin poder validar nada, y eso tiene que ser visible, no silencioso.
    pub async fn refresh(&self) -> Result<usize, AuthError> {
        let set: JwkSet = self
            .http
            .get(&self.cfg.jwks_url)
            .send()
            .await
            .and_then(|r| r.error_for_status())
            .map_err(|e| {
                warn!(error = %e, url = %self.cfg.jwks_url, "no se pudo bajar el JWKS");
                AuthError::JwksUnavailable
            })?
            .json()
            .await
            .map_err(|e| {
                warn!(error = %e, "JWKS con forma inesperada");
                AuthError::JwksUnavailable
            })?;

        let mut fresh = HashMap::new();
        for k in set.keys {
            if k.kty != "RSA" {
                continue;
            }
            let (Some(kid), Some(n), Some(e)) = (k.kid, k.n, k.e) else {
                continue;
            };
            match DecodingKey::from_rsa_components(&n, &e) {
                Ok(dk) => {
                    fresh.insert(kid, dk);
                }
                Err(err) => warn!(error = %err, "clave RSA del JWKS inválida, se ignora"),
            }
        }
        if fresh.is_empty() {
            warn!("el JWKS no trae ninguna clave RSA usable");
            return Err(AuthError::JwksUnavailable);
        }
        let n = fresh.len();
        *self.keys.write().expect("keys lock") = fresh;
        info!(claves = n, "JWKS de syntroAuth cargado");
        Ok(n)
    }

    fn key_for(&self, kid: &str) -> Option<DecodingKey> {
        self.keys.read().expect("keys lock").get(kid).cloned()
    }

    /// Refresca por kid desconocido solo si pasó el cooldown desde el último intento (exitoso o
    /// no: un JWKS caído tampoco se martilla). El lock se sostiene durante el GET a propósito:
    /// N requests simultáneas con el mismo kid nuevo producen UN refresh, no N.
    async fn refresh_for_unknown_kid(&self) {
        let mut last = self.last_kid_refresh.lock().await;
        if let Some(t) = *last
            && t.elapsed() < REFRESH_COOLDOWN
        {
            return;
        }
        *last = Some(Instant::now());
        let _ = self.refresh().await;
    }

    pub async fn validate(&self, token: &str) -> Result<Claims, AuthError> {
        let header =
            decode_header(token).map_err(|_| AuthError::Unauthorized("header ilegible"))?;
        if header.alg != Algorithm::RS256 {
            return Err(AuthError::Unauthorized("alg distinto de RS256"));
        }
        let kid = header.kid.ok_or(AuthError::Unauthorized("sin kid"))?;

        let key = match self.key_for(&kid) {
            Some(k) => k,
            None => {
                let had_keys = self.key_count() > 0;
                self.refresh_for_unknown_kid().await;
                match self.key_for(&kid) {
                    Some(k) => k,
                    // Sin ninguna clave nunca cargada es un problema nuestro (503); con claves
                    // pero sin este kid es un token que no reconocemos (401).
                    None if !had_keys && self.key_count() == 0 => {
                        return Err(AuthError::JwksUnavailable);
                    }
                    None => return Err(AuthError::Unauthorized("kid desconocido")),
                }
            }
        };

        decode::<Claims>(token, &key, &self.validation)
            .map(|d| d.claims)
            .map_err(|e| {
                warn!(error = %e, "JWT rechazado");
                AuthError::Unauthorized("firma o claims inválidos")
            })
    }
}
