//! Un syntroAuth de mentira: clave RSA generada acá, JWKS servido por un axum efímero en
//! 127.0.0.1:0, y un fabricante de tokens con los defaults del contrato real (`iss`/`aud`
//! `SyntroAuth`, RS256, `kid`). Cada test elige qué torcer.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock, RwLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use axum::{Router, routing::get};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use jsonwebtoken::{Algorithm, EncodingKey, Header, encode};
use rsa::pkcs1::EncodeRsaPrivateKey;
use rsa::traits::PublicKeyParts;
use rsa::{RsaPrivateKey, RsaPublicKey};
use serde_json::{Value, json};
use task_dashboard_api::auth::JwtConfig;

pub const ISS: &str = "SyntroAuth";
pub const AUD: &str = "SyntroAuth";

/// Par RSA ya generado. Generar 2048 bits cuesta ~1-2 s; se hace una vez por `kid` y por
/// binario de tests (ver `KEY_STORE`).
struct KeyMaterial {
    enc: EncodingKey,
    jwk: Value,
}

/// Depósito de claves del binario de tests: vive desde la primera clave pedida hasta que el
/// proceso termina — los tests de un mismo binario corren en hilos del mismo proceso, así que
/// comparten el depósito y ninguno vuelve a pagar la generación. Cada `kid` tiene SU par fijo.
static KEY_STORE: OnceLock<Mutex<HashMap<String, Arc<KeyMaterial>>>> = OnceLock::new();

fn key_for(kid: &str) -> Arc<KeyMaterial> {
    let store = KEY_STORE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Some(k) = store.lock().unwrap().get(kid) {
        return k.clone();
    }
    // Generar FUERA del lock: los tests arrancan en paralelo y cada kid se genera en su hilo.
    // Si dos hilos piden el mismo kid a la vez, gana el primero que inserta y el otro descarta
    // la suya — se paga una clave de más, nunca se reparten dos claves distintas para un kid.
    let fresh = Arc::new(fresh_key(kid));
    store
        .lock()
        .unwrap()
        .entry(kid.to_string())
        .or_insert(fresh)
        .clone()
}

fn fresh_key(kid: &str) -> KeyMaterial {
    let mut rng = rand::thread_rng();
    let private = RsaPrivateKey::new(&mut rng, 2048).expect("rsa keygen");
    let public = RsaPublicKey::from(&private);
    let pem = private
        .to_pkcs1_pem(rsa::pkcs1::LineEnding::LF)
        .expect("pem");
    let jwk = json!({
        "kty": "RSA", "use": "sig", "alg": "RS256", "kid": kid,
        "n": URL_SAFE_NO_PAD.encode(public.n().to_bytes_be()),
        "e": URL_SAFE_NO_PAD.encode(public.e().to_bytes_be()),
    });
    KeyMaterial {
        enc: EncodingKey::from_rsa_pem(pem.as_bytes()).expect("encoding key"),
        jwk,
    }
}

pub struct Signer {
    pub kid: String,
    key: Arc<KeyMaterial>,
}

impl Signer {
    /// La clave del depósito para este `kid`: misma clave en todos los tests del binario.
    pub fn generate(kid: &str) -> Signer {
        Signer {
            kid: kid.to_string(),
            key: key_for(kid),
        }
    }

    /// Una clave NUEVA que no entra al depósito: para simular un impostor que firma con el
    /// mismo `kid` pero otra clave privada.
    pub fn generate_fresh(kid: &str) -> Signer {
        Signer {
            kid: kid.to_string(),
            key: Arc::new(fresh_key(kid)),
        }
    }

    pub fn token(&self, tweak: impl FnOnce(&mut TokenSpec)) -> String {
        let mut spec = TokenSpec::default();
        tweak(&mut spec);
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        let mut claims = json!({
            "sub": spec.sub, "email": "gabriel@example.test", "tenant_id": "t-1",
            "iat": now, "exp": now + spec.exp_offset_secs, "jti": "j-1",
        });
        if let Some(iss) = &spec.iss {
            claims["iss"] = json!(iss);
        }
        if let Some(aud) = &spec.aud {
            claims["aud"] = json!(aud);
        }
        let kid = spec.kid.clone().unwrap_or_else(|| self.kid.clone());
        let mut header = Header::new(Algorithm::RS256);
        header.kid = Some(kid.clone());
        let token = encode(&header, &claims, &self.key.enc).expect("encode");
        if spec.alg == Algorithm::RS256 {
            return token;
        }
        // El crate se niega a firmar HS256 con una clave RSA (y hace bien). Para simular un
        // atacante que reescribe el header, se reemplaza el primer segmento a mano: la firma
        // RSA queda intacta y el validador tiene que cortar por el `alg` antes de mirarla.
        let alg = serde_json::to_value(spec.alg).unwrap();
        let forged =
            URL_SAFE_NO_PAD.encode(json!({ "alg": alg, "typ": "JWT", "kid": kid }).to_string());
        let mut parts = token.splitn(2, '.');
        let _ = parts.next();
        format!("{forged}.{}", parts.next().unwrap())
    }
}

pub struct TokenSpec {
    pub sub: String,
    pub iss: Option<String>,
    pub aud: Option<String>,
    pub exp_offset_secs: i64,
    pub alg: Algorithm,
    pub kid: Option<String>,
}

impl Default for TokenSpec {
    fn default() -> Self {
        TokenSpec {
            sub: "8d5a1f2e-0000-4000-8000-000000000001".into(),
            iss: Some(ISS.into()),
            aud: Some(AUD.into()),
            exp_offset_secs: 1800,
            alg: Algorithm::RS256,
            kid: None,
        }
    }
}

/// Qué responde `/api/auth/validate` del syntroAuth falso.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ValidateMode {
    Ok,
    Reject,
    Down,
}

/// Servidor JWKS efímero. `keys` es mutable para simular rotación y caída entre requests.
pub struct FakeJwks {
    pub url: String,
    keys: Arc<RwLock<Vec<Value>>>,
    pub hits: Arc<RwLock<usize>>,
    validate: Arc<RwLock<ValidateMode>>,
}

impl FakeJwks {
    pub async fn serve(signers: &[&Signer]) -> FakeJwks {
        let keys = Arc::new(RwLock::new(
            signers
                .iter()
                .map(|s| s.key.jwk.clone())
                .collect::<Vec<_>>(),
        ));
        let hits = Arc::new(RwLock::new(0usize));
        let validate = Arc::new(RwLock::new(ValidateMode::Ok));
        let (k, h, v) = (keys.clone(), hits.clone(), validate.clone());
        let app = Router::new()
            .route(
                "/.well-known/jwks.json",
                get(move || {
                    let (k, h) = (k.clone(), h.clone());
                    async move {
                        *h.write().unwrap() += 1;
                        axum::Json(json!({ "keys": *k.read().unwrap() }))
                    }
                }),
            )
            .route(
                "/api/auth/validate",
                get(move |headers: axum::http::HeaderMap| {
                    let v = v.clone();
                    async move {
                        let has_bearer = headers
                            .get("authorization")
                            .and_then(|h| h.to_str().ok())
                            .is_some_and(|h| h.starts_with("Bearer "));
                        match *v.read().unwrap() {
                            ValidateMode::Ok if has_bearer => axum::http::StatusCode::OK,
                            ValidateMode::Ok | ValidateMode::Reject => {
                                axum::http::StatusCode::UNAUTHORIZED
                            }
                            ValidateMode::Down => axum::http::StatusCode::BAD_GATEWAY,
                        }
                    }
                }),
            );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        tokio::time::sleep(Duration::from_millis(20)).await;
        FakeJwks {
            url: format!("http://{addr}/.well-known/jwks.json"),
            keys,
            hits,
            validate,
        }
    }

    pub fn set_validate(&self, mode: ValidateMode) {
        *self.validate.write().unwrap() = mode;
    }

    pub fn rotate_to(&self, signer: &Signer) {
        *self.keys.write().unwrap() = vec![signer.key.jwk.clone()];
    }

    pub fn hits(&self) -> usize {
        *self.hits.read().unwrap()
    }

    pub fn config(&self) -> JwtConfig {
        JwtConfig {
            issuer: ISS.into(),
            audience: AUD.into(),
            jwks_url: self.url.clone(),
        }
    }
}
