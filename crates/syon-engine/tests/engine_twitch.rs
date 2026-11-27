//! Twitch activado en `settings.toml`: sin las credenciales de la aplicación no arranca y lo dice;
//! con una sesión guardada y vigente se registra sin pedir el navegador y se detiene sin esperar a
//! la red. Vive en su propio binario porque `Engine::start` instala el registro global del proceso.

use std::{env, fs, time::SystemTime};
use syon_app::{ConnectionStatus, CredentialStore, Credentials};
use syon_domain::{Platform, Timestamp};
use syon_engine::{AppCredentials, AppPaths, Engine, PlatformCredentials, PlatformStartError};
use syon_platform_twitch::DEFAULT_SCOPES;
use syon_storage_credentials::TomlCredentialStore;

/// Milisegundos de un año: la sesión de la prueba no vence mientras corre.
const YEAR_MS: i64 = 365 * 24 * 60 * 60 * 1000;

#[test]
fn twitch_sin_credenciales_no_arranca_y_con_sesion_guardada_se_registra() {
    let root = env::temp_dir().join(format!("syon-engine-twitch-{}", std::process::id()));
    let paths = AppPaths::under(&root);
    fs::create_dir_all(paths.config_dir()).unwrap();
    fs::write(paths.settings_file(), "[twitch]\nenabled = true\n").unwrap();
    let mut engine = match Engine::start(paths.clone(), "1.2.3") {
        Ok(engine) => engine,
        Err(error) => panic!("el engine debía arrancar: {error}"),
    };
    let no_url = |_: Platform, _: &str| panic!("no debía pedir un inicio de sesión nuevo");

    // Sin las credenciales de la aplicación: no arranca, lo dice y no registra nada.
    let started = engine.start_platforms(&PlatformCredentials::default(), no_url);
    assert_eq!(started.len(), 1);
    assert_eq!(started[0].platform, Platform::Twitch);
    assert_eq!(
        started[0].outcome,
        Err(PlatformStartError::MissingCredentials)
    );
    assert!(engine.connection_status().is_empty());

    // Con una sesión guardada que cubre los permisos y no vence: se reutiliza.
    let now_ms = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64;
    TomlCredentialStore::new(paths.credentials_file())
        .save(
            Platform::Twitch,
            &Credentials {
                access_token: "token-de-acceso".to_owned(),
                refresh_token: Some("token-de-renovacion".to_owned()),
                expires_at: Some(Timestamp::from_unix_ms(now_ms + YEAR_MS)),
                scopes: DEFAULT_SCOPES.iter().map(|s| (*s).to_owned()).collect(),
            },
        )
        .unwrap();
    let credentials = PlatformCredentials {
        kick: None,
        twitch: Some(AppCredentials {
            client_id: "cid".to_owned(),
            client_secret: "secreto-de-la-app".to_owned(),
        }),
    };
    let started = engine.start_platforms(&credentials, no_url);
    assert!(started[0].outcome.is_ok(), "{:?}", started[0].outcome);
    let status = engine.connection_status();
    assert_eq!(status.len(), 1);
    assert_eq!(status[0].0, Platform::Twitch);
    assert_ne!(status[0].1, ConnectionStatus::Disconnected);

    let again = engine.start_platforms(&credentials, no_url);
    assert_eq!(again[0].outcome, Err(PlatformStartError::AlreadyRunning));

    engine.stop_platforms();
    assert_eq!(
        engine.connection_status(),
        [(Platform::Twitch, ConnectionStatus::Disconnected)]
    );

    let log = fs::read_to_string(paths.log_file()).unwrap();
    assert!(log.contains("sesión de Twitch lista"), "{log}");
    for secret in [
        "token-de-acceso",
        "token-de-renovacion",
        "secreto-de-la-app",
    ] {
        assert!(
            !log.contains(secret),
            "el registro no debe incluir {secret}"
        );
    }
    let _ = fs::remove_dir_all(&root);
}
