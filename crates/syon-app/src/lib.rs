//! Puertos y casos de uso de SYON. Solo depende de `syon-domain`; los adaptadores
//! implementan estos traits en otros crates.

#![warn(missing_docs)]

mod backoff;
mod chat_sender;
mod credential_store;
mod event_sink;
mod platform_connector;
mod settings;
mod settings_store;

pub use backoff::Backoff;
pub use chat_sender::{ChatSender, ChatText, MAX_CHAT_LENGTH, SendError, SendFuture, SentMessage};
pub use credential_store::{CredentialError, CredentialStore, Credentials};
pub use event_sink::EventSink;
pub use platform_connector::{ConnectionStatus, PlatformConnector};
pub use settings::{
    ChatCommand, ChatSettings, ChatUserFilter, KickSettings, LogLevel, LoggingSettings,
    MinimumChatRole, SCHEMA_VERSION, Settings, TwitchSettings,
};
pub use settings_store::{SettingsError, SettingsStore};
