use std::fmt;

/// Credenciales de la aplicación de SYON en una plataforma (ADR-0018). Son de la aplicación y
/// no de la persona: quien arranca el engine las resuelve (el binario las lee del entorno) y el
/// engine nunca toca el entorno.
#[derive(Clone, PartialEq, Eq)]
pub struct AppCredentials {
    /// Identificador público de la aplicación.
    pub client_id: String,
    /// Secreto de la aplicación. `Debug` no lo imprime.
    pub client_secret: String,
}

impl fmt::Debug for AppCredentials {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AppCredentials")
            .field("client_id", &self.client_id)
            .field("client_secret", &"<oculto>")
            .finish()
    }
}

/// Las credenciales de la aplicación de cada plataforma; `None` si esta compilación no las
/// incluye.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PlatformCredentials {
    /// Aplicación de Kick.
    pub kick: Option<AppCredentials>,
    /// Aplicación de Twitch.
    pub twitch: Option<AppCredentials>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_no_imprime_el_secreto() {
        let credentials = AppCredentials {
            client_id: "id-publico".to_owned(),
            client_secret: "ultra-secreto".to_owned(),
        };
        let text = format!("{credentials:?}");
        assert!(text.contains("id-publico"), "{text}");
        assert!(!text.contains("ultra-secreto"), "{text}");
        let all = PlatformCredentials {
            kick: Some(credentials),
            twitch: None,
        };
        assert!(!format!("{all:?}").contains("ultra-secreto"));
    }
}
