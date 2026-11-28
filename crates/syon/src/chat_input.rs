use std::{
    io::{self, BufRead},
    thread,
};
use syon_domain::Platform;
use syon_engine::{ChatHandle, SendResult};

/// Línea de consola ya interpretada.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedLine<'a> {
    /// Plataformas indicadas con `@kick` o `@twitch`. Vacío significa «todas las disponibles».
    pub platforms: Vec<Platform>,
    /// Texto del mensaje tras quitar los prefijos iniciales y espacios sobrantes.
    pub text: &'a str,
}

/// Interpreta una línea de entrada buscando prefijos `@kick` y `@twitch` al principio. Devuelve
/// `None` si la línea está vacía o solo contiene prefijos.
pub fn parse_line(line: &str) -> Option<ParsedLine<'_>> {
    let mut rest = line.trim();
    let mut platforms = Vec::new();
    loop {
        let Some((token, after)) = split_first_word(rest) else {
            break;
        };
        let platform = if token.eq_ignore_ascii_case("@kick") {
            Some(Platform::Kick)
        } else if token.eq_ignore_ascii_case("@twitch") {
            Some(Platform::Twitch)
        } else {
            None
        };
        let Some(platform) = platform else {
            break;
        };
        if !platforms.contains(&platform) {
            platforms.push(platform);
        }
        rest = after.trim_start();
    }
    if rest.is_empty() {
        return None;
    }
    Some(ParsedLine {
        platforms,
        text: rest,
    })
}

/// Lanza un hilo dedicado que lee líneas de `stdin` y las envía con `chat` hasta que la entrada
/// se cierre. No se espera al hilo al salir de `run`.
pub fn spawn_reader(chat: ChatHandle, available: Vec<Platform>) {
    thread::spawn(move || {
        let stdin = io::stdin();
        for line in stdin.lock().lines() {
            let Ok(line) = line else {
                break;
            };
            let Some(parsed) = parse_line(&line) else {
                continue;
            };
            let targets = if parsed.platforms.is_empty() {
                &available
            } else {
                &parsed.platforms
            };
            if targets.is_empty() {
                eprintln!(
                    "No hay plataformas con envío disponible (inicia sesión con `syon login`)."
                );
                continue;
            }
            for result in chat.send(targets, parsed.text) {
                print_result(&result);
            }
        }
    });
}

fn print_result(result: &SendResult) {
    let label = match result.platform {
        Platform::Kick => "Kick",
        Platform::Twitch => "Twitch",
    };
    match &result.outcome {
        Ok(_) => println!("{label}: enviado."),
        Err(error) => eprintln!("{label}: no se pudo enviar: {error}"),
    }
}

fn split_first_word(input: &str) -> Option<(&str, &str)> {
    if input.is_empty() {
        return None;
    }
    match input.find(char::is_whitespace) {
        Some(idx) => Some((&input[..idx], &input[idx..])),
        None => Some((input, "")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sin_prefijo_deja_plataformas_vacias_y_conserva_el_texto() {
        assert_eq!(
            parse_line("  hola a todos  "),
            Some(ParsedLine {
                platforms: vec![],
                text: "hola a todos",
            })
        );
    }

    #[test]
    fn reconoce_uno_o_varios_prefijos_al_inicio_sin_repetir() {
        assert_eq!(
            parse_line("@kick hola kick"),
            Some(ParsedLine {
                platforms: vec![Platform::Kick],
                text: "hola kick",
            })
        );
        assert_eq!(
            parse_line("@twitch @kick @twitch  hola ambos"),
            Some(ParsedLine {
                platforms: vec![Platform::Twitch, Platform::Kick],
                text: "hola ambos",
            })
        );
    }

    #[test]
    fn ignora_lineas_vacias_o_solo_con_prefijos() {
        assert_eq!(parse_line(""), None);
        assert_eq!(parse_line("   \t  "), None);
        assert_eq!(parse_line("@kick"), None);
        assert_eq!(parse_line("  @kick @twitch  "), None);
    }

    #[test]
    fn no_confunde_menciones_en_medio_del_mensaje() {
        assert_eq!(
            parse_line("hola @kick como vas"),
            Some(ParsedLine {
                platforms: vec![],
                text: "hola @kick como vas",
            })
        );
    }
}
