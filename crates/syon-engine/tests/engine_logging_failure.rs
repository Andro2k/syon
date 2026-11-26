//! Si el registro no se puede instalar, `Engine::start` falla sin haber cambiado nada: el archivo
//! de ajustes inválido sigue donde estaba y no aparece ningún apartado (ADR-0010). Va en su propio
//! binario como las demás pruebas de arranque.

use std::{env, fs};
use syon_engine::{AppPaths, Engine, LoggingError, StartError};

#[test]
fn si_falla_el_registro_el_archivo_de_ajustes_invalido_no_se_mueve() {
    let root = env::temp_dir().join(format!("syon-engine-logfail-{}", std::process::id()));
    let paths = AppPaths::under(&root);
    fs::create_dir_all(paths.config_dir()).unwrap();
    let broken = "[logging]\nlevel = 5\n";
    fs::write(paths.settings_file(), broken).unwrap();
    // Un archivo con el nombre de la carpeta de datos impide crear `syon.log`.
    fs::write(paths.data_dir(), "no soy una carpeta").unwrap();

    let result = Engine::start(paths.clone(), "1.0.0");

    assert!(matches!(
        result.err(),
        Some(StartError::Logging(LoggingError::File { .. }))
    ));
    assert_eq!(fs::read_to_string(paths.settings_file()).unwrap(), broken);
    assert!(!paths.config_dir().join("settings.toml.invalid").exists());
    let _ = fs::remove_dir_all(&root);
}
