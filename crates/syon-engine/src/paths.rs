use std::{
    env,
    error::Error,
    fmt,
    path::{Path, PathBuf},
};

/// Carpeta de SYON dentro de la carpeta personal del usuario, la misma en todos los sistemas
/// (ADR-0020). En Windows es una carpeta visible: el punto es solo parte del nombre.
const APP_DIR: &str = ".SYON";
const SETTINGS_FILE: &str = "settings.toml";
const LOG_FILE: &str = "syon.log";
const CREDENTIALS_FILE: &str = "credentials.toml";

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
    home_dir: Option<PathBuf>,
}

impl AppPaths {
    /// Resuelve las carpetas bajo `~/.SYON` (`config`, `data` y `cache`), con `~` la carpeta
    /// personal que da el sistema (`HOME` o `USERPROFILE`; ADR-0020). No crea nada.
    pub fn from_system() -> Result<Self, PathsError> {
        Self::from_home(env::home_dir())
    }

    /// Reparte las carpetas bajo `home/.SYON`. Una carpeta personal ausente o relativa es un
    /// error: una ruta relativa dejaría los archivos donde esté el directorio de trabajo.
    fn from_home(home: Option<PathBuf>) -> Result<Self, PathsError> {
        let home = home
            .filter(|home| home.is_absolute())
            .ok_or(PathsError::HomeDirUnavailable)?;
        let mut paths = Self::under(home.join(APP_DIR));
        paths.home_dir = Some(home);
        Ok(paths)
    }

    /// Reparte las tres carpetas bajo `root` (`config`, `data` y `cache`), sin consultar el
    /// sistema. Sirve para pruebas y para una instalación portátil; no tiene carpeta personal.
    pub fn under(root: impl AsRef<Path>) -> Self {
        let root = root.as_ref();
        Self {
            config_dir: root.join("config"),
            data_dir: root.join("data"),
            cache_dir: root.join("cache"),
            home_dir: None,
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

    /// Carpeta personal del usuario, si se resolvió con el sistema. El registro la oculta en sus
    /// líneas; [`AppPaths::under`] no la tiene.
    pub fn home_dir(&self) -> Option<&Path> {
        self.home_dir.as_deref()
    }

    /// Archivo de ajustes (`settings.toml`, en la carpeta de configuración).
    pub fn settings_file(&self) -> PathBuf {
        self.config_dir.join(SETTINGS_FILE)
    }

    /// Archivo de registro actual (`syon.log`, en la carpeta de datos).
    pub fn log_file(&self) -> PathBuf {
        self.data_dir.join(LOG_FILE)
    }

    /// Archivo de tokens de las plataformas (`credentials.toml`, en la carpeta de datos). Los
    /// tokens son estado y no preferencias, así que viven junto al log y no con los ajustes.
    pub fn credentials_file(&self) -> PathBuf {
        self.data_dir.join(CREDENTIALS_FILE)
    }
}

/// No se pudieron determinar las carpetas del sistema.
#[derive(Debug)]
pub enum PathsError {
    /// El sistema no indica una carpeta personal del usuario, o no es una ruta absoluta.
    HomeDirUnavailable,
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
        assert_eq!(
            paths.credentials_file(),
            Path::new("/raiz/data/credentials.toml")
        );
    }

    #[test]
    fn under_no_tiene_carpeta_personal_y_el_sistema_si() {
        assert_eq!(AppPaths::under("/raiz").home_dir(), None);
        let home = AppPaths::from_system()
            .unwrap()
            .home_dir()
            .map(Path::to_path_buf);
        assert!(home.is_some_and(|home| home.is_absolute()));
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

    /// En todos los sistemas las tres carpetas cuelgan de `~/.SYON` (ADR-0020).
    #[test]
    fn las_carpetas_del_sistema_cuelgan_de_punto_syon_en_la_carpeta_personal() {
        let home = env::home_dir().unwrap();
        let paths = AppPaths::from_system().unwrap();
        let root = home.join(".SYON");
        assert_eq!(paths.config_dir(), root.join("config"));
        assert_eq!(paths.data_dir(), root.join("data"));
        assert_eq!(paths.cache_dir(), root.join("cache"));
        assert_eq!(paths.home_dir(), Some(home.as_path()));
    }

    #[test]
    fn from_home_reparte_las_carpetas_bajo_punto_syon_y_guarda_la_carpeta_personal() {
        let home = env::temp_dir();
        let paths = AppPaths::from_home(Some(home.clone())).unwrap();
        assert_eq!(paths, {
            let mut esperado = AppPaths::under(home.join(".SYON"));
            esperado.home_dir = Some(home);
            esperado
        });
    }

    #[test]
    fn sin_carpeta_personal_o_con_una_relativa_es_un_error() {
        assert!(AppPaths::from_home(None).is_err());
        assert!(AppPaths::from_home(Some(PathBuf::from("relativa/casa"))).is_err());
        assert!(AppPaths::from_home(Some(PathBuf::new())).is_err());
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
        assert_eq!(
            paths.credentials_file().file_name().unwrap(),
            "credentials.toml"
        );
        assert_eq!(paths.credentials_file().parent(), Some(paths.data_dir()));
    }
}
