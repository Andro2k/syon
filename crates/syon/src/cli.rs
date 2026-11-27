use syon_domain::Platform;

/// Lo que la persona pidió en la línea de comandos.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    /// Arrancar las plataformas activadas y mostrar los eventos.
    Run,
    /// Dejar lista la sesión de una plataforma.
    Login(Platform),
    /// Imprimir la versión.
    Version,
    /// Imprimir el uso.
    Help,
}

/// Texto de uso, también el de `--help`.
pub const USAGE: &str = "\
Uso:
  syon [run]              arranca las plataformas activadas y muestra los eventos
  syon login <kick|twitch>  deja lista la sesión de la plataforma
  syon --version          muestra la versión
  syon --help             muestra este texto";

/// Interpreta los argumentos (sin el nombre del programa). El error es un mensaje para la persona.
pub fn parse<I, S>(args: I) -> Result<Command, String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let args: Vec<String> = args.into_iter().map(|a| a.as_ref().to_owned()).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    match args.as_slice() {
        [] | ["run"] => Ok(Command::Run),
        ["--version" | "-V"] => Ok(Command::Version),
        ["--help" | "-h" | "help"] => Ok(Command::Help),
        ["login", platform] => parse_platform(platform).map(Command::Login),
        ["login"] => Err("falta la plataforma: syon login <kick|twitch>".to_owned()),
        [other, ..] => Err(format!("argumento no reconocido: {}", show(other))),
    }
}

fn parse_platform(text: &str) -> Result<Platform, String> {
    match text.to_ascii_lowercase().as_str() {
        "kick" => Ok(Platform::Kick),
        "twitch" => Ok(Platform::Twitch),
        _ => Err(format!(
            "plataforma desconocida: {} (usa kick o twitch)",
            show(text)
        )),
    }
}

/// Texto de la persona para un mensaje de error, sin caracteres de control.
fn show(text: &str) -> String {
    text.chars().filter(|c| !c.is_control()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sin_argumentos_o_con_run_se_ejecuta() {
        assert_eq!(parse::<_, &str>([]), Ok(Command::Run));
        assert_eq!(parse(["run"]), Ok(Command::Run));
    }

    #[test]
    fn version_y_ayuda() {
        assert_eq!(parse(["--version"]), Ok(Command::Version));
        assert_eq!(parse(["-V"]), Ok(Command::Version));
        assert_eq!(parse(["--help"]), Ok(Command::Help));
        assert_eq!(parse(["-h"]), Ok(Command::Help));
    }

    #[test]
    fn login_pide_una_plataforma_conocida() {
        assert_eq!(parse(["login", "kick"]), Ok(Command::Login(Platform::Kick)));
        assert_eq!(
            parse(["login", "Twitch"]),
            Ok(Command::Login(Platform::Twitch))
        );
        assert!(parse(["login"]).unwrap_err().contains("falta"));
        assert!(parse(["login", "youtube"]).unwrap_err().contains("youtube"));
    }

    #[test]
    fn lo_desconocido_o_sobrante_es_un_error_de_uso() {
        assert!(parse(["volar"]).is_err());
        assert!(parse(["run", "extra"]).is_err());
        assert!(parse(["login", "kick", "extra"]).is_err());
    }

    #[test]
    fn el_error_no_repite_caracteres_de_control() {
        let error = parse(["\x1b[31mrojo"]).unwrap_err();
        assert!(!error.contains('\x1b'), "{error:?}");
    }
}
