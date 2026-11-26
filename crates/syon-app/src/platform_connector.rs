use syon_domain::Platform;

/// Estado de la conexión de una plataforma.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionStatus {
    /// Sin conexión y sin intentar conectar.
    Disconnected,
    /// Conectando por primera vez.
    Connecting,
    /// Recibiendo eventos.
    Connected,
    /// Se perdió la conexión y se espera para reintentar.
    Reconnecting {
        /// Número de intento desde la última conexión lograda.
        attempt: u32,
    },
}

/// Conexión de una plataforma que el engine arranca y vigila. Cada adaptador publica los eventos
/// que recibe en un [`EventSink`](crate::EventSink) que recibe al crearse.
///
/// Los métodos son síncronos y no esperan red: el adaptador trabaja en sus propias tareas y estos
/// métodos solo leen o piden un cambio de estado.
pub trait PlatformConnector: Send + Sync {
    /// Plataforma a la que pertenece la conexión.
    fn platform(&self) -> Platform;

    /// Estado actual de la conexión.
    fn status(&self) -> ConnectionStatus;

    /// Pide cerrar la conexión y dejar de reconectar. Puede llamarse más de una vez.
    fn stop(&self);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// Conector de prueba que cumple el contrato del puerto.
    struct Fake(Mutex<ConnectionStatus>);

    impl PlatformConnector for Fake {
        fn platform(&self) -> Platform {
            Platform::Twitch
        }

        fn status(&self) -> ConnectionStatus {
            *self.0.lock().unwrap()
        }

        fn stop(&self) {
            *self.0.lock().unwrap() = ConnectionStatus::Disconnected;
        }
    }

    #[test]
    fn el_engine_puede_vigilar_conectores_de_distintas_plataformas_como_trait_objects() {
        let connectors: Vec<Box<dyn PlatformConnector>> =
            vec![Box::new(Fake(Mutex::new(ConnectionStatus::Connected)))];
        assert_eq!(connectors[0].platform(), Platform::Twitch);
        assert_eq!(connectors[0].status(), ConnectionStatus::Connected);
        connectors[0].stop();
        connectors[0].stop();
        assert_eq!(connectors[0].status(), ConnectionStatus::Disconnected);
    }
}
