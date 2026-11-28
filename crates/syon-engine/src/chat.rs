use std::sync::{Arc, Mutex, PoisonError};
use syon_app::{ChatSender, ChatText, SendError, SentMessage};
use syon_domain::Platform;
use tokio::runtime::Handle;

/// Resultado de intentar enviar un mensaje a una plataforma.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SendResult {
    /// Plataforma destino.
    pub platform: Platform,
    /// Resultado devuelto por esa plataforma.
    pub outcome: Result<SentMessage, SendError>,
}

#[derive(Default)]
struct SenderState {
    senders: Vec<Arc<dyn ChatSender>>,
    logged_out: Vec<Platform>,
}

/// Registro interno de emisores de chat y plataformas activas sin sesión de envío.
#[derive(Clone, Default)]
pub(crate) struct SenderRegistry {
    state: Arc<Mutex<SenderState>>,
}

impl SenderRegistry {
    pub(crate) fn add_sender(&self, sender: Arc<dyn ChatSender>) {
        let platform = sender.platform();
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        state
            .senders
            .retain(|existing| existing.platform() != platform);
        state.logged_out.retain(|&existing| existing != platform);
        state.senders.push(sender);
        log::info!("engine: emisor de chat de {platform:?} registrado");
    }

    pub(crate) fn mark_logged_out(&self, platform: Platform) {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        if !state.senders.iter().any(|s| s.platform() == platform)
            && !state.logged_out.contains(&platform)
        {
            state.logged_out.push(platform);
        }
    }

    pub(crate) fn platforms(&self) -> Vec<Platform> {
        let state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        state.senders.iter().map(|s| s.platform()).collect()
    }

    fn snapshot(&self) -> (Vec<Arc<dyn ChatSender>>, Vec<Platform>) {
        let state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        (state.senders.clone(), state.logged_out.clone())
    }
}

/// Manejador clonable para enviar mensajes de chat desde fuera del runtime del engine.
#[derive(Clone)]
pub struct ChatHandle {
    registry: SenderRegistry,
    runtime: Handle,
}

impl ChatHandle {
    pub(crate) fn new(registry: SenderRegistry, runtime: Handle) -> Self {
        Self { registry, runtime }
    }

    /// Envía `text` a cada plataforma de `platforms` en paralelo y devuelve un resultado por
    /// plataforma en el orden pedido (sin repetidas).
    ///
    /// Bloquea sobre el runtime del engine, así que debe llamarse desde fuera de él.
    pub fn send(&self, platforms: &[Platform], text: &str) -> Vec<SendResult> {
        self.runtime.block_on(self.send_async(platforms, text))
    }

    /// Envía `text` desde una tarea asíncrona y devuelve un resultado por destino, en el orden pedido.
    pub async fn send_async(&self, platforms: &[Platform], text: &str) -> Vec<SendResult> {
        let unique = dedup_platforms(platforms);
        if unique.is_empty() {
            return Vec::new();
        }
        let chat_text = match ChatText::new(text) {
            Ok(valid) => valid,
            Err(error) => {
                return unique
                    .into_iter()
                    .map(|platform| SendResult {
                        platform,
                        outcome: Err(error.clone()),
                    })
                    .collect();
            }
        };

        let (senders, logged_out) = self.registry.snapshot();
        enum Pending {
            Immediate(Result<SentMessage, SendError>),
            Task(tokio::task::JoinHandle<Result<SentMessage, SendError>>),
        }

        let mut pending = Vec::with_capacity(unique.len());
        for platform in unique {
            if let Some(sender) = senders.iter().find(|s| s.platform() == platform).cloned() {
                let text_for_task = chat_text.clone();
                let task = self
                    .runtime
                    .spawn(async move { sender.send(&text_for_task).await });
                pending.push((platform, Pending::Task(task)));
            } else if logged_out.contains(&platform) {
                pending.push((platform, Pending::Immediate(Err(SendError::NotLoggedIn))));
            } else {
                pending.push((platform, Pending::Immediate(Err(SendError::NotRunning))));
            }
        }

        let mut results = Vec::with_capacity(pending.len());
        for (platform, item) in pending {
            let outcome = match item {
                Pending::Immediate(outcome) => outcome,
                Pending::Task(task) => match task.await {
                    Ok(outcome) => outcome,
                    Err(error) => Err(SendError::Failed(format!(
                        "fallo interno al enviar: {error}"
                    ))),
                },
            };
            results.push(SendResult { platform, outcome });
        }
        results
    }
}

fn dedup_platforms(platforms: &[Platform]) -> Vec<Platform> {
    let mut unique = Vec::with_capacity(platforms.len());
    for &platform in platforms {
        if !unique.contains(&platform) {
            unique.push(platform);
        }
    }
    unique
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use syon_app::SendFuture;
    use syon_domain::MessageId;
    use tokio::runtime::Builder;

    struct FakeSender {
        platform: Platform,
        outcome: Result<SentMessage, SendError>,
        calls: Arc<AtomicUsize>,
    }

    impl ChatSender for FakeSender {
        fn platform(&self) -> Platform {
            self.platform
        }

        fn send<'a>(&'a self, _text: &'a ChatText) -> SendFuture<'a> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            let outcome = self.outcome.clone();
            Box::pin(async move { outcome })
        }
    }

    #[test]
    fn envia_en_orden_sin_repetir_y_un_fallo_no_afecta_al_otro() {
        let runtime = Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .unwrap();
        let registry = SenderRegistry::default();
        let kick_calls = Arc::new(AtomicUsize::new(0));
        let twitch_calls = Arc::new(AtomicUsize::new(0));

        registry.add_sender(Arc::new(FakeSender {
            platform: Platform::Kick,
            outcome: Err(SendError::Rejected("bloqueado".into())),
            calls: Arc::clone(&kick_calls),
        }));
        registry.add_sender(Arc::new(FakeSender {
            platform: Platform::Twitch,
            outcome: Ok(SentMessage {
                id: Some(MessageId::new("tw-1")),
            }),
            calls: Arc::clone(&twitch_calls),
        }));

        let handle = ChatHandle::new(registry, runtime.handle().clone());
        let results = handle.send(
            &[Platform::Twitch, Platform::Kick, Platform::Twitch],
            "  hola chat  ",
        );

        assert_eq!(kick_calls.load(Ordering::SeqCst), 1);
        assert_eq!(twitch_calls.load(Ordering::SeqCst), 1);
        assert_eq!(
            results,
            vec![
                SendResult {
                    platform: Platform::Twitch,
                    outcome: Ok(SentMessage {
                        id: Some(MessageId::new("tw-1")),
                    }),
                },
                SendResult {
                    platform: Platform::Kick,
                    outcome: Err(SendError::Rejected("bloqueado".into())),
                },
            ]
        );
    }

    #[test]
    fn valida_el_texto_antes_de_llamar_a_los_senders() {
        let runtime = Builder::new_current_thread().build().unwrap();
        let registry = SenderRegistry::default();
        let calls = Arc::new(AtomicUsize::new(0));
        registry.add_sender(Arc::new(FakeSender {
            platform: Platform::Kick,
            outcome: Ok(SentMessage { id: None }),
            calls: Arc::clone(&calls),
        }));
        let handle = ChatHandle::new(registry, runtime.handle().clone());
        let results = handle.send(&[Platform::Kick], "   ");
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        assert_eq!(
            results,
            vec![SendResult {
                platform: Platform::Kick,
                outcome: Err(SendError::Empty),
            }]
        );
    }

    #[test]
    fn distingue_not_logged_in_de_not_running() {
        let runtime = Builder::new_current_thread().build().unwrap();
        let registry = SenderRegistry::default();
        registry.mark_logged_out(Platform::Kick);
        let handle = ChatHandle::new(registry, runtime.handle().clone());

        let results = handle.send(&[Platform::Kick, Platform::Twitch], "hola");
        assert_eq!(
            results,
            vec![
                SendResult {
                    platform: Platform::Kick,
                    outcome: Err(SendError::NotLoggedIn),
                },
                SendResult {
                    platform: Platform::Twitch,
                    outcome: Err(SendError::NotRunning),
                },
            ]
        );
    }

    #[test]
    fn send_async_se_puede_usar_desde_una_tarea_del_runtime() {
        let runtime = Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .unwrap();
        let registry = SenderRegistry::default();
        let calls = Arc::new(AtomicUsize::new(0));
        registry.add_sender(Arc::new(FakeSender {
            platform: Platform::Kick,
            outcome: Ok(SentMessage {
                id: Some(MessageId::new("echo-1")),
            }),
            calls: Arc::clone(&calls),
        }));
        let handle = ChatHandle::new(registry, runtime.handle().clone());
        let results = runtime.block_on(async move {
            tokio::spawn(async move { handle.send_async(&[Platform::Kick], "hola").await })
                .await
                .unwrap()
        });
        assert!(results[0].outcome.is_ok());
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }
}
