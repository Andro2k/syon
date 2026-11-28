//! Tipos puros del dominio de SYON: eventos de las plataformas, usuarios y marca
//! de tiempo. Sin I/O, sin async y sin dependencias externas.

#![warn(missing_docs)]

mod event;
mod platform;
mod timestamp;
mod user;

pub use event::{ChatMessage, ChatRoles, Event, EventKind, MessageId};
pub use platform::Platform;
pub use timestamp::Timestamp;
pub use user::User;
