use crate::format::{self, CredentialsFile};
use std::{
    fs::{self, File},
    io::{self, Write},
    path::{Path, PathBuf},
    sync::{Mutex, PoisonError},
};
use syon_app::{CredentialError, CredentialStore, Credentials};
use syon_domain::Platform;

/// Serializa los guardados de todo el proceso, igual que el almacén de ajustes: `save` y `clear`
/// leen el archivo entero, cambian una sección y lo reescriben, y dos a la vez se pisarían.
static WRITE_LOCK: Mutex<()> = Mutex::new(());

/// Guarda las credenciales en un archivo TOML cuya ruta recibe por constructor: este tipo no
/// consulta el sistema operativo para ubicarla. En Unix el archivo queda legible solo por su dueño.
#[derive(Debug, Clone)]
pub struct TomlCredentialStore {
    path: PathBuf,
}

impl TomlCredentialStore {
    /// Crea el adaptador para el archivo `path`. No toca el disco hasta `load`, `save` o `clear`.
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    /// Ruta del archivo de credenciales.
    pub fn path(&self) -> &Path {
        &self.path
    }

    fn io_error(&self, source: io::Error) -> CredentialError {
        CredentialError::Io {
            path: self.path.clone(),
            source,
        }
    }

    fn invalid(&self, detail: impl ToString) -> CredentialError {
        CredentialError::Invalid {
            path: self.path.clone(),
            detail: detail.to_string(),
        }
    }

    /// Lee el archivo; si no existe, devuelve uno vacío.
    fn read(&self) -> Result<CredentialsFile, CredentialError> {
        let bytes = match fs::read(&self.path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Ok(CredentialsFile::default());
            }
            Err(error) => return Err(self.io_error(error)),
        };
        let text = String::from_utf8(bytes)
            .map_err(|_| self.invalid("el archivo no es texto UTF-8 válido"))?;
        format::parse(&text).map_err(|detail| self.invalid(detail))
    }

    /// Cambia la sección de `platform` y reescribe el archivo de forma atómica. Un archivo
    /// existente pero inválido no se sobrescribe.
    fn update(
        &self,
        platform: Platform,
        credentials: Option<&Credentials>,
    ) -> Result<(), CredentialError> {
        let _guard = WRITE_LOCK.lock().unwrap_or_else(PoisonError::into_inner);
        let mut file = self.read()?;
        file.set(platform, credentials);
        let content = format::render(&mut file).map_err(|error| self.invalid(error))?;
        if let Some(parent) = self.path.parent().filter(|p| !p.as_os_str().is_empty()) {
            fs::create_dir_all(parent).map_err(|error| self.io_error(error))?;
        }
        let temp = self.temp_path();
        let result = write_private(&temp, &content).and_then(|()| fs::rename(&temp, &self.path));
        if let Err(error) = result {
            // Mejor esfuerzo: el temporal no debe quedar huérfano; el original sigue intacto.
            let _ = fs::remove_file(&temp);
            return Err(self.io_error(error));
        }
        Ok(())
    }

    /// Temporal de `update`: misma carpeta que el destino para que el `rename` no cruce de
    /// sistema de archivos; lleva el pid para no chocar con otro proceso.
    fn temp_path(&self) -> PathBuf {
        let mut name = self.path.file_name().unwrap_or_default().to_os_string();
        name.push(format!(".{}.tmp", std::process::id()));
        self.path.with_file_name(name)
    }
}

impl CredentialStore for TomlCredentialStore {
    fn load(&self, platform: Platform) -> Result<Option<Credentials>, CredentialError> {
        Ok(self.read()?.get(platform))
    }

    fn save(&self, platform: Platform, credentials: &Credentials) -> Result<(), CredentialError> {
        self.update(platform, Some(credentials))
    }

    fn clear(&self, platform: Platform) -> Result<(), CredentialError> {
        if !self.path.exists() {
            return Ok(());
        }
        self.update(platform, None)
    }
}

/// Escribe `content` en `path`, lo sincroniza a disco y, en Unix, deja el archivo legible solo
/// por su dueño desde antes de escribir el primer byte.
fn write_private(path: &Path, content: &str) -> io::Result<()> {
    let mut options = fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file: File = options.open(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        file.set_permissions(fs::Permissions::from_mode(0o600))?;
    }
    file.write_all(content.as_bytes())?;
    file.sync_all()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};
    use syon_domain::Timestamp;

    /// Carpeta temporal única que se borra al soltarla (sin dependencias de prueba).
    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            static COUNTER: AtomicU32 = AtomicU32::new(0);
            let n = COUNTER.fetch_add(1, Ordering::Relaxed);
            let dir = std::env::temp_dir().join(format!("syon-cred-{}-{n}", std::process::id()));
            fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }

        fn file(&self) -> PathBuf {
            self.0.join("credentials.toml")
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn credentials(token: &str) -> Credentials {
        Credentials {
            access_token: token.to_owned(),
            refresh_token: Some(format!("{token}-renovacion")),
            expires_at: Some(Timestamp::from_unix_ms(9_000)),
            scopes: vec!["chat:write".to_owned()],
        }
    }

    #[test]
    fn sin_archivo_no_hay_credenciales_y_no_se_escribe_nada() {
        let dir = TempDir::new();
        let store = TomlCredentialStore::new(dir.file());
        assert_eq!(store.load(Platform::Kick).unwrap(), None);
        store.clear(Platform::Kick).unwrap();
        assert_eq!(fs::read_dir(&dir.0).unwrap().count(), 0);
    }

    #[test]
    fn guardar_y_cargar_devuelve_lo_mismo_y_cada_plataforma_tiene_lo_suyo() {
        let dir = TempDir::new();
        let store = TomlCredentialStore::new(dir.file());
        store.save(Platform::Kick, &credentials("k")).unwrap();
        store.save(Platform::Twitch, &credentials("t")).unwrap();
        assert_eq!(store.load(Platform::Kick).unwrap(), Some(credentials("k")));
        assert_eq!(
            store.load(Platform::Twitch).unwrap(),
            Some(credentials("t"))
        );
    }

    #[test]
    fn guardar_una_plataforma_no_toca_la_otra_ni_deja_temporales() {
        let dir = TempDir::new();
        let store = TomlCredentialStore::new(dir.file());
        store.save(Platform::Kick, &credentials("k1")).unwrap();
        store.save(Platform::Twitch, &credentials("t")).unwrap();
        store.save(Platform::Kick, &credentials("k2")).unwrap();
        assert_eq!(
            store.load(Platform::Twitch).unwrap(),
            Some(credentials("t"))
        );
        assert_eq!(store.load(Platform::Kick).unwrap(), Some(credentials("k2")));
        assert_eq!(fs::read_dir(&dir.0).unwrap().count(), 1);
    }

    #[test]
    fn clear_borra_solo_esa_plataforma() {
        let dir = TempDir::new();
        let store = TomlCredentialStore::new(dir.file());
        store.save(Platform::Kick, &credentials("k")).unwrap();
        store.save(Platform::Twitch, &credentials("t")).unwrap();
        store.clear(Platform::Kick).unwrap();
        store.clear(Platform::Kick).unwrap();
        assert_eq!(store.load(Platform::Kick).unwrap(), None);
        assert_eq!(
            store.load(Platform::Twitch).unwrap(),
            Some(credentials("t"))
        );
    }

    #[test]
    fn save_crea_la_carpeta_si_no_existe() {
        let dir = TempDir::new();
        let path = dir.0.join("datos").join("syon").join("credentials.toml");
        let store = TomlCredentialStore::new(&path);
        store.save(Platform::Kick, &credentials("k")).unwrap();
        assert!(path.is_file());
    }

    #[test]
    fn un_archivo_invalido_es_un_error_y_nunca_se_sobrescribe() {
        let dir = TempDir::new();
        let text = "[kick]\naccess_token = \"token-secreto-123\"\nscopes = 7\n";
        fs::write(dir.file(), text).unwrap();
        let store = TomlCredentialStore::new(dir.file());

        let error = store.load(Platform::Kick).err().unwrap();
        assert!(matches!(error, CredentialError::Invalid { .. }));
        assert!(!error.to_string().contains("token-secreto-123"), "{error}");
        assert!(store.save(Platform::Twitch, &credentials("t")).is_err());
        assert!(store.clear(Platform::Kick).is_err());
        assert_eq!(fs::read_to_string(dir.file()).unwrap(), text);
    }

    #[test]
    fn un_archivo_que_no_es_utf8_es_invalido() {
        let dir = TempDir::new();
        fs::write(dir.file(), [0xff, 0xfe, 0x00]).unwrap();
        let store = TomlCredentialStore::new(dir.file());
        assert!(matches!(
            store.load(Platform::Kick),
            Err(CredentialError::Invalid { .. })
        ));
    }

    #[cfg(unix)]
    #[test]
    fn en_unix_el_archivo_solo_lo_lee_su_dueno() {
        use std::os::unix::fs::PermissionsExt;
        let dir = TempDir::new();
        let store = TomlCredentialStore::new(dir.file());
        store.save(Platform::Kick, &credentials("k")).unwrap();
        let mode = fs::metadata(dir.file()).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
    }

    #[test]
    fn varios_guardados_simultaneos_no_pierden_ninguna_plataforma() {
        let dir = TempDir::new();
        let store = TomlCredentialStore::new(dir.file());
        std::thread::scope(|scope| {
            for n in 0..8 {
                let store = store.clone();
                scope.spawn(move || {
                    let platform = if n % 2 == 0 {
                        Platform::Kick
                    } else {
                        Platform::Twitch
                    };
                    store
                        .save(platform, &credentials(&format!("t{n}")))
                        .unwrap();
                });
            }
        });
        assert!(store.load(Platform::Kick).unwrap().is_some());
        assert!(store.load(Platform::Twitch).unwrap().is_some());
    }
}
