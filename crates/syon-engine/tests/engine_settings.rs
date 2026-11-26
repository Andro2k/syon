//! Arranque del engine con un `settings.toml` válido: el nivel de los ajustes llega al registro.
//! Vive en su propio binario porque `Engine::start` instala el registro global del proceso. Lee
//! `SYON_LOG` del entorno, que no debe estar definida al ejecutar las pruebas.

use std::{env, fs};
use syon_app::LogLevel;
use syon_engine::{AppPaths, Engine};

#[test]
fn el_nivel_de_los_ajustes_llega_al_registro_y_no_se_aparta_nada() {
    let root = env::temp_dir().join(format!("syon-engine-settings-{}", std::process::id()));
    let paths = AppPaths::under(&root);
    fs::create_dir_all(paths.config_dir()).unwrap();
    let text = "schema_version = 1\n\n[logging]\nlevel = \"debug\"\n";
    fs::write(paths.settings_file(), text).unwrap();

    let engine = match Engine::start(paths.clone(), "1.2.3") {
        Ok(engine) => engine,
        Err(error) => panic!("el engine debía arrancar: {error}"),
    };
    assert_eq!(engine.settings().logging.level, LogLevel::Debug);
    log::debug!("kick: detalle visible con nivel debug");

    let log = fs::read_to_string(paths.log_file()).unwrap();
    assert!(log.contains("logging: syon 1.2.3 iniciado"), "{log}");
    assert!(
        log.contains("kick: detalle visible con nivel debug"),
        "{log}"
    );
    assert!(!log.contains("WARN"), "no debe haber avisos: {log}");
    assert_eq!(fs::read_to_string(paths.settings_file()).unwrap(), text);
    assert!(!paths.config_dir().join("settings.toml.invalid").exists());
    let _ = fs::remove_dir_all(&root);
}
