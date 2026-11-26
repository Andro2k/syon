use serde::{Deserialize, Serialize};
use syon_app::Credentials;
use syon_domain::{Platform, Timestamp};

/// Versión del formato de `credentials.toml`.
const SCHEMA_VERSION: u32 = 1;

/// Contenido de `credentials.toml`: una sección por plataforma. Es propio del adaptador y no
/// implementa `Debug` a propósito, porque contiene tokens.
#[derive(Serialize, Deserialize, Default)]
pub(crate) struct CredentialsFile {
    #[serde(default)]
    schema_version: u32,
    kick: Option<Entry>,
    twitch: Option<Entry>,
}

/// Sección de una plataforma.
#[derive(Serialize, Deserialize)]
struct Entry {
    access_token: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    refresh_token: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    expires_at_ms: Option<i64>,
    #[serde(default)]
    scopes: Vec<String>,
}

impl CredentialsFile {
    /// Credenciales de `platform`, si hay una sección para ella.
    pub(crate) fn get(&self, platform: Platform) -> Option<Credentials> {
        let entry = match platform {
            Platform::Kick => self.kick.as_ref(),
            Platform::Twitch => self.twitch.as_ref(),
        }?;
        Some(Credentials {
            access_token: entry.access_token.clone(),
            refresh_token: entry.refresh_token.clone(),
            expires_at: entry.expires_at_ms.map(Timestamp::from_unix_ms),
            scopes: entry.scopes.clone(),
        })
    }

    /// Sustituye la sección de `platform`; `None` la borra. No toca las demás.
    pub(crate) fn set(&mut self, platform: Platform, credentials: Option<&Credentials>) {
        let entry = credentials.map(|c| Entry {
            access_token: c.access_token.clone(),
            refresh_token: c.refresh_token.clone(),
            expires_at_ms: c.expires_at.map(Timestamp::as_unix_ms),
            scopes: c.scopes.clone(),
        });
        match platform {
            Platform::Kick => self.kick = entry,
            Platform::Twitch => self.twitch = entry,
        }
    }
}

/// Lee el texto del archivo. El error describe qué falló y en qué línea, pero nunca copia el
/// contenido: el mensaje completo de `toml` incluye un fragmento del texto, que puede ser un token.
pub(crate) fn parse(text: &str) -> Result<CredentialsFile, String> {
    toml::from_str(text).map_err(|error| match error.span() {
        Some(span) => {
            let line = text[..span.start.min(text.len())].matches('\n').count() + 1;
            format!("{} (línea {line})", error.message())
        }
        None => error.message().to_owned(),
    })
}

/// Convierte el contenido al texto TOML que se escribe en disco.
pub(crate) fn render(file: &mut CredentialsFile) -> Result<String, String> {
    file.schema_version = SCHEMA_VERSION;
    toml::to_string_pretty(file).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn credentials() -> Credentials {
        Credentials {
            access_token: "acceso".to_owned(),
            refresh_token: Some("renovacion".to_owned()),
            expires_at: Some(Timestamp::from_unix_ms(5_000)),
            scopes: vec!["chat:write".to_owned(), "events:subscribe".to_owned()],
        }
    }

    #[test]
    fn ida_y_vuelta_conserva_todos_los_campos() {
        let mut file = CredentialsFile::default();
        file.set(Platform::Kick, Some(&credentials()));
        let text = render(&mut file).unwrap();
        let loaded = parse(&text).unwrap();
        assert_eq!(loaded.get(Platform::Kick), Some(credentials()));
        assert_eq!(loaded.get(Platform::Twitch), None);
    }

    #[test]
    fn el_archivo_lleva_la_version_y_una_seccion_por_plataforma() {
        let mut file = CredentialsFile::default();
        file.set(Platform::Twitch, Some(&credentials()));
        let text = render(&mut file).unwrap();
        assert!(text.contains("schema_version = 1"));
        assert!(text.contains("[twitch]"));
        assert!(!text.contains("[kick]"));
    }

    #[test]
    fn solo_el_token_de_acceso_es_obligatorio() {
        let loaded = parse("[kick]\naccess_token = \"a\"\n").unwrap();
        let kick = loaded.get(Platform::Kick).unwrap();
        assert_eq!(kick.refresh_token, None);
        assert_eq!(kick.expires_at, None);
        assert!(kick.scopes.is_empty());
    }

    #[test]
    fn las_claves_desconocidas_se_ignoran() {
        let text = "futura = 1\n[kick]\naccess_token = \"a\"\notra = true\n";
        assert!(parse(text).unwrap().get(Platform::Kick).is_some());
    }

    #[test]
    fn el_error_de_lectura_indica_la_linea_y_no_copia_el_contenido() {
        let text = "[kick]\naccess_token = \"token-secreto-123\"\nscopes = 7\n";
        let error = parse(text).err().unwrap();
        assert!(error.contains("línea 3"), "{error}");
        assert!(!error.contains("token-secreto-123"), "{error}");
    }

    #[test]
    fn set_none_borra_solo_esa_plataforma() {
        let mut file = CredentialsFile::default();
        file.set(Platform::Kick, Some(&credentials()));
        file.set(Platform::Twitch, Some(&credentials()));
        file.set(Platform::Kick, None);
        assert_eq!(file.get(Platform::Kick), None);
        assert!(file.get(Platform::Twitch).is_some());
    }
}
