//! Presentación en consola: formatea eventos y estados. No interpreta ni envía nada.

use chrono::Local;
use serde_json::Value;

use crate::events::{KickEvent, Published};

const RESET: &str = "\x1b[0m";
const DIM: &str = "\x1b[2m";
const RED: &str = "\x1b[31m";
const GREEN: &str = "\x1b[32m";
const YELLOW: &str = "\x1b[33m";
const MAGENTA: &str = "\x1b[35m";
const CYAN: &str = "\x1b[36m";

const OTHER_PREVIEW_CHARS: usize = 160;

#[derive(Clone, Copy)]
pub struct Console {
    color: bool,
    raw: bool,
}

impl Console {
    /// Los colores se desactivan con la variable estándar `NO_COLOR`.
    pub fn new(raw: bool) -> Self {
        Self {
            color: std::env::var_os("NO_COLOR").is_none(),
            raw,
        }
    }

    pub fn event(&self, feed: &str, published: &Published) {
        println!("{} {}", self.stamp(), self.format_event(&published.event));
        if self.raw {
            let payload = compact(&published.payload, usize::MAX);
            println!(
                "{}",
                self.paint(
                    DIM,
                    &format!("         {feed} {} {payload}", published.name)
                )
            );
        }
    }

    pub fn status(&self, text: &str) {
        println!(
            "{} {}",
            self.stamp(),
            self.paint(DIM, &format!("--     {text}"))
        );
    }

    pub fn format_event(&self, event: &KickEvent) -> String {
        match event {
            KickEvent::Chat { user, content } => self.line(
                RESET,
                "CHAT",
                &format!("{user}: {}", simplify_emotes(content)),
            ),
            KickEvent::MessageDeleted { message_id } => {
                self.line(DIM, "DEL", &format!("mensaje {message_id} eliminado"))
            }
            KickEvent::UserBanned {
                user,
                banned_by,
                permanent,
            } => {
                let by = banned_by
                    .as_deref()
                    .map(|b| format!(" por {b}"))
                    .unwrap_or_default();
                let kind = if *permanent {
                    "ban permanente"
                } else {
                    "timeout"
                };
                self.line(RED, "BAN", &format!("{user}: {kind}{by}"))
            }
            KickEvent::Subscription { user, months } => self.line(
                MAGENTA,
                "SUB",
                &format!("{user} se suscribió ({months} meses)"),
            ),
            KickEvent::GiftedSubs { gifter, count } => {
                self.line(MAGENTA, "GIFT", &format!("{gifter} regaló {count} subs"))
            }
            KickEvent::Reward { user, title, input } => {
                let extra = input
                    .as_deref()
                    .map(|i| format!(" - \"{i}\""))
                    .unwrap_or_default();
                self.line(CYAN, "REWARD", &format!("{user} canjeó {title}{extra}"))
            }
            KickEvent::Followers { count } => {
                self.line(GREEN, "FOLLOW", &format!("seguidores: {count}"))
            }
            KickEvent::StreamLive { title } => {
                let title = title.as_deref().unwrap_or("sin título");
                self.line(GREEN, "LIVE", &format!("directo iniciado: {title}"))
            }
            KickEvent::StreamOffline => self.line(YELLOW, "OFFLINE", "directo terminado"),
            KickEvent::Other { name, data } => self.line(
                YELLOW,
                "OTHER",
                &format!("{name} {}", compact(data, OTHER_PREVIEW_CHARS)),
            ),
        }
    }

    fn line(&self, color: &str, tag: &str, body: &str) -> String {
        format!("{}{body}", self.paint(color, &format!("{tag:<7}")) + " ")
    }

    fn stamp(&self) -> String {
        self.paint(DIM, &Local::now().format("%H:%M:%S").to_string())
    }

    fn paint(&self, color: &str, text: &str) -> String {
        if self.color && color != RESET {
            format!("{color}{text}{RESET}")
        } else {
            text.to_string()
        }
    }
}

fn compact(value: &Value, max_chars: usize) -> String {
    let json = value.to_string();
    if json.chars().count() <= max_chars {
        return json;
    }
    let cut: String = json.chars().take(max_chars).collect();
    format!("{cut}...")
}

/// `[emote:123:KEKW]` -> `:KEKW:`
pub fn simplify_emotes(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find("[emote:") {
        out.push_str(&rest[..start]);
        let tail = &rest[start..];
        let Some(end) = tail.find(']') else {
            out.push_str(tail);
            return out;
        };
        match tail[1..end].splitn(3, ':').nth(2) {
            Some(name) => {
                out.push(':');
                out.push_str(name);
                out.push(':');
            }
            None => out.push_str(&tail[..=end]),
        }
        rest = &tail[end + 1..];
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn plain() -> Console {
        Console {
            color: false,
            raw: false,
        }
    }

    #[test]
    fn simplifica_emotes_y_respeta_el_resto() {
        assert_eq!(
            simplify_emotes("hola [emote:37226:KEKW] jaja [emote:1:POG]"),
            "hola :KEKW: jaja :POG:"
        );
        assert_eq!(simplify_emotes("sin emotes"), "sin emotes");
        assert_eq!(
            simplify_emotes("roto [emote:12:KEKW"),
            "roto [emote:12:KEKW"
        );
        assert_eq!(simplify_emotes("[emote:solo]"), "[emote:solo]");
    }

    #[test]
    fn formatea_chat_sin_colores() {
        let line = plain().format_event(&KickEvent::Chat {
            user: "ana".into(),
            content: "hola".into(),
        });
        assert_eq!(line, "CHAT    ana: hola");
    }

    #[test]
    fn formatea_ban_con_y_sin_moderador() {
        let c = plain();
        let temporal = KickEvent::UserBanned {
            user: "x".into(),
            banned_by: None,
            permanent: false,
        };
        let permanente = KickEvent::UserBanned {
            user: "x".into(),
            banned_by: Some("mod".into()),
            permanent: true,
        };
        assert_eq!(c.format_event(&temporal), "BAN     x: timeout");
        assert_eq!(
            c.format_event(&permanente),
            "BAN     x: ban permanente por mod"
        );
    }

    #[test]
    fn el_evento_desconocido_recorta_el_json() {
        let data = json!({ "texto": "a".repeat(400) });
        let line = plain().format_event(&KickEvent::Other {
            name: "Raro".into(),
            data,
        });
        assert!(line.starts_with("OTHER   Raro {"));
        assert!(line.ends_with("..."));
        assert!(line.chars().count() < 200);
    }

    #[test]
    fn con_color_envuelve_solo_la_etiqueta() {
        let c = Console {
            color: true,
            raw: false,
        };
        let line = c.format_event(&KickEvent::StreamOffline);
        assert!(line.starts_with(YELLOW) && line.contains(RESET));
    }
}
