//! El engine arranca el conector de Kick activado en `settings.toml`, lo vigila y lo detiene sin
//! esperar a la red. Vive en su propio binario porque `Engine::start` instala el registro global
//! del proceso.

use std::{env, fs};
use syon_app::ConnectionStatus;
use syon_domain::Platform;
use syon_engine::{AppPaths, Engine, PlatformCredentials, PlatformStartError};

#[test]
fn kick_activado_se_registra_y_se_detiene_sin_esperar_a_la_red() {
    let root = env::temp_dir().join(format!("syon-engine-platforms-{}", std::process::id()));
    let paths = AppPaths::under(&root);
    fs::create_dir_all(paths.config_dir()).unwrap();
    let text = "[kick]\nenabled = true\nchannel = \"mi_canal\"\nchannel_id = 1\nchatroom_id = 2\n";
    fs::write(paths.settings_file(), text).unwrap();

    let mut engine = match Engine::start(paths, "1.2.3") {
        Ok(engine) => engine,
        Err(error) => panic!("el engine debía arrancar: {error}"),
    };
    assert!(engine.connection_status().is_empty());

    let started = engine.start_platforms(&PlatformCredentials::default(), |_, _| {
        panic!("Kick no pide un inicio de sesión")
    });
    assert_eq!(started.len(), 1);
    assert_eq!(started[0].platform, Platform::Kick);
    assert!(started[0].outcome.is_ok(), "{:?}", started[0].outcome);
    let status = engine.connection_status();
    assert_eq!(status.len(), 1);
    assert_eq!(status[0].0, Platform::Kick);
    assert_ne!(status[0].1, ConnectionStatus::Disconnected);

    let again = engine.start_platforms(&PlatformCredentials::default(), |_, _| {
        panic!("Kick no pide un inicio de sesión")
    });
    assert_eq!(again[0].outcome, Err(PlatformStartError::AlreadyRunning));
    assert_eq!(engine.connection_status().len(), 1);

    engine.stop_platforms();
    assert_eq!(
        engine.connection_status(),
        [(Platform::Kick, ConnectionStatus::Disconnected)]
    );
    engine.stop_platforms();
    let _ = fs::remove_dir_all(&root);
}
