use std::{error::Error, fmt};
use syon_domain::Platform;
use syon_platform_kick::Source;

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

impl From<Source> for LoginOutcome {
    fn from(source: Source) -> Self {
        match source {
            Source::Restored => Self::Restored,
            Source::Refreshed => Self::Refreshed,
            Source::NewLogin => Self::NewLogin,
        }
    }
}

/// No se pudo dejar lista la sesión de una plataforma. Ningún mensaje incluye tokens ni el secreto
/// de la aplicación.
#[derive(Debug, PartialEq, Eq)]
pub enum LoginError {
    /// El engine todavía no sabe iniciar sesión en esta plataforma.
    Unsupported(Platform),
    /// Falló el inicio de sesión; el texto explica por qué.
    Failed(String),
}

impl fmt::Display for LoginError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unsupported(platform) => {
                write!(f, "todavía no se puede iniciar sesión en {platform:?}")
            }
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
        assert_eq!(LoginOutcome::from(Source::Restored), LoginOutcome::Restored);
        assert_eq!(
            LoginOutcome::from(Source::Refreshed),
            LoginOutcome::Refreshed
        );
        assert_eq!(LoginOutcome::from(Source::NewLogin), LoginOutcome::NewLogin);
    }

    #[test]
    fn los_mensajes_de_error_estan_en_espanol() {
        assert!(
            LoginError::Unsupported(Platform::Twitch)
                .to_string()
                .contains("todavía")
        );
    }
}
