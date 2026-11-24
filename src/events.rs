//! Interpretación de los eventos del feed de Kick. Sin red ni formato de consola.
//!
//! Las formas de los datos vienen del feed no oficial y pueden cambiar: todo campo
//! se lee de forma tolerante y un evento que no encaja termina como `Other`.

use serde_json::Value;

#[derive(Debug, Clone, PartialEq)]
pub enum KickEvent {
    Chat {
        user: String,
        content: String,
    },
    MessageDeleted {
        message_id: String,
    },
    UserBanned {
        user: String,
        banned_by: Option<String>,
        permanent: bool,
    },
    Subscription {
        user: String,
        months: u64,
    },
    GiftedSubs {
        gifter: String,
        count: usize,
    },
    Reward {
        user: String,
        title: String,
        input: Option<String>,
    },
    Followers {
        count: u64,
    },
    StreamLive {
        title: Option<String>,
    },
    StreamOffline,
    Other {
        name: String,
        data: Value,
    },
}

/// Evento ya interpretado junto con su nombre y datos originales (para el modo `--raw`).
#[derive(Debug, Clone, PartialEq)]
pub struct Published {
    pub name: String,
    pub payload: Value,
    pub event: KickEvent,
}

/// `App\Events\ChatMessageEvent` -> `ChatMessageEvent`.
fn short_name(name: &str) -> &str {
    name.rsplit('\\').next().unwrap_or(name)
}

fn text(value: &Value, path: &[&str]) -> Option<String> {
    let found = path
        .iter()
        .try_fold(value, |current, key| current.get(*key))?;
    match found {
        Value::String(s) if !s.is_empty() => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

/// El dato interno puede ser un objeto JSON o una cadena que contiene JSON.
fn decode_payload(raw: &Value) -> Value {
    match raw {
        Value::String(serialized) => serde_json::from_str(serialized).unwrap_or(Value::Null),
        other => other.clone(),
    }
}

/// Interpreta una publicación del feed (`push.pub`). `None` si no trae un nombre de evento.
pub fn from_publication(publication: &Value) -> Option<Published> {
    let name = publication["data"]["event"].as_str()?.to_string();
    let payload = decode_payload(&publication["data"]["data"]);
    let event = parse(&name, &payload);
    Some(Published {
        name,
        payload,
        event,
    })
}

pub fn parse(name: &str, data: &Value) -> KickEvent {
    let known = match short_name(name) {
        "ChatMessageEvent" => chat(data),
        "MessageDeletedEvent" => message_deleted(data),
        "UserBannedEvent" => user_banned(data),
        "SubscriptionEvent" => subscription(data),
        "GiftedSubscriptionsEvent" => gifted_subs(data),
        "RewardRedeemedEvent" => reward(data),
        "FollowersUpdated" => followers(data),
        "StreamerIsLive" => Some(KickEvent::StreamLive {
            title: text(data, &["livestream", "session_title"]),
        }),
        "StopStreamBroadcast" => Some(KickEvent::StreamOffline),
        _ => None,
    };
    known.unwrap_or_else(|| KickEvent::Other {
        name: short_name(name).to_string(),
        data: data.clone(),
    })
}

fn chat(d: &Value) -> Option<KickEvent> {
    Some(KickEvent::Chat {
        user: text(d, &["sender", "username"])?,
        content: text(d, &["content"])?,
    })
}

fn message_deleted(d: &Value) -> Option<KickEvent> {
    let message_id = text(d, &["message", "id"]).or_else(|| text(d, &["id"]))?;
    Some(KickEvent::MessageDeleted { message_id })
}

fn user_banned(d: &Value) -> Option<KickEvent> {
    Some(KickEvent::UserBanned {
        user: text(d, &["user", "username"])?,
        banned_by: text(d, &["banned_by", "username"]),
        permanent: d["permanent"].as_bool().unwrap_or(false),
    })
}

fn subscription(d: &Value) -> Option<KickEvent> {
    Some(KickEvent::Subscription {
        user: text(d, &["username"])?,
        months: d["months"].as_u64().unwrap_or(1),
    })
}

fn gifted_subs(d: &Value) -> Option<KickEvent> {
    Some(KickEvent::GiftedSubs {
        gifter: text(d, &["gifter_username"])?,
        count: d["gifted_usernames"].as_array().map_or(0, Vec::len),
    })
}

fn reward(d: &Value) -> Option<KickEvent> {
    Some(KickEvent::Reward {
        user: text(d, &["username"])?,
        title: text(d, &["reward_title"])?,
        input: text(d, &["user_input"]),
    })
}

fn followers(d: &Value) -> Option<KickEvent> {
    Some(KickEvent::Followers {
        count: d["followersCount"].as_u64()?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn publication(event: &str, data: Value) -> Value {
        json!({ "data": { "event": event, "data": data } })
    }

    #[test]
    fn chat_como_cadena_json_y_como_objeto() {
        let body = json!({ "content": "hola", "sender": { "username": "ana" } });
        let expected = KickEvent::Chat {
            user: "ana".into(),
            content: "hola".into(),
        };

        let como_objeto =
            from_publication(&publication("App\\Events\\ChatMessageEvent", body.clone()));
        let como_cadena = from_publication(&publication(
            "App\\Events\\ChatMessageEvent",
            json!(body.to_string()),
        ));

        assert_eq!(como_objeto.unwrap().event, expected);
        assert_eq!(como_cadena.unwrap().event, expected);
    }

    #[test]
    fn acepta_el_nombre_con_o_sin_espacio_de_nombres() {
        let data = json!({ "username": "luis", "months": 3 });
        let esperado = KickEvent::Subscription {
            user: "luis".into(),
            months: 3,
        };
        assert_eq!(parse("SubscriptionEvent", &data), esperado);
        assert_eq!(parse("App\\Events\\SubscriptionEvent", &data), esperado);
    }

    #[test]
    fn interpreta_regalos_recompensas_y_bans() {
        let regalo = json!({ "gifter_username": "ana", "gifted_usernames": ["a", "b", "c"] });
        assert_eq!(
            parse("GiftedSubscriptionsEvent", &regalo),
            KickEvent::GiftedSubs {
                gifter: "ana".into(),
                count: 3
            }
        );

        let premio = json!({ "username": "bob", "reward_title": "Agua", "user_input": "ya" });
        assert_eq!(
            parse("RewardRedeemedEvent", &premio),
            KickEvent::Reward {
                user: "bob".into(),
                title: "Agua".into(),
                input: Some("ya".into())
            }
        );

        let ban = json!({ "user": { "username": "spam" }, "banned_by": { "username": "mod" }, "permanent": true });
        assert_eq!(
            parse("UserBannedEvent", &ban),
            KickEvent::UserBanned {
                user: "spam".into(),
                banned_by: Some("mod".into()),
                permanent: true
            }
        );
    }

    #[test]
    fn evento_desconocido_o_incompleto_termina_en_other() {
        assert!(matches!(
            parse("PollUpdateEvent", &json!({})),
            KickEvent::Other { .. }
        ));
        assert!(matches!(
            parse("ChatMessageEvent", &json!({ "content": "sin autor" })),
            KickEvent::Other { .. }
        ));
    }

    #[test]
    fn publicacion_sin_evento_se_ignora() {
        assert!(from_publication(&json!({ "data": {} })).is_none());
    }
}
