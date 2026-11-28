use std::{error::Error, fmt, future::Future, pin::Pin};
use syon_domain::{MessageId, Platform};

/// Longitud máxima común de un mensaje de chat saliente, contada en caracteres.
pub const MAX_CHAT_LENGTH: usize = 500;

/// Texto de chat ya validado: sin espacios sobrantes, no vacío y de [`MAX_CHAT_LENGTH`]
/// caracteres como máximo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatText(String);

impl ChatText {
    /// Recorta los espacios de los extremos y valida que el texto no quede vacío ni supere el
    /// máximo de caracteres.
    pub fn new(text: &str) -> Result<Self, SendError> {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return Err(SendError::Empty);
        }
        if trimmed.chars().count() > MAX_CHAT_LENGTH {
            return Err(SendError::TooLong {
                max: MAX_CHAT_LENGTH,
            });
        }
        Ok(Self(trimmed.to_owned()))
    }

    /// Texto validado listo para enviar.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Resultado que devuelve una plataforma cuando acepta un mensaje saliente.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SentMessage {
    /// Identificador asignado por la plataforma, si lo devuelve.
    pub id: Option<MessageId>,
}

/// Motivo por el que no se pudo enviar un mensaje de chat.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SendError {
    /// El texto quedó vacío tras recortar espacios.
    Empty,
    /// El texto supera el límite de caracteres permitido.
    TooLong {
        /// Máximo de caracteres admitido.
        max: usize,
    },
    /// La plataforma no está activada o no tiene envío disponible.
    NotRunning,
    /// No hay sesión guardada para esa plataforma (`syon login <plataforma>`).
    NotLoggedIn,
    /// La plataforma descartó el mensaje (moderación automática, permisos o límite de ritmo).
    Rejected(String),
    /// Fallo de red o de protocolo, sin tokens ni secretos.
    Failed(String),
}

impl fmt::Display for SendError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => f.write_str("el mensaje está vacío"),
            Self::TooLong { max } => {
                write!(f, "el mensaje supera el máximo de {max} caracteres")
            }
            Self::NotRunning => f.write_str("la plataforma no está activa"),
            Self::NotLoggedIn => f.write_str("no hay sesión guardada (`syon login`)"),
            Self::Rejected(reason) => write!(f, "la plataforma rechazó el mensaje: {reason}"),
            Self::Failed(detail) => write!(f, "no se pudo enviar el mensaje: {detail}"),
        }
    }
}

impl Error for SendError {}

/// Futuro en caja que devuelve [`ChatSender::send`] para que el trait sea un objeto.
pub type SendFuture<'a> = Pin<Box<dyn Future<Output = Result<SentMessage, SendError>> + Send + 'a>>;

/// Puerto de envío de mensajes al chat de una plataforma.
///
/// A diferencia de [`PlatformConnector`](crate::PlatformConnector), enviar espera a la red y
/// devuelve el resultado de la plataforma.
pub trait ChatSender: Send + Sync {
    /// Plataforma a la que envía este adaptador.
    fn platform(&self) -> Platform;

    /// Envía un mensaje ya validado al canal configurado.
    fn send<'a>(&'a self, text: &'a ChatText) -> SendFuture<'a>;
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FakeSender;

    impl ChatSender for FakeSender {
        fn platform(&self) -> Platform {
            Platform::Kick
        }

        fn send<'a>(&'a self, text: &'a ChatText) -> SendFuture<'a> {
            let id = MessageId::new(format!("id-{}", text.as_str().len()));
            Box::pin(async move { Ok(SentMessage { id: Some(id) }) })
        }
    }

    #[test]
    fn recorta_espacios_y_acepta_texto_valido() {
        let text = ChatText::new("  hola chat \n").unwrap();
        assert_eq!(text.as_str(), "hola chat");
    }

    #[test]
    fn rechaza_texto_vacio_o_solo_espacios() {
        assert_eq!(ChatText::new(""), Err(SendError::Empty));
        assert_eq!(ChatText::new("   \t\n "), Err(SendError::Empty));
    }

    #[test]
    fn cuenta_caracteres_y_no_bytes_en_el_limite() {
        let exact = "á".repeat(MAX_CHAT_LENGTH);
        assert!(exact.len() > MAX_CHAT_LENGTH);
        assert_eq!(ChatText::new(&exact).unwrap().as_str(), exact);

        let too_long = "á".repeat(MAX_CHAT_LENGTH + 1);
        assert_eq!(
            ChatText::new(&too_long),
            Err(SendError::TooLong {
                max: MAX_CHAT_LENGTH
            })
        );
    }

    #[test]
    fn chat_sender_funciona_como_trait_object() {
        let senders: Vec<Box<dyn ChatSender>> = vec![Box::new(FakeSender)];
        assert_eq!(senders[0].platform(), Platform::Kick);
    }
}
