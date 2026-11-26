use crate::{
    log_file::{MARKER_TARGET, RotatingFile},
    panic_hook,
    paths::AppPaths,
    redact::Redactor,
};
use env_logger::{Builder, Target, fmt::Formatter};
use log::{Level, LevelFilter, Log, Metadata, Record};
use std::{
    env,
    error::Error,
    fmt,
    io::{self, Write},
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
use syon_app::{LogLevel, LoggingSettings};

/// Marca que un `init` ya empezó, para que uno repetido falle sin tocar el archivo de registro.
static INIT_STARTED: AtomicBool = AtomicBool::new(false);
/// Variable de entorno que reemplaza el filtro del archivo (`SYON_LOG=debug,bus=trace`).
const FILTER_ENV: &str = "SYON_LOG";
/// Tamaño a partir del cual `syon.log` pasa a `syon.log.1` (valor inicial, ADR-0006).
const MAX_LOG_BYTES: u64 = 16 * 1024 * 1024;
/// Ejecuciones que se conservan, contando la que arranca (valor inicial, ADR-0006).
const KEPT_RUNS: usize = 5;
/// Nivel mínimo de la consola.
const CONSOLE_LEVEL: LevelFilter = LevelFilter::Warn;
/// Sangría de las líneas de un mensaje de varias líneas: así solo la primera de cada registro
/// empieza en la columna cero y un texto de chat no puede imitar otro registro.
const CONTINUATION_INDENT: &str = "    ";

/// Cómo iniciar el registro de diagnóstico.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoggingConfig {
    /// Nivel mínimo del archivo, de `[logging].level` en los ajustes.
    pub level: LogLevel,
    /// Filtro del archivo con la sintaxis de `env_logger`; si existe, reemplaza a `level`.
    pub filter_override: Option<String>,
    /// Versión de SYON que se anota en la línea de marca de cada arranque.
    pub app_version: String,
}

impl LoggingConfig {
    /// Toma el nivel de los ajustes y el filtro de la variable `SYON_LOG`, si no está vacía.
    pub fn from_env(settings: &LoggingSettings, app_version: impl Into<String>) -> Self {
        Self {
            level: settings.level,
            filter_override: non_blank(env::var(FILTER_ENV).ok()),
            app_version: app_version.into(),
        }
    }
}

/// No se pudo iniciar el registro.
#[derive(Debug)]
pub enum LoggingError {
    /// No se pudo abrir el archivo de registro.
    File {
        /// Archivo que se intentó abrir.
        path: PathBuf,
        /// Causa del sistema.
        source: io::Error,
    },
    /// El proceso ya tenía un registro instalado.
    AlreadyInitialized,
}

impl fmt::Display for LoggingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::File { path, source } => {
                write!(
                    f,
                    "no se pudo abrir el registro {}: {source}",
                    path.display()
                )
            }
            Self::AlreadyInitialized => write!(f, "el registro ya estaba iniciado"),
        }
    }
}

impl Error for LoggingError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::File { source, .. } => Some(source),
            Self::AlreadyInitialized => None,
        }
    }
}

/// Instala el registro del proceso: la consola muestra `warn` y el archivo `syon.log` (en la
/// carpeta de datos de `paths`) el nivel de `config`. Toda línea pasa por el redactor de secretos.
/// Escribe la línea de marca del arranque e instala el hook de pánico.
///
/// Solo puede llamarse una vez por proceso; la segunda devuelve
/// [`LoggingError::AlreadyInitialized`] sin tocar el archivo. Si falla por otro motivo se puede
/// reintentar. Los demás crates registran con las macros de `log`.
pub fn init(paths: &AppPaths, config: &LoggingConfig) -> Result<(), LoggingError> {
    if INIT_STARTED.swap(true, Ordering::SeqCst) {
        return Err(LoggingError::AlreadyInitialized);
    }
    install(paths, config).inspect_err(|_| INIT_STARTED.store(false, Ordering::SeqCst))
}

fn install(paths: &AppPaths, config: &LoggingConfig) -> Result<(), LoggingError> {
    let log_file = paths.log_file();
    let file = RotatingFile::open(&log_file, MAX_LOG_BYTES, KEPT_RUNS - 1).map_err(|source| {
        LoggingError::File {
            path: log_file,
            source,
        }
    })?;
    let redactor = Arc::new(Redactor::new(paths.home_dir()));
    let logger = DualLogger::new(Box::new(file), Target::Stderr, config, &redactor);
    let max_level = logger.max_level();
    log::set_boxed_logger(Box::new(logger)).map_err(|_| LoggingError::AlreadyInitialized)?;
    log::set_max_level(max_level);
    log::info!(target: MARKER_TARGET, "logging: syon {} iniciado", config.app_version);
    panic_hook::install();
    Ok(())
}

/// Reparte cada registro a la consola y al archivo, cada uno con su propio filtro.
struct DualLogger {
    console: env_logger::Logger,
    file: env_logger::Logger,
}

impl DualLogger {
    fn new(
        file: Box<dyn Write + Send>,
        console: Target,
        config: &LoggingConfig,
        redactor: &Arc<Redactor>,
    ) -> Self {
        let mut console_builder = Builder::new();
        console_builder
            .filter_level(CONSOLE_LEVEL)
            .format(redacted_format(Arc::clone(redactor)))
            .target(console);
        let mut file_builder = Builder::new();
        match &config.filter_override {
            Some(spec) => file_builder.parse_filters(spec),
            None => file_builder.filter_level(level_filter(config.level)),
        };
        // La marca de cada arranque se escribe siempre, aunque el filtro sea más estricto.
        file_builder
            .filter_module(MARKER_TARGET, LevelFilter::Info)
            .format(redacted_format(Arc::clone(redactor)))
            .target(Target::Pipe(file));
        Self {
            console: console_builder.build(),
            file: file_builder.build(),
        }
    }

    /// Nivel más detallado que acepta alguno de los dos destinos.
    fn max_level(&self) -> LevelFilter {
        self.console.filter().max(self.file.filter())
    }
}

impl Log for DualLogger {
    fn enabled(&self, metadata: &Metadata<'_>) -> bool {
        self.console.enabled(metadata) || self.file.enabled(metadata)
    }

    fn log(&self, record: &Record<'_>) {
        self.console.log(record);
        self.file.log(record);
    }

    fn flush(&self) {
        self.console.flush();
        self.file.flush();
    }
}

/// Traduce el nivel de los ajustes al filtro de `log`. Vive aquí, y no en `syon-app`, porque solo
/// el engine conoce `log`; `syon-storage-toml` mantiene aparte los nombres del archivo TOML.
fn level_filter(level: LogLevel) -> LevelFilter {
    match level {
        LogLevel::Error => LevelFilter::Error,
        LogLevel::Warn => LevelFilter::Warn,
        LogLevel::Info => LevelFilter::Info,
        LogLevel::Debug => LevelFilter::Debug,
        LogLevel::Trace => LevelFilter::Trace,
    }
}

/// Formato común de la consola y del archivo: arma la línea y la pasa por el redactor.
fn redacted_format(
    redactor: Arc<Redactor>,
) -> impl Fn(&mut Formatter, &Record<'_>) -> io::Result<()> + Send + Sync + 'static {
    move |buf, record| {
        let line = format_record(
            &buf.timestamp_millis().to_string(),
            record.level(),
            record.target(),
            &record.args().to_string(),
        );
        writeln!(buf, "{}", redactor.redact(&line))
    }
}

/// `<hora> <NIVEL> [<destino>] <mensaje>`, con sangría en las líneas siguientes del mensaje.
fn format_record(timestamp: &str, level: Level, target: &str, message: &str) -> String {
    let mut lines = message.lines();
    let mut text = format!(
        "{timestamp} {level:<5} [{target}] {}",
        lines.next().unwrap_or_default()
    );
    for line in lines {
        text.push('\n');
        text.push_str(CONTINUATION_INDENT);
        text.push_str(line);
    }
    text
}

fn non_blank(value: Option<String>) -> Option<String> {
    value.filter(|value| !value.trim().is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::log_file::is_run_marker;
    use std::{path::Path, sync::Mutex};

    /// Destino de prueba que guarda lo escrito en memoria.
    #[derive(Clone, Default)]
    struct SharedBuf(Arc<Mutex<Vec<u8>>>);

    impl SharedBuf {
        fn text(&self) -> String {
            String::from_utf8(self.0.lock().unwrap().clone()).unwrap()
        }
    }

    impl Write for SharedBuf {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    struct Fixture {
        logger: DualLogger,
        file: SharedBuf,
        console: SharedBuf,
    }

    fn fixture(level: LogLevel, filter_override: Option<&str>, home: Option<&str>) -> Fixture {
        let (file, console) = (SharedBuf::default(), SharedBuf::default());
        let config = LoggingConfig {
            level,
            filter_override: filter_override.map(str::to_owned),
            app_version: "0.1.0".to_owned(),
        };
        let redactor = Arc::new(Redactor::new(home.map(Path::new)));
        let logger = DualLogger::new(
            Box::new(file.clone()),
            Target::Pipe(Box::new(console.clone())),
            &config,
            &redactor,
        );
        Fixture {
            logger,
            file,
            console,
        }
    }

    fn emit(logger: &DualLogger, level: Level, target: &str, message: &str) {
        logger.log(
            &Record::builder()
                .level(level)
                .target(target)
                .args(format_args!("{message}"))
                .build(),
        );
    }

    #[test]
    fn el_nivel_de_los_ajustes_se_traduce_al_filtro_de_log() {
        let pairs = [
            (LogLevel::Error, LevelFilter::Error),
            (LogLevel::Warn, LevelFilter::Warn),
            (LogLevel::Info, LevelFilter::Info),
            (LogLevel::Debug, LevelFilter::Debug),
            (LogLevel::Trace, LevelFilter::Trace),
        ];
        for (level, filter) in pairs {
            assert_eq!(level_filter(level), filter);
        }
    }

    #[test]
    fn el_formato_alinea_el_nivel_y_sangra_las_lineas_siguientes() {
        assert_eq!(
            format_record("T", Level::Warn, "x", "uno\ndos\ntres"),
            "T WARN  [x] uno\n    dos\n    tres"
        );
        assert_eq!(format_record("T", Level::Error, "x", ""), "T ERROR [x] ");
    }

    #[test]
    fn el_archivo_usa_el_nivel_de_los_ajustes_y_la_consola_solo_warn() {
        let f = fixture(LogLevel::Info, None, None);
        emit(&f.logger, Level::Debug, "x", "bus: detalle");
        emit(&f.logger, Level::Info, "x", "bus: normal");
        emit(&f.logger, Level::Warn, "x", "bus: aviso");
        let (file, console) = (f.file.text(), f.console.text());
        assert!(!file.contains("detalle") && file.contains("normal") && file.contains("aviso"));
        assert!(!console.contains("normal") && console.contains("aviso"));
    }

    #[test]
    fn syon_log_reemplaza_el_filtro_del_archivo_pero_no_el_de_la_consola() {
        let f = fixture(LogLevel::Error, Some("debug"), None);
        emit(&f.logger, Level::Debug, "x", "bus: detalle");
        assert!(f.file.text().contains("detalle"));
        assert_eq!(f.console.text(), "");
    }

    #[test]
    fn syon_log_admite_filtros_por_modulo() {
        let f = fixture(LogLevel::Info, Some("bus=debug"), None);
        emit(&f.logger, Level::Debug, "bus", "bus: del bus");
        emit(&f.logger, Level::Info, "otro", "otro: ajeno");
        let file = f.file.text();
        assert!(file.contains("del bus") && !file.contains("ajeno"));
    }

    #[test]
    fn la_marca_de_arranque_se_escribe_aunque_el_filtro_sea_mas_estricto() {
        for f in [
            fixture(LogLevel::Error, None, None),
            fixture(LogLevel::Error, Some("error"), None),
            fixture(LogLevel::Info, Some("logging=off"), None),
        ] {
            emit(
                &f.logger,
                Level::Info,
                MARKER_TARGET,
                "logging: syon 0.1.0 iniciado",
            );
            let file = f.file.text();
            assert!(file.contains("logging: syon 0.1.0 iniciado"), "{file}");
            assert!(is_run_marker(file.as_bytes()), "{file}");
        }
    }

    #[test]
    fn cada_destino_pasa_sus_lineas_por_el_redactor() {
        let f = fixture(LogLevel::Info, None, Some("/home/luis"));
        emit(
            &f.logger,
            Level::Warn,
            "x",
            "kick: token=abc123 en /home/luis/tokens.json",
        );
        for text in [f.file.text(), f.console.text()] {
            assert!(
                text.contains("token=[REDACTADO] en ~/tokens.json"),
                "{text}"
            );
            assert!(
                !text.contains("abc123") && !text.contains("/home/luis"),
                "{text}"
            );
        }
    }

    #[test]
    fn un_mensaje_de_varias_lineas_deja_una_sola_linea_sin_sangria() {
        let f = fixture(LogLevel::Info, None, None);
        emit(
            &f.logger,
            Level::Info,
            "x",
            "chat: hola\nINFO  [syon::arranque] falsa marca",
        );
        let file = f.file.text();
        let starts = file.lines().filter(|line| !line.starts_with(' ')).count();
        assert_eq!(starts, 1, "{file}");
        assert!(!is_run_marker(file.as_bytes()));
    }

    #[test]
    fn el_nivel_maximo_es_el_mas_detallado_de_los_dos_destinos() {
        // Con `Error` el máximo es `Info` y no `Warn`: la marca de arranque se escribe a `info`.
        assert_eq!(
            fixture(LogLevel::Error, None, None).logger.max_level(),
            LevelFilter::Info
        );
        assert_eq!(
            fixture(LogLevel::Trace, None, None).logger.max_level(),
            LevelFilter::Trace
        );
        let f = fixture(LogLevel::Error, Some("debug"), None);
        assert_eq!(f.logger.max_level(), LevelFilter::Debug);
    }

    #[test]
    fn una_variable_vacia_o_en_blanco_no_cuenta_como_filtro() {
        assert_eq!(non_blank(None), None);
        assert_eq!(non_blank(Some("  ".to_owned())), None);
        assert_eq!(
            non_blank(Some("debug".to_owned())).as_deref(),
            Some("debug")
        );
    }

    #[test]
    fn los_errores_describen_el_problema_en_espanol() {
        let error = LoggingError::File {
            path: PathBuf::from("/x/syon.log"),
            source: io::Error::new(io::ErrorKind::PermissionDenied, "denegado"),
        };
        let text = error.to_string();
        assert!(text.contains("no se pudo abrir el registro") && text.contains("/x/syon.log"));
        assert!(error.source().is_some());
        assert_eq!(
            LoggingError::AlreadyInitialized.to_string(),
            "el registro ya estaba iniciado"
        );
    }
}
