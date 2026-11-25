use crate::subscriber::{Delivery, Subscriber, SubscriberStats};
use std::sync::{Arc, PoisonError, RwLock};
use syon_app::EventSink;
use syon_domain::Event;
use tokio::sync::mpsc;

/// Bus de eventos: reparte cada evento publicado a todos los consumidores suscritos.
///
/// Cada consumidor tiene su propio canal acotado de `Arc<Event>`; el evento no se copia.
/// `publish` nunca espera: si la cola de un consumidor está llena, el evento se descarta
/// solo para ese consumidor, queda contado en [`EventBus::stats`] y se avisa en el log.
/// Un consumidor lento no frena a quien publica ni a los demás consumidores.
///
/// Es barato de clonar: todas las copias comparten los mismos consumidores.
#[derive(Clone, Default)]
pub struct EventBus {
    subscribers: Arc<RwLock<Vec<Arc<Subscriber>>>>,
}

impl EventBus {
    /// Crea un bus sin consumidores.
    pub fn new() -> Self {
        Self::default()
    }

    /// Suscribe un consumidor con su propia cola de `capacity` eventos (mínimo 1; un
    /// valor menor se trata como 1). Recibe los eventos publicados a partir de ahora.
    /// Al soltar el receptor, el consumidor se retira del bus.
    pub fn subscribe(
        &self,
        name: impl Into<String>,
        capacity: usize,
    ) -> mpsc::Receiver<Arc<Event>> {
        let (tx, rx) = mpsc::channel(capacity.max(1));
        let subscriber = Arc::new(Subscriber::new(name.into(), tx));
        self.subscribers
            .write()
            .unwrap_or_else(PoisonError::into_inner)
            .push(subscriber);
        rx
    }

    /// Estado de cada consumidor activo, en el orden en que se suscribieron.
    pub fn stats(&self) -> Vec<SubscriberStats> {
        self.subscribers
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .iter()
            .map(|subscriber| subscriber.stats())
            .collect()
    }
}

impl EventSink for EventBus {
    fn publish(&self, event: Event) {
        let event = Arc::new(event);
        let mut any_closed = false;
        for subscriber in self
            .subscribers
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .iter()
        {
            any_closed |= subscriber.deliver(&event) == Delivery::Closed;
        }
        if any_closed {
            self.subscribers
                .write()
                .unwrap_or_else(PoisonError::into_inner)
                .retain(|subscriber| !subscriber.is_closed());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{sync::mpsc as std_mpsc, thread, time::Duration};
    use syon_domain::{EventKind, Platform, Timestamp};

    fn event(ms: i64) -> Event {
        Event {
            platform: Platform::Kick,
            received_at: Timestamp::from_unix_ms(ms),
            kind: EventKind::StreamEnded,
        }
    }

    /// Publica `count` eventos desde otro hilo y falla si tarda más de 5 s, para que un
    /// bloqueo del bus rompa la prueba en lugar de colgarla.
    fn publish_without_blocking(bus: &EventBus, count: i64) {
        let bus = bus.clone();
        let (done_tx, done_rx) = std_mpsc::channel();
        thread::spawn(move || {
            for ms in 0..count {
                bus.publish(event(ms));
            }
            done_tx.send(()).unwrap();
        });
        done_rx
            .recv_timeout(Duration::from_secs(5))
            .expect("publish se bloqueó");
    }

    #[test]
    fn entrega_cada_evento_a_todos_los_consumidores_sin_copiarlo() {
        let bus = EventBus::new();
        let mut first = bus.subscribe("primero", 4);
        let mut second = bus.subscribe("segundo", 4);
        bus.publish(event(7));
        let from_first = first.try_recv().unwrap();
        let from_second = second.try_recv().unwrap();
        assert_eq!(from_first.received_at.as_unix_ms(), 7);
        assert!(Arc::ptr_eq(&from_first, &from_second));
    }

    #[test]
    fn un_consumidor_detenido_descarta_y_cuenta_sin_afectar_a_otro_ni_bloquear() {
        let bus = EventBus::new();
        let _stalled = bus.subscribe("detenido", 2);
        let mut healthy = bus.subscribe("sano", 64);

        publish_without_blocking(&bus, 10);

        let mut received = 0;
        while healthy.try_recv().is_ok() {
            received += 1;
        }
        assert_eq!(received, 10);
        let stats = bus.stats();
        assert_eq!(
            stats[0],
            SubscriberStats {
                name: "detenido".to_owned(),
                capacity: 2,
                queued: 2,
                dropped: 8,
            }
        );
        assert_eq!(stats[1].dropped, 0);
    }

    #[test]
    fn las_estadisticas_reflejan_la_cola_de_cada_consumidor() {
        let bus = EventBus::new();
        let mut consumer = bus.subscribe("uno", 8);
        for ms in 0..3 {
            bus.publish(event(ms));
        }
        assert_eq!(bus.stats()[0].queued, 3);
        consumer.try_recv().unwrap();
        assert_eq!(bus.stats()[0].queued, 2);
    }

    #[test]
    fn un_consumidor_que_suelta_su_receptor_se_retira() {
        let bus = EventBus::new();
        let gone = bus.subscribe("se_va", 4);
        let mut stays = bus.subscribe("se_queda", 4);
        drop(gone);
        bus.publish(event(1));
        let stats = bus.stats();
        assert_eq!(stats.len(), 1);
        assert_eq!(stats[0].name, "se_queda");
        assert!(stays.try_recv().is_ok());
    }

    #[test]
    fn una_capacidad_de_cero_se_trata_como_uno() {
        let bus = EventBus::new();
        let _consumer = bus.subscribe("minimo", 0);
        assert_eq!(bus.stats()[0].capacity, 1);
    }

    #[test]
    fn publicar_sin_consumidores_no_falla() {
        EventBus::new().publish(event(1));
    }

    #[test]
    fn varios_hilos_publican_sin_perder_eventos_si_hay_capacidad() {
        let bus = EventBus::new();
        let mut consumer = bus.subscribe("todo", 1_024);
        thread::scope(|scope| {
            for thread in 0..4 {
                let bus = bus.clone();
                scope.spawn(move || {
                    for n in 0..100 {
                        bus.publish(event(thread * 1_000 + n));
                    }
                });
            }
        });
        let mut received = 0;
        while consumer.try_recv().is_ok() {
            received += 1;
        }
        assert_eq!(received, 400);
        assert_eq!(bus.stats()[0].dropped, 0);
    }

    #[test]
    fn el_bus_se_usa_como_sumidero_compartido() {
        let bus = EventBus::new();
        let mut consumer = bus.subscribe("uno", 4);
        let sink: Arc<dyn EventSink> = Arc::new(bus);
        sink.publish(event(3));
        assert_eq!(consumer.try_recv().unwrap().received_at.as_unix_ms(), 3);
    }
}
