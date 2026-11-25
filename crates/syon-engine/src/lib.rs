//! Ensamblado de SYON: aquí viven las piezas que conectan los puertos de `syon-app` con
//! el mundo real. Por ahora, el bus de eventos.

#![warn(missing_docs)]

mod event_bus;
mod subscriber;

pub use event_bus::EventBus;
pub use subscriber::SubscriberStats;
