//! Sin sesión guardada, Kick arranca para leer pero enviar devuelve `NotLoggedIn` y nunca abre
//! el navegador. Vive en su propio binario porque `Engine::start` instala el registro global.

use std::{env, fs};
use syon_app::SendError;
use syon_domain::Platform;
use syon_engine::{AppCredentials, AppPaths, Engine, PlatformCredentials};

#[test]
fn kick_sin_sesion_devuelve_not_logged_in_y_no_abre_el_navegador() {
    let root = env::temp_dir().join(format!("syon-engine-chat-{}", std::process::id()));
    let paths = AppPaths::under(&root);
    fs::create_dir_all(paths.config_dir()).unwrap();
    let text = "[kick]\nenabled = true\nchannel = \"mi_canal\"\nchannel_id = 1\nchatroom_id = 2\n";
    fs::write(paths.settings_file(), text).unwrap();

    let mut engine = Engine::start(paths, "1.2.3").unwrap();
    let credentials = PlatformCredentials {
        kick: Some(AppCredentials {
            client_id: "cid".into(),
            client_secret: "sec".into(),
        }),
        twitch: None,
    };
    let started = engine.start_platforms(&credentials, |_, _| {
        panic!("enviar ni arrancar Kick deben abrir el navegador")
    });
    assert_eq!(started.len(), 1);
    assert!(started[0].outcome.is_ok());
    assert!(engine.chat_platforms().is_empty());

    let handle = engine.chat_handle();
    let results = handle.send(&[Platform::Kick, Platform::Twitch], "hola");
    assert_eq!(results.len(), 2);
    assert_eq!(results[0].platform, Platform::Kick);
    assert_eq!(results[0].outcome, Err(SendError::NotLoggedIn));
    assert_eq!(results[1].platform, Platform::Twitch);
    assert_eq!(results[1].outcome, Err(SendError::NotRunning));

    engine.stop_platforms();
    let _ = fs::remove_dir_all(&root);
}
