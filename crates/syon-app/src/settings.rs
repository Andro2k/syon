/// Versión actual del esquema de `settings.toml`.
pub const SCHEMA_VERSION: u32 = 1;

/// Ajustes persistentes del usuario. Modelo plano, sin formato de archivo: cada
/// adaptador de [`SettingsStore`](crate::SettingsStore) traduce a su propio formato.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Settings {
    /// Versión del esquema con el que se escribieron los ajustes.
    pub schema_version: u32,
    /// Ajustes del registro de diagnóstico.
    pub logging: LoggingSettings,
    /// Ajustes de la conexión a Kick.
    pub kick: KickSettings,
    /// Ajustes de la conexión a Twitch.
    pub twitch: TwitchSettings,
    /// Ajustes de comandos, filtros y antispam del chat.
    pub chat: ChatSettings,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            logging: LoggingSettings::default(),
            kick: KickSettings::default(),
            twitch: TwitchSettings::default(),
            chat: ChatSettings::default(),
        }
    }
}

/// Ajustes del registro de diagnóstico.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LoggingSettings {
    /// Nivel mínimo que se escribe en el archivo de log.
    pub level: LogLevel,
}

/// Ajustes de la conexión a Kick. Por defecto la plataforma está desactivada y sin canal.
/// Aquí no van el `client_id` ni el `client_secret` de la aplicación: no son ajustes de la
/// persona (ver ADR-0018).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct KickSettings {
    /// Si el engine debe conectar con Kick al arrancar las plataformas.
    pub enabled: bool,
    /// Nombre (slug) del canal que se escucha. Vacío significa que no hay canal elegido.
    pub channel: String,
    /// Id numérico del canal. Junto con `chatroom_id` evita consultar a Kick, que a veces
    /// responde 403.
    pub channel_id: Option<u64>,
    /// Id numérico de la sala de chat. Solo se usa si `channel_id` también está definido.
    pub chatroom_id: Option<u64>,
}

/// Ajustes de la conexión a Twitch. Por defecto la plataforma está desactivada.
///
/// No hay canal: SYON escucha el del usuario que inicia sesión. Tampoco van aquí el `client_id` ni
/// el `client_secret` de la aplicación (ver ADR-0018).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TwitchSettings {
    /// Si el engine debe conectar con Twitch al arrancar las plataformas.
    pub enabled: bool,
}

/// Configuración de automatización del chat. Sin comandos configurados permanece inerte.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatSettings {
    /// Prefijo que inicia un comando.
    pub prefix: String,
    /// Comandos disponibles.
    pub commands: Vec<ChatCommand>,
    /// Usuarios que no pueden activar comandos, por id o nombre.
    pub ignored_users: Vec<ChatUserFilter>,
    /// Frases que deben aparecer para permitir comandos.
    pub required_phrases: Vec<String>,
    /// Frases que impiden ejecutar comandos.
    pub blocked_phrases: Vec<String>,
    /// Ventana en segundos para descartar mensajes repetidos.
    pub duplicate_window_seconds: u64,
    /// Enfriamiento en segundos por usuario y comando.
    pub command_cooldown_seconds: u64,
    /// Enfriamiento entre respuestas idénticas por plataforma.
    pub response_cooldown_seconds: u64,
    /// Intervalo mínimo en segundos entre respuestas por plataforma.
    pub platform_interval_seconds: u64,
}

impl Default for ChatSettings {
    fn default() -> Self {
        Self {
            prefix: "!".into(),
            commands: Vec::new(),
            ignored_users: Vec::new(),
            required_phrases: Vec::new(),
            blocked_phrases: Vec::new(),
            duplicate_window_seconds: 10,
            command_cooldown_seconds: 5,
            response_cooldown_seconds: 5,
            platform_interval_seconds: 1,
        }
    }
}

/// Comando de texto y sus restricciones de uso.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatCommand {
    /// Nombre principal del comando.
    pub name: String,
    /// Nombres alternativos.
    pub aliases: Vec<String>,
    /// Texto de respuesta con sustituciones literales admitidas.
    pub response: String,
    /// Si el comando está habilitado.
    pub enabled: bool,
    /// Plataformas de origen que pueden activar el comando; vacío permite todas.
    pub platforms: Vec<syon_domain::Platform>,
    /// Plataformas destino; vacío envía a la plataforma de origen.
    pub destinations: Vec<syon_domain::Platform>,
    /// Nivel mínimo para usar el comando.
    pub minimum_role: MinimumChatRole,
}

/// Usuario excluido por id estable o por nombre visible normalizado.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatUserFilter {
    /// Identificador de plataforma opcional.
    pub author_id: Option<String>,
    /// Nombre visible opcional.
    pub name: Option<String>,
}

/// Nivel mínimo requerido para activar un comando.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MinimumChatRole {
    /// Cualquier autor identificable.
    #[default]
    Everyone,
    /// Suscriptor o superior.
    Subscriber,
    /// VIP o superior.
    Vip,
    /// Moderador o superior.
    Moderator,
    /// Solo broadcaster.
    Broadcaster,
}

/// Nivel de detalle del registro. `Info` es el valor por defecto porque el texto
/// completo del chat solo se registra en `Debug`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LogLevel {
    /// Solo errores.
    Error,
    /// Errores y avisos.
    Warn,
    /// Funcionamiento normal.
    #[default]
    Info,
    /// Detalle para diagnosticar problemas.
    Debug,
    /// Máximo detalle.
    Trace,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn los_ajustes_por_defecto_usan_el_esquema_actual_y_nivel_info() {
        let settings = Settings::default();
        assert_eq!(settings.schema_version, SCHEMA_VERSION);
        assert_eq!(settings.logging.level, LogLevel::Info);
    }

    #[test]
    fn twitch_esta_desactivado_por_defecto() {
        assert!(!Settings::default().twitch.enabled);
    }

    #[test]
    fn kick_esta_desactivado_y_sin_canal_por_defecto() {
        let kick = Settings::default().kick;
        assert!(!kick.enabled);
        assert!(kick.channel.is_empty());
        assert_eq!(kick.channel_id, None);
        assert_eq!(kick.chatroom_id, None);
    }

    #[test]
    fn chat_por_defecto_no_ejecuta_comandos_y_aplica_limites_conservadores() {
        let chat = ChatSettings::default();
        assert_eq!(chat.prefix, "!");
        assert!(chat.commands.is_empty());
        assert_eq!(chat.command_cooldown_seconds, 5);
        assert_eq!(chat.response_cooldown_seconds, 5);
        assert_eq!(chat.platform_interval_seconds, 1);
        assert_eq!(chat.duplicate_window_seconds, 10);
    }

    #[test]
    fn rol_minimo_tiene_niveles_distintos_y_everyone_por_defecto() {
        assert_eq!(MinimumChatRole::default(), MinimumChatRole::Everyone);
        let roles = [
            MinimumChatRole::Everyone,
            MinimumChatRole::Subscriber,
            MinimumChatRole::Vip,
            MinimumChatRole::Moderator,
            MinimumChatRole::Broadcaster,
        ];
        for (index, role) in roles.iter().enumerate() {
            assert_eq!(roles[index], *role);
        }
    }
}
