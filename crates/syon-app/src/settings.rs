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
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            logging: LoggingSettings::default(),
            kick: KickSettings::default(),
            twitch: TwitchSettings::default(),
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
}
