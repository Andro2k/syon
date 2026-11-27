use crate::{
    AppPaths, EventBus, LoggingConfig, LoggingError, PlatformStart, PlatformStartError,
    connectors::ConnectorRegistry,
    init_logging,
    platforms::kick_channel,
    settings_loading::{read_settings, recover},
};
use std::{error::Error, fmt, io, sync::Arc};
use syon_app::{ConnectionStatus, PlatformConnector, Settings};
use syon_domain::Platform;
use syon_platform_kick::KickConnector;
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
    /// 1. Lee `settings.toml` con [`TomlSettingsStore`], sin tocar el disco. Un problema con los
    ///    ajustes no impide arrancar: se usan los valores por defecto.
    /// 2. Instala el registro con el nivel de los ajustes y la variable `SYON_LOG`
    ///    ([`init_logging`]), con `app_version` en la marca de arranque, y crea el runtime
    ///    asíncrono (ADR-0011). Si falla alguno, `start` termina sin haber cambiado nada en el
    ///    disco por culpa de los ajustes.
    /// 3. Atiende el problema del paso 1: un archivo inválido se aparta como
    ///    `settings.toml.invalid` (nunca se guardan valores por defecto encima) y se avisa en el
    ///    registro. Es lo último que puede fallar, y después crea el [`EventBus`].
    ///
    /// Solo puede llamarse una vez por proceso, porque el registro es global.
    pub fn start(paths: AppPaths, app_version: &str) -> Result<Self, StartError> {
        let settings_file = paths.settings_file();
        let loaded = read_settings(&TomlSettingsStore::new(&settings_file));
        let logging = LoggingConfig::from_env(&loaded.settings.logging, app_version);
        init_logging(&paths, &logging).map_err(StartError::Logging)?;
        let runtime = Builder::new_multi_thread()
            .worker_threads(WORKER_THREADS)
            .thread_name("syon-worker")
            .enable_all()
            .build()
            .map_err(StartError::Runtime)?;
        if let Some(problem) = &loaded.problem {
            log::warn!("engine: {}", recover(problem, &settings_file));
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

    /// Construye, arranca y registra el conector de cada plataforma activada en los ajustes, y
    /// devuelve para cada una si arrancó o por qué no (diseño 003, sección 5). Las plataformas
    /// desactivadas no aparecen en el resultado.
    ///
    /// Por ahora solo Kick, que recibe eventos sin credenciales: un fallo de una plataforma se
    /// avisa en el registro y no impide arrancar las demás.
    pub fn start_platforms(&mut self) -> Vec<PlatformStart> {
        let mut results = Vec::new();
        if self.settings.kick.enabled {
            let outcome = self.start_kick();
            if let Err(error) = &outcome {
                log::warn!("engine: Kick no arrancó: {error}");
            }
            results.push(PlatformStart {
                platform: Platform::Kick,
                outcome,
            });
        }
        results
    }

    /// Pide detenerse a todos los conectores registrados. Puede llamarse más de una vez.
    pub fn stop_platforms(&self) {
        self.connectors.stop_all();
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

impl Engine {
    /// Arranca el conector de Kick con el canal de los ajustes y lo registra.
    fn start_kick(&mut self) -> Result<(), PlatformStartError> {
        if self.connectors.contains(Platform::Kick) {
            return Err(PlatformStartError::AlreadyRunning);
        }
        let channel = kick_channel(&self.settings.kick)?;
        let sink = Arc::new(self.bus.clone());
        let connector = KickConnector::start(&self.runtime.handle().clone(), channel, sink)
            .map_err(|error| PlatformStartError::Connect(error.to_string()))?;
        self.connectors.add(Box::new(connector));
        Ok(())
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
