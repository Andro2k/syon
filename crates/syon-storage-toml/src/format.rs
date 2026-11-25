use serde::{Deserialize, Serialize, de::DeserializeOwned};
use syon_app::{LogLevel, Settings};
use toml::{Table, Value};

/// Forma de escritura de `settings.toml`. Es propia del adaptador: `syon-app` no conoce
/// `serde`, así que el modelo se traduce hacia este tipo al guardar y se lee clave a
/// clave al cargar (ver [`parse`]).
#[derive(Serialize)]
struct SettingsFile {
    schema_version: u32,
    logging: LoggingSection,
}

/// Sección `[logging]`.
#[derive(Serialize)]
struct LoggingSection {
    level: LevelName,
}

/// Nombre del nivel de log tal como se escribe en el archivo.
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
enum LevelName {
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

impl From<LogLevel> for LevelName {
    fn from(level: LogLevel) -> Self {
        match level {
            LogLevel::Error => Self::Error,
            LogLevel::Warn => Self::Warn,
            LogLevel::Info => Self::Info,
            LogLevel::Debug => Self::Debug,
            LogLevel::Trace => Self::Trace,
        }
    }
}

impl From<LevelName> for LogLevel {
    fn from(name: LevelName) -> Self {
        match name {
            LevelName::Error => Self::Error,
            LevelName::Warn => Self::Warn,
            LevelName::Info => Self::Info,
            LevelName::Debug => Self::Debug,
            LevelName::Trace => Self::Trace,
        }
    }
}

/// Convierte los ajustes al texto TOML que se escribe en disco.
pub(crate) fn render(settings: &Settings) -> Result<String, toml::ser::Error> {
    toml::to_string_pretty(&SettingsFile {
        schema_version: settings.schema_version,
        logging: LoggingSection {
            level: settings.logging.level.into(),
        },
    })
}

/// Lee los ajustes de un texto TOML. Las claves ausentes toman su valor por defecto y las
/// desconocidas se ignoran. Se lee clave a clave, en lugar de derivar `Deserialize` sobre
/// todo el archivo, porque así un tipo incorrecto nombra la clave (`logging.level: …`),
/// algo que los errores de `toml` no hacen por sí solos.
pub(crate) fn parse(text: &str) -> Result<Settings, String> {
    let table: Table = text
        .parse()
        .map_err(|error: toml::de::Error| error.to_string())?;
    let mut settings = Settings::default();
    if let Some(version) = read::<u32>(&table, &["schema_version"])? {
        settings.schema_version = version;
    }
    if let Some(level) = read::<LevelName>(&table, &["logging", "level"])? {
        settings.logging.level = level.into();
    }
    Ok(settings)
}

/// Busca `path` (por ejemplo `["logging", "level"]`) y lo convierte a `T`. Devuelve `None`
/// si la clave no existe; si existe con otro tipo, el error nombra la clave completa.
fn read<T: DeserializeOwned>(table: &Table, path: &[&str]) -> Result<Option<T>, String> {
    let mut current = table;
    let (last, parents) = path
        .split_last()
        .expect("la ruta de una clave no está vacía");
    for (depth, segment) in parents.iter().enumerate() {
        match current.get(*segment) {
            None => return Ok(None),
            Some(Value::Table(inner)) => current = inner,
            Some(other) => {
                let key = path[..=depth].join(".");
                return Err(format!(
                    "{key}: se esperaba una tabla y es {}",
                    other.type_str()
                ));
            }
        }
    }
    match current.get(*last) {
        None => Ok(None),
        Some(value) => value
            .clone()
            .try_into()
            .map(Some)
            .map_err(|error: toml::de::Error| format!("{}: {}", path.join("."), error.message())),
    }
}
