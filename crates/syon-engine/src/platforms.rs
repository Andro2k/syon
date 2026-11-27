use std::{error::Error, fmt};
use syon_app::KickSettings;
use syon_domain::Platform;
use syon_platform_kick::KickChannel;

/// Resultado de arrancar una plataforma activada en los ajustes (diseño 003, sección 5).
#[derive(Debug)]
pub struct PlatformStart {
    /// Plataforma que se intentó arrancar.
    pub platform: Platform,
    /// `Ok` si el conector quedó registrado y en marcha; si no, por qué no arrancó.
    pub outcome: Result<(), PlatformStartError>,
}

/// Por qué no arrancó una plataforma activada.
#[derive(Debug, PartialEq, Eq)]
pub enum PlatformStartError {
    /// Está activada pero los ajustes no indican qué canal escuchar.
    MissingChannel,
    /// Ya hay un conector de esta plataforma registrado.
    AlreadyRunning,
    /// Esta compilación no tiene las credenciales de la aplicación de esta plataforma.
    MissingCredentials,
    /// No se pudo dejar lista la sesión de la persona; el texto explica por qué.
    Login(String),
    /// El adaptador no pudo preparar la conexión.
    Connect(String),
}

impl fmt::Display for PlatformStartError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingChannel => write!(f, "está activada pero falta el canal en los ajustes"),
            Self::AlreadyRunning => write!(f, "ya estaba en marcha"),
            Self::MissingCredentials => {
                write!(
                    f,
                    "faltan las credenciales de la aplicación para esta plataforma"
                )
            }
            Self::Login(detail) => write!(f, "no se pudo iniciar sesión: {detail}"),
            Self::Connect(detail) => write!(f, "{detail}"),
        }
    }
}

impl Error for PlatformStartError {}

/// Canal de Kick que indican los ajustes. Un nombre vacío o en blanco cuenta como ausente.
pub(crate) fn kick_channel(settings: &KickSettings) -> Result<KickChannel, PlatformStartError> {
    let slug = settings.channel.trim();
    if slug.is_empty() {
        return Err(PlatformStartError::MissingChannel);
    }
    let mut channel = KickChannel::new(slug);
    channel.id = settings.channel_id;
    channel.chatroom_id = settings.chatroom_id;
    Ok(channel)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings(channel: &str) -> KickSettings {
        KickSettings {
            enabled: true,
            channel: channel.to_owned(),
            ..KickSettings::default()
        }
    }

    #[test]
    fn sin_canal_o_con_el_canal_en_blanco_falta_el_canal() {
        for channel in ["", "   "] {
            assert_eq!(
                kick_channel(&settings(channel)),
                Err(PlatformStartError::MissingChannel)
            );
        }
    }

    #[test]
    fn el_canal_se_recorta_y_lleva_los_ids_de_los_ajustes() {
        let mut kick = settings("  mi_canal ");
        kick.channel_id = Some(1);
        kick.chatroom_id = Some(2);
        let channel = kick_channel(&kick).unwrap();
        assert_eq!(channel.slug, "mi_canal");
        assert_eq!(channel.id, Some(1));
        assert_eq!(channel.chatroom_id, Some(2));
    }

    #[test]
    fn los_mensajes_de_error_estan_en_espanol() {
        assert!(
            PlatformStartError::MissingChannel
                .to_string()
                .contains("canal")
        );
        assert!(
            PlatformStartError::MissingCredentials
                .to_string()
                .contains("credenciales")
        );
        assert!(
            PlatformStartError::Login("x".to_owned())
                .to_string()
                .contains("iniciar sesión")
        );
    }
}
