//! `Engine::login` reutiliza una sesión guardada y vigente sin pedir el navegador, y rechaza las
//! plataformas que todavía no sabe iniciar. Vive en su propio binario porque `Engine::start`
//! instala el registro global del proceso. No usa la red.

use std::{env, fs};
use syon_app::{CredentialStore, Credentials};
use syon_domain::{Platform, Timestamp};
use syon_engine::{AppCredentials, AppPaths, Engine, LoginError, LoginOutcome};
use syon_storage_credentials::TomlCredentialStore;

/// Milisegundos de un año: la sesión de la prueba no vence mientras corre.
const YEAR_MS: i64 = 365 * 24 * 60 * 60 * 1000;

#[test]
fn una_sesion_guardada_y_vigente_se_reutiliza_sin_pedir_el_enlace() {
    let root = env::temp_dir().join(format!("syon-engine-login-{}", std::process::id()));
    let paths = AppPaths::under(&root);
    let engine = match Engine::start(paths.clone(), "1.2.3") {
        Ok(engine) => engine,
        Err(error) => panic!("el engine debía arrancar: {error}"),
    };
    let store = TomlCredentialStore::new(paths.credentials_file());
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64;
    store
        .save(
            Platform::Kick,
            &Credentials {
                access_token: "token-de-acceso".to_owned(),
                refresh_token: Some("token-de-renovacion".to_owned()),
                expires_at: Some(Timestamp::from_unix_ms(now_ms + YEAR_MS)),
                scopes: Vec::new(),
            },
        )
        .unwrap();
    let app = AppCredentials {
        client_id: "cid".to_owned(),
        client_secret: "secreto-de-la-app".to_owned(),
    };

    let outcome = engine.login(Platform::Kick, &app, |_| {
        panic!("no debía pedir un inicio de sesión nuevo")
    });
    assert_eq!(outcome, Ok(LoginOutcome::Restored));

    let unsupported = engine.login(Platform::Twitch, &app, |_| panic!("no debía llamarse"));
    assert_eq!(unsupported, Err(LoginError::Unsupported(Platform::Twitch)));

    let log = fs::read_to_string(paths.log_file()).unwrap();
    assert!(log.contains("sesión de Kick lista"), "{log}");
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
