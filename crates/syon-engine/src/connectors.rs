use syon_app::{ConnectionStatus, PlatformConnector};
use syon_domain::Platform;

/// Conectores de plataforma que el engine arrancó y vigila (diseño 002, sección 4).
#[derive(Default)]
pub(crate) struct ConnectorRegistry {
    connectors: Vec<Box<dyn PlatformConnector>>,
}

impl ConnectorRegistry {
    /// Registra un conector ya arrancado.
    pub(crate) fn add(&mut self, connector: Box<dyn PlatformConnector>) {
        log::info!("engine: conector de {:?} registrado", connector.platform());
        self.connectors.push(connector);
    }

    /// Estado de cada conector, en el orden en que se registraron.
    pub(crate) fn statuses(&self) -> Vec<(Platform, ConnectionStatus)> {
        self.connectors
            .iter()
            .map(|connector| (connector.platform(), connector.status()))
            .collect()
    }

    /// Pide a todos los conectores que se detengan. Puede llamarse más de una vez.
    pub(crate) fn stop_all(&self) {
        for connector in &self.connectors {
            connector.stop();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    /// Conector de prueba que anota cuántas veces lo detuvieron.
    struct Fake {
        platform: Platform,
        stops: Arc<Mutex<u32>>,
    }

    impl PlatformConnector for Fake {
        fn platform(&self) -> Platform {
            self.platform
        }

        fn status(&self) -> ConnectionStatus {
            if *self.stops.lock().unwrap() == 0 {
                ConnectionStatus::Connected
            } else {
                ConnectionStatus::Disconnected
            }
        }

        fn stop(&self) {
            *self.stops.lock().unwrap() += 1;
        }
    }

    fn fake(platform: Platform) -> (Box<dyn PlatformConnector>, Arc<Mutex<u32>>) {
        let stops = Arc::new(Mutex::new(0));
        let connector = Fake {
            platform,
            stops: Arc::clone(&stops),
        };
        (Box::new(connector), stops)
    }

    #[test]
    fn el_estado_sale_en_el_orden_de_registro() {
        let mut registry = ConnectorRegistry::default();
        assert!(registry.statuses().is_empty());
        registry.add(fake(Platform::Twitch).0);
        registry.add(fake(Platform::Kick).0);
        assert_eq!(
            registry.statuses(),
            [
                (Platform::Twitch, ConnectionStatus::Connected),
                (Platform::Kick, ConnectionStatus::Connected)
            ]
        );
    }

    #[test]
    fn stop_all_detiene_a_todos_y_se_puede_repetir() {
        let mut registry = ConnectorRegistry::default();
        let (kick, kick_stops) = fake(Platform::Kick);
        let (twitch, twitch_stops) = fake(Platform::Twitch);
        registry.add(kick);
        registry.add(twitch);
        registry.stop_all();
        registry.stop_all();
        assert_eq!(*kick_stops.lock().unwrap(), 2);
        assert_eq!(*twitch_stops.lock().unwrap(), 2);
        assert_eq!(registry.statuses()[0].1, ConnectionStatus::Disconnected);
    }
}
