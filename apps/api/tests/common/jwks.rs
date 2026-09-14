//! Un syntroAuth de mentira: clave RSA generada acá, JWKS servido por un axum efímero en
//! 127.0.0.1:0, y un fabricante de tokens con los defaults del contrato real (`iss`/`aud`
//! `SyntroAuth`, RS256, `kid`). Cada test elige qué torcer.

use std::sync::{Arc, RwLock};
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

pub struct Signer {
    pub kid: String,
    enc: EncodingKey,
    jwk: Value,
}

impl Signer {
    pub fn generate(kid: &str) -> Signer {
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
        Signer {
            kid: kid.to_string(),
            enc: EncodingKey::from_rsa_pem(pem.as_bytes()).expect("encoding key"),
            jwk,
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
        let token = encode(&header, &claims, &self.enc).expect("encode");
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

/// Servidor JWKS efímero. `keys` es mutable para simular rotación y caída entre requests.
pub struct FakeJwks {
    pub url: String,
    keys: Arc<RwLock<Vec<Value>>>,
    pub hits: Arc<RwLock<usize>>,
}

impl FakeJwks {
    pub async fn serve(signers: &[&Signer]) -> FakeJwks {
        let keys = Arc::new(RwLock::new(
            signers.iter().map(|s| s.jwk.clone()).collect::<Vec<_>>(),
        ));
        let hits = Arc::new(RwLock::new(0usize));
        let (k, h) = (keys.clone(), hits.clone());
        let app = Router::new().route(
            "/.well-known/jwks.json",
            get(move || {
                let (k, h) = (k.clone(), h.clone());
                async move {
                    *h.write().unwrap() += 1;
                    axum::Json(json!({ "keys": *k.read().unwrap() }))
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
        }
    }

    pub fn rotate_to(&self, signer: &Signer) {
        *self.keys.write().unwrap() = vec![signer.jwk.clone()];
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
