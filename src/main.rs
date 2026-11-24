mod api;
mod config;
mod console;
mod events;
mod input;
mod oauth;
mod realtime;

use anyhow::{Context, Result};
use tokio::sync::mpsc;

use api::KickApi;
use console::Console;
use oauth::{OAuthClient, Source, TokenManager, TokenStore};
use realtime::FeedItem;

const USAGE: &str = "\
Uso: cargo run -- <nombre-del-canal> [--raw] [--login]
     cargo run -- --login

Ejemplo: cargo run -- xqc

  --raw     muestra también el JSON original de cada evento
  --login   inicia sesión con OAuth (credenciales en .env); sin canal, solo guarda la sesión";

#[derive(Debug, PartialEq)]
struct Cli {
    channel: Option<String>,
    raw: bool,
    login: bool,
}

fn parse_args(args: impl Iterator<Item = String>) -> Result<Cli, String> {
    let mut channel = None;
    let mut raw = false;
    let mut login = false;
    for arg in args {
        match arg.as_str() {
            "--raw" => raw = true,
            "--login" => login = true,
            flag if flag.starts_with("--") => return Err(format!("Opción desconocida: {flag}")),
            _ if channel.is_none() => channel = Some(arg),
            _ => return Err(format!("Argumento inesperado: {arg}")),
        }
    }
    if channel.is_none() && !login {
        return Err("Falta el nombre del canal".to_string());
    }
    Ok(Cli {
        channel,
        raw,
        login,
    })
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = match parse_args(std::env::args().skip(1)) {
        Ok(cli) => cli,
        Err(message) => {
            eprintln!("{message}\n\n{USAGE}");
            std::process::exit(2);
        }
    };
    let console = Console::new(cli.raw);

    let http = reqwest::Client::builder()
        .user_agent("chat-server/0.1 (Rust learning project)")
        .build()?;

    let tokens = if cli.login {
        Some(authenticate(&http, &console).await?)
    } else {
        None
    };
    let Some(slug) = cli.channel else {
        return Ok(());
    };

    console.status(&format!("Buscando el chat de Kick/{slug}..."));
    let channel = match config::channel_override(&slug) {
        Some(channel) => channel,
        None => realtime::resolve_channel(&http, &slug)
            .await
            .context("No se pudo resolver el canal (si Kick devuelve 403, define KICK_CHANNEL_ID y KICK_CHATROOM_ID en .env)")?,
    };

    if let Some(tokens) = tokens {
        start_chat_input(KickApi::new(http.clone(), tokens), &slug, console).await;
    }

    let (tx, mut rx) = mpsc::channel(256);
    tokio::spawn(realtime::run(http, channel, tx));
    console.status("Escuchando (Ctrl+C para salir)");

    loop {
        tokio::select! {
            item = rx.recv() => match item {
                Some(FeedItem::Event { feed, published }) => console.event(&feed, &published),
                Some(FeedItem::Status(text)) => console.status(&text),
                None => break,
            },
            _ = tokio::signal::ctrl_c() => break,
        }
    }
    console.status("Hasta luego.");
    Ok(())
}

async fn authenticate(http: &reqwest::Client, console: &Console) -> Result<TokenManager> {
    let settings = config::Settings::from_env()?;
    let store = TokenStore::new(settings.token_file.clone());
    let oauth = OAuthClient::new(http.clone(), settings);
    let (manager, source) = TokenManager::start(oauth, store, |url| {
        console.status(&format!(
            "Abre este enlace en tu navegador para autorizar:\n{url}"
        ));
    })
    .await
    .context("No se pudo iniciar sesión con OAuth")?;

    let tokens = manager.snapshot().await;
    let origin = match source {
        Source::Restored => "restaurada",
        Source::Refreshed => "renovada",
        Source::NewLogin => "iniciada",
    };
    console.status(&format!(
        "Sesión OAuth {origin} (scopes: {}; expira en {} min)",
        tokens.scope,
        tokens.seconds_left(oauth::unix_now()) / 60
    ));
    Ok(manager)
}

/// Con sesión OAuth, lo que se escriba en la consola se envía al chat del canal.
/// Si algo falla, se sigue escuchando en modo solo lectura.
async fn start_chat_input(api: KickApi, slug: &str, console: Console) {
    let setup = async { Ok::<_, anyhow::Error>((api.me().await?, api.channel(slug).await?)) }.await;
    match setup {
        Ok((me, channel)) => {
            let state = if channel.is_live {
                "en vivo"
            } else {
                "sin directo"
            };
            console.status(&format!(
                "Autenticado como {} en Kick/{} ({state}). Escribe y pulsa Enter para enviar al chat.",
                me.name, channel.slug
            ));
            tokio::spawn(input::forward_stdin(
                api,
                channel.broadcaster_user_id,
                console,
            ));
        }
        Err(error) => console.status(&format!("Envío de chat desactivado: {error:#}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> impl Iterator<Item = String> {
        list.iter()
            .map(|s| s.to_string())
            .collect::<Vec<_>>()
            .into_iter()
    }

    fn cli(channel: Option<&str>, raw: bool, login: bool) -> Cli {
        Cli {
            channel: channel.map(str::to_string),
            raw,
            login,
        }
    }

    #[test]
    fn parsea_canal_y_raw_en_cualquier_orden() {
        assert_eq!(
            parse_args(args(&["xqc", "--raw"])),
            Ok(cli(Some("xqc"), true, false))
        );
        assert_eq!(
            parse_args(args(&["--raw", "xqc"])),
            Ok(cli(Some("xqc"), true, false))
        );
        assert_eq!(
            parse_args(args(&["xqc"])),
            Ok(cli(Some("xqc"), false, false))
        );
    }

    #[test]
    fn login_puede_ir_solo_o_con_canal() {
        assert_eq!(parse_args(args(&["--login"])), Ok(cli(None, false, true)));
        assert_eq!(
            parse_args(args(&["xqc", "--login"])),
            Ok(cli(Some("xqc"), false, true))
        );
    }

    #[test]
    fn rechaza_entradas_invalidas() {
        assert!(parse_args(args(&[])).is_err());
        assert!(parse_args(args(&["--raw"])).is_err());
        assert!(parse_args(args(&["a", "b"])).is_err());
        assert!(parse_args(args(&["a", "--nope"])).is_err());
    }
}
