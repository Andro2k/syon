use std::{
    sync::{
        Arc, Mutex, PoisonError,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};
use syon_domain::Event;
use tokio::sync::mpsc::{self, error::TrySendError};

/// Intervalo mínimo entre dos avisos de saturación del mismo consumidor.
const WARNING_INTERVAL: Duration = Duration::from_secs(1);

/// Resultado de entregar un evento a un consumidor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Delivery {
    /// El evento quedó en la cola del consumidor.
    Delivered,
    /// La cola estaba llena y el evento se descartó para este consumidor.
    Dropped,
    /// El consumidor ya no escucha.
    Closed,
}

/// Estado del canal de un consumidor del bus, para diagnóstico.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubscriberStats {
    /// Nombre con el que se suscribió el consumidor.
    pub name: String,
    /// Capacidad de su cola.
    pub capacity: usize,
    /// Eventos que esperan en la cola.
    pub queued: usize,
    /// Eventos descartados desde que se suscribió porque la cola estaba llena.
    pub dropped: u64,
}

/// Un consumidor del bus: su canal acotado y sus contadores.
pub(crate) struct Subscriber {
    name: String,
    tx: mpsc::Sender<Arc<Event>>,
    dropped: AtomicU64,
    last_warning: Mutex<Option<Instant>>,
}

impl Subscriber {
    pub(crate) fn new(name: String, tx: mpsc::Sender<Arc<Event>>) -> Self {
        Self {
            name,
            tx,
            dropped: AtomicU64::new(0),
            last_warning: Mutex::new(None),
        }
    }

    /// Intenta entregar el evento sin esperar. Si la cola está llena lo descarta, lo
    /// cuenta y avisa en el log como máximo una vez por segundo.
    pub(crate) fn deliver(&self, event: &Arc<Event>) -> Delivery {
        match self.tx.try_send(Arc::clone(event)) {
            Ok(()) => Delivery::Delivered,
            Err(TrySendError::Full(_)) => {
                let total = self.dropped.fetch_add(1, Ordering::Relaxed) + 1;
                if self.should_warn(Instant::now()) {
                    log::warn!(
                        "bus: el consumidor «{}» está saturado; lleva {total} eventos descartados",
                        self.name
                    );
                }
                Delivery::Dropped
            }
            Err(TrySendError::Closed(_)) => Delivery::Closed,
        }
    }

    pub(crate) fn is_closed(&self) -> bool {
        self.tx.is_closed()
    }

    pub(crate) fn stats(&self) -> SubscriberStats {
        SubscriberStats {
            name: self.name.clone(),
            capacity: self.tx.max_capacity(),
            queued: self.tx.max_capacity() - self.tx.capacity(),
            dropped: self.dropped.load(Ordering::Relaxed),
        }
    }

    /// Decide si toca emitir un aviso en `now`, y lo registra como emitido si es así.
    fn should_warn(&self, now: Instant) -> bool {
        let mut last = self
            .last_warning
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        match *last {
            Some(previous) if now.saturating_duration_since(previous) < WARNING_INTERVAL => false,
            _ => {
                *last = Some(now);
                true
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn subscriber() -> Subscriber {
        let (tx, _rx) = mpsc::channel(1);
        Subscriber::new("prueba".to_owned(), tx)
    }

    #[test]
    fn avisa_la_primera_vez_y_luego_como_maximo_una_vez_por_segundo() {
        let subscriber = subscriber();
        let start = Instant::now();
        assert!(subscriber.should_warn(start));
        assert!(!subscriber.should_warn(start + Duration::from_millis(500)));
        assert!(!subscriber.should_warn(start + Duration::from_millis(999)));
        assert!(subscriber.should_warn(start + Duration::from_secs(1)));
        assert!(!subscriber.should_warn(start + Duration::from_millis(1_500)));
    }
}
