use crate::api::SubscriptionRequest;

/// Forma de la condición de una suscripción.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Condition {
    /// Solo el canal: `broadcaster_user_id`.
    Broadcaster,
    /// Canal y usuario que lee el chat: `broadcaster_user_id` y `user_id`.
    Chat,
    /// Canal y moderador que autoriza: `broadcaster_user_id` y `moderator_user_id`.
    Moderator,
}

/// Una de las suscripciones de EventSub que SYON crea al conectar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SubscriptionSpec {
    /// Nombre del tipo de suscripción.
    pub kind: &'static str,
    /// Versión del tipo.
    pub version: &'static str,
    /// Permiso que exige el token de usuario; `None` si no exige ninguno.
    pub scope: Option<&'static str>,
    condition: Condition,
}

/// Las ocho suscripciones del diseño 002 (sección 7). Cada permiso está en
/// [`DEFAULT_SCOPES`](crate::DEFAULT_SCOPES).
pub const SUBSCRIPTIONS: [SubscriptionSpec; 8] = [
    SubscriptionSpec {
        kind: "channel.chat.message",
        version: "1",
        scope: Some("user:read:chat"),
        condition: Condition::Chat,
    },
    SubscriptionSpec {
        kind: "channel.chat.message_delete",
        version: "1",
        scope: Some("user:read:chat"),
        condition: Condition::Chat,
    },
    SubscriptionSpec {
        kind: "channel.chat.notification",
        version: "1",
        scope: Some("user:read:chat"),
        condition: Condition::Chat,
    },
    SubscriptionSpec {
        kind: "channel.ban",
        version: "1",
        scope: Some("channel:moderate"),
        condition: Condition::Broadcaster,
    },
    SubscriptionSpec {
        kind: "channel.channel_points_custom_reward_redemption.add",
        version: "1",
        scope: Some("channel:read:redemptions"),
        condition: Condition::Broadcaster,
    },
    SubscriptionSpec {
        kind: "channel.follow",
        version: "2",
        scope: Some("moderator:read:followers"),
        condition: Condition::Moderator,
    },
    SubscriptionSpec {
        kind: "stream.online",
        version: "1",
        scope: None,
        condition: Condition::Broadcaster,
    },
    SubscriptionSpec {
        kind: "stream.offline",
        version: "1",
        scope: None,
        condition: Condition::Broadcaster,
    },
];

/// Las peticiones de suscripción de un canal propio: quien inició sesión (`user_id`) es a la vez el
/// canal, el usuario que lee el chat y el moderador que autoriza los seguidores.
pub fn subscription_requests(user_id: &str) -> Vec<SubscriptionRequest> {
    SUBSCRIPTIONS
        .iter()
        .map(|spec| {
            let mut condition = vec![("broadcaster_user_id", user_id.to_owned())];
            match spec.condition {
                Condition::Broadcaster => {}
                Condition::Chat => condition.push(("user_id", user_id.to_owned())),
                Condition::Moderator => condition.push(("moderator_user_id", user_id.to_owned())),
            }
            SubscriptionRequest {
                kind: spec.kind,
                version: spec.version,
                condition,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DEFAULT_SCOPES;
    use std::collections::HashSet;

    #[test]
    fn son_ocho_suscripciones_distintas() {
        let kinds: HashSet<_> = SUBSCRIPTIONS.iter().map(|s| s.kind).collect();
        assert_eq!(kinds.len(), 8);
    }

    #[test]
    fn cada_permiso_exigido_esta_entre_los_que_se_piden_al_iniciar_sesion() {
        for spec in SUBSCRIPTIONS {
            if let Some(scope) = spec.scope {
                assert!(
                    DEFAULT_SCOPES.contains(&scope),
                    "{} exige {scope}, que no se pide",
                    spec.kind
                );
            }
        }
    }

    #[test]
    fn cada_permiso_que_se_pide_lo_necesita_alguna_suscripcion_salvo_el_de_escribir() {
        for scope in DEFAULT_SCOPES.iter().filter(|s| **s != "user:write:chat") {
            assert!(
                SUBSCRIPTIONS.iter().any(|spec| spec.scope == Some(*scope)),
                "{scope} no lo usa ninguna suscripción"
            );
        }
    }

    #[test]
    fn las_condiciones_llevan_el_canal_y_lo_que_pide_cada_tipo() {
        let requests = subscription_requests("1337");
        let condition = |kind: &str| {
            requests
                .iter()
                .find(|r| r.kind == kind)
                .unwrap()
                .condition
                .clone()
        };
        let id = |key: &'static str| (key, "1337".to_owned());
        assert_eq!(
            condition("channel.chat.message"),
            [id("broadcaster_user_id"), id("user_id")]
        );
        assert_eq!(
            condition("channel.chat.notification"),
            [id("broadcaster_user_id"), id("user_id")]
        );
        assert_eq!(
            condition("channel.follow"),
            [id("broadcaster_user_id"), id("moderator_user_id")]
        );
        assert_eq!(condition("channel.ban"), [id("broadcaster_user_id")]);
        assert_eq!(condition("stream.online"), [id("broadcaster_user_id")]);
        assert_eq!(
            requests
                .iter()
                .find(|r| r.kind == "channel.follow")
                .unwrap()
                .version,
            "2"
        );
    }
}
