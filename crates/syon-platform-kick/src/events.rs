use serde_json::Value;
use syon_domain::{ChatMessage, EventKind, MessageId, Timestamp, User};

/// Interpreta una publicación del feed (`push.pub`) y la traduce a un [`EventKind`].
///
/// Devuelve `None` si la publicación no trae el nombre de un evento. Un evento que no se conoce o
/// al que le faltan campos obligatorios llega como [`EventKind::Unknown`]. `received_at` solo se
/// usa para fabricar el id de un mensaje de chat que no lo trae.
pub fn event_from_publication(publication: &Value, received_at: Timestamp) -> Option<EventKind> {
    let name = publication["data"]["event"].as_str()?;
    let data = decode_payload(&publication["data"]["data"]);
    Some(translate(name, &data, received_at))
}

/// Traduce un evento por su nombre (con o sin espacio de nombres) y sus datos ya decodificados.
fn translate(name: &str, data: &Value, received_at: Timestamp) -> EventKind {
    let known = match short_name(name) {
        "ChatMessageEvent" => chat(data, received_at),
        "MessageDeletedEvent" => message_deleted(data),
        "UserBannedEvent" => user_banned(data),
        "SubscriptionEvent" => subscription(data),
        "GiftedSubscriptionsEvent" => gifted_subs(data),
        "RewardRedeemedEvent" => reward(data),
        "FollowersUpdated" => followers(data),
        "StreamerIsLive" => Some(EventKind::StreamStarted {
            title: text(data, &["livestream", "session_title"]),
        }),
        "StopStreamBroadcast" => Some(EventKind::StreamEnded),
        _ => None,
    };
    known.unwrap_or_else(|| EventKind::Unknown {
        name: short_name(name).to_owned(),
    })
}

/// `App\Events\ChatMessageEvent` -> `ChatMessageEvent`.
fn short_name(name: &str) -> &str {
    name.rsplit('\\').next().unwrap_or(name)
}

/// El dato interno puede ser un objeto JSON o una cadena que contiene JSON.
fn decode_payload(raw: &Value) -> Value {
    match raw {
        Value::String(serialized) => serde_json::from_str(serialized).unwrap_or(Value::Null),
        other => other.clone(),
    }
}

/// Lee un texto o un número no vacío en la ruta indicada.
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

/// Convierte a `u32` saturando en lugar de fallar.
fn saturating_u32(value: u64) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}

fn chat(d: &Value, received_at: Timestamp) -> Option<EventKind> {
    let author = text(d, &["sender", "username"])?;
    // Si el feed no trae el id, se fabrica uno para no perder el mensaje; un borrado no lo hallará.
    let id = text(d, &["id"]).unwrap_or_else(|| format!("{author}@{}", received_at.as_unix_ms()));
    Some(EventKind::Chat(ChatMessage {
        id: MessageId::new(id),
        author: User::new(author),
        text: text(d, &["content"])?,
    }))
}

fn message_deleted(d: &Value) -> Option<EventKind> {
    let id = text(d, &["message", "id"]).or_else(|| text(d, &["id"]))?;
    Some(EventKind::ChatMessageDeleted {
        id: MessageId::new(id),
    })
}

fn user_banned(d: &Value) -> Option<EventKind> {
    Some(EventKind::UserBanned {
        user: User::new(text(d, &["user", "username"])?),
        by: text(d, &["banned_by", "username"]).map(User::new),
        permanent: d["permanent"].as_bool().unwrap_or(false),
    })
}

fn subscription(d: &Value) -> Option<EventKind> {
    Some(EventKind::Subscription {
        user: User::new(text(d, &["username"])?),
        months: saturating_u32(d["months"].as_u64().unwrap_or(1)),
    })
}

fn gifted_subs(d: &Value) -> Option<EventKind> {
    let count = d["gifted_usernames"].as_array().map_or(0, Vec::len);
    Some(EventKind::GiftedSubs {
        gifter: User::new(text(d, &["gifter_username"])?),
        count: saturating_u32(count as u64),
    })
}

fn reward(d: &Value) -> Option<EventKind> {
    Some(EventKind::RewardRedeemed {
        user: User::new(text(d, &["username"])?),
        title: text(d, &["reward_title"])?,
        input: text(d, &["user_input"]),
    })
}

fn followers(d: &Value) -> Option<EventKind> {
    Some(EventKind::FollowersUpdated {
        count: d["followersCount"].as_u64()?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const AT: Timestamp = Timestamp::from_unix_ms(1_700);

    fn publication(event: &str, data: Value) -> Value {
        json!({ "data": { "event": event, "data": data } })
    }

    fn user(name: &str) -> User {
        User::new(name)
    }

    #[test]
    fn chat_como_cadena_json_y_como_objeto_con_su_id() {
        let body = json!({ "id": "m-1", "content": "hola", "sender": { "username": "ana" } });
        let expected = EventKind::Chat(ChatMessage {
            id: MessageId::new("m-1"),
            author: user("ana"),
            text: "hola".to_owned(),
        });

        let como_objeto = publication("App\\Events\\ChatMessageEvent", body.clone());
        let como_cadena = publication("App\\Events\\ChatMessageEvent", json!(body.to_string()));

        assert_eq!(
            event_from_publication(&como_objeto, AT),
            Some(expected.clone())
        );
        assert_eq!(event_from_publication(&como_cadena, AT), Some(expected));
    }

    #[test]
    fn un_chat_sin_id_recibe_uno_fabricado_con_el_autor_y_la_hora() {
        let body = json!({ "content": "hola", "sender": { "username": "ana" } });
        let Some(EventKind::Chat(message)) =
            event_from_publication(&publication("ChatMessageEvent", body), AT)
        else {
            panic!("debía ser un chat");
        };
        assert_eq!(message.id.as_str(), "ana@1700");
    }

    #[test]
    fn un_borrado_usa_el_mismo_id_que_el_chat() {
        let anidado = json!({ "message": { "id": "m-1" } });
        let plano = json!({ "id": "m-1" });
        let expected = EventKind::ChatMessageDeleted {
            id: MessageId::new("m-1"),
        };
        assert_eq!(translate("MessageDeletedEvent", &anidado, AT), expected);
        assert_eq!(translate("MessageDeletedEvent", &plano, AT), expected);
    }

    #[test]
    fn acepta_el_nombre_con_o_sin_espacio_de_nombres() {
        let data = json!({ "username": "luis", "months": 3 });
        let esperado = EventKind::Subscription {
            user: user("luis"),
            months: 3,
        };
        assert_eq!(translate("SubscriptionEvent", &data, AT), esperado);
        assert_eq!(
            translate("App\\Events\\SubscriptionEvent", &data, AT),
            esperado
        );
    }

    #[test]
    fn una_suscripcion_sin_meses_cuenta_uno() {
        let data = json!({ "username": "luis" });
        assert_eq!(
            translate("SubscriptionEvent", &data, AT),
            EventKind::Subscription {
                user: user("luis"),
                months: 1
            }
        );
    }

    #[test]
    fn interpreta_regalos_recompensas_y_bans() {
        let regalo = json!({ "gifter_username": "ana", "gifted_usernames": ["a", "b", "c"] });
        assert_eq!(
            translate("GiftedSubscriptionsEvent", &regalo, AT),
            EventKind::GiftedSubs {
                gifter: user("ana"),
                count: 3
            }
        );

        let premio = json!({ "username": "bob", "reward_title": "Agua", "user_input": "ya" });
        assert_eq!(
            translate("RewardRedeemedEvent", &premio, AT),
            EventKind::RewardRedeemed {
                user: user("bob"),
                title: "Agua".to_owned(),
                input: Some("ya".to_owned())
            }
        );

        let ban = json!({
            "user": { "username": "spam" },
            "banned_by": { "username": "mod" },
            "permanent": true
        });
        assert_eq!(
            translate("UserBannedEvent", &ban, AT),
            EventKind::UserBanned {
                user: user("spam"),
                by: Some(user("mod")),
                permanent: true
            }
        );
    }

    #[test]
    fn interpreta_seguidores_y_el_estado_del_directo() {
        assert_eq!(
            translate("FollowersUpdated", &json!({ "followersCount": 42 }), AT),
            EventKind::FollowersUpdated { count: 42 }
        );
        let live = json!({ "livestream": { "session_title": "Hoy jugamos" } });
        assert_eq!(
            translate("StreamerIsLive", &live, AT),
            EventKind::StreamStarted {
                title: Some("Hoy jugamos".to_owned())
            }
        );
        assert_eq!(
            translate("StopStreamBroadcast", &json!({}), AT),
            EventKind::StreamEnded
        );
    }

    #[test]
    fn evento_desconocido_o_incompleto_llega_como_unknown_con_su_nombre() {
        assert_eq!(
            translate("App\\Events\\PollUpdateEvent", &json!({}), AT),
            EventKind::Unknown {
                name: "PollUpdateEvent".to_owned()
            }
        );
        assert_eq!(
            translate("ChatMessageEvent", &json!({ "content": "sin autor" }), AT),
            EventKind::Unknown {
                name: "ChatMessageEvent".to_owned()
            }
        );
    }

    #[test]
    fn un_contenido_que_no_es_json_no_rompe_la_traduccion() {
        let publicacion = publication("ChatMessageEvent", json!("esto { no es json"));
        assert_eq!(
            event_from_publication(&publicacion, AT),
            Some(EventKind::Unknown {
                name: "ChatMessageEvent".to_owned()
            })
        );
    }

    #[test]
    fn publicacion_sin_evento_se_ignora() {
        assert_eq!(event_from_publication(&json!({ "data": {} }), AT), None);
    }

    #[test]
    fn los_numeros_enormes_se_saturan_en_lugar_de_fallar() {
        let data = json!({ "username": "luis", "months": u64::MAX });
        assert_eq!(
            translate("SubscriptionEvent", &data, AT),
            EventKind::Subscription {
                user: user("luis"),
                months: u32::MAX
            }
        );
    }
}
