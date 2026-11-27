//! El binario compilado: versión, ayuda, uso incorrecto y arranque sin plataformas activadas.

use std::process::{Command, Output};

fn syon(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_syon"))
        .args(args)
        .output()
        .expect("no se pudo ejecutar syon")
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

#[test]
fn la_version_sale_por_stdout_con_codigo_0() {
    let output = syon(&["--version"]);
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(
        text(&output.stdout).trim(),
        format!("syon {}", env!("CARGO_PKG_VERSION"))
    );
}

#[test]
fn la_ayuda_describe_los_comandos() {
    let output = syon(&["--help"]);
    assert_eq!(output.status.code(), Some(0));
    let out = text(&output.stdout);
    assert!(out.contains("syon login"), "{out}");
}

#[test]
fn un_argumento_desconocido_sale_con_2_y_el_uso_por_stderr() {
    let output = syon(&["volar"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(text(&output.stdout).is_empty());
    assert!(text(&output.stderr).contains("Uso:"));
}

#[test]
fn login_sin_plataforma_sale_con_2() {
    assert_eq!(syon(&["login"]).status.code(), Some(2));
}

/// Sin credenciales en el entorno ni incrustadas, `login` lo dice y nombra las variables, no su
/// valor. Sale antes de arrancar el engine, así que no toca ninguna carpeta del sistema.
#[test]
fn login_sin_credenciales_nombra_las_variables_y_sale_con_1() {
    let output = Command::new(env!("CARGO_BIN_EXE_syon"))
        .args(["login", "kick"])
        .env_remove("SYON_KICK_CLIENT_ID")
        .env_remove("SYON_KICK_CLIENT_SECRET")
        .output()
        .unwrap();
    // Si esta compilación incrusta credenciales, el caso no aplica.
    if option_env!("SYON_KICK_CLIENT_ID").is_some() {
        return;
    }
    assert_eq!(output.status.code(), Some(1));
    let err = text(&output.stderr);
    assert!(err.contains("SYON_KICK_CLIENT_ID"), "{err}");
    assert!(err.contains("SYON_KICK_CLIENT_SECRET"), "{err}");
}

/// Con carpetas vacías no hay plataformas activadas: dice qué archivo editar y sale con 1. Solo
/// en sistemas donde las carpetas se pueden redirigir con variables de entorno (XDG).
#[cfg(unix)]
#[test]
fn run_sin_plataformas_activadas_dice_que_archivo_editar_y_sale_con_1() {
    let root = std::env::temp_dir().join(format!("syon-bin-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let output = Command::new(env!("CARGO_BIN_EXE_syon"))
        .arg("run")
        .env("HOME", &root)
        .env("XDG_CONFIG_HOME", root.join("config"))
        .env("XDG_DATA_HOME", root.join("data"))
        .env("XDG_CACHE_HOME", root.join("cache"))
        .output()
        .unwrap();
    let err = text(&output.stderr);
    let _ = std::fs::remove_dir_all(&root);
    assert_eq!(output.status.code(), Some(1), "{err}");
    assert!(err.contains("settings.toml"), "{err}");
    assert!(err.contains(root.to_str().unwrap()), "{err}");
}

/// Twitch activado y sin credenciales de la aplicación: no arranca, nombra las variables (no sus
/// valores) y, al no haber nada en marcha, sale con 1. Solo en sistemas con carpetas XDG.
#[cfg(unix)]
#[test]
fn run_con_twitch_sin_credenciales_nombra_las_variables_y_sale_con_1() {
    // Si esta compilación incrusta credenciales de Twitch, el caso no aplica.
    if option_env!("SYON_TWITCH_CLIENT_ID").is_some() {
        return;
    }
    let root = std::env::temp_dir().join(format!("syon-bin-twitch-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("config/syon")).unwrap();
    std::fs::write(
        root.join("config/syon/settings.toml"),
        "[twitch]\nenabled = true\n",
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_syon"))
        .arg("run")
        .env("HOME", &root)
        .env("XDG_CONFIG_HOME", root.join("config"))
        .env("XDG_DATA_HOME", root.join("data"))
        .env("XDG_CACHE_HOME", root.join("cache"))
        .env_remove("SYON_TWITCH_CLIENT_ID")
        .env_remove("SYON_TWITCH_CLIENT_SECRET")
        .output()
        .unwrap();
    let err = text(&output.stderr);
    let _ = std::fs::remove_dir_all(&root);
    assert_eq!(output.status.code(), Some(1), "{err}");
    assert!(err.contains("SYON_TWITCH_CLIENT_ID"), "{err}");
    assert!(err.contains("SYON_TWITCH_CLIENT_SECRET"), "{err}");
}
