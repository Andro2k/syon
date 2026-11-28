//! Ensamblado de SYON: aquí viven las piezas que conectan los puertos de `syon-app` con
//! el mundo real. Por ahora, el arranque, el bus de eventos, las rutas de datos, el registro de diagnóstico, el runtime asíncrono y
//! el registro de conectores de plataforma.

#![warn(missing_docs)]

mod chat;
mod connectors;
mod credentials;
mod engine;
mod event_bus;
mod log_file;
mod logging;
mod login;
mod panic_hook;
mod paths;
mod platforms;
mod redact;
mod settings_loading;
mod subscriber;

pub use chat::{ChatHandle, SendResult};
pub use credentials::{AppCredentials, PlatformCredentials};
pub use engine::{Engine, StartError};
pub use event_bus::EventBus;
pub use logging::{LoggingConfig, LoggingError, init as init_logging};
pub use login::{LoginError, LoginOutcome};
pub use paths::{AppPaths, PathsError};
pub use platforms::{PlatformStart, PlatformStartError};
pub use subscriber::SubscriberStats;
