//! Los comandos `run` y `login`. Devuelven el código de salida del proceso.

use crate::{credentials, events};
use std::{
    io::{self, Write},
    process::ExitCode,
};
use syon_domain::Platform;
use syon_engine::{AppPaths, Engine, LoginOutcome, PlatformStartError};

/// Versión que se muestra y que recibe el engine.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Capacidad de la cola del consumidor de la consola dentro del bus.
const CONSOLE_QUEUE: usize = 1024;

fn failure() -> ExitCode {
    ExitCode::from(1)
}

/// Arranca el engine. Un fallo se dice por `stderr`, porque puede no haber registro todavía.
fn start_engine() -> Result<Engine, ExitCode> {
    let paths = AppPaths::from_system().map_err(|error| {
        eprintln!("No se pudieron resolver las carpetas de SYON: {error}");
        failure()
    })?;
    Engine::start(paths, VERSION).map_err(|error| {
        eprintln!("No se pudo arrancar SYON: {error}");
        failure()
    })
}

fn platform_label(platform: Platform) -> &'static str {
    match platform {
        Platform::Kick => "Kick",
        Platform::Twitch => "Twitch",
    }
}

/// `syon` / `syon run`: conecta las plataformas activadas, imprime los eventos y espera a Ctrl+C.
pub fn run() -> ExitCode {
    let mut engine = match start_engine() {
        Ok(engine) => engine,
        Err(code) => return code,
    };
    // Se suscribe antes de arrancar los conectores para no perder los primeros eventos.
    let mut events_rx = engine.bus().subscribe("consola", CONSOLE_QUEUE);
    let handle = engine.runtime_handle();
    handle.spawn(async move {
        let mut stdout = io::stdout();
        while let Some(event) = events_rx.recv().await {
            // Si la salida se cierra (por ejemplo una tubería), se deja de imprimir sin entrar
            // en pánico.
            if writeln!(stdout, "{}", events::format_event(&event)).is_err() {
                break;
            }
        }
    });

    let started = engine.start_platforms(&credentials::all(), |platform, url| {
        println!(
            "{}: abre este enlace en el navegador y autoriza a SYON:\n\n{url}\n\nEsperando la autorización…",
            platform_label(platform)
        );
    });
    if started.is_empty() {
        eprintln!(
            "No hay plataformas activadas. Edita {} y añade, por ejemplo:\n\n[kick]\nenabled = true\nchannel = \"tu_canal\"\n\n[twitch]\nenabled = true",
            engine.paths().settings_file().display()
        );
        return failure();
    }
    let mut running = 0;
    for start in &started {
        let name = platform_label(start.platform);
        match &start.outcome {
            Ok(()) => {
                running += 1;
                println!("{name}: escuchando. Ctrl+C para salir.");
            }
            Err(PlatformStartError::MissingCredentials) => {
                // El engine no sabe de variables de entorno: el detalle lo da el binario.
                match credentials::for_platform(start.platform) {
                    Err(missing) => eprintln!("{name}: no arrancó: {missing}."),
                    Ok(_) => eprintln!("{name}: no arrancó: faltan credenciales."),
                }
            }
            Err(error) => eprintln!("{name}: no arrancó: {error}"),
        }
    }
    if running == 0 {
        return failure();
    }

    let waited = handle.block_on(tokio::signal::ctrl_c());
    if let Err(error) = &waited {
        eprintln!("No se pudo escuchar Ctrl+C: {error}");
    }
    engine.stop_platforms();
    if waited.is_ok() {
        ExitCode::SUCCESS
    } else {
        failure()
    }
}

/// `syon login <plataforma>`: deja lista la sesión, imprimiendo el enlace si hay que autorizar.
pub fn login(platform: Platform) -> ExitCode {
    let name = platform_label(platform);
    let app = match credentials::for_platform(platform) {
        Ok(app) => app,
        Err(missing) => {
            eprintln!("No se puede iniciar sesión en {name}: {missing}.");
            return failure();
        }
    };
    let engine = match start_engine() {
        Ok(engine) => engine,
        Err(code) => return code,
    };
    let result = engine.login(platform, &app, |url| {
        println!("Abre este enlace en el navegador y autoriza a SYON:\n\n{url}\n\nEsperando la autorización…");
    });
    match result {
        Ok(LoginOutcome::Restored) => println!("{name}: ya había una sesión vigente."),
        Ok(LoginOutcome::Refreshed) => println!("{name}: sesión renovada."),
        Ok(LoginOutcome::NewLogin) => println!("{name}: sesión iniciada y guardada."),
        Err(error) => {
            eprintln!("{name}: no se pudo iniciar sesión: {error}");
            return failure();
        }
    }
    ExitCode::SUCCESS
}
