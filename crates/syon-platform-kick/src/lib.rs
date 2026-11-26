//! Adaptador de Kick (diseño 002). Traduce lo que entrega el feed en tiempo real a
//! [`syon_domain::EventKind`] y gestiona el inicio de sesión OAuth 2.1 con PKCE; las formas de los
//! datos del feed vienen del feed no oficial y pueden cambiar.

#![warn(missing_docs)]

mod callback;
mod config;
mod error;
mod events;
mod oauth_client;
mod pkce;
mod session;
#[cfg(test)]
mod test_support;
mod token;

pub use config::{DEFAULT_REDIRECT_URI, DEFAULT_SCOPES, KickAppConfig};
pub use error::OAuthError;
pub use events::event_from_publication;
pub use oauth_client::OAuthClient;
pub use session::{Source, TokenManager};
