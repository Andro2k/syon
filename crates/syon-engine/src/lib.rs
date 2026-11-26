//! Ensamblado de SYON: aquí viven las piezas que conectan los puertos de `syon-app` con
//! el mundo real. Por ahora, el bus de eventos, las rutas de datos y el registro de diagnóstico.

#![warn(missing_docs)]

mod event_bus;
mod log_file;
mod logging;
mod panic_hook;
mod paths;
mod redact;
mod subscriber;

pub use event_bus::EventBus;
pub use logging::{LoggingConfig, LoggingError, init as init_logging};
pub use paths::{AppPaths, PathsError};
pub use subscriber::SubscriberStats;
