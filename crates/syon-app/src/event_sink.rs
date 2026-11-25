use syon_domain::Event;

/// Puerto por el que las plataformas entregan eventos al resto de SYON.
///
/// `publish` nunca bloquea ni espera: si el destino no puede atender el evento, lo
/// descarta por su cuenta y lo registra, para que una plataforma no frene a las demás.
pub trait EventSink: Send + Sync {
    /// Entrega un evento. No devuelve error: el productor no puede hacer nada útil con él.
    fn publish(&self, event: Event);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};
    use syon_domain::{EventKind, Platform, Timestamp};

    #[derive(Default)]
    struct Collector(Mutex<Vec<Event>>);

    impl EventSink for Collector {
        fn publish(&self, event: Event) {
            self.0.lock().unwrap().push(event);
        }
    }

    fn event(ms: i64) -> Event {
        Event {
            platform: Platform::Kick,
            received_at: Timestamp::from_unix_ms(ms),
            kind: EventKind::StreamEnded,
        }
    }

    #[test]
    fn se_puede_publicar_a_traves_de_un_trait_object_compartido() {
        let collector = Arc::new(Collector::default());
        let sink: Arc<dyn EventSink> = collector.clone();
        sink.publish(event(1));
        sink.publish(event(2));
        let received = collector.0.lock().unwrap();
        assert_eq!(received.len(), 2);
        assert_eq!(received[1].received_at.as_unix_ms(), 2);
    }

    #[test]
    fn varios_hilos_pueden_publicar_en_el_mismo_sumidero() {
        let collector = Arc::new(Collector::default());
        let sink: Arc<dyn EventSink> = collector.clone();
        std::thread::scope(|scope| {
            for thread in 0..4 {
                let sink = Arc::clone(&sink);
                scope.spawn(move || {
                    for n in 0..25 {
                        sink.publish(event(thread * 100 + n));
                    }
                });
            }
        });
        assert_eq!(collector.0.lock().unwrap().len(), 100);
    }
}
