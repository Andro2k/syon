//! Adaptador de Twitch (diseño 002). Gestiona el inicio de sesión OAuth con callback local y mantiene
//! vigente la sesión; la API de Helix y los eventos de EventSub por WebSocket llegan en parches
//! posteriores. Los nombres de campo, endpoints y permisos vienen de la documentación de Twitch y se
//! contrastan en vivo por el autor.

#![warn(missing_docs)]

mod callback;
mod clock;
mod config;
mod error;
mod oauth_client;
mod session;
#[cfg(test)]
mod test_support;
mod token;

pub use config::{DEFAULT_REDIRECT_URI, DEFAULT_SCOPES, TwitchAppConfig};
pub use error::OAuthError;
pub use oauth_client::OAuthClient;
pub use session::{Source, TokenManager};
