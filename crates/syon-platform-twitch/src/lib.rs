//! Adaptador de Twitch (diseño 002). Gestiona el inicio de sesión OAuth con callback local, mantiene
//! vigente la sesión y ofrece un cliente mínimo de la API de Helix; los eventos de EventSub por
//! WebSocket llegan en parches posteriores. Los nombres de campo, endpoints y permisos vienen de la documentación de Twitch y se
//! contrastan en vivo por el autor.

#![warn(missing_docs)]

mod api;
mod callback;
mod clock;
mod config;
mod error;
mod events;
mod oauth_client;
mod session;
#[cfg(test)]
mod test_support;
mod token;

pub use api::{
    CreatedSubscription, ListedSubscription, SubscriptionList, SubscriptionRequest, TokenInfo,
    TwitchApi,
};
pub use config::{DEFAULT_REDIRECT_URI, DEFAULT_SCOPES, TwitchAppConfig};
pub use error::{ApiError, OAuthError};
pub use events::event_from_notification;
pub use oauth_client::OAuthClient;
pub use session::{Source, TokenManager};
