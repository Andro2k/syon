//! Prueba de extremo a extremo del registro. Instala el logger y el hook de pánico, que son
//! globales del proceso, así que vive en su propio binario de pruebas y tiene una sola prueba:
//! dentro de `src/` afectaría a las demás pruebas del crate.

use std::{env, fs, thread};
use syon_app::LogLevel;
use syon_engine::{AppPaths, LoggingConfig, LoggingError, init_logging};

fn old_run(n: u32) -> String {
    format!(
        "2026-11-2{n}T10:00:00.000Z INFO  [syon::arranque] logging: syon 0.0.{n} iniciado\n\
         2026-11-2{n}T10:00:01.000Z INFO  [x] ejecucion antigua {n}\n"
    )
}

#[test]
fn el_registro_escribe_la_marca_redacta_conserva_cinco_ejecuciones_y_registra_el_panico() {
    let root = env::temp_dir().join(format!("syon-logging-{}", std::process::id()));
    let paths = AppPaths::under(&root);
    fs::create_dir_all(paths.data_dir()).unwrap();
    let history: String = (1..=6).map(old_run).collect();
    fs::write(paths.log_file(), history).unwrap();

    let config = LoggingConfig {
        level: LogLevel::Info,
        filter_override: None,
        app_version: "9.9.9".to_owned(),
    };
    init_logging(&paths, &config).unwrap();
    log::info!("kick: access_token=abc123 recibido");
    log::debug!("kick: detalle que no debe escribirse");
    let panicked = thread::Builder::new()
        .name("prueba-panico".to_owned())
        .spawn(|| panic!("boom con client_secret=s3cr3t"))
        .unwrap()
        .join();
    assert!(panicked.is_err());
    assert!(matches!(
        init_logging(&paths, &config),
        Err(LoggingError::AlreadyInitialized)
    ));

    let log = fs::read_to_string(paths.log_file()).unwrap();
    let markers = log
        .lines()
        .filter(|line| line.contains("[syon::arranque] logging: syon"))
        .count();
    assert_eq!(markers, 5, "4 ejecuciones anteriores y la actual:\n{log}");
    assert!(!log.contains("ejecucion antigua 1") && !log.contains("ejecucion antigua 2"));
    assert!(log.contains("ejecucion antigua 6"));
    assert!(log.contains("logging: syon 9.9.9 iniciado"));
    assert!(log.contains("access_token=[REDACTADO] recibido"));
    assert!(!log.contains("abc123") && !log.contains("s3cr3t"), "{log}");
    assert!(!log.contains("detalle que no debe escribirse"));
    assert!(
        log.contains("pánico: el hilo 'prueba-panico' falló en") && log.contains("boom con"),
        "{log}"
    );
    let _ = fs::remove_dir_all(&root);
}
