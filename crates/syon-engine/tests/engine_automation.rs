//! El consumidor de automatización se suscribe antes de recibir eventos y responde fuera de publish.

use std::{env, fs, sync::mpsc, time::Duration};
use syon_app::{ChatSender, ChatText, EventSink, SendError, SendFuture, SentMessage};
use syon_domain::{ChatMessage, ChatRoles, Event, EventKind, MessageId, Platform, Timestamp, User};
use syon_engine::{AppPaths, Engine};

struct Sender(mpsc::Sender<String>);

impl ChatSender for Sender {
    fn platform(&self) -> Platform {
        Platform::Twitch
    }

    fn send<'a>(&'a self, text: &'a ChatText) -> SendFuture<'a> {
        let result = self
            .0
            .send(text.as_str().to_owned())
            .map_err(|_| SendError::Failed("canal de prueba cerrado".into()));
        Box::pin(async move {
            result?;
            Ok(SentMessage {
                id: Some(MessageId::new("echo-1")),
            })
        })
    }
}

#[test]
fn el_consumidor_automatico_responde_tras_publicar_sin_bloquear_al_productor() {
    let root = env::temp_dir().join(format!("syon-engine-automation-{}", std::process::id()));
    let paths = AppPaths::under(&root);
    fs::create_dir_all(paths.config_dir()).unwrap();
    fs::write(paths.settings_file(), "[[chat.commands]]\nname = \"hola\"\nresponse = \"Hola {user} {platform} {args}\"\nminimum_role = \"everyone\"\n").unwrap();
    let mut engine = Engine::start(paths, "1.2.3").unwrap();
    let (sent_tx, sent_rx) = mpsc::channel();
    engine.add_chat_sender(std::sync::Arc::new(Sender(sent_tx)));
    engine.bus().publish(Event {
        platform: Platform::Twitch,
        received_at: Timestamp::from_unix_ms(1),
        kind: EventKind::Chat(ChatMessage {
            id: MessageId::new("incoming-1"),
            author: User::new("ana"),
            author_id: Some("42".into()),
            roles: ChatRoles::default(),
            is_self: Some(false),
            text: "!hola mundo".into(),
        }),
    });
    assert_eq!(
        sent_rx.recv_timeout(Duration::from_secs(2)).unwrap(),
        "Hola ana twitch mundo"
    );
    drop(engine);
    let _ = fs::remove_dir_all(&root);
}
