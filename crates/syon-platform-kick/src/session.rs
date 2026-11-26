use crate::{
    clock::now,
    error::OAuthError,
    oauth_client::OAuthClient,
    token::{covers, needs_refresh},
};
use std::sync::Arc;
use syon_app::{CredentialStore, Credentials};
use syon_domain::Platform;
use tokio::sync::Mutex;

/// De dónde salió la sesión con la que arrancó el [`TokenManager`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    /// Credenciales guardadas, todavía vigentes.
    Restored,
    /// Credenciales guardadas, renovadas con su `refresh_token`.
    Refreshed,
    /// Inicio de sesión nuevo en el navegador.
    NewLogin,
}

/// Mantiene vigente la sesión de Kick: reutiliza, renueva o pide un inicio de sesión nuevo, y
/// guarda cada cambio en el [`CredentialStore`]. Las llamadas al almacén son síncronas y breves
/// (un archivo pequeño); se hacen dentro del bloqueo para que dos renovaciones no se pisen.
pub struct TokenManager {
    oauth: OAuthClient,
    store: Arc<dyn CredentialStore>,
    credentials: Mutex<Credentials>,
}

impl TokenManager {
    /// Deja lista una sesión:
    ///
    /// 1. Si hay credenciales guardadas que cubren los permisos pedidos y no van a vencer, las usa.
    /// 2. Si van a vencer, las renueva; si la renovación falla, inicia sesión de nuevo.
    /// 3. Si no hay, o faltan permisos, inicia sesión: `on_url` recibe el enlace para el navegador.
    pub async fn start(
        oauth: OAuthClient,
        store: Arc<dyn CredentialStore>,
        on_url: impl FnOnce(&str),
    ) -> Result<(Self, Source), OAuthError> {
        let stored = store
            .load(Platform::Kick)?
            .filter(|credentials| covers(credentials, oauth.scopes()));
        let (credentials, source) = match stored {
            Some(credentials) if !needs_refresh(&credentials, now()) => {
                (credentials, Source::Restored)
            }
            Some(credentials) => match oauth.refresh(&credentials).await {
                Ok(renewed) => (renewed, Source::Refreshed),
                Err(error) => {
                    log::warn!(
                        "kick: no se pudo renovar la sesión ({error}); se pide iniciar sesión"
                    );
                    (oauth.login(on_url).await?, Source::NewLogin)
                }
            },
            None => (oauth.login(on_url).await?, Source::NewLogin),
        };
        if source != Source::Restored {
            store.save(Platform::Kick, &credentials)?;
        }
        Ok((
            Self {
                oauth,
                store,
                credentials: Mutex::new(credentials),
            },
            source,
        ))
    }

    /// Token de acceso vigente; lo renueva (y lo guarda) si está por vencer.
    pub async fn access_token(&self) -> Result<String, OAuthError> {
        let mut credentials = self.credentials.lock().await;
        if needs_refresh(&credentials, now()) {
            *credentials = self.renew(&credentials).await?;
        }
        Ok(credentials.access_token.clone())
    }

    /// Renueva sin mirar la fecha: para cuando Kick responde 401 a un token que parecía vigente.
    pub async fn force_refresh(&self) -> Result<String, OAuthError> {
        let mut credentials = self.credentials.lock().await;
        *credentials = self.renew(&credentials).await?;
        Ok(credentials.access_token.clone())
    }

    async fn renew(&self, current: &Credentials) -> Result<Credentials, OAuthError> {
        let renewed = self.oauth.refresh(current).await?;
        self.store.save(Platform::Kick, &renewed)?;
        Ok(renewed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        config::KickAppConfig,
        test_support::{fake_server, free_port},
    };
    use std::{collections::HashMap, sync::Mutex as StdMutex};
    use syon_app::CredentialError;
    use syon_domain::Timestamp;

    const TOKEN_JSON: &str = r#"{"access_token":"NUEVO","refresh_token":"R2","expires_in":3600,"scope":"user:read channel:read chat:write"}"#;

    /// Almacén en memoria que anota cada guardado.
    #[derive(Default)]
    struct MemoryStore {
        data: StdMutex<HashMap<&'static str, Credentials>>,
        saves: StdMutex<u32>,
    }

    impl MemoryStore {
        fn with(credentials: Credentials) -> Arc<Self> {
            let store = Self::default();
            store.data.lock().unwrap().insert("kick", credentials);
            Arc::new(store)
        }

        fn saves(&self) -> u32 {
            *self.saves.lock().unwrap()
        }
    }

    impl CredentialStore for MemoryStore {
        fn load(&self, _: Platform) -> Result<Option<Credentials>, CredentialError> {
            Ok(self.data.lock().unwrap().get("kick").cloned())
        }

        fn save(&self, _: Platform, credentials: &Credentials) -> Result<(), CredentialError> {
            *self.saves.lock().unwrap() += 1;
            self.data
                .lock()
                .unwrap()
                .insert("kick", credentials.clone());
            Ok(())
        }

        fn clear(&self, _: Platform) -> Result<(), CredentialError> {
            self.data.lock().unwrap().remove("kick");
            Ok(())
        }
    }

    fn credentials(token: &str, expires_in_ms: i64, scopes: &[&str]) -> Credentials {
        Credentials {
            access_token: token.to_owned(),
            refresh_token: Some("R1".to_owned()),
            expires_at: Some(Timestamp::from_unix_ms(
                now().as_unix_ms().saturating_add(expires_in_ms),
            )),
            scopes: scopes.iter().map(|s| (*s).to_owned()).collect(),
        }
    }

    const ALL: [&str; 3] = ["user:read", "channel:read", "chat:write"];

    fn oauth(token_url: String) -> OAuthClient {
        OAuthClient::for_tests(KickAppConfig::new("cid", "s"), token_url)
    }

    #[tokio::test]
    async fn con_credenciales_vigentes_las_reutiliza_sin_llamar_a_kick_ni_guardar() {
        let server = fake_server(vec![]).await;
        let store = MemoryStore::with(credentials("GUARDADO", 3_600_000, &ALL));
        let (manager, source) = TokenManager::start(oauth(server.url()), store.clone(), |_| {})
            .await
            .unwrap();
        assert_eq!(source, Source::Restored);
        assert_eq!(manager.access_token().await.unwrap(), "GUARDADO");
        assert!(server.requests().is_empty());
        assert_eq!(store.saves(), 0);
    }

    #[tokio::test]
    async fn con_credenciales_vencidas_las_renueva_y_las_guarda() {
        let server = fake_server(vec![(200, TOKEN_JSON)]).await;
        let store = MemoryStore::with(credentials("VIEJO", -1_000, &ALL));
        let (manager, source) = TokenManager::start(oauth(server.url()), store.clone(), |_| {})
            .await
            .unwrap();
        assert_eq!(source, Source::Refreshed);
        assert_eq!(manager.access_token().await.unwrap(), "NUEVO");
        let saved = store.load(Platform::Kick).unwrap().unwrap();
        assert_eq!(saved.access_token, "NUEVO");
        assert_eq!(store.saves(), 1);
    }

    #[tokio::test]
    async fn access_token_renueva_cuando_esta_por_vencer() {
        let server = fake_server(vec![(200, TOKEN_JSON)]).await;
        let store = MemoryStore::with(credentials("A", 3_600_000, &ALL));
        let (manager, _) = TokenManager::start(oauth(server.url()), store.clone(), |_| {})
            .await
            .unwrap();
        *manager.credentials.lock().await = credentials("A", 10_000, &ALL);
        assert_eq!(manager.access_token().await.unwrap(), "NUEVO");
        assert_eq!(store.saves(), 1);
        assert_eq!(server.requests().len(), 1);
    }

    #[tokio::test]
    async fn force_refresh_renueva_aunque_el_token_parezca_vigente() {
        let server = fake_server(vec![(200, TOKEN_JSON)]).await;
        let store = MemoryStore::with(credentials("A", 3_600_000, &ALL));
        let (manager, _) = TokenManager::start(oauth(server.url()), store.clone(), |_| {})
            .await
            .unwrap();
        assert_eq!(manager.force_refresh().await.unwrap(), "NUEVO");
        assert_eq!(store.saves(), 1);
    }

    #[tokio::test]
    async fn si_faltan_permisos_se_pide_un_login_nuevo() {
        let server = fake_server(vec![]).await;
        let store = MemoryStore::with(credentials("A", 3_600_000, &["user:read"]));
        let mut config = KickAppConfig::new("cid", "s");
        config.redirect_uri = format!("http://127.0.0.1:{}/auth/callback", free_port());
        let client = OAuthClient::for_tests(config, server.url());
        let mut url_pedida = false;

        let result = TokenManager::start(client, store.clone(), |_| url_pedida = true).await;

        assert!(url_pedida, "debía pedir el enlace del navegador");
        assert!(matches!(result, Err(OAuthError::CallbackTimeout)));
        assert_eq!(store.saves(), 0);
        assert!(server.requests().is_empty());
    }

    #[tokio::test]
    async fn si_la_renovacion_falla_se_pide_un_login_nuevo() {
        let server = fake_server(vec![(400, r#"{"error":"invalid_grant"}"#)]).await;
        let store = MemoryStore::with(credentials("VIEJO", -1_000, &ALL));
        let mut config = KickAppConfig::new("cid", "s");
        config.redirect_uri = format!("http://127.0.0.1:{}/auth/callback", free_port());
        let client = OAuthClient::for_tests(config, server.url());
        let mut url_pedida = false;

        let result = TokenManager::start(client, store, |_| url_pedida = true).await;

        assert!(url_pedida);
        assert!(matches!(result, Err(OAuthError::CallbackTimeout)));
    }
}
