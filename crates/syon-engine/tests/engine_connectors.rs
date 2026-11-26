//! El engine ofrece su runtime a los adaptadores, vigila sus conectores y los detiene al cerrarse.
//! Vive en su propio binario porque `Engine::start` instala el registro global del proceso.

use std::{
    env, fs,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
use syon_app::{ConnectionStatus, PlatformConnector};
use syon_domain::Platform;
use syon_engine::{AppPaths, Engine};

/// Conector de prueba que anota si lo detuvieron.
struct Fake(Arc<AtomicBool>);

impl PlatformConnector for Fake {
    fn platform(&self) -> Platform {
        Platform::Kick
    }

    fn status(&self) -> ConnectionStatus {
        if self.0.load(Ordering::SeqCst) {
            ConnectionStatus::Disconnected
        } else {
            ConnectionStatus::Connected
        }
    }

    fn stop(&self) {
        self.0.store(true, Ordering::SeqCst);
    }
}

#[test]
fn el_runtime_corre_tareas_y_cerrar_el_engine_detiene_los_conectores() {
    let root = env::temp_dir().join(format!("syon-engine-connectors-{}", std::process::id()));
    let paths = AppPaths::under(&root);
    let mut engine = match Engine::start(paths, "1.2.3") {
        Ok(engine) => engine,
        Err(error) => panic!("el engine debía arrancar: {error}"),
    };

    let value = engine
        .runtime_handle()
        .block_on(async { tokio::spawn(async { 40 + 2 }).await.unwrap() });
    assert_eq!(value, 42);

    let stopped = Arc::new(AtomicBool::new(false));
    engine.add_connector(Box::new(Fake(Arc::clone(&stopped))));
    assert_eq!(
        engine.connection_status(),
        [(Platform::Kick, ConnectionStatus::Connected)]
    );

    drop(engine);
    assert!(
        stopped.load(Ordering::SeqCst),
        "el conector debía detenerse"
    );
    let _ = fs::remove_dir_all(&root);
}
