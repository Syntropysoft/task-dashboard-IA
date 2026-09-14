//! Configuración por variables de entorno. Railway inyecta `PORT`; todo lo demás llega después
//! (paso 2 y 3a del plan). Un valor inválido es un error de arranque, no un default silencioso.

use std::net::{Ipv4Addr, SocketAddr};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    pub addr: SocketAddr,
}

#[derive(Debug, PartialEq, Eq)]
pub enum ConfigError {
    InvalidPort(String),
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigError::InvalidPort(v) => write!(f, "PORT inválido: {v:?} (se espera 1..=65535)"),
        }
    }
}

impl std::error::Error for ConfigError {}

impl Config {
    /// `PORT` ausente → 8080 (default fuera de Railway). Presente pero inválido → error, no default.
    pub fn from_env<F>(get: F) -> Result<Self, ConfigError>
    where
        F: Fn(&str) -> Option<String>,
    {
        let port = match get("PORT") {
            None => 8080,
            Some(raw) => match raw.trim().parse::<u16>() {
                Ok(p) if p > 0 => p,
                _ => return Err(ConfigError::InvalidPort(raw)),
            },
        };
        Ok(Config {
            addr: SocketAddr::from((Ipv4Addr::UNSPECIFIED, port)),
        })
    }
}
