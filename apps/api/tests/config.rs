//! Fail-paths de la configuración: un PORT inválido no puede degradar en silencio a 8080.

use task_dashboard_api::config::{Config, ConfigError};

const AUTH: [(&str, &str); 3] = [
    ("SYNTROAUTH_ISSUER", "SyntroAuth"),
    ("SYNTROAUTH_AUDIENCE", "SyntroAuth"),
    ("SYNTROAUTH_JWKS_URL", "http://auth/.well-known/jwks.json"),
];

/// Variables de prueba: las tres de syntroAuth siempre presentes salvo que el test las pise.
fn env(extra: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
    let vars: Vec<(String, String)> = AUTH
        .iter()
        .chain(extra.iter())
        .map(|(n, v)| (n.to_string(), v.to_string()))
        .collect();
    // El último gana: así un test puede pisar una de AUTH con "" para probar su ausencia.
    move |k| {
        vars.iter()
            .rev()
            .find(|(n, _)| n == k)
            .map(|(_, v)| v.clone())
    }
}

#[test]
fn sin_port_usa_8080() {
    let c = Config::from_env(env(&[("DATABASE_URL", "postgres://x")])).unwrap();
    assert_eq!(c.addr.port(), 8080);
}

#[test]
fn port_de_railway_se_respeta() {
    let c = Config::from_env(env(&[("PORT", "6543"), ("DATABASE_URL", "postgres://x")])).unwrap();
    assert_eq!(c.addr.port(), 6543);
}

#[test]
fn port_invalido_es_error_no_default() {
    for raw in ["abc", "0", "70000", ""] {
        let r = Config::from_env(env(&[("PORT", raw), ("DATABASE_URL", "postgres://x")]));
        assert_eq!(
            r,
            Err(ConfigError::InvalidPort(raw.to_string())),
            "PORT={raw:?}"
        );
    }
}

#[test]
fn sin_database_url_no_arranca() {
    for vars in [
        vec![],
        vec![("DATABASE_URL", "")],
        vec![("DATABASE_URL", "   ")],
    ] {
        assert_eq!(
            Config::from_env(env(&vars)),
            Err(ConfigError::MissingDatabaseUrl)
        );
    }
}

#[test]
fn sin_variables_de_syntroauth_no_arranca() {
    for k in [
        "SYNTROAUTH_ISSUER",
        "SYNTROAUTH_AUDIENCE",
        "SYNTROAUTH_JWKS_URL",
    ] {
        let r = Config::from_env(env(&[("DATABASE_URL", "postgres://x"), (k, "")]));
        assert_eq!(r, Err(ConfigError::Missing(k)), "{k}");
    }
}
