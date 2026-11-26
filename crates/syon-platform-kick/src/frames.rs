//! Lógica pura del feed de Kick (Centrifugo por WebSocket): nombres de los feeds, clasificación de
//! los mensajes recibidos y lectura de las respuestas de negociación. No hace red.

use serde_json::Value;
use std::time::Duration;

/// Intervalo de ping que se asume hasta que el servidor indique otro.
pub(crate) const DEFAULT_PING: Duration = Duration::from_secs(25);
/// Identificador del mensaje `connect`; los de `subscribe` empiezan en [`FIRST_SUBSCRIBE_ID`].
pub(crate) const CONNECT_ID: u64 = 1;
pub(crate) const FIRST_SUBSCRIBE_ID: u64 = 2;

/// Identificadores de un canal de Kick.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Channel {
    pub(crate) slug: String,
    pub(crate) id: u64,
    pub(crate) chatroom_id: u64,
}

/// Lee los ids del canal de la respuesta de `GET /channels/{slug}`.
pub(crate) fn channel_from_json(slug: &str, body: &Value) -> Result<Channel, String> {
    let id = body["id"]
        .as_u64()
        .ok_or("no se encontró el id del canal en la respuesta de Kick")?;
    let chatroom_id = body["chatroom"]["id"]
        .as_u64()
        .ok_or("no se encontró el id del chatroom en la respuesta de Kick")?;
    Ok(Channel {
        slug: slug.to_owned(),
        id,
        chatroom_id,
    })
}

/// Feeds que un invitado puede leer: chat y moderación, regalos y recompensas, estado del directo
/// y Kicks. Los feeds `private-*` requieren sesión de usuario.
pub(crate) fn feed_names(channel: &Channel) -> Vec<String> {
    vec![
        format!("chatrooms.{}.v2", channel.chatroom_id),
        format!("chatroom_{}", channel.chatroom_id),
        format!("channel.{}", channel.id),
        format!("channel_{}", channel.id),
    ]
}

/// Qué significa un mensaje recibido del servidor.
#[derive(Debug, PartialEq)]
pub(crate) enum Inbound {
    /// Mensaje vacío del servidor; hay que responder con otro vacío.
    Ping,
    /// El servidor aceptó la conexión e indicó cada cuánto hace ping.
    Connected {
        ping: Duration,
    },
    ConnectFailed(String),
    Subscribed(String),
    SubscribeFailed {
        feed: String,
        message: String,
    },
    /// Algo publicado en un feed (`push.pub`).
    Publication {
        feed: String,
        data: Value,
    },
    Disconnect {
        code: u16,
        reason: String,
    },
    Ignored,
}

/// Clasifica un mensaje JSON del servidor. `feeds` permite saber a qué feed responde cada `id`.
pub(crate) fn classify(frame: &Value, feeds: &[String]) -> Inbound {
    if frame.as_object().is_some_and(|object| object.is_empty()) {
        return Inbound::Ping;
    }
    if let Some(connect) = frame.get("connect") {
        let ping = connect["ping"]
            .as_u64()
            .map_or(DEFAULT_PING, Duration::from_secs);
        return Inbound::Connected { ping };
    }
    if let Some(push) = frame.get("push") {
        return match push.get("pub") {
            Some(data) => Inbound::Publication {
                feed: push["channel"].as_str().unwrap_or_default().to_owned(),
                data: data.clone(),
            },
            None => Inbound::Ignored,
        };
    }
    if let Some(disconnect) = frame.get("disconnect") {
        return Inbound::Disconnect {
            code: disconnect["code"]
                .as_u64()
                .and_then(|code| u16::try_from(code).ok())
                .unwrap_or(0),
            reason: disconnect["reason"].as_str().unwrap_or_default().to_owned(),
        };
    }
    let Some(id) = frame["id"].as_u64() else {
        return Inbound::Ignored;
    };
    let error = frame
        .get("error")
        .map(|e| format!("{} {}", e["code"], e["message"].as_str().unwrap_or("")));
    if id == CONNECT_ID {
        return error.map_or(Inbound::Ignored, Inbound::ConnectFailed);
    }
    let feed = id
        .checked_sub(FIRST_SUBSCRIBE_ID)
        .and_then(|index| usize::try_from(index).ok())
        .and_then(|index| feeds.get(index))
        .cloned();
    match (feed, error) {
        (Some(feed), Some(message)) => Inbound::SubscribeFailed { feed, message },
        (Some(feed), None) if frame.get("subscribe").is_some() => Inbound::Subscribed(feed),
        _ => Inbound::Ignored,
    }
}

/// Códigos de desconexión que Centrifugo marca como terminales (3500-3999 y 4500-4999): no
/// conviene reintentar de inmediato.
pub(crate) fn is_terminal(code: u16) -> bool {
    (3500..=3999).contains(&code) || (4500..=4999).contains(&code)
}

/// Dirección del WebSocket Centrifugo en la respuesta de negociación, si Kick ofrece ese proveedor.
pub(crate) fn centrifugo_url(response: &Value) -> Option<String> {
    response["data"]["connections"]
        .as_array()?
        .iter()
        .find(|connection| connection["provider"] == "centrifugo")
        .and_then(|connection| connection["credentials"]["url"].as_str())
        .map(str::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn channel() -> Channel {
        Channel {
            slug: "ana".to_owned(),
            id: 7,
            chatroom_id: 9,
        }
    }

    fn feeds() -> Vec<String> {
        feed_names(&channel())
    }

    #[test]
    fn nombres_de_feeds() {
        assert_eq!(
            feeds(),
            ["chatrooms.9.v2", "chatroom_9", "channel.7", "channel_7"]
        );
    }

    #[test]
    fn lee_los_ids_del_canal_y_falla_si_faltan() {
        let body = json!({ "id": 7, "chatroom": { "id": 9 } });
        assert_eq!(channel_from_json("ana", &body), Ok(channel()));
        assert!(channel_from_json("ana", &json!({ "chatroom": { "id": 9 } })).is_err());
        assert!(channel_from_json("ana", &json!({ "id": 7 })).is_err());
    }

    #[test]
    fn clasifica_ping_conexion_y_suscripciones() {
        let feeds = feeds();
        assert_eq!(classify(&json!({}), &feeds), Inbound::Ping);
        assert_eq!(
            classify(&json!({ "id": 1, "connect": { "ping": 30 } }), &feeds),
            Inbound::Connected {
                ping: Duration::from_secs(30)
            }
        );
        assert_eq!(
            classify(&json!({ "id": 1, "connect": {} }), &feeds),
            Inbound::Connected { ping: DEFAULT_PING }
        );
        assert_eq!(
            classify(&json!({ "id": 2, "subscribe": {} }), &feeds),
            Inbound::Subscribed("chatrooms.9.v2".into())
        );
        assert_eq!(
            classify(&json!({ "id": 99, "subscribe": {} }), &feeds),
            Inbound::Ignored
        );
    }

    #[test]
    fn clasifica_los_errores() {
        let feeds = feeds();
        assert_eq!(
            classify(
                &json!({ "id": 5, "error": { "code": 102, "message": "unknown channel" } }),
                &feeds
            ),
            Inbound::SubscribeFailed {
                feed: "channel_7".into(),
                message: "102 unknown channel".into()
            }
        );
        assert!(matches!(
            classify(
                &json!({ "id": 1, "error": { "code": 3, "message": "x" } }),
                &feeds
            ),
            Inbound::ConnectFailed(_)
        ));
    }

    #[test]
    fn clasifica_publicaciones_y_desconexiones() {
        let feeds = feeds();
        let push = json!({ "push": { "channel": "chatroom_9", "pub": { "data": {} } } });
        assert!(matches!(
            classify(&push, &feeds),
            Inbound::Publication { feed, .. } if feed == "chatroom_9"
        ));
        assert_eq!(
            classify(&json!({ "push": { "channel": "x" } }), &feeds),
            Inbound::Ignored
        );
        assert_eq!(
            classify(
                &json!({ "disconnect": { "code": 3503, "reason": "bye" } }),
                &feeds
            ),
            Inbound::Disconnect {
                code: 3503,
                reason: "bye".into()
            }
        );
    }

    #[test]
    fn un_codigo_de_desconexion_fuera_de_rango_cuenta_como_cero() {
        assert_eq!(
            classify(&json!({ "disconnect": { "code": 99_999_999 } }), &feeds()),
            Inbound::Disconnect {
                code: 0,
                reason: String::new()
            }
        );
    }

    #[test]
    fn los_codigos_terminales_son_los_de_centrifugo() {
        for code in [3500, 3999, 4500, 4999] {
            assert!(is_terminal(code), "{code}");
        }
        for code in [0, 1000, 3499, 4000, 4499, 5000] {
            assert!(!is_terminal(code), "{code}");
        }
    }

    #[test]
    fn elige_el_proveedor_centrifugo() {
        let response = json!({ "data": { "connections": [
            { "provider": "pusher", "credentials": { "url": "wss://pusher" } },
            { "provider": "centrifugo", "credentials": { "url": "wss://centrifugo" } }
        ] } });
        assert_eq!(
            centrifugo_url(&response).as_deref(),
            Some("wss://centrifugo")
        );
        assert_eq!(
            centrifugo_url(&json!({ "data": { "connections": [] } })),
            None
        );
        assert_eq!(centrifugo_url(&json!({})), None);
    }
}
