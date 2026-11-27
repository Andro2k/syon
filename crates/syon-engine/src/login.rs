use std::{error::Error, fmt};
use syon_platform_kick as kick;
use syon_platform_twitch as twitch;

/// Cómo se obtuvo la sesión de una plataforma. Es un tipo del engine para que quien lo llama no
/// vea los tipos de los adaptadores.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoginOutcome {
    /// Había una sesión guardada y todavía vigente.
    Restored,
    /// Había una sesión guardada y se renovó con su `refresh_token`.
    Refreshed,
    /// La persona autorizó en el navegador un inicio de sesión nuevo.
    NewLogin,
}

impl From<kick::Source> for LoginOutcome {
    fn from(source: kick::Source) -> Self {
        match source {
            kick::Source::Restored => Self::Restored,
            kick::Source::Refreshed => Self::Refreshed,
            kick::Source::NewLogin => Self::NewLogin,
        }
    }
}

impl From<twitch::Source> for LoginOutcome {
    fn from(source: twitch::Source) -> Self {
        match source {
            twitch::Source::Restored => Self::Restored,
            twitch::Source::Refreshed => Self::Refreshed,
            twitch::Source::NewLogin => Self::NewLogin,
        }
    }
}

/// No se pudo dejar lista la sesión de una plataforma. Ningún mensaje incluye tokens ni el secreto
/// de la aplicación.
#[derive(Debug, PartialEq, Eq)]
pub enum LoginError {
    /// Falló el inicio de sesión; el texto explica por qué.
    Failed(String),
}

impl fmt::Display for LoginError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Failed(detail) => write!(f, "{detail}"),
        }
    }
}

impl Error for LoginError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cada_origen_de_la_sesion_tiene_su_resultado() {
        assert_eq!(
            LoginOutcome::from(kick::Source::Restored),
            LoginOutcome::Restored
        );
        assert_eq!(
            LoginOutcome::from(kick::Source::Refreshed),
            LoginOutcome::Refreshed
        );
        assert_eq!(
            LoginOutcome::from(kick::Source::NewLogin),
            LoginOutcome::NewLogin
        );
        assert_eq!(
            LoginOutcome::from(twitch::Source::Restored),
            LoginOutcome::Restored
        );
        assert_eq!(
            LoginOutcome::from(twitch::Source::Refreshed),
            LoginOutcome::Refreshed
        );
        assert_eq!(
            LoginOutcome::from(twitch::Source::NewLogin),
            LoginOutcome::NewLogin
        );
    }

    #[test]
    fn los_mensajes_de_error_estan_en_espanol() {
        assert!(
            LoginError::Failed("falló".to_owned())
                .to_string()
                .contains("falló")
        );
    }
}
