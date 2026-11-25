//! Puertos y casos de uso de SYON. Solo depende de `syon-domain`; los adaptadores
//! implementan estos traits en otros crates.

#![warn(missing_docs)]

mod event_sink;
mod settings;
mod settings_store;

pub use event_sink::EventSink;
pub use settings::{LogLevel, LoggingSettings, SCHEMA_VERSION, Settings};
pub use settings_store::{SettingsError, SettingsStore};
