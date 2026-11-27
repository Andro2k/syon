use serde_json::Value;
use std::{
    collections::{HashSet, VecDeque},
    time::Duration,
};

/// Plazo de silencio que Twitch anuncia por defecto cuando el mensaje de bienvenida no lo trae.
pub(crate) const DEFAULT_KEEPALIVE: Duration = Duration::from_secs(10);
/// Cuántos ids de mensaje se recuerdan para descartar entregas repetidas.
const DEDUP_CAPACITY: usize = 128;

/// Qué significa un mensaje del WebSocket de EventSub.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Inbound {
    /// Bienvenida: trae el id de la sesión (para crear las suscripciones) y el plazo de silencio.
    Welcome {
        session_id: String,
        keepalive: Duration,
    },
    /// La conexión está sana; no ha habido eventos en el plazo anunciado.
    Keepalive,
    /// Un evento de una suscripción.
    Notification,
    /// Twitch va a cerrar la conexión: hay que abrir otra en `url`, tal cual viene.
    Reconnect { url: String },
    /// Twitch canceló una suscripción.
    Revocation {
        subscription_type: String,
        status: String,
    },
    /// Cualquier otra cosa, o un mensaje al que le faltan campos imprescindibles.
    Ignored,
}

/// Clasifica un mensaje por su `metadata.message_type`.
pub(crate) fn classify(message: &Value) -> Inbound {
    let session = &message["payload"]["session"];
    match message["metadata"]["message_type"].as_str() {
        Some("session_welcome") => match session["id"].as_str() {
            Some(id) => Inbound::Welcome {
                session_id: id.to_owned(),
                keepalive: session["keepalive_timeout_seconds"]
                    .as_u64()
                    .map_or(DEFAULT_KEEPALIVE, Duration::from_secs),
            },
            None => Inbound::Ignored,
        },
        Some("session_keepalive") => Inbound::Keepalive,
        Some("notification") => Inbound::Notification,
        Some("session_reconnect") => match session["reconnect_url"].as_str() {
            Some(url) if !url.is_empty() => Inbound::Reconnect {
                url: url.to_owned(),
            },
            _ => Inbound::Ignored,
        },
        Some("revocation") => {
            let subscription = &message["payload"]["subscription"];
            Inbound::Revocation {
                subscription_type: subscription["type"].as_str().unwrap_or("?").to_owned(),
                status: subscription["status"].as_str().unwrap_or("?").to_owned(),
            }
        }
        _ => Inbound::Ignored,
    }
}

/// Recuerda los últimos ids de mensaje para descartar un evento que Twitch entregue dos veces (por
/// ejemplo, por la conexión vieja y la nueva durante un cambio de conexión).
#[derive(Debug, Default)]
pub(crate) struct Dedup {
    order: VecDeque<String>,
    seen: HashSet<String>,
}

impl Dedup {
    /// `true` si el id ya se había visto; si no, lo recuerda. Un mensaje sin id nunca es repetido.
    pub(crate) fn is_repeat(&mut self, message: &Value) -> bool {
        let Some(id) = message["metadata"]["message_id"].as_str() else {
            return false;
        };
        if self.seen.contains(id) {
            return true;
        }
        if self.order.len() == DEDUP_CAPACITY
            && let Some(oldest) = self.order.pop_front()
        {
            self.seen.remove(&oldest);
        }
        self.order.push_back(id.to_owned());
        self.seen.insert(id.to_owned());
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn with_type(message_type: &str, session: Value) -> Value {
        json!({
            "metadata": { "message_type": message_type },
            "payload": { "session": session }
        })
    }

    #[test]
    fn la_bienvenida_trae_la_sesion_y_el_plazo() {
        let welcome = with_type(
            "session_welcome",
            json!({ "id": "AQoQILE98gtq", "status": "connected", "keepalive_timeout_seconds": 30, "reconnect_url": null }),
        );
        assert_eq!(
            classify(&welcome),
            Inbound::Welcome {
                session_id: "AQoQILE98gtq".into(),
                keepalive: Duration::from_secs(30),
            }
        );
    }

    #[test]
    fn sin_plazo_la_bienvenida_usa_el_de_diez_segundos_y_sin_id_se_ignora() {
        let sin_plazo = with_type("session_welcome", json!({ "id": "S1" }));
        assert_eq!(
            classify(&sin_plazo),
            Inbound::Welcome {
                session_id: "S1".into(),
                keepalive: DEFAULT_KEEPALIVE,
            }
        );
        assert_eq!(
            classify(&with_type("session_welcome", json!({}))),
            Inbound::Ignored
        );
    }

    #[test]
    fn reconoce_keepalive_notificacion_y_reconexion() {
        assert_eq!(
            classify(&with_type("session_keepalive", json!(null))),
            Inbound::Keepalive
        );
        assert_eq!(
            classify(&with_type("notification", json!(null))),
            Inbound::Notification
        );
        let reconnect = with_type(
            "session_reconnect",
            json!({ "id": "S1", "status": "reconnecting", "keepalive_timeout_seconds": null, "reconnect_url": "wss://eventsub.wss.twitch.tv?a=b" }),
        );
        assert_eq!(
            classify(&reconnect),
            Inbound::Reconnect {
                url: "wss://eventsub.wss.twitch.tv?a=b".into()
            }
        );
        assert_eq!(
            classify(&with_type(
                "session_reconnect",
                json!({ "reconnect_url": "" })
            )),
            Inbound::Ignored
        );
    }

    #[test]
    fn la_revocacion_dice_que_suscripcion_y_por_que() {
        let revocation = json!({
            "metadata": { "message_type": "revocation" },
            "payload": { "subscription": { "type": "channel.ban", "status": "authorization_revoked" } }
        });
        assert_eq!(
            classify(&revocation),
            Inbound::Revocation {
                subscription_type: "channel.ban".into(),
                status: "authorization_revoked".into(),
            }
        );
    }

    #[test]
    fn lo_demas_se_ignora() {
        assert_eq!(classify(&json!({})), Inbound::Ignored);
        assert_eq!(
            classify(&with_type("algo_nuevo", json!(null))),
            Inbound::Ignored
        );
    }

    fn message(id: &str) -> Value {
        json!({ "metadata": { "message_id": id } })
    }

    #[test]
    fn un_id_repetido_se_detecta_y_uno_sin_id_no() {
        let mut dedup = Dedup::default();
        assert!(!dedup.is_repeat(&message("a")));
        assert!(dedup.is_repeat(&message("a")));
        assert!(!dedup.is_repeat(&message("b")));
        assert!(!dedup.is_repeat(&json!({})));
        assert!(!dedup.is_repeat(&json!({})));
    }

    #[test]
    fn solo_se_recuerdan_los_ultimos_ids() {
        let mut dedup = Dedup::default();
        for n in 0..DEDUP_CAPACITY + 1 {
            assert!(!dedup.is_repeat(&message(&n.to_string())));
        }
        // El primero salió de la memoria; el último sigue ahí.
        assert!(!dedup.is_repeat(&message("0")));
        assert!(dedup.is_repeat(&message(&DEDUP_CAPACITY.to_string())));
    }
}
