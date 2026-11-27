//! Puertos y casos de uso de SYON. Solo depende de `syon-domain`; los adaptadores
//! implementan estos traits en otros crates.

#![warn(missing_docs)]

mod backoff;
mod credential_store;
mod event_sink;
mod platform_connector;
mod settings;
mod settings_store;

pub use backoff::Backoff;
pub use credential_store::{CredentialError, CredentialStore, Credentials};
pub use event_sink::EventSink;
pub use platform_connector::{ConnectionStatus, PlatformConnector};
pub use settings::{
    KickSettings, LogLevel, LoggingSettings, SCHEMA_VERSION, Settings, TwitchSettings,
};
pub use settings_store::{SettingsError, SettingsStore};
