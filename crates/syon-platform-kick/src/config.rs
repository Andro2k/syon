use std::fmt;

/// Dirección a la que Kick devuelve al usuario tras autorizar. Debe coincidir con la registrada en la
/// aplicación de Kick y apuntar a esta máquina.
pub const DEFAULT_REDIRECT_URI: &str = "http://localhost:8080/auth/callback";

/// Permisos que se piden en el primer inicio de sesión: leer el usuario y el canal y escribir en el
/// chat. Se piden todos juntos para no obligar a autorizar de nuevo cuando F3 envíe mensajes.
pub const DEFAULT_SCOPES: [&str; 3] = ["user:read", "channel:read", "chat:write"];

/// Datos de la aplicación de Kick con la que SYON inicia sesión.
///
/// Contiene el `client_secret`: `Debug` está escrito a mano y no lo imprime.
#[derive(Clone, PartialEq, Eq)]
pub struct KickAppConfig {
    /// Identificador de la aplicación de Kick.
    pub client_id: String,
    /// Secreto de la aplicación de Kick. Nunca va al registro ni al archivo de credenciales.
    pub client_secret: String,
    /// Dirección de callback local; debe usar `localhost` o `127.0.0.1`.
    pub redirect_uri: String,
    /// Permisos que se piden al usuario.
    pub scopes: Vec<String>,
}

impl KickAppConfig {
    /// Crea la configuración con la dirección de callback y los permisos por defecto.
    pub fn new(client_id: impl Into<String>, client_secret: impl Into<String>) -> Self {
        Self {
            client_id: client_id.into(),
            client_secret: client_secret.into(),
            redirect_uri: DEFAULT_REDIRECT_URI.to_owned(),
            scopes: DEFAULT_SCOPES.iter().map(|s| (*s).to_owned()).collect(),
        }
    }
}

impl fmt::Debug for KickAppConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("KickAppConfig")
            .field("client_id", &self.client_id)
            .field("client_secret", &"<oculto>")
            .field("redirect_uri", &self.redirect_uri)
            .field("scopes", &self.scopes)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn los_valores_por_defecto_piden_lectura_y_escritura_de_chat() {
        let config = KickAppConfig::new("cid", "secreto");
        assert_eq!(config.redirect_uri, DEFAULT_REDIRECT_URI);
        assert_eq!(config.scopes, ["user:read", "channel:read", "chat:write"]);
    }

    #[test]
    fn debug_no_imprime_el_secreto() {
        let texto = format!("{:?}", KickAppConfig::new("cid", "secreto-de-kick"));
        assert!(texto.contains("cid"));
        assert!(!texto.contains("secreto-de-kick"));
    }
}
