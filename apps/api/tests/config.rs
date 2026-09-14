//! Fail-paths de la configuración: un PORT inválido no puede degradar en silencio a 8080.

use task_dashboard_api::config::{Config, ConfigError};

fn env(vars: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
    let vars: Vec<(String, String)> = vars
        .iter()
        .map(|(n, v)| (n.to_string(), v.to_string()))
        .collect();
    move |k| vars.iter().find(|(n, _)| n == k).map(|(_, v)| v.clone())
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
