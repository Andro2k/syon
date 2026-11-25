use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Instante en milisegundos desde el epoch Unix (1970-01-01 UTC). Es negativo para
/// fechas anteriores al epoch y se ordena cronológicamente.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Timestamp(i64);

impl Timestamp {
    /// Crea la marca a partir de milisegundos Unix.
    pub const fn from_unix_ms(milliseconds: i64) -> Self {
        Self(milliseconds)
    }

    /// Milisegundos Unix de esta marca.
    pub const fn as_unix_ms(self) -> i64 {
        self.0
    }
}

impl From<SystemTime> for Timestamp {
    /// Conversión pura: no lee el reloj, quien llama pasa `SystemTime::now()`. Trunca
    /// al milisegundo hacia el epoch y satura los valores fuera del rango de `i64`.
    fn from(time: SystemTime) -> Self {
        let milliseconds = match time.duration_since(UNIX_EPOCH) {
            Ok(after) => saturating_milliseconds(after),
            Err(before) => saturating_milliseconds(before.duration()).saturating_neg(),
        };
        Self(milliseconds)
    }
}

fn saturating_milliseconds(duration: Duration) -> i64 {
    i64::try_from(duration.as_millis()).unwrap_or(i64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn el_epoch_es_cero() {
        assert_eq!(Timestamp::from(UNIX_EPOCH).as_unix_ms(), 0);
    }

    #[test]
    fn convierte_instantes_posteriores_y_anteriores_al_epoch() {
        let after = UNIX_EPOCH + Duration::from_millis(1_500);
        let before = UNIX_EPOCH - Duration::from_secs(2);
        assert_eq!(Timestamp::from(after).as_unix_ms(), 1_500);
        assert_eq!(Timestamp::from(before).as_unix_ms(), -2_000);
    }

    #[test]
    fn trunca_al_milisegundo_hacia_el_epoch() {
        let time = UNIX_EPOCH + Duration::from_micros(2_999);
        assert_eq!(Timestamp::from(time).as_unix_ms(), 2);
    }

    #[test]
    fn se_ordena_cronologicamente() {
        assert!(Timestamp::from_unix_ms(-1) < Timestamp::from_unix_ms(0));
        assert!(Timestamp::from_unix_ms(1) > Timestamp::from_unix_ms(0));
    }

    #[test]
    fn satura_cuando_la_duracion_no_cabe_en_i64() {
        assert_eq!(saturating_milliseconds(Duration::MAX), i64::MAX);
    }
}
