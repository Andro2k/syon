use std::{error::Error, fmt, io, path::PathBuf};
use syon_domain::{Platform, Timestamp};

/// Credenciales OAuth de un usuario en una plataforma.
///
/// Contienen secretos: `Debug` está escrito a mano y nunca imprime los tokens.
#[derive(Clone, PartialEq, Eq)]
pub struct Credentials {
    /// Token de acceso.
    pub access_token: String,
    /// Token para renovar el acceso, si la plataforma lo entrega.
    pub refresh_token: Option<String>,
    /// Momento en que caduca el token de acceso, si se conoce.
    pub expires_at: Option<Timestamp>,
    /// Permisos que el usuario concedió.
    pub scopes: Vec<String>,
}

impl fmt::Debug for Credentials {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Credentials")
            .field("access_token", &"<oculto>")
            .field(
                "refresh_token",
                &self.refresh_token.as_ref().map(|_| "<oculto>"),
            )
            .field("expires_at", &self.expires_at)
            .field("scopes", &self.scopes)
            .finish()
    }
}

/// Puerto de almacenamiento de las credenciales de cada plataforma.
pub trait CredentialStore: Send + Sync {
    /// Lee las credenciales de una plataforma. `None` si el usuario no ha iniciado sesión.
    /// Un contenido ilegible es un error y nunca se sobrescribe.
    fn load(&self, platform: Platform) -> Result<Option<Credentials>, CredentialError>;

    /// Guarda las credenciales de una plataforma. Si falla, el contenido anterior queda intacto.
    fn save(&self, platform: Platform, credentials: &Credentials) -> Result<(), CredentialError>;

    /// Borra las credenciales de una plataforma. Si no había, no hace nada.
    fn clear(&self, platform: Platform) -> Result<(), CredentialError>;
}

/// Error de un [`CredentialStore`]. Nunca incluye el valor de un token.
#[derive(Debug)]
pub enum CredentialError {
    /// No se pudo leer o escribir el almacén.
    Io {
        /// Ruta del archivo afectado.
        path: PathBuf,
        /// Error del sistema operativo.
        source: io::Error,
    },
    /// El contenido existe pero no es válido.
    Invalid {
        /// Ruta del archivo afectado.
        path: PathBuf,
        /// Qué falló, sin reproducir el contenido.
        detail: String,
    },
}

impl fmt::Display for CredentialError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, source } => {
                write!(
                    f,
                    "no se pudo acceder a las credenciales en {}: {source}",
                    path.display()
                )
            }
            Self::Invalid { path, detail } => {
                write!(f, "credenciales inválidas en {}: {detail}", path.display())
            }
        }
    }
}

impl Error for CredentialError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::Invalid { .. } => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{collections::HashMap, sync::Mutex};

    /// Almacén en memoria que cumple el contrato del puerto.
    #[derive(Default)]
    struct MemoryStore(Mutex<HashMap<&'static str, Credentials>>);

    fn key(platform: Platform) -> &'static str {
        match platform {
            Platform::Kick => "kick",
            Platform::Twitch => "twitch",
        }
    }

    impl CredentialStore for MemoryStore {
        fn load(&self, platform: Platform) -> Result<Option<Credentials>, CredentialError> {
            Ok(self.0.lock().unwrap().get(key(platform)).cloned())
        }

        fn save(
            &self,
            platform: Platform,
            credentials: &Credentials,
        ) -> Result<(), CredentialError> {
            self.0
                .lock()
                .unwrap()
                .insert(key(platform), credentials.clone());
            Ok(())
        }

        fn clear(&self, platform: Platform) -> Result<(), CredentialError> {
            self.0.lock().unwrap().remove(key(platform));
            Ok(())
        }
    }

    fn credentials() -> Credentials {
        Credentials {
            access_token: "secreto-de-acceso".to_owned(),
            refresh_token: Some("secreto-de-renovacion".to_owned()),
            expires_at: Some(Timestamp::from_unix_ms(1_000)),
            scopes: vec!["chat:write".to_owned()],
        }
    }

    #[test]
    fn debug_no_imprime_los_tokens() {
        let texto = format!("{:?}", credentials());
        assert!(!texto.contains("secreto-de-acceso"));
        assert!(!texto.contains("secreto-de-renovacion"));
        assert!(texto.contains("chat:write"));
    }

    #[test]
    fn cada_plataforma_guarda_las_suyas_y_clear_las_borra() {
        let store: Box<dyn CredentialStore> = Box::new(MemoryStore::default());
        assert_eq!(store.load(Platform::Kick).unwrap(), None);
        store.save(Platform::Kick, &credentials()).unwrap();
        assert_eq!(store.load(Platform::Kick).unwrap(), Some(credentials()));
        assert_eq!(store.load(Platform::Twitch).unwrap(), None);
        store.clear(Platform::Kick).unwrap();
        store.clear(Platform::Kick).unwrap();
        assert_eq!(store.load(Platform::Kick).unwrap(), None);
    }

    #[test]
    fn los_errores_indican_la_ruta_y_no_el_contenido() {
        let error = CredentialError::Invalid {
            path: PathBuf::from("/datos/credentials.toml"),
            detail: "falta la clave access_token".to_owned(),
        };
        let texto = error.to_string();
        assert!(texto.contains("/datos/credentials.toml"));
        assert!(texto.contains("falta la clave access_token"));
    }
}
