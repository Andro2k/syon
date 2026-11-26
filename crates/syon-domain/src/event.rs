use crate::{Platform, Timestamp, User};

/// Identificador de un mensaje de chat, tal como lo entrega la plataforma. Un borrado
/// se refiere al mensaje por este valor. El adaptador garantiza que no esté vacío.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MessageId(String);

impl MessageId {
    /// Crea un identificador a partir del texto de la plataforma.
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Texto del identificador.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Mensaje escrito en el chat de una plataforma.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatMessage {
    /// Identificador del mensaje, usado por los borrados.
    pub id: MessageId,
    /// Quien lo escribió.
    pub author: User,
    /// Texto del mensaje.
    pub text: String,
}

/// Algo que ocurrió en una plataforma, ya neutral a su formato original.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Event {
    /// Plataforma de origen.
    pub platform: Platform,
    /// Momento en que SYON recibió el evento, no el que declare la plataforma.
    pub received_at: Timestamp,
    /// Qué ocurrió.
    pub kind: EventKind,
}

/// Tipo de evento. Un evento que el adaptador no sabe interpretar llega como
/// [`EventKind::Unknown`], sin su contenido original.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EventKind {
    /// Mensaje de chat.
    Chat(ChatMessage),
    /// Un mensaje de chat fue borrado.
    ChatMessageDeleted {
        /// Mensaje borrado.
        id: MessageId,
    },
    /// Un usuario fue baneado o expulsado temporalmente.
    UserBanned {
        /// Usuario afectado.
        user: User,
        /// Quien aplicó la sanción, si la plataforma lo indica.
        by: Option<User>,
        /// `true` si es un ban permanente; `false` si es un timeout.
        permanent: bool,
    },
    /// Alguien se suscribió o renovó su suscripción.
    Subscription {
        /// Suscriptor.
        user: User,
        /// Meses acumulados de suscripción.
        months: u32,
    },
    /// Alguien regaló suscripciones.
    GiftedSubs {
        /// Quien regala.
        gifter: User,
        /// Cantidad de suscripciones regaladas.
        count: u32,
    },
    /// Alguien canjeó una recompensa del canal.
    RewardRedeemed {
        /// Quien la canjeó.
        user: User,
        /// Título de la recompensa.
        title: String,
        /// Texto que escribió al canjearla, si la recompensa lo pide.
        input: Option<String>,
    },
    /// Alguien empezó a seguir el canal. Lo entregan las plataformas que avisan de cada
    /// seguidor; las que solo informan del total usan [`EventKind::FollowersUpdated`].
    Followed {
        /// Nuevo seguidor.
        user: User,
    },
    /// Cambió el total de seguidores del canal.
    FollowersUpdated {
        /// Total de seguidores.
        count: u64,
    },
    /// El directo comenzó.
    StreamStarted {
        /// Título del directo, si la plataforma lo indica.
        title: Option<String>,
    },
    /// El directo terminó.
    StreamEnded,
    /// Evento que el adaptador no supo interpretar.
    Unknown {
        /// Nombre del evento tal como lo entrega la plataforma.
        name: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chat(id: &str) -> ChatMessage {
        ChatMessage {
            id: MessageId::new(id),
            author: User::new("ana"),
            text: "hola".to_owned(),
        }
    }

    #[test]
    fn el_evento_se_comparte_entre_hilos() {
        fn assert_send_sync<T: Send + Sync + Clone>() {}
        assert_send_sync::<Event>();
    }

    #[test]
    fn un_borrado_se_refiere_al_mensaje_por_su_id() {
        let message = chat("m1");
        let deleted = EventKind::ChatMessageDeleted {
            id: MessageId::new("m1"),
        };
        assert_eq!(
            deleted,
            EventKind::ChatMessageDeleted {
                id: message.id.clone()
            }
        );
        assert_ne!(
            deleted,
            EventKind::ChatMessageDeleted {
                id: MessageId::new("m2")
            }
        );
    }

    #[test]
    fn los_eventos_se_comparan_por_valor() {
        let at = Timestamp::from_unix_ms(10);
        let kick = Event {
            platform: Platform::Kick,
            received_at: at,
            kind: EventKind::Chat(chat("m1")),
        };
        let twitch = Event {
            platform: Platform::Twitch,
            ..kick.clone()
        };
        assert_eq!(kick, kick.clone());
        assert_ne!(kick, twitch);
    }

    #[test]
    fn un_seguidor_nuevo_y_el_total_son_eventos_distintos() {
        let followed = EventKind::Followed {
            user: User::new("ana"),
        };
        assert_eq!(
            followed,
            EventKind::Followed {
                user: User::new("ana")
            }
        );
        assert_ne!(followed, EventKind::FollowersUpdated { count: 1 });
    }

    #[test]
    fn el_texto_del_id_se_conserva() {
        assert_eq!(MessageId::new("abc").as_str(), "abc");
    }
}
