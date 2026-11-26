//! Arranque del engine con un `settings.toml` inválido. `Engine::start` instala el registro y el
//! hook de pánico del proceso, así que este archivo tiene una sola prueba y su propio binario;
//! `engine_settings.rs` cubre el camino con ajustes válidos.

use std::{env, fs};
use syon_app::Settings;
use syon_engine::{AppPaths, Engine, LoggingError, StartError};

#[test]
fn un_archivo_de_ajustes_invalido_se_aparta_y_el_engine_arranca_con_los_valores_por_defecto() {
    let root = env::temp_dir().join(format!("syon-engine-recovery-{}", std::process::id()));
    let paths = AppPaths::under(&root);
    fs::create_dir_all(paths.config_dir()).unwrap();
    let broken = "[logging]\nlevel = 5\n";
    fs::write(paths.settings_file(), broken).unwrap();

    let engine = match Engine::start(paths.clone(), "9.9.9") {
        Ok(engine) => engine,
        Err(error) => panic!("el engine debía arrancar: {error}"),
    };

    assert_eq!(engine.settings(), &Settings::default());
    assert_eq!(engine.paths(), &paths);
    let _consumer = engine.bus().subscribe("prueba", 4);
    assert_eq!(engine.bus().stats().len(), 1);

    // El archivo roto se apartó y no se guardó nada encima.
    assert!(!paths.settings_file().exists());
    let apartado = paths.config_dir().join("settings.toml.invalid");
    assert_eq!(fs::read_to_string(apartado).unwrap(), broken);

    // El registro está instalado con el nivel por defecto y deja constancia del aviso.
    let log = fs::read_to_string(paths.log_file()).unwrap();
    assert!(log.contains("logging: syon 9.9.9 iniciado"), "{log}");
    assert!(log.contains("WARN"), "{log}");
    assert!(log.contains("engine: ajustes inválidos en"), "{log}");
    assert!(log.contains("logging.level"), "{log}");
    assert!(log.contains("se movió el archivo a"), "{log}");
    assert!(log.contains("engine: listo"), "{log}");

    // El registro es global: un segundo arranque en el mismo proceso falla sin tocar nada.
    assert!(matches!(
        Engine::start(paths, "9.9.9").err(),
        Some(StartError::Logging(LoggingError::AlreadyInitialized))
    ));
    let _ = fs::remove_dir_all(&root);
}
