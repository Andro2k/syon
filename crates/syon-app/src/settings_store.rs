use crate::Settings;
use std::{error::Error, fmt, io, path::PathBuf};

/// Puerto de lectura y escritura de los ajustes del usuario.
pub trait SettingsStore: Send + Sync {
    /// Lee los ajustes. Si todavía no existen devuelve los valores por defecto sin
    /// escribir nada. Un contenido ilegible es un error y nunca se sobrescribe.
    fn load(&self) -> Result<Settings, SettingsError>;

    /// Guarda los ajustes. Si falla, el contenido anterior queda intacto.
    fn save(&self, settings: &Settings) -> Result<(), SettingsError>;
}

/// Error de un [`SettingsStore`].
#[derive(Debug)]
pub enum SettingsError {
    /// No se pudo leer o escribir el almacén.
    Io {
        /// Ruta del archivo afectado.
        path: PathBuf,
        /// Error del sistema operativo.
        source: io::Error,
    },
    /// El contenido existe pero no es válido.
    Invalid {
        /// Ruta del archivo afectado.
        path: PathBuf,
        /// Qué falló y, si se conoce, en qué clave.
        detail: String,
    },
}

impl fmt::Display for SettingsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, source } => {
                write!(
                    f,
                    "no se pudo acceder a los ajustes en {}: {source}",
                    path.display()
                )
            }
            Self::Invalid { path, detail } => {
                write!(f, "ajustes inválidos en {}: {detail}", path.display())
            }
        }
    }
}

impl Error for SettingsError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::Invalid { .. } => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// Almacén en memoria que cumple el contrato del puerto.
    #[derive(Default)]
    struct MemoryStore(Mutex<Option<Settings>>);

    impl SettingsStore for MemoryStore {
        fn load(&self) -> Result<Settings, SettingsError> {
            Ok(self.0.lock().unwrap().clone().unwrap_or_default())
        }

        fn save(&self, settings: &Settings) -> Result<(), SettingsError> {
            *self.0.lock().unwrap() = Some(settings.clone());
            Ok(())
        }
    }

    #[test]
    fn sin_ajustes_guardados_se_cargan_los_valores_por_defecto() {
        let store: Box<dyn SettingsStore> = Box::new(MemoryStore::default());
        assert_eq!(store.load().unwrap(), Settings::default());
    }

    #[test]
    fn lo_guardado_se_recupera_tal_cual() {
        let store: Box<dyn SettingsStore> = Box::new(MemoryStore::default());
        let mut settings = Settings::default();
        settings.logging.level = crate::LogLevel::Debug;
        store.save(&settings).unwrap();
        assert_eq!(store.load().unwrap(), settings);
    }

    #[test]
    fn los_errores_indican_la_ruta_y_el_detalle() {
        let invalid = SettingsError::Invalid {
            path: PathBuf::from("settings.toml"),
            detail: "logging.level: valor desconocido".to_owned(),
        };
        let message = invalid.to_string();
        assert!(message.contains("settings.toml"));
        assert!(message.contains("logging.level"));
        assert!(invalid.source().is_none());
    }

    #[test]
    fn el_error_de_io_conserva_su_causa() {
        let error = SettingsError::Io {
            path: PathBuf::from("settings.toml"),
            source: io::Error::new(io::ErrorKind::PermissionDenied, "sin permiso"),
        };
        assert!(error.to_string().contains("sin permiso"));
        assert!(error.source().is_some());
    }
}
