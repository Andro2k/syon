use std::{error::Error, fmt, io};
use syon_app::CredentialError;

/// Fallo de inicio de sesión o de renovación de tokens de Twitch. Ningún mensaje incluye tokens ni
/// el secreto de la aplicación.
#[derive(Debug)]
pub enum OAuthError {
    /// `redirect_uri` no es una URL válida o no apunta a `localhost` o `127.0.0.1`.
    InvalidRedirect(String),
    /// No se pudo abrir el puerto del callback o leer de él (¿puerto ocupado?).
    Callback(io::Error),
    /// Se agotó el tiempo esperando la autorización en el navegador.
    CallbackTimeout,
    /// Twitch rechazó la autorización o la respuesta del navegador no es válida.
    Authorization(String),
    /// No hay un `refresh_token` con el que renovar.
    NoRefreshToken,
    /// Fallo de red al hablar con Twitch.
    Network(String),
    /// Twitch rechazó la petición de token.
    TokenRejected {
        /// Código HTTP de la respuesta.
        status: u16,
        /// Primeros caracteres del cuerpo de la respuesta.
        body: String,
    },
    /// Falló el almacén de credenciales.
    Store(CredentialError),
}

impl fmt::Display for OAuthError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidRedirect(detail) => write!(f, "redirect_uri no es válida: {detail}"),
            Self::Callback(error) => write!(
                f,
                "no se pudo usar el puerto del callback (¿está ocupado?): {error}"
            ),
            Self::CallbackTimeout => {
                f.write_str("se agotó el tiempo esperando la autorización en el navegador")
            }
            Self::Authorization(detail) => write!(f, "autorización fallida: {detail}"),
            Self::NoRefreshToken => f.write_str("no hay refresh token guardado"),
            Self::Network(detail) => write!(f, "no se pudo hablar con Twitch: {detail}"),
            Self::TokenRejected { status, body } => {
                write!(f, "Twitch respondió {status} al pedir el token: {body}")
            }
            Self::Store(error) => write!(f, "{error}"),
        }
    }
}

impl Error for OAuthError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Callback(error) => Some(error),
            Self::Store(error) => Some(error),
            _ => None,
        }
    }
}

impl From<CredentialError> for OAuthError {
    fn from(error: CredentialError) -> Self {
        Self::Store(error)
    }
}

/// Fallo al hablar con la API de Twitch (Helix o el servicio de validación). Ningún mensaje incluye
/// tokens ni el contenido de una respuesta ilegible.
#[derive(Debug, PartialEq, Eq)]
pub enum ApiError {
    /// Fallo de red o no se pudo preparar el cliente HTTP.
    Network(String),
    /// Twitch rechazó el token (401): está vencido, revocado o no es válido. Quien llama puede
    /// renovarlo y reintentar.
    Unauthorized,
    /// Twitch rechazó la petición por otro motivo (permisos, datos, límite de suscripciones...).
    Rejected {
        /// Código HTTP de la respuesta.
        status: u16,
        /// Primeros caracteres del cuerpo de la respuesta.
        body: String,
    },
    /// La respuesta no es JSON o le falta un campo imprescindible.
    Unreadable(String),
}

impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Network(detail) => write!(f, "no se pudo hablar con Twitch: {detail}"),
            Self::Unauthorized => f.write_str("Twitch rechazó el token (401): vencido o revocado"),
            Self::Rejected { status, body } => {
                write!(f, "Twitch respondió {status}: {body}")
            }
            Self::Unreadable(detail) => write!(f, "respuesta de Twitch ilegible: {detail}"),
        }
    }
}

impl Error for ApiError {}
