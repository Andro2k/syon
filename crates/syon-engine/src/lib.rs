//! Ensamblado de SYON: aquí viven las piezas que conectan los puertos de `syon-app` con
//! el mundo real. Por ahora, el bus de eventos y las rutas de datos.

#![warn(missing_docs)]

mod event_bus;
mod paths;
mod subscriber;

pub use event_bus::EventBus;
pub use paths::{AppPaths, PathsError};
pub use subscriber::SubscriberStats;
