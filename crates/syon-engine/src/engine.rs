use crate::{
    AppCredentials, AppPaths, EventBus, LoggingConfig, LoggingError, LoginError, LoginOutcome,
    PlatformCredentials, PlatformStart, PlatformStartError,
    connectors::ConnectorRegistry,
    init_logging,
    platforms::kick_channel,
    settings_loading::{read_settings, recover},
};
use std::{error::Error, fmt, io, sync::Arc};
use syon_app::{ConnectionStatus, PlatformConnector, Settings};
use syon_domain::Platform;
use syon_platform_kick as kick;
use syon_platform_twitch as twitch;
use syon_storage_credentials::TomlCredentialStore;
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
    /// desactivadas no aparecen en el resultado. Un fallo de una plataforma se avisa en el registro
    /// y no impide arrancar las demás.
    ///
    /// Kick recibe eventos sin credenciales. Twitch necesita la sesión de la persona antes de
    /// conectar: reutiliza la guardada o la renueva y, si no hay una válida, inicia sesión en el
    /// navegador. En ese caso `on_login_url` recibe el enlace y la llamada espera a que la persona
    /// autorice, así que debe hacerse desde fuera del runtime del engine. Sin las credenciales de la
    /// aplicación, Twitch no arranca y el resultado lo dice.
    pub fn start_platforms(
        &mut self,
        credentials: &PlatformCredentials,
        on_login_url: impl Fn(Platform, &str),
    ) -> Vec<PlatformStart> {
        let mut results = Vec::new();
        if self.settings.kick.enabled {
            let outcome = self.start_kick();
            results.push(report(Platform::Kick, outcome));
        }
        if self.settings.twitch.enabled {
            let outcome = self.start_twitch(credentials.twitch.as_ref(), &on_login_url);
            results.push(report(Platform::Twitch, outcome));
        }
        results
    }

    /// Pide detenerse a todos los conectores registrados. Puede llamarse más de una vez.
    pub fn stop_platforms(&self) {
        self.connectors.stop_all();
    }

    /// Deja lista la sesión de `platform` con las credenciales de la aplicación: reutiliza la
    /// guardada si sigue vigente, la renueva si va a vencer o inicia sesión en el navegador. Cada
    /// cambio se guarda en `credentials.toml`. `on_url` recibe el enlace de autorización solo si
    /// hace falta un inicio de sesión nuevo, y la llamada espera a que la persona autorice.
    ///
    /// Bloquea sobre el runtime del engine, así que debe llamarse desde fuera de él (por ejemplo,
    /// desde el hilo principal) y no desde una tarea asíncrona.
    pub fn login(
        &self,
        platform: Platform,
        credentials: &AppCredentials,
        on_url: impl FnOnce(&str),
    ) -> Result<LoginOutcome, LoginError> {
        let result = match platform {
            Platform::Kick => self.kick_login(credentials, on_url),
            Platform::Twitch => self
                .twitch_session(credentials, on_url)
                .map(|(_tokens, source)| LoginOutcome::from(source)),
        };
        match result {
            Ok(outcome) => {
                log::info!("engine: sesión de {platform:?} lista ({outcome:?})");
                Ok(outcome)
            }
            Err(detail) => {
                log::warn!("engine: no se pudo iniciar sesión en {platform:?}: {detail}");
                Err(LoginError::Failed(detail))
            }
        }
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
        let connector = kick::KickConnector::start(&self.runtime.handle().clone(), channel, sink)
            .map_err(|error| PlatformStartError::Connect(error.to_string()))?;
        self.connectors.add(Box::new(connector));
        Ok(())
    }

    /// Deja lista la sesión de Twitch, arranca su conector y lo registra.
    fn start_twitch(
        &mut self,
        app: Option<&AppCredentials>,
        on_login_url: &impl Fn(Platform, &str),
    ) -> Result<(), PlatformStartError> {
        if self.connectors.contains(Platform::Twitch) {
            return Err(PlatformStartError::AlreadyRunning);
        }
        let app = app.ok_or(PlatformStartError::MissingCredentials)?;
        let (tokens, source) = self
            .twitch_session(app, |url| on_login_url(Platform::Twitch, url))
            .map_err(PlatformStartError::Login)?;
        log::info!(
            "engine: sesión de Twitch lista ({:?})",
            LoginOutcome::from(source)
        );
        let sink = Arc::new(self.bus.clone());
        let connector =
            twitch::TwitchConnector::start(&self.runtime.handle().clone(), tokens, sink)
                .map_err(|error| PlatformStartError::Connect(error.to_string()))?;
        self.connectors.add(Box::new(connector));
        Ok(())
    }

    /// Sesión de Kick lista; los errores se devuelven como texto sin tokens ni secretos.
    fn kick_login(
        &self,
        app: &AppCredentials,
        on_url: impl FnOnce(&str),
    ) -> Result<LoginOutcome, String> {
        let config = kick::KickAppConfig::new(&app.client_id, &app.client_secret);
        let store = Arc::new(TomlCredentialStore::new(self.paths.credentials_file()));
        self.runtime
            .block_on(async {
                let oauth = kick::OAuthClient::new(config)?;
                kick::TokenManager::start(oauth, store, on_url).await
            })
            .map(|(_session, source)| LoginOutcome::from(source))
            .map_err(|error| error.to_string())
    }

    /// Sesión de Twitch lista, con su gestor de tokens, que el conector comparte.
    fn twitch_session(
        &self,
        app: &AppCredentials,
        on_url: impl FnOnce(&str),
    ) -> Result<(Arc<twitch::TokenManager>, twitch::Source), String> {
        let config = twitch::TwitchAppConfig::new(&app.client_id, &app.client_secret);
        let store = Arc::new(TomlCredentialStore::new(self.paths.credentials_file()));
        self.runtime
            .block_on(async {
                let oauth = twitch::OAuthClient::new(config)?;
                twitch::TokenManager::start(oauth, store, on_url).await
            })
            .map(|(tokens, source)| (Arc::new(tokens), source))
            .map_err(|error| error.to_string())
    }
}

/// Anota en el registro el fallo de una plataforma y lo empaqueta para quien llama.
fn report(platform: Platform, outcome: Result<(), PlatformStartError>) -> PlatformStart {
    if let Err(error) = &outcome {
        log::warn!("engine: {platform:?} no arrancó: {error}");
    }
    PlatformStart { platform, outcome }
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
