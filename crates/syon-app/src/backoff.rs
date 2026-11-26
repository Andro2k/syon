use std::time::Duration;

const MIN_DELAY: Duration = Duration::from_secs(1);
const MAX_DELAY: Duration = Duration::from_secs(30);

/// Espera creciente entre reconexiones: 1 s, 2 s, 4 s... hasta 30 s.
///
/// Es lógica pura: quien la usa decide cómo esperar. Tras una conexión lograda hay que llamar a
/// [`Backoff::reset`] para que la próxima caída empiece otra vez en 1 s.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Backoff {
    next: Duration,
    attempt: u32,
}

impl Backoff {
    /// Crea una espera que empieza en 1 s.
    pub fn new() -> Self {
        Self {
            next: MIN_DELAY,
            attempt: 0,
        }
    }

    /// Devuelve la espera de este intento y prepara la siguiente, que dobla la actual sin pasar de 30 s.
    pub fn next_delay(&mut self) -> Duration {
        let delay = self.next;
        self.next = (self.next * 2).min(MAX_DELAY);
        self.attempt = self.attempt.saturating_add(1);
        delay
    }

    /// Número de esperas pedidas desde el último `reset`.
    pub fn attempt(&self) -> u32 {
        self.attempt
    }

    /// Vuelve a la espera inicial tras una conexión lograda.
    pub fn reset(&mut self) {
        *self = Self::new();
    }
}

impl Default for Backoff {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn la_espera_se_dobla_hasta_el_tope() {
        let mut backoff = Backoff::new();
        let secs: Vec<u64> = (0..8).map(|_| backoff.next_delay().as_secs()).collect();
        assert_eq!(secs, [1, 2, 4, 8, 16, 30, 30, 30]);
    }

    #[test]
    fn reset_vuelve_a_empezar_y_pone_el_contador_a_cero() {
        let mut backoff = Backoff::new();
        backoff.next_delay();
        backoff.next_delay();
        assert_eq!(backoff.attempt(), 2);
        backoff.reset();
        assert_eq!(backoff.attempt(), 0);
        assert_eq!(backoff.next_delay(), Duration::from_secs(1));
    }
}
