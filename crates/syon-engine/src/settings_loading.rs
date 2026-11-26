use std::{
    fs,
    path::{Path, PathBuf},
};
use syon_app::{Settings, SettingsError, SettingsStore};

/// Ajustes con los que arranca SYON y, si hubo un problema al leerlos, cuál fue.
#[derive(Debug)]
pub(crate) struct LoadedSettings {
    pub(crate) settings: Settings,
    /// Problema de la lectura. Se atiende con [`recover`] cuando el registro ya está instalado:
    /// el nivel del registro sale de los ajustes, así que todavía no hay dónde avisar.
    pub(crate) problem: Option<SettingsError>,
}

/// Lee los ajustes sin tocar el disco ni fallar (ADR-0010). Si no se pueden leer, devuelve los
/// valores por defecto y el problema, para que lo atienda [`recover`] más tarde.
pub(crate) fn read_settings(store: &dyn SettingsStore) -> LoadedSettings {
    match store.load() {
        Ok(settings) => LoadedSettings {
            settings,
            problem: None,
        },
        Err(error) => LoadedSettings {
            settings: Settings::default(),
            problem: Some(error),
        },
    }
}

/// Atiende el problema de una lectura y devuelve el aviso para el registro (ADR-0010):
///
/// - Si el contenido es inválido, el archivo se aparta como `<nombre>.invalid` (sustituyendo a un
///   apartado anterior). No se guarda nada: el siguiente `save` crea un archivo nuevo, y lo que
///   había escrito el usuario sigue en el apartado.
/// - Si no se puede acceder al archivo, no se toca nada.
///
/// Es lo único del arranque que cambia el disco por culpa de los ajustes, y por eso se llama solo
/// cuando el registro ya se instaló: un arranque que va a fallar no debe mover el archivo.
pub(crate) fn recover(error: &SettingsError, file: &Path) -> String {
    match error {
        SettingsError::Invalid { .. } => {
            let backup = backup_path(file);
            let outcome = match fs::rename(file, &backup) {
                Ok(()) => format!(
                    "se movió el archivo a {} y se usan los valores por defecto",
                    backup.display()
                ),
                Err(rename_error) => format!(
                    "no se pudo apartar el archivo ({rename_error}); se usan los valores por \
                     defecto y no se escribe nada"
                ),
            };
            format!("{error}; {outcome}")
        }
        SettingsError::Io { .. } => {
            format!("{error}; se usan los valores por defecto y no se escribe nada")
        }
    }
}

/// `settings.toml` → `settings.toml.invalid`.
fn backup_path(file: &Path) -> PathBuf {
    let mut name = file.file_name().unwrap_or_default().to_os_string();
    name.push(".invalid");
    file.with_file_name(name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};
    use syon_app::LogLevel;
    use syon_storage_toml::TomlSettingsStore;

    /// Carpeta temporal única que se borra al soltarla.
    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            static COUNTER: AtomicU32 = AtomicU32::new(0);
            let n = COUNTER.fetch_add(1, Ordering::Relaxed);
            let dir =
                std::env::temp_dir().join(format!("syon-settings-{}-{n}", std::process::id()));
            fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }

        fn file(&self) -> PathBuf {
            self.0.join("settings.toml")
        }

        fn entries(&self) -> usize {
            fs::read_dir(&self.0).unwrap().count()
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    /// Resultado de leer y, si hubo un problema, atenderlo: lo que hace `Engine::start`.
    struct Outcome {
        settings: Settings,
        warning: Option<String>,
    }

    fn load(dir: &TempDir) -> Outcome {
        load_with(&TomlSettingsStore::new(dir.file()), &dir.file())
    }

    fn load_with(store: &TomlSettingsStore, file: &Path) -> Outcome {
        let loaded = read_settings(store);
        Outcome {
            settings: loaded.settings,
            warning: loaded.problem.map(|error| recover(&error, file)),
        }
    }

    #[test]
    fn sin_archivo_usa_los_valores_por_defecto_sin_aviso_ni_escribir() {
        let dir = TempDir::new();
        let loaded = load(&dir);
        assert_eq!(loaded.settings, Settings::default());
        assert!(loaded.warning.is_none());
        assert_eq!(dir.entries(), 0);
    }

    #[test]
    fn un_archivo_valido_se_carga_sin_aviso() {
        let dir = TempDir::new();
        fs::write(dir.file(), "[logging]\nlevel = \"debug\"\n").unwrap();
        let loaded = load(&dir);
        assert_eq!(loaded.settings.logging.level, LogLevel::Debug);
        assert!(loaded.warning.is_none());
        assert!(dir.file().is_file());
    }

    #[test]
    fn un_archivo_invalido_se_aparta_y_se_usan_los_valores_por_defecto() {
        let dir = TempDir::new();
        let broken = "[logging]\nlevel = 5\n";
        fs::write(dir.file(), broken).unwrap();
        let loaded = load(&dir);
        assert_eq!(loaded.settings, Settings::default());
        let warning = loaded.warning.expect("debe avisar");
        assert!(warning.contains("logging.level"), "{warning}");
        assert!(warning.contains("settings.toml.invalid"), "{warning}");
        assert!(
            !dir.file().exists(),
            "no se guardan valores por defecto encima"
        );
        assert_eq!(
            fs::read_to_string(backup_path(&dir.file())).unwrap(),
            broken
        );
        assert_eq!(dir.entries(), 1);
    }

    #[test]
    fn leer_no_toca_el_disco_aunque_el_archivo_sea_invalido() {
        let dir = TempDir::new();
        let broken = "[logging]\nlevel = 5\n";
        fs::write(dir.file(), broken).unwrap();
        let loaded = read_settings(&TomlSettingsStore::new(dir.file()));
        assert_eq!(loaded.settings, Settings::default());
        assert!(matches!(
            loaded.problem,
            Some(SettingsError::Invalid { .. })
        ));
        assert_eq!(fs::read_to_string(dir.file()).unwrap(), broken);
        assert_eq!(dir.entries(), 1);
    }

    #[test]
    fn un_segundo_archivo_invalido_sustituye_al_apartado_anterior() {
        let dir = TempDir::new();
        fs::write(dir.file(), "primero = [").unwrap();
        load(&dir);
        fs::write(dir.file(), "segundo = [").unwrap();
        let loaded = load(&dir);
        assert!(loaded.warning.is_some());
        assert_eq!(
            fs::read_to_string(backup_path(&dir.file())).unwrap(),
            "segundo = ["
        );
        assert_eq!(dir.entries(), 1);
    }

    #[test]
    fn despues_de_apartarlo_un_guardado_crea_un_archivo_nuevo_y_conserva_el_apartado() {
        let dir = TempDir::new();
        fs::write(dir.file(), "roto = [").unwrap();
        let store = TomlSettingsStore::new(dir.file());
        let loaded = load_with(&store, &dir.file());
        let mut settings = loaded.settings;
        settings.logging.level = LogLevel::Warn;
        syon_app::SettingsStore::save(&store, &settings).unwrap();
        assert_eq!(store.load().unwrap(), settings);
        assert_eq!(
            fs::read_to_string(backup_path(&dir.file())).unwrap(),
            "roto = ["
        );
    }

    #[test]
    fn si_no_se_puede_apartar_el_archivo_queda_intacto_y_se_avisa() {
        let dir = TempDir::new();
        fs::write(dir.file(), "roto = [").unwrap();
        // Una carpeta no vacía con el nombre del apartado hace fallar el renombrado.
        fs::create_dir_all(backup_path(&dir.file()).join("dentro")).unwrap();
        let loaded = load(&dir);
        assert_eq!(loaded.settings, Settings::default());
        let warning = loaded.warning.expect("debe avisar");
        assert!(
            warning.contains("no se pudo apartar el archivo"),
            "{warning}"
        );
        assert_eq!(fs::read_to_string(dir.file()).unwrap(), "roto = [");
    }

    #[test]
    fn un_error_de_acceso_usa_los_valores_por_defecto_y_no_toca_nada() {
        let dir = TempDir::new();
        // Una carpeta con el nombre del archivo: leerla es un error de E/S, no un contenido inválido.
        fs::create_dir(dir.file()).unwrap();
        let loaded = load(&dir);
        assert_eq!(loaded.settings, Settings::default());
        let warning = loaded.warning.expect("debe avisar");
        assert!(warning.contains("no se pudo acceder"), "{warning}");
        assert!(dir.file().is_dir());
        assert_eq!(dir.entries(), 1);
    }
}
