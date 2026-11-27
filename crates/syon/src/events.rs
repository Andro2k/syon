//! Una línea de texto por evento, para la consola. El formato no es una API estable: la interfaz
//! de F9 lo sustituye.

use syon_domain::{Event, EventKind, Platform, Timestamp};

/// Caracteres que no se imprimen: los de control (incluida la secuencia de escape de terminal) y
/// los de formato bidireccional, que sirven para falsear lo que se ve en pantalla.
fn is_unsafe(c: char) -> bool {
    c.is_control() || matches!(c, '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}')
}

/// Quita los caracteres inseguros. El texto de los eventos lo escribe cualquiera.
pub fn sanitize(text: &str) -> String {
    text.chars().filter(|c| !is_unsafe(*c)).collect()
}

fn clock(at: Timestamp) -> String {
    let seconds = at.as_unix_ms().div_euclid(1000).rem_euclid(86_400);
    format!(
        "{:02}:{:02}:{:02}",
        seconds / 3600,
        seconds % 3600 / 60,
        seconds % 60
    )
}

fn platform_name(platform: Platform) -> &'static str {
    match platform {
        Platform::Kick => "kick",
        Platform::Twitch => "twitch",
    }
}

fn summary(kind: &EventKind) -> String {
    match kind {
        EventKind::Chat(message) => format!("{}: {}", message.author.name, message.text),
        EventKind::ChatMessageDeleted { id } => format!("mensaje borrado ({})", id.as_str()),
        EventKind::UserBanned {
            user,
            by,
            permanent,
        } => {
            let action = if *permanent {
                "baneado"
            } else {
                "expulsado temporalmente"
            };
            match by {
                Some(by) => format!("{} {action} por {}", user.name, by.name),
                None => format!("{} {action}", user.name),
            }
        }
        EventKind::Subscription { user, months } => {
            format!("{} se suscribió ({months} meses)", user.name)
        }
        EventKind::GiftedSubs { gifter, count } => {
            format!("{} regaló {count} suscripciones", gifter.name)
        }
        EventKind::RewardRedeemed { user, title, input } => match input {
            Some(input) => format!("{} canjeó «{title}»: {input}", user.name),
            None => format!("{} canjeó «{title}»", user.name),
        },
        EventKind::Followed { user } => format!("{} empezó a seguir el canal", user.name),
        EventKind::FollowersUpdated { count } => format!("seguidores: {count}"),
        EventKind::StreamStarted { title } => match title {
            Some(title) => format!("directo iniciado: {title}"),
            None => "directo iniciado".to_owned(),
        },
        EventKind::StreamEnded => "directo terminado".to_owned(),
        EventKind::Unknown { name } => format!("evento desconocido ({name})"),
    }
}

/// `<hora UTC> <plataforma> <resumen>`, sin caracteres de control.
pub fn format_event(event: &Event) -> String {
    let line = format!(
        "{} {} {}",
        clock(event.received_at),
        platform_name(event.platform),
        summary(&event.kind)
    );
    sanitize(&line)
}

#[cfg(test)]
mod tests {
    use super::*;
    use syon_domain::{ChatMessage, MessageId, User};

    fn event(kind: EventKind) -> Event {
        Event {
            platform: Platform::Kick,
            // 1970-01-02 03:04:05 UTC.
            received_at: Timestamp::from_unix_ms((86_400 + 3 * 3600 + 4 * 60 + 5) * 1000),
            kind,
        }
    }

    fn chat(text: &str) -> EventKind {
        EventKind::Chat(ChatMessage {
            id: MessageId::new("m1"),
            author: User::new("ana"),
            text: text.to_owned(),
        })
    }

    #[test]
    fn la_linea_lleva_hora_plataforma_y_resumen() {
        assert_eq!(
            format_event(&event(chat("hola"))),
            "03:04:05 kick ana: hola"
        );
        assert_eq!(
            format_event(&event(EventKind::StreamEnded)),
            "03:04:05 kick directo terminado"
        );
    }

    #[test]
    fn el_texto_de_terceros_no_lleva_control_ni_escapes() {
        let line = format_event(&event(chat("\x1b[2J\x1b]0;titulo\x07borrado\r\nlinea")));
        assert!(!line.chars().any(char::is_control), "{line:?}");
        assert!(line.contains("borrado"));
        assert_eq!(line.lines().count(), 1);
    }

    #[test]
    fn tampoco_lleva_formato_bidireccional() {
        let line = format_event(&event(chat("a\u{202E}b\u{2066}c")));
        assert_eq!(line, "03:04:05 kick ana: abc");
    }

    #[test]
    fn los_nombres_y_los_titulos_tambien_se_limpian() {
        let kind = EventKind::StreamStarted {
            title: Some("\x1b[31mrojo".to_owned()),
        };
        assert!(!format_event(&event(kind)).contains('\x1b'));
        let kind = EventKind::Followed {
            user: User::new("a\x1bb"),
        };
        assert!(!format_event(&event(kind)).contains('\x1b'));
    }

    #[test]
    fn cada_tipo_de_evento_tiene_resumen() {
        let user = || User::new("ana");
        let kinds = [
            chat("x"),
            EventKind::ChatMessageDeleted {
                id: MessageId::new("m"),
            },
            EventKind::UserBanned {
                user: user(),
                by: Some(User::new("mod")),
                permanent: true,
            },
            EventKind::Subscription {
                user: user(),
                months: 3,
            },
            EventKind::GiftedSubs {
                gifter: user(),
                count: 5,
            },
            EventKind::RewardRedeemed {
                user: user(),
                title: "t".to_owned(),
                input: None,
            },
            EventKind::Followed { user: user() },
            EventKind::FollowersUpdated { count: 9 },
            EventKind::StreamStarted { title: None },
            EventKind::StreamEnded,
            EventKind::Unknown {
                name: "n".to_owned(),
            },
        ];
        for kind in kinds {
            assert!(format_event(&event(kind)).len() > "03:04:05 kick ".len());
        }
    }

    #[test]
    fn la_hora_da_la_vuelta_cada_dia_y_acepta_tiempos_negativos() {
        assert_eq!(clock(Timestamp::from_unix_ms(0)), "00:00:00");
        assert_eq!(clock(Timestamp::from_unix_ms(-1000)), "23:59:59");
    }
}
