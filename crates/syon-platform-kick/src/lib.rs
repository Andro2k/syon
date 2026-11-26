//! Adaptador de Kick (diseño 002). Traduce lo que entrega el feed en tiempo real a
//! [`syon_domain::EventKind`]; las formas de los datos vienen del feed no oficial y pueden cambiar.

#![warn(missing_docs)]

mod events;

pub use events::event_from_publication;
