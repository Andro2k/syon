use crate::{
    AppPaths, EventBus, LoggingConfig, LoggingError, init_logging, settings_loading::load_settings,
};
use std::{error::Error, fmt};
use syon_app::Settings;
use syon_storage_toml::TomlSettingsStore;

/// SYON en marcha, sin interfaz: rutas, ajustes del usuario y bus de eventos, con el registro de
/// diagnóstico ya instalado. Es lo que un binario crea al arrancar (ADR-0010).
pub struct Engine {
    paths: AppPaths,
    settings: Settings,
    bus: EventBus,
}

impl Engine {
    /// Arranca el engine, en este orden:
    ///
    /// 1. Lee `settings.toml` con [`TomlSettingsStore`]. Un archivo inválido se aparta como
    ///    `settings.toml.invalid` y se usan los valores por defecto; nunca se guardan encima. Un
    ///    problema con los ajustes no impide arrancar: queda como aviso en el registro.
    /// 2. Instala el registro con el nivel de los ajustes y la variable `SYON_LOG`
    ///    ([`init_logging`]), con `app_version` en la marca de arranque.
    /// 3. Escribe los avisos del paso 1 y crea el [`EventBus`].
    ///
    /// Solo puede llamarse una vez por proceso, porque el registro es global.
    pub fn start(paths: AppPaths, app_version: &str) -> Result<Self, StartError> {
        let settings_file = paths.settings_file();
        let loaded = load_settings(&TomlSettingsStore::new(&settings_file), &settings_file);
        let logging = LoggingConfig::from_env(&loaded.settings.logging, app_version);
        init_logging(&paths, &logging).map_err(StartError::Logging)?;
        if let Some(warning) = &loaded.warning {
            log::warn!("engine: {warning}");
        }
        log::info!("engine: listo; ajustes en {}", settings_file.display());
        Ok(Self {
            paths,
            settings: loaded.settings,
            bus: EventBus::new(),
        })
    }

    /// Carpetas y archivos de SYON.
    pub fn paths(&self) -> &AppPaths {
        &self.paths
    }

    /// Ajustes con los que arrancó el engine.
    pub fn settings(&self) -> &Settings {
        &self.settings
    }

    /// Bus de eventos; es barato de clonar y las copias comparten consumidores.
    pub fn bus(&self) -> &EventBus {
        &self.bus
    }
}

/// No se pudo arrancar el engine.
#[derive(Debug)]
pub enum StartError {
    /// No se pudo instalar el registro de diagnóstico.
    Logging(LoggingError),
}

impl fmt::Display for StartError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Logging(error) => write!(f, "no se pudo iniciar el registro: {error}"),
        }
    }
}

impl Error for StartError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Logging(error) => Some(error),
        }
    }
}
