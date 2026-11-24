//! Configuración leída del entorno (y de `.env`). No hace red ni conoce el flujo OAuth.

use std::path::PathBuf;

use anyhow::{Result, bail};

use crate::realtime::Channel;

const DEFAULT_REDIRECT_URI: &str = "http://localhost:8080/auth/callback";
const DEFAULT_SCOPES: &str = "user:read channel:read chat:write";
const DEFAULT_TOKEN_FILE: &str = ".kick_tokens.json";

#[derive(Debug, Clone, PartialEq)]
pub struct Settings {
    pub client_id: String,
    pub client_secret: String,
    pub redirect_uri: String,
    pub scopes: Vec<String>,
    pub token_file: PathBuf,
}

impl Settings {
    /// Carga `.env` (si existe) y lee las variables `KICK_*` del entorno.
    pub fn from_env() -> Result<Self> {
        load_dotenv();
        Self::from_lookup(env_var)
    }

    pub fn from_lookup(lookup: impl Fn(&str) -> Option<String>) -> Result<Self> {
        let missing: Vec<&str> = ["KICK_CLIENT_ID", "KICK_CLIENT_SECRET"]
            .into_iter()
            .filter(|name| lookup(name).is_none())
            .collect();
        if !missing.is_empty() {
            bail!(
                "Faltan credenciales en .env o en el entorno: {} (ver .env.example)",
                missing.join(", ")
            );
        }
        let get = |name: &str, default: &str| lookup(name).unwrap_or_else(|| default.to_string());
        Ok(Self {
            client_id: get("KICK_CLIENT_ID", ""),
            client_secret: get("KICK_CLIENT_SECRET", ""),
            redirect_uri: get("KICK_REDIRECT_URI", DEFAULT_REDIRECT_URI),
            scopes: get("KICK_SCOPES", DEFAULT_SCOPES)
                .split([' ', ','])
                .filter(|scope| !scope.is_empty())
                .map(str::to_string)
                .collect(),
            token_file: PathBuf::from(get("KICK_TOKEN_FILE", DEFAULT_TOKEN_FILE)),
        })
    }
}

/// Ids manuales para saltarse la consulta a `kick.com/api/v2` si esta es bloqueada.
pub fn channel_override(slug: &str) -> Option<Channel> {
    load_dotenv();
    channel_override_from(slug, env_var)
}

fn channel_override_from(slug: &str, lookup: impl Fn(&str) -> Option<String>) -> Option<Channel> {
    Some(Channel {
        slug: slug.to_string(),
        id: lookup("KICK_CHANNEL_ID")?.parse().ok()?,
        chatroom_id: lookup("KICK_CHATROOM_ID")?.parse().ok()?,
    })
}

fn load_dotenv() {
    // Un .env ausente no es un error: las variables pueden venir del entorno.
    dotenvy::dotenv().ok();
}

fn env_var(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn lookup(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
        let map: HashMap<String, String> = pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        move |name| map.get(name).cloned()
    }

    #[test]
    fn aplica_valores_por_defecto() {
        let settings = Settings::from_lookup(lookup(&[
            ("KICK_CLIENT_ID", "id"),
            ("KICK_CLIENT_SECRET", "sec"),
        ]))
        .unwrap();
        assert_eq!(settings.redirect_uri, DEFAULT_REDIRECT_URI);
        assert_eq!(settings.scopes, ["user:read", "channel:read", "chat:write"]);
        assert_eq!(settings.token_file, PathBuf::from(".kick_tokens.json"));
    }

    #[test]
    fn respeta_variables_y_separa_scopes_por_espacio_o_coma() {
        let settings = Settings::from_lookup(lookup(&[
            ("KICK_CLIENT_ID", "id"),
            ("KICK_CLIENT_SECRET", "sec"),
            ("KICK_REDIRECT_URI", "http://localhost:9000/cb"),
            ("KICK_SCOPES", "user:read, chat:write"),
        ]))
        .unwrap();
        assert_eq!(settings.redirect_uri, "http://localhost:9000/cb");
        assert_eq!(settings.scopes, ["user:read", "chat:write"]);
    }

    #[test]
    fn el_error_lista_las_credenciales_que_faltan_sin_exponer_valores() {
        let error = Settings::from_lookup(lookup(&[("KICK_CLIENT_ID", "id")]))
            .unwrap_err()
            .to_string();
        assert!(error.contains("KICK_CLIENT_SECRET") && !error.contains("KICK_CLIENT_ID,"));
    }

    #[test]
    fn ids_manuales_requieren_ambos_y_numericos() {
        let ok = channel_override_from(
            "x",
            lookup(&[("KICK_CHANNEL_ID", "7"), ("KICK_CHATROOM_ID", "9")]),
        );
        assert_eq!(
            ok,
            Some(Channel {
                slug: "x".into(),
                id: 7,
                chatroom_id: 9
            })
        );
        assert!(channel_override_from("x", lookup(&[("KICK_CHANNEL_ID", "7")])).is_none());
        assert!(
            channel_override_from(
                "x",
                lookup(&[("KICK_CHANNEL_ID", "a"), ("KICK_CHATROOM_ID", "9")])
            )
            .is_none()
        );
    }
}
