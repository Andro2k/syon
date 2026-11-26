use serde_json::Value;
use syon_domain::{ChatMessage, EventKind, MessageId, User};

/// Nombre con el que se publica a quien regala suscripciones de forma anónima.
const ANONYMOUS: &str = "Anónimo";

/// Interpreta un mensaje `notification` de EventSub y lo traduce a un [`EventKind`].
///
/// Devuelve `None` si el mensaje no es una notificación (bienvenida, keepalive...) o si es una que no
/// debe publicarse: cada suscripción regalada dentro de un regalo a la comunidad, que el aviso del
/// regalo ya cuenta. Un evento que no se conoce o al que le faltan campos obligatorios llega como
/// [`EventKind::Unknown`], con el nombre de la suscripción (y el tipo de aviso si es un aviso de
/// chat).
pub fn event_from_notification(message: &Value) -> Option<EventKind> {
    if message["metadata"]["message_type"].as_str()? != "notification" {
        return None;
    }
    let subscription_type = message["metadata"]["subscription_type"]
        .as_str()
        .or_else(|| message["payload"]["subscription"]["type"].as_str())?;
    translate(subscription_type, &message["payload"]["event"])
}

/// Traduce un evento por el nombre de su suscripción y sus datos.
fn translate(subscription_type: &str, event: &Value) -> Option<EventKind> {
    let known = match subscription_type {
        "channel.chat.message" => chat(event),
        "channel.chat.message_delete" => message_deleted(event),
        "channel.ban" => user_banned(event),
        "channel.chat.notification" => return chat_notification(event),
        "channel.channel_points_custom_reward_redemption.add" => reward(event),
        "channel.follow" => followed(event),
        "stream.online" => Some(EventKind::StreamStarted {
            // El aviso de EventSub no trae el título del directo.
            title: None,
        }),
        "stream.offline" => Some(EventKind::StreamEnded),
        _ => None,
    };
    Some(known.unwrap_or_else(|| EventKind::Unknown {
        name: subscription_type.to_owned(),
    }))
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

/// Nombre visible de quien actúa con el prefijo de campos dado (`chatter_user`, `user`,
/// `moderator_user`...); si no hay nombre visible usa el login.
fn person(event: &Value, prefix: &str) -> Option<User> {
    text(event, &[&format!("{prefix}_name")])
        .or_else(|| text(event, &[&format!("{prefix}_login")]))
        .map(User::new)
}

fn chat(e: &Value) -> Option<EventKind> {
    Some(EventKind::Chat(ChatMessage {
        id: MessageId::new(text(e, &["message_id"])?),
        author: person(e, "chatter_user")?,
        text: text(e, &["message", "text"])?,
    }))
}

fn message_deleted(e: &Value) -> Option<EventKind> {
    Some(EventKind::ChatMessageDeleted {
        id: MessageId::new(text(e, &["message_id"])?),
    })
}

fn user_banned(e: &Value) -> Option<EventKind> {
    // Si falta `is_permanent`, `ends_at` nulo distingue el ban permanente del timeout.
    let permanent = e["is_permanent"]
        .as_bool()
        .unwrap_or_else(|| e.get("ends_at").is_some_and(Value::is_null));
    Some(EventKind::UserBanned {
        user: person(e, "user")?,
        by: person(e, "moderator_user"),
        permanent,
    })
}

fn reward(e: &Value) -> Option<EventKind> {
    Some(EventKind::RewardRedeemed {
        user: person(e, "user")?,
        title: text(e, &["reward", "title"])?,
        input: text(e, &["user_input"]),
    })
}

fn followed(e: &Value) -> Option<EventKind> {
    Some(EventKind::Followed {
        user: person(e, "user")?,
    })
}

/// Traduce un aviso de chat (`channel.chat.notification`) según su `notice_type`.
fn chat_notification(e: &Value) -> Option<EventKind> {
    let Some(notice) = text(e, &["notice_type"]) else {
        return Some(unknown("channel.chat.notification"));
    };
    let known = match notice.as_str() {
        // Un suscriptor nuevo cumple su primer mes.
        "sub" => person(e, "chatter_user").map(|user| EventKind::Subscription { user, months: 1 }),
        "resub" => person(e, "chatter_user")
            .zip(e["resub"]["cumulative_months"].as_u64())
            .map(|(user, months)| EventKind::Subscription {
                user,
                months: saturating_u32(months),
            }),
        "community_sub_gift" => {
            e["community_sub_gift"]["total"]
                .as_u64()
                .map(|total| EventKind::GiftedSubs {
                    gifter: gifter(e),
                    count: saturating_u32(total),
                })
        }
        "sub_gift" => {
            // Cada suscripción de un regalo a la comunidad ya está contada en su aviso.
            if !e["sub_gift"]["community_gift_id"].is_null() {
                return None;
            }
            e["sub_gift"].is_object().then(|| EventKind::GiftedSubs {
                gifter: gifter(e),
                count: 1,
            })
        }
        _ => None,
    };
    Some(known.unwrap_or_else(|| unknown(&format!("channel.chat.notification:{notice}"))))
}

/// Quien regala: el que escribe el aviso, o [`ANONYMOUS`] si el regalo es anónimo o sin nombre.
fn gifter(e: &Value) -> User {
    if e["chatter_is_anonymous"].as_bool().unwrap_or(false) {
        return User::new(ANONYMOUS);
    }
    person(e, "chatter_user").unwrap_or_else(|| User::new(ANONYMOUS))
}

fn unknown(name: &str) -> EventKind {
    EventKind::Unknown {
        name: name.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn notification(subscription_type: &str, event: Value) -> Value {
        json!({
            "metadata": {
                "message_id": "befa7b53-d79d-478f-86b9-120f112b044e",
                "message_type": "notification",
                "message_timestamp": "2022-11-16T10:11:12.464757833Z",
                "subscription_type": subscription_type,
                "subscription_version": "1"
            },
            "payload": { "subscription": { "type": subscription_type }, "event": event }
        })
    }

    fn translate_event(subscription_type: &str, event: Value) -> Option<EventKind> {
        event_from_notification(&notification(subscription_type, event))
    }

    fn user(name: &str) -> User {
        User::new(name)
    }

    #[test]
    fn un_mensaje_de_chat_conserva_su_id_autor_y_texto() {
        let event = json!({
            "broadcaster_user_id": "1971641",
            "chatter_user_id": "4145994",
            "chatter_user_login": "viewer32",
            "chatter_user_name": "Viewer32",
            "message_id": "cc106a89-1814-919d-454c-f4f2f970aae7",
            "message": { "text": "Hi chat", "fragments": [] },
            "message_type": "text",
            "reply": null
        });
        assert_eq!(
            translate_event("channel.chat.message", event),
            Some(EventKind::Chat(ChatMessage {
                id: MessageId::new("cc106a89-1814-919d-454c-f4f2f970aae7"),
                author: user("Viewer32"),
                text: "Hi chat".to_owned(),
            }))
        );
    }

    #[test]
    fn sin_nombre_visible_el_autor_usa_el_login() {
        let event = json!({
            "chatter_user_login": "viewer32",
            "message_id": "m1",
            "message": { "text": "hola" }
        });
        assert!(matches!(
            translate_event("channel.chat.message", event),
            Some(EventKind::Chat(ChatMessage { author, .. })) if author == user("viewer32")
        ));
    }

    #[test]
    fn un_mensaje_borrado_se_refiere_al_mensaje_por_su_id() {
        let event = json!({
            "target_user_id": "7734",
            "target_user_name": "Uncool_viewer",
            "message_id": "ab24e0b0-2260-4bac-94e4-05eedd4ecd0e"
        });
        assert_eq!(
            translate_event("channel.chat.message_delete", event),
            Some(EventKind::ChatMessageDeleted {
                id: MessageId::new("ab24e0b0-2260-4bac-94e4-05eedd4ecd0e")
            })
        );
    }

    #[test]
    fn un_timeout_y_un_ban_permanente_se_distinguen() {
        let timeout = json!({
            "user_name": "Cool_User",
            "moderator_user_name": "Mod_User",
            "reason": "Offensive language",
            "banned_at": "2020-07-15T18:15:11.17106713Z",
            "ends_at": "2020-07-15T18:16:11.17106713Z",
            "is_permanent": false
        });
        assert_eq!(
            translate_event("channel.ban", timeout),
            Some(EventKind::UserBanned {
                user: user("Cool_User"),
                by: Some(user("Mod_User")),
                permanent: false,
            })
        );
        let permanent = json!({ "user_name": "Cool_User", "ends_at": null, "is_permanent": true });
        assert_eq!(
            translate_event("channel.ban", permanent),
            Some(EventKind::UserBanned {
                user: user("Cool_User"),
                by: None,
                permanent: true,
            })
        );
    }

    #[test]
    fn sin_is_permanent_el_fin_nulo_indica_un_ban_permanente() {
        let con_fin_nulo = json!({ "user_name": "Ana", "ends_at": null });
        let sin_nada = json!({ "user_name": "Ana" });
        let con_fin = json!({ "user_name": "Ana", "ends_at": "2020-07-15T18:16:11Z" });
        let permanent = |event| match translate_event("channel.ban", event) {
            Some(EventKind::UserBanned { permanent, .. }) => permanent,
            otro => panic!("no es un ban: {otro:?}"),
        };
        assert!(permanent(con_fin_nulo));
        assert!(!permanent(sin_nada));
        assert!(!permanent(con_fin));
    }

    #[test]
    fn una_suscripcion_nueva_cuenta_su_primer_mes_y_una_renovacion_los_acumulados() {
        let sub = json!({
            "chatter_user_name": "viewer23",
            "notice_type": "sub",
            "sub": { "sub_tier": "1000", "is_prime": false, "duration_months": 1 },
            "resub": null
        });
        assert_eq!(
            translate_event("channel.chat.notification", sub),
            Some(EventKind::Subscription {
                user: user("viewer23"),
                months: 1,
            })
        );
        let resub = json!({
            "chatter_user_name": "viewer23",
            "notice_type": "resub",
            "resub": { "cumulative_months": 10, "duration_months": 0, "streak_months": null }
        });
        assert_eq!(
            translate_event("channel.chat.notification", resub),
            Some(EventKind::Subscription {
                user: user("viewer23"),
                months: 10,
            })
        );
    }

    #[test]
    fn un_regalo_a_la_comunidad_cuenta_el_total_y_sus_suscripciones_no_se_repiten() {
        let community = json!({
            "chatter_user_name": "Generoso",
            "chatter_is_anonymous": false,
            "notice_type": "community_sub_gift",
            "community_sub_gift": { "id": "G1", "total": 5, "sub_tier": "1000" }
        });
        assert_eq!(
            translate_event("channel.chat.notification", community),
            Some(EventKind::GiftedSubs {
                gifter: user("Generoso"),
                count: 5,
            })
        );
        let inside_community = json!({
            "chatter_user_name": "Generoso",
            "notice_type": "sub_gift",
            "sub_gift": { "recipient_user_name": "Ana", "community_gift_id": "G1" }
        });
        assert_eq!(
            translate_event("channel.chat.notification", inside_community),
            None
        );
    }

    #[test]
    fn una_suscripcion_regalada_suelta_cuenta_una_y_un_regalo_anonimo_usa_el_nombre_anonimo() {
        let single = json!({
            "chatter_user_name": "Generoso",
            "notice_type": "sub_gift",
            "sub_gift": { "recipient_user_name": "Ana", "community_gift_id": null }
        });
        assert_eq!(
            translate_event("channel.chat.notification", single),
            Some(EventKind::GiftedSubs {
                gifter: user("Generoso"),
                count: 1,
            })
        );
        let anonymous = json!({
            "chatter_user_id": null,
            "chatter_user_name": null,
            "chatter_is_anonymous": true,
            "notice_type": "community_sub_gift",
            "community_sub_gift": { "id": "G2", "total": 2 }
        });
        assert_eq!(
            translate_event("channel.chat.notification", anonymous),
            Some(EventKind::GiftedSubs {
                gifter: user("Anónimo"),
                count: 2,
            })
        );
    }

    #[test]
    fn otros_avisos_de_chat_llegan_como_desconocidos_con_su_tipo_de_aviso() {
        let raid =
            json!({ "notice_type": "raid", "raid": { "user_name": "Otro", "viewer_count": 9 } });
        assert_eq!(
            translate_event("channel.chat.notification", raid),
            Some(unknown("channel.chat.notification:raid"))
        );
        assert_eq!(
            translate_event("channel.chat.notification", json!({})),
            Some(unknown("channel.chat.notification"))
        );
    }

    #[test]
    fn un_aviso_conocido_al_que_le_faltan_campos_llega_como_desconocido() {
        let resub_sin_meses =
            json!({ "chatter_user_name": "Ana", "notice_type": "resub", "resub": {} });
        assert_eq!(
            translate_event("channel.chat.notification", resub_sin_meses),
            Some(unknown("channel.chat.notification:resub"))
        );
        let regalo_sin_total =
            json!({ "chatter_user_name": "Ana", "notice_type": "community_sub_gift" });
        assert_eq!(
            translate_event("channel.chat.notification", regalo_sin_total),
            Some(unknown("channel.chat.notification:community_sub_gift"))
        );
    }

    #[test]
    fn una_recompensa_canjeada_trae_el_titulo_y_el_texto_si_lo_hay() {
        let with_input = json!({
            "id": "1234",
            "user_name": "Cooler_User",
            "user_input": "pogchamp",
            "status": "unfulfilled",
            "reward": { "id": "9001", "title": "title", "cost": 100 }
        });
        assert_eq!(
            translate_event(
                "channel.channel_points_custom_reward_redemption.add",
                with_input
            ),
            Some(EventKind::RewardRedeemed {
                user: user("Cooler_User"),
                title: "title".to_owned(),
                input: Some("pogchamp".to_owned()),
            })
        );
        let without_input = json!({
            "user_name": "Cooler_User",
            "user_input": "",
            "reward": { "title": "Hidratarse" }
        });
        assert_eq!(
            translate_event(
                "channel.channel_points_custom_reward_redemption.add",
                without_input
            ),
            Some(EventKind::RewardRedeemed {
                user: user("Cooler_User"),
                title: "Hidratarse".to_owned(),
                input: None,
            })
        );
    }

    #[test]
    fn un_seguidor_nuevo_es_un_evento_followed() {
        let event = json!({
            "user_id": "1337",
            "user_login": "awesome_user",
            "user_name": "Awesome_User",
            "followed_at": "2020-07-15T18:16:11.17106713Z"
        });
        assert_eq!(
            translate_event("channel.follow", event),
            Some(EventKind::Followed {
                user: user("Awesome_User")
            })
        );
    }

    #[test]
    fn el_directo_empieza_sin_titulo_y_termina() {
        assert_eq!(
            translate_event("stream.online", json!({ "id": "9001", "type": "live" })),
            Some(EventKind::StreamStarted { title: None })
        );
        assert_eq!(
            translate_event("stream.offline", json!({ "broadcaster_user_id": "1337" })),
            Some(EventKind::StreamEnded)
        );
    }

    #[test]
    fn un_tipo_desconocido_o_con_campos_obligatorios_ausentes_llega_como_desconocido() {
        assert_eq!(
            translate_event("channel.raid", json!({ "viewers": 3 })),
            Some(unknown("channel.raid"))
        );
        assert_eq!(
            translate_event("channel.chat.message", json!({ "message_id": "m1" })),
            Some(unknown("channel.chat.message"))
        );
        assert_eq!(
            translate_event("channel.follow", Value::Null),
            Some(unknown("channel.follow"))
        );
        assert_eq!(
            translate_event("channel.chat.message_delete", json!({ "message_id": 7 })),
            Some(EventKind::ChatMessageDeleted {
                id: MessageId::new("7")
            })
        );
    }

    #[test]
    fn solo_las_notificaciones_se_traducen() {
        let welcome = json!({
            "metadata": { "message_type": "session_welcome" },
            "payload": { "session": { "id": "S1" } }
        });
        assert_eq!(event_from_notification(&welcome), None);
        assert_eq!(event_from_notification(&json!({})), None);
        let sin_tipo = json!({ "metadata": { "message_type": "notification" }, "payload": {} });
        assert_eq!(event_from_notification(&sin_tipo), None);
    }

    #[test]
    fn sin_subscription_type_en_los_metadatos_usa_el_de_la_suscripcion() {
        let message = json!({
            "metadata": { "message_type": "notification" },
            "payload": {
                "subscription": { "type": "stream.offline" },
                "event": {}
            }
        });
        assert_eq!(
            event_from_notification(&message),
            Some(EventKind::StreamEnded)
        );
    }
}
