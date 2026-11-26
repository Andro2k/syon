use std::time::SystemTime;
use syon_domain::Timestamp;

/// Hora actual. El dominio no lee el reloj, así que el adaptador la toma aquí.
pub(crate) fn now() -> Timestamp {
    Timestamp::from(SystemTime::now())
}
