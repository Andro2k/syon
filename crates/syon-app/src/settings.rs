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
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            logging: LoggingSettings::default(),
        }
    }
}

/// Ajustes del registro de diagnóstico.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LoggingSettings {
    /// Nivel mínimo que se escribe en el archivo de log.
    pub level: LogLevel,
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
}
