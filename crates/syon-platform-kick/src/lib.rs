//! Adaptador de Kick (diseño 002). Traduce lo que entrega el feed en tiempo real a
//! [`syon_domain::EventKind`], mantiene la conexión a ese feed con reconexión y gestiona el inicio de
//! sesión OAuth 2.1 con PKCE; las formas de los datos del feed vienen del feed no oficial y pueden
//! cambiar.

#![warn(missing_docs)]

mod callback;
mod clock;
mod config;
mod connector;
mod error;
mod events;
mod frames;
mod oauth_client;
mod pkce;
mod session;
#[cfg(test)]
mod test_support;
mod token;

pub use config::{DEFAULT_REDIRECT_URI, DEFAULT_SCOPES, KickAppConfig};
pub use connector::{KickChannel, KickConnector};
pub use error::{ConnectError, OAuthError};
pub use events::event_from_publication;
pub use oauth_client::OAuthClient;
pub use session::{Source, TokenManager};
