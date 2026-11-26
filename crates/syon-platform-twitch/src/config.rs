use std::fmt;

/// Dirección a la que Twitch devuelve al usuario tras autorizar. Debe coincidir con una de las
/// registradas en la aplicación de Twitch y apuntar a esta máquina. Usa un puerto distinto del de
/// Kick (8080) para que los dos inicios de sesión no se estorben.
pub const DEFAULT_REDIRECT_URI: &str = "http://localhost:8081/auth/callback";

/// Permisos que se piden en el primer inicio de sesión: los de lectura de las ocho suscripciones de
/// EventSub y el de escritura de chat. Se piden todos juntos para no obligar a autorizar de nuevo
/// cuando F3 envíe mensajes. `stream.online` y `stream.offline` no exigen ninguno.
pub const DEFAULT_SCOPES: [&str; 5] = [
    // channel.chat.message, channel.chat.message_delete y channel.chat.notification.
    "user:read:chat",
    // channel.ban.
    "channel:moderate",
    // channel.channel_points_custom_reward_redemption.add.
    "channel:read:redemptions",
    // channel.follow (v2).
    "moderator:read:followers",
    // Enviar mensajes al chat (F3).
    "user:write:chat",
];

/// Datos de la aplicación de Twitch con la que SYON inicia sesión.
///
/// Contiene el `client_secret`: `Debug` está escrito a mano y no lo imprime.
#[derive(Clone, PartialEq, Eq)]
pub struct TwitchAppConfig {
    /// Identificador de la aplicación de Twitch.
    pub client_id: String,
    /// Secreto de la aplicación de Twitch. Nunca va al registro ni al archivo de credenciales.
    pub client_secret: String,
    /// Dirección de callback local; debe usar `localhost` o `127.0.0.1`.
    pub redirect_uri: String,
    /// Permisos que se piden al usuario.
    pub scopes: Vec<String>,
}

impl TwitchAppConfig {
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

impl fmt::Debug for TwitchAppConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TwitchAppConfig")
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
    fn los_valores_por_defecto_piden_lectura_de_eventos_y_escritura_de_chat() {
        let config = TwitchAppConfig::new("cid", "secreto");
        assert_eq!(config.redirect_uri, DEFAULT_REDIRECT_URI);
        assert_eq!(
            config.scopes,
            [
                "user:read:chat",
                "channel:moderate",
                "channel:read:redemptions",
                "moderator:read:followers",
                "user:write:chat",
            ]
        );
    }

    #[test]
    fn el_callback_por_defecto_no_choca_con_el_de_kick() {
        assert!(DEFAULT_REDIRECT_URI.contains(":8081/"));
    }

    #[test]
    fn debug_no_imprime_el_secreto() {
        let texto = format!("{:?}", TwitchAppConfig::new("cid", "secreto-de-twitch"));
        assert!(texto.contains("cid"));
        assert!(!texto.contains("secreto-de-twitch"));
    }
}
