use crate::format;
use std::{
    fs::{self, File},
    io::{self, Write},
    path::{Path, PathBuf},
    sync::{Mutex, PoisonError},
};
use syon_app::{SCHEMA_VERSION, Settings, SettingsError, SettingsStore};

/// Serializa los guardados de todo el proceso. Dos `save` a la vez usarían el mismo temporal
/// (`settings.toml.<pid>.tmp`) y uno encontraría que el otro ya lo renombró. Además, dos
/// `rename` simultáneos sobre el mismo destino pueden fallar según la plataforma (no se ha
/// verificado en Windows). Guardar es raro y el archivo es pequeño, así que un cierre único
/// es más simple que un temporal distinto por llamada.
static SAVE_LOCK: Mutex<()> = Mutex::new(());

/// Guarda los ajustes en un archivo TOML cuya ruta recibe por constructor: este tipo no
/// consulta el sistema operativo para ubicarla (diseño 001, sección 7).
#[derive(Debug, Clone)]
pub struct TomlSettingsStore {
    path: PathBuf,
}

impl TomlSettingsStore {
    /// Crea el adaptador para el archivo `path`. No toca el disco hasta `load` o `save`.
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    /// Ruta del archivo de ajustes.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Archivo temporal de `save`: misma carpeta que el destino para que el `rename` no
    /// cruce de sistema de archivos; lleva el pid para no chocar con otro proceso.
    fn temp_path(&self) -> PathBuf {
        let mut name = self.path.file_name().unwrap_or_default().to_os_string();
        name.push(format!(".{}.tmp", std::process::id()));
        self.path.with_file_name(name)
    }

    fn io_error(&self, source: io::Error) -> SettingsError {
        SettingsError::Io {
            path: self.path.clone(),
            source,
        }
    }

    fn invalid(&self, detail: impl ToString) -> SettingsError {
        SettingsError::Invalid {
            path: self.path.clone(),
            detail: detail.to_string(),
        }
    }

    /// Rechaza el guardado si el archivo en disco, o los ajustes recibidos, son de un esquema más
    /// nuevo que `SCHEMA_VERSION` (ADR-0019). Reescribirlo borraría las claves que esta versión no
    /// conoce. Un archivo ausente o ilegible no bloquea: `load` ya lo trata aparte.
    fn refuse_newer_schema(&self, settings: &Settings) -> Result<(), SettingsError> {
        let on_disk = fs::read_to_string(&self.path)
            .ok()
            .and_then(|text| format::schema_version(&text));
        let found = on_disk.unwrap_or(0).max(settings.schema_version);
        if found > SCHEMA_VERSION {
            return Err(self.invalid(format!(
                "schema_version {found} es más nuevo que el {SCHEMA_VERSION} que conoce esta \
                 versión de SYON; no se guarda para no perder las claves que no conoce"
            )));
        }
        Ok(())
    }

    /// Escribe `content` en el temporal y lo sincroniza a disco antes del `rename`.
    fn write_temp(temp: &Path, content: &str) -> io::Result<()> {
        let mut file = File::create(temp)?;
        file.write_all(content.as_bytes())?;
        file.sync_all()
    }
}

impl SettingsStore for TomlSettingsStore {
    fn load(&self) -> Result<Settings, SettingsError> {
        let bytes = match fs::read(&self.path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Ok(Settings::default());
            }
            Err(error) => return Err(self.io_error(error)),
        };
        let text = String::from_utf8(bytes)
            .map_err(|_| self.invalid("el archivo no es texto UTF-8 válido"))?;
        format::parse(&text).map_err(|detail| self.invalid(detail))
    }

    fn save(&self, settings: &Settings) -> Result<(), SettingsError> {
        let content = format::render(settings).map_err(|error| self.invalid(error))?;
        let _guard = SAVE_LOCK.lock().unwrap_or_else(PoisonError::into_inner);
        self.refuse_newer_schema(settings)?;
        if let Some(parent) = self.path.parent().filter(|p| !p.as_os_str().is_empty()) {
            fs::create_dir_all(parent).map_err(|error| self.io_error(error))?;
        }
        let temp = self.temp_path();
        let result = Self::write_temp(&temp, &content).and_then(|()| fs::rename(&temp, &self.path));
        if let Err(error) = result {
            // Mejor esfuerzo: el temporal no debe quedar huérfano; el original sigue intacto.
            let _ = fs::remove_file(&temp);
            return Err(self.io_error(error));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};
    use syon_app::LogLevel;

    /// Carpeta temporal única que se borra al soltarla (sin dependencias de prueba).
    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            static COUNTER: AtomicU32 = AtomicU32::new(0);
            let n = COUNTER.fetch_add(1, Ordering::Relaxed);
            let dir = std::env::temp_dir().join(format!("syon-toml-{}-{n}", std::process::id()));
            fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }

        fn file(&self) -> PathBuf {
            self.0.join("settings.toml")
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn debug_settings() -> Settings {
        let mut settings = Settings::default();
        settings.logging.level = LogLevel::Debug;
        settings
    }

    #[test]
    fn sin_archivo_se_cargan_los_valores_por_defecto_y_no_se_escribe_nada() {
        let dir = TempDir::new();
        let store = TomlSettingsStore::new(dir.file());
        assert_eq!(store.load().unwrap(), Settings::default());
        assert!(!dir.file().exists());
        assert_eq!(fs::read_dir(&dir.0).unwrap().count(), 0);
    }

    #[test]
    fn guardar_y_cargar_devuelve_lo_mismo_para_cada_nivel() {
        let levels = [
            LogLevel::Error,
            LogLevel::Warn,
            LogLevel::Info,
            LogLevel::Debug,
            LogLevel::Trace,
        ];
        for level in levels {
            let dir = TempDir::new();
            let store = TomlSettingsStore::new(dir.file());
            let mut settings = Settings::default();
            settings.logging.level = level;
            store.save(&settings).unwrap();
            assert_eq!(store.load().unwrap(), settings);
        }
    }

    #[test]
    fn el_archivo_guardado_lleva_schema_version_y_la_seccion_logging() {
        let dir = TempDir::new();
        let store = TomlSettingsStore::new(dir.file());
        store.save(&debug_settings()).unwrap();
        let text = fs::read_to_string(dir.file()).unwrap();
        assert!(text.contains(&format!("schema_version = {SCHEMA_VERSION}")));
        assert!(text.contains("[logging]"));
        assert!(text.contains("level = \"debug\""));
    }

    #[test]
    fn save_crea_la_carpeta_si_no_existe_y_no_deja_temporales() {
        let dir = TempDir::new();
        let path = dir.0.join("config").join("syon").join("settings.toml");
        let store = TomlSettingsStore::new(&path);
        store.save(&Settings::default()).unwrap();
        assert!(path.is_file());
        let names: Vec<_> = fs::read_dir(path.parent().unwrap()).unwrap().collect();
        assert_eq!(names.len(), 1);
    }

    #[test]
    fn las_claves_desconocidas_se_ignoran() {
        let dir = TempDir::new();
        fs::write(
            dir.file(),
            "schema_version = 1\nfutura = true\n[logging]\nlevel = \"warn\"\ncolor = 3\n[otra]\nx = 1\n",
        )
        .unwrap();
        let loaded = TomlSettingsStore::new(dir.file()).load().unwrap();
        assert_eq!(loaded.logging.level, LogLevel::Warn);
        assert_eq!(loaded.schema_version, 1);
    }

    fn kick_settings() -> Settings {
        let mut settings = Settings::default();
        settings.kick.enabled = true;
        settings.kick.channel = "mi_canal".to_string();
        settings.kick.channel_id = Some(123);
        settings.kick.chatroom_id = Some(456);
        settings
    }

    #[test]
    fn guardar_y_cargar_devuelve_la_seccion_kick_completa() {
        let dir = TempDir::new();
        let store = TomlSettingsStore::new(dir.file());
        store.save(&kick_settings()).unwrap();
        let text = fs::read_to_string(dir.file()).unwrap();
        assert!(text.contains("[kick]"));
        assert!(text.contains("channel = \"mi_canal\""));
        assert_eq!(store.load().unwrap(), kick_settings());
    }

    #[test]
    fn guardar_y_cargar_devuelve_la_seccion_twitch() {
        let dir = TempDir::new();
        let store = TomlSettingsStore::new(dir.file());
        let mut settings = Settings::default();
        settings.twitch.enabled = true;
        store.save(&settings).unwrap();
        let text = fs::read_to_string(dir.file()).unwrap();
        assert!(text.contains("[twitch]"), "{text}");
        assert_eq!(store.load().unwrap(), settings);
    }

    #[test]
    fn un_archivo_sin_twitch_o_con_claves_desconocidas_sigue_cargando() {
        let dir = TempDir::new();
        fs::write(dir.file(), "[kick]\nenabled = true\n").unwrap();
        let loaded = TomlSettingsStore::new(dir.file()).load().unwrap();
        assert!(!loaded.twitch.enabled);
        fs::write(dir.file(), "[twitch]\nenabled = true\ncanal = \"x\"\n").unwrap();
        let loaded = TomlSettingsStore::new(dir.file()).load().unwrap();
        assert!(loaded.twitch.enabled);
    }

    #[test]
    fn un_tipo_incorrecto_en_twitch_nombra_la_clave() {
        let dir = TempDir::new();
        fs::write(dir.file(), "[twitch]\nenabled = 3\n").unwrap();
        let error = TomlSettingsStore::new(dir.file()).load().unwrap_err();
        assert!(error.to_string().contains("twitch.enabled"), "{error}");
    }

    #[test]
    fn los_ids_de_kick_sin_definir_no_se_escriben() {
        let dir = TempDir::new();
        let store = TomlSettingsStore::new(dir.file());
        let mut settings = kick_settings();
        settings.kick.channel_id = None;
        settings.kick.chatroom_id = None;
        store.save(&settings).unwrap();
        let text = fs::read_to_string(dir.file()).unwrap();
        assert!(!text.contains("channel_id"));
        assert!(!text.contains("chatroom_id"));
        assert_eq!(store.load().unwrap(), settings);
    }

    #[test]
    fn un_archivo_de_f1_sin_la_seccion_kick_sigue_cargando() {
        let dir = TempDir::new();
        fs::write(
            dir.file(),
            "schema_version = 1\n[logging]\nlevel = \"warn\"\n",
        )
        .unwrap();
        let loaded = TomlSettingsStore::new(dir.file()).load().unwrap();
        assert_eq!(loaded.logging.level, LogLevel::Warn);
        assert_eq!(loaded.kick, Default::default());
    }

    #[test]
    fn kick_ignora_claves_desconocidas_y_lee_las_conocidas() {
        let dir = TempDir::new();
        fs::write(
            dir.file(),
            "[kick]\nenabled = true\nchannel = \"x\"\nchatroom_id = 7\nfutura = 1\n",
        )
        .unwrap();
        let loaded = TomlSettingsStore::new(dir.file()).load().unwrap();
        assert!(loaded.kick.enabled);
        assert_eq!(loaded.kick.channel, "x");
        assert_eq!(loaded.kick.channel_id, None);
        assert_eq!(loaded.kick.chatroom_id, Some(7));
    }

    #[test]
    fn un_tipo_incorrecto_en_kick_nombra_la_clave() {
        let dir = TempDir::new();
        fs::write(dir.file(), "[kick]\nenabled = \"si\"\n").unwrap();
        let error = TomlSettingsStore::new(dir.file()).load().unwrap_err();
        assert!(error.to_string().contains("kick.enabled"), "{error}");
    }

    #[test]
    fn las_claves_ausentes_toman_su_valor_por_defecto() {
        let dir = TempDir::new();
        fs::write(dir.file(), "").unwrap();
        let store = TomlSettingsStore::new(dir.file());
        assert_eq!(store.load().unwrap(), Settings::default());
    }

    fn assert_invalid_and_untouched(content: &[u8], expected_in_detail: &[&str]) {
        let dir = TempDir::new();
        fs::write(dir.file(), content).unwrap();
        let store = TomlSettingsStore::new(dir.file());
        let error = store.load().unwrap_err();
        match &error {
            SettingsError::Invalid { path, detail } => {
                assert_eq!(path, &dir.file());
                for part in expected_in_detail {
                    assert!(detail.contains(part), "falta {part:?} en: {detail}");
                }
            }
            other => panic!("se esperaba Invalid y llegó {other:?}"),
        }
        assert!(error.to_string().contains("settings.toml"));
        assert_eq!(
            fs::read(dir.file()).unwrap(),
            content,
            "el archivo no debe tocarse"
        );
    }

    #[test]
    fn un_tipo_incorrecto_es_invalid_con_la_clave_y_no_se_sobrescribe() {
        assert_invalid_and_untouched(b"[logging]\nlevel = 5\n", &["logging.level"]);
        assert_invalid_and_untouched(b"schema_version = \"uno\"\n", &["schema_version:"]);
    }

    #[test]
    fn un_nivel_desconocido_es_invalid_y_no_se_sobrescribe() {
        assert_invalid_and_untouched(
            b"[logging]\nlevel = \"ruidoso\"\n",
            &["logging.level", "ruidoso"],
        );
    }

    #[test]
    fn una_seccion_con_otro_tipo_nombra_la_clave_de_la_seccion() {
        assert_invalid_and_untouched(b"logging = 3\n", &["logging:"]);
    }

    #[test]
    fn un_toml_roto_es_invalid_y_no_se_sobrescribe() {
        assert_invalid_and_untouched(b"[logging\nlevel = ", &[]);
    }

    #[test]
    fn un_archivo_que_no_es_utf8_es_invalid_y_no_se_sobrescribe() {
        assert_invalid_and_untouched(&[0xff, 0xfe, 0x00, 0x41], &["UTF-8"]);
    }

    #[test]
    fn si_la_ruta_es_una_carpeta_load_devuelve_io() {
        let dir = TempDir::new();
        let store = TomlSettingsStore::new(&dir.0);
        assert!(matches!(store.load(), Err(SettingsError::Io { .. })));
    }

    #[test]
    fn si_el_guardado_falla_el_archivo_anterior_queda_intacto() {
        let dir = TempDir::new();
        let store = TomlSettingsStore::new(dir.file());
        store.save(&Settings::default()).unwrap();
        let before = fs::read(dir.file()).unwrap();
        // Una carpeta con el nombre del temporal impide crearlo y hace fallar el guardado.
        fs::create_dir(store.temp_path()).unwrap();
        let error = store.save(&debug_settings()).unwrap_err();
        assert!(matches!(error, SettingsError::Io { .. }));
        assert_eq!(fs::read(dir.file()).unwrap(), before);
        assert_eq!(store.load().unwrap(), Settings::default());
    }

    #[test]
    fn si_el_rename_falla_no_queda_el_temporal() {
        let dir = TempDir::new();
        // El destino es una carpeta no vacía: el rename del temporal sobre ella falla.
        fs::create_dir_all(dir.file().join("dentro")).unwrap();
        let store = TomlSettingsStore::new(dir.file());
        let error = store.save(&Settings::default()).unwrap_err();
        assert!(matches!(error, SettingsError::Io { .. }));
        assert!(!store.temp_path().exists());
        assert!(dir.file().join("dentro").is_dir());
    }

    #[test]
    fn un_segundo_guardado_reemplaza_el_contenido() {
        let dir = TempDir::new();
        let store = TomlSettingsStore::new(dir.file());
        store.save(&Settings::default()).unwrap();
        store.save(&debug_settings()).unwrap();
        assert_eq!(store.load().unwrap(), debug_settings());
    }

    #[test]
    fn varios_guardados_a_la_vez_no_fallan_ni_dejan_temporales() {
        let dir = TempDir::new();
        // Dos instancias sobre el mismo archivo, como harían dos partes de la aplicación.
        let stores = [
            TomlSettingsStore::new(dir.file()),
            TomlSettingsStore::new(dir.file()),
        ];
        for _ in 0..25 {
            let results: Vec<_> = std::thread::scope(|scope| {
                let handles: Vec<_> = (0..8)
                    .map(|n| {
                        let store = &stores[n % 2];
                        scope.spawn(move || {
                            let mut settings = Settings::default();
                            settings.logging.level = [LogLevel::Warn, LogLevel::Debug][n % 2];
                            store.save(&settings)
                        })
                    })
                    .collect();
                handles.into_iter().map(|h| h.join().unwrap()).collect()
            });
            assert!(results.iter().all(Result::is_ok), "{results:?}");
        }
        let level = stores[0].load().unwrap().logging.level;
        assert!(matches!(level, LogLevel::Warn | LogLevel::Debug));
        assert_eq!(fs::read_dir(&dir.0).unwrap().count(), 1);
    }

    #[test]
    fn el_adaptador_se_usa_como_trait_object() {
        let dir = TempDir::new();
        let store: Box<dyn SettingsStore> = Box::new(TomlSettingsStore::new(dir.file()));
        store.save(&debug_settings()).unwrap();
        assert_eq!(store.load().unwrap(), debug_settings());
    }

    #[test]
    fn no_guarda_si_el_archivo_es_de_un_esquema_mas_nuevo() {
        let dir = TempDir::new();
        let original = format!(
            "schema_version = {}\nfutura = true\n[logging]\nlevel = \"warn\"\n",
            SCHEMA_VERSION + 1
        );
        fs::write(dir.file(), &original).unwrap();
        let store = TomlSettingsStore::new(dir.file());
        let error = store.save(&Settings::default()).unwrap_err();
        assert!(matches!(error, SettingsError::Invalid { .. }), "{error}");
        assert!(error.to_string().contains("schema_version"), "{error}");
        assert_eq!(fs::read_to_string(dir.file()).unwrap(), original);
    }

    #[test]
    fn no_guarda_unos_ajustes_de_un_esquema_mas_nuevo_ni_crea_el_archivo() {
        let dir = TempDir::new();
        let store = TomlSettingsStore::new(dir.file());
        let settings = Settings {
            schema_version: SCHEMA_VERSION + 1,
            ..Settings::default()
        };
        assert!(matches!(
            store.save(&settings),
            Err(SettingsError::Invalid { .. })
        ));
        assert!(!dir.file().exists());
    }

    #[test]
    fn un_archivo_del_esquema_actual_o_anterior_si_se_guarda() {
        let dir = TempDir::new();
        fs::write(dir.file(), format!("schema_version = {SCHEMA_VERSION}\n")).unwrap();
        let store = TomlSettingsStore::new(dir.file());
        store.save(&debug_settings()).unwrap();
        assert_eq!(store.load().unwrap(), debug_settings());
        fs::write(dir.file(), "schema_version = 0\n").unwrap();
        store.save(&debug_settings()).unwrap();
    }
}
