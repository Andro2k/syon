use crate::{
    AppPaths, EventBus, LoggingConfig, LoggingError, connectors::ConnectorRegistry, init_logging,
    settings_loading::load_settings,
};
use std::{error::Error, fmt, io};
use syon_app::{ConnectionStatus, PlatformConnector, Settings};
use syon_domain::Platform;
use syon_storage_toml::TomlSettingsStore;
use tokio::runtime::{Builder, Handle, Runtime};

/// Hilos del runtime: la red de dos plataformas no necesita más y SYON es una aplicación de escritorio.
const WORKER_THREADS: usize = 2;

/// SYON en marcha, sin interfaz: rutas, ajustes del usuario, bus de eventos, runtime asíncrono y
/// conectores de plataforma, con el registro de diagnóstico ya instalado. Es lo que un binario crea
/// al arrancar (ADR-0010 y ADR-0011).
pub struct Engine {
    paths: AppPaths,
    settings: Settings,
    bus: EventBus,
    connectors: ConnectorRegistry,
    // Último campo: se destruye después de detener los conectores en `Drop`.
    runtime: Runtime,
}

impl Engine {
    /// Arranca el engine, en este orden:
    ///
    /// 1. Lee `settings.toml` con [`TomlSettingsStore`]. Un archivo inválido se aparta como
    ///    `settings.toml.invalid` y se usan los valores por defecto; nunca se guardan encima. Un
    ///    problema con los ajustes no impide arrancar: queda como aviso en el registro.
    /// 2. Instala el registro con el nivel de los ajustes y la variable `SYON_LOG`
    ///    ([`init_logging`]), con `app_version` en la marca de arranque.
    /// 3. Crea el runtime asíncrono (ADR-0011), escribe los avisos del paso 1 y crea el [`EventBus`].
    ///
    /// Solo puede llamarse una vez por proceso, porque el registro es global.
    pub fn start(paths: AppPaths, app_version: &str) -> Result<Self, StartError> {
        let settings_file = paths.settings_file();
        let loaded = load_settings(&TomlSettingsStore::new(&settings_file), &settings_file);
        let logging = LoggingConfig::from_env(&loaded.settings.logging, app_version);
        init_logging(&paths, &logging).map_err(StartError::Logging)?;
        let runtime = Builder::new_multi_thread()
            .worker_threads(WORKER_THREADS)
            .thread_name("syon-worker")
            .enable_all()
            .build()
            .map_err(StartError::Runtime)?;
        if let Some(warning) = &loaded.warning {
            log::warn!("engine: {warning}");
        }
        log::info!("engine: listo; ajustes en {}", settings_file.display());
        Ok(Self {
            paths,
            settings: loaded.settings,
            bus: EventBus::new(),
            connectors: ConnectorRegistry::default(),
            runtime,
        })
    }

    /// Entrada al runtime asíncrono. Los adaptadores lanzan en él sus tareas; no crean runtimes
    /// propios (ADR-0011).
    pub fn runtime_handle(&self) -> Handle {
        self.runtime.handle().clone()
    }

    /// Registra un conector ya arrancado para vigilar su estado y detenerlo al cerrar el engine.
    pub fn add_connector(&mut self, connector: Box<dyn PlatformConnector>) {
        self.connectors.add(connector);
    }

    /// Estado de la conexión de cada plataforma registrada.
    pub fn connection_status(&self) -> Vec<(Platform, ConnectionStatus)> {
        self.connectors.statuses()
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

impl Drop for Engine {
    /// Pide detenerse a los conectores antes de que el runtime se destruya.
    fn drop(&mut self) {
        self.connectors.stop_all();
    }
}

/// No se pudo arrancar el engine.
#[derive(Debug)]
pub enum StartError {
    /// No se pudo instalar el registro de diagnóstico.
    Logging(LoggingError),
    /// No se pudo crear el runtime asíncrono.
    Runtime(io::Error),
}

impl fmt::Display for StartError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Logging(error) => write!(f, "no se pudo iniciar el registro: {error}"),
            Self::Runtime(error) => write!(f, "no se pudo crear el runtime asíncrono: {error}"),
        }
    }
}

impl Error for StartError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Logging(error) => Some(error),
            Self::Runtime(error) => Some(error),
        }
    }
}
