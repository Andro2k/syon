use etcetera::{
    HomeDirError,
    app_strategy::{AppStrategy, AppStrategyArgs, choose_app_strategy},
};
use std::{
    error::Error,
    fmt,
    path::{Path, PathBuf},
};

/// Nombre de la carpeta de SYON dentro de las carpetas del sistema. En Windows se usa tal cual
/// (`SYON`) y en Linux `etcetera` lo pasa a minúsculas (`syon`).
const APP_NAME: &str = "SYON";
const SETTINGS_FILE: &str = "settings.toml";
const LOG_FILE: &str = "syon.log";

/// Carpetas donde SYON guarda sus archivos: configuración, datos y caché.
///
/// Solo [`AppPaths::from_system`] consulta el sistema operativo; el resto del engine y los
/// adaptadores reciben rutas ya resueltas, de modo que las pruebas usan carpetas temporales
/// con [`AppPaths::under`] (diseño 001, sección 7). Construir el valor no crea carpetas.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppPaths {
    config_dir: PathBuf,
    data_dir: PathBuf,
    cache_dir: PathBuf,
}

impl AppPaths {
    /// Resuelve las carpetas según las convenciones del sistema operativo (ADR-0009).
    pub fn from_system() -> Result<Self, PathsError> {
        let strategy = choose_app_strategy(AppStrategyArgs {
            top_level_domain: String::new(),
            author: String::new(),
            app_name: APP_NAME.to_owned(),
        })
        .map_err(PathsError::from)?;
        Ok(Self {
            config_dir: strategy.config_dir(),
            data_dir: strategy.data_dir(),
            cache_dir: strategy.cache_dir(),
        })
    }

    /// Reparte las tres carpetas bajo `root` (`config`, `data` y `cache`), sin consultar el
    /// sistema. Sirve para pruebas y para una instalación portátil.
    pub fn under(root: impl AsRef<Path>) -> Self {
        let root = root.as_ref();
        Self {
            config_dir: root.join("config"),
            data_dir: root.join("data"),
            cache_dir: root.join("cache"),
        }
    }

    /// Carpeta de las preferencias del usuario.
    pub fn config_dir(&self) -> &Path {
        &self.config_dir
    }

    /// Carpeta de los datos de la aplicación: log y estado de ejecución.
    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    /// Carpeta de datos regenerables.
    pub fn cache_dir(&self) -> &Path {
        &self.cache_dir
    }

    /// Archivo de ajustes (`settings.toml`, en la carpeta de configuración).
    pub fn settings_file(&self) -> PathBuf {
        self.config_dir.join(SETTINGS_FILE)
    }

    /// Archivo de registro actual (`syon.log`, en la carpeta de datos).
    pub fn log_file(&self) -> PathBuf {
        self.data_dir.join(LOG_FILE)
    }
}

/// No se pudieron determinar las carpetas del sistema.
#[derive(Debug)]
pub enum PathsError {
    /// El sistema no indica cuál es la carpeta personal del usuario.
    HomeDirUnavailable,
}

impl From<HomeDirError> for PathsError {
    fn from(_: HomeDirError) -> Self {
        Self::HomeDirUnavailable
    }
}

impl fmt::Display for PathsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::HomeDirUnavailable => {
                write!(f, "no se pudo determinar la carpeta personal del usuario")
            }
        }
    }
}

impl Error for PathsError {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn under_reparte_las_carpetas_bajo_la_raiz_y_nombra_los_archivos() {
        let paths = AppPaths::under("/raiz");
        assert_eq!(paths.config_dir(), Path::new("/raiz/config"));
        assert_eq!(paths.data_dir(), Path::new("/raiz/data"));
        assert_eq!(paths.cache_dir(), Path::new("/raiz/cache"));
        assert_eq!(
            paths.settings_file(),
            Path::new("/raiz/config/settings.toml")
        );
        assert_eq!(paths.log_file(), Path::new("/raiz/data/syon.log"));
    }

    #[test]
    fn construir_las_rutas_no_crea_carpetas() {
        let root = env::temp_dir().join(format!("syon-paths-{}", std::process::id()));
        let _ = AppPaths::under(&root);
        assert!(!root.exists());
    }

    #[test]
    fn el_error_describe_el_problema_en_espanol() {
        let message = PathsError::HomeDirUnavailable.to_string();
        assert!(message.contains("carpeta personal"));
    }

    /// En Linux la configuración, los datos y la caché siguen XDG: las variables
    /// `XDG_*_HOME` si son rutas absolutas y, si no, las carpetas bajo `$HOME`.
    #[cfg(target_os = "linux")]
    #[test]
    fn en_linux_las_carpetas_siguen_xdg() {
        fn base(variable: &str, fallback: &str) -> PathBuf {
            match env::var_os(variable).map(PathBuf::from) {
                Some(path) if path.is_absolute() => path,
                _ => PathBuf::from(env::var_os("HOME").unwrap()).join(fallback),
            }
        }
        let paths = AppPaths::from_system().unwrap();
        assert_eq!(
            paths.config_dir(),
            base("XDG_CONFIG_HOME", ".config").join("syon")
        );
        assert_eq!(
            paths.data_dir(),
            base("XDG_DATA_HOME", ".local/share").join("syon")
        );
        assert_eq!(
            paths.cache_dir(),
            base("XDG_CACHE_HOME", ".cache").join("syon")
        );
    }

    /// En Windows la configuración y los datos viven en `%APPDATA%` (Roaming) y la caché en
    /// `%LOCALAPPDATA%`, dentro de una carpeta `SYON` (ADR-0009). Solo corre en Windows (CI).
    #[cfg(windows)]
    #[test]
    fn en_windows_las_carpetas_corresponden_a_appdata_y_localappdata() {
        let roaming = PathBuf::from(env::var_os("APPDATA").unwrap());
        let local = PathBuf::from(env::var_os("LOCALAPPDATA").unwrap());
        let paths = AppPaths::from_system().unwrap();
        assert_eq!(paths.config_dir(), roaming.join("SYON").join("config"));
        assert_eq!(paths.data_dir(), roaming.join("SYON").join("data"));
        assert_eq!(paths.cache_dir(), local.join("SYON").join("cache"));
    }

    #[test]
    fn las_rutas_del_sistema_son_absolutas_y_distintas_entre_si() {
        let paths = AppPaths::from_system().unwrap();
        for dir in [paths.config_dir(), paths.data_dir(), paths.cache_dir()] {
            assert!(dir.is_absolute(), "{} no es absoluta", dir.display());
        }
        assert_ne!(paths.config_dir(), paths.data_dir());
        assert_ne!(paths.data_dir(), paths.cache_dir());
        assert_eq!(paths.settings_file().file_name().unwrap(), "settings.toml");
        assert_eq!(paths.log_file().file_name().unwrap(), "syon.log");
    }
}
