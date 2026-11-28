//! Adaptador de Twitch (diseño 002). Gestiona el inicio de sesión OAuth con callback local, mantiene
//! vigente la sesión y ofrece un cliente mínimo de la API de Helix; los eventos de EventSub por
//! WebSocket llegan en parches posteriores. Los nombres de campo, endpoints y permisos vienen de la documentación de Twitch y se
//! contrastan en vivo por el autor.

#![warn(missing_docs)]

mod api;
mod callback;
mod clock;
mod config;
mod connector;
mod error;
mod events;
mod messages;
mod oauth_client;
mod sender;
mod session;
mod subscriptions;
#[cfg(test)]
mod test_support;
mod token;

pub use api::{
    CreatedSubscription, ListedSubscription, SentChat, SubscriptionList, SubscriptionRequest,
    TokenInfo, TwitchApi,
};
pub use config::{DEFAULT_REDIRECT_URI, DEFAULT_SCOPES, TwitchAppConfig};
pub use connector::TwitchConnector;
pub use error::{ApiError, ConnectError, OAuthError};
pub use events::event_from_notification;
pub use oauth_client::OAuthClient;
pub use sender::TwitchChatSender;
pub use session::{Source, TokenManager};
pub use subscriptions::{SUBSCRIPTIONS, SubscriptionSpec, subscription_requests};
