use crate::{api::KickApi, error::ApiError, session::TokenManager};
use std::sync::Arc;
use syon_app::{ChatSender, ChatText, SendError, SendFuture, SentMessage};
use syon_domain::{MessageId, Platform};
use tokio::sync::Mutex;

/// Envía mensajes al chat de Kick mediante la API pública (`POST /chat`) usando un [`TokenManager`]
/// con sesión ya iniciada.
///
/// Resuelve el `broadcaster_user_id` del canal a partir de su `slug` en el primer envío y lo
/// reutiliza en los siguientes. Nunca registra el texto del mensaje ni los tokens.
pub struct KickChatSender {
    api: KickApi,
    tokens: Arc<TokenManager>,
    slug: String,
    broadcaster_user_id: Mutex<Option<u64>>,
}

impl KickChatSender {
    /// Crea el emisor para el canal `slug` usando la sesión de `tokens`.
    pub fn new(tokens: Arc<TokenManager>, slug: impl Into<String>) -> Result<Self, ApiError> {
        Ok(Self::with_api(KickApi::new()?, tokens, slug))
    }

    pub(crate) fn with_api(
        api: KickApi,
        tokens: Arc<TokenManager>,
        slug: impl Into<String>,
    ) -> Self {
        Self {
            api,
            tokens,
            slug: slug.into(),
            broadcaster_user_id: Mutex::new(None),
        }
    }

    async fn resolve_broadcaster_id(&self, token: &mut String) -> Result<u64, SendError> {
        let mut cached = self.broadcaster_user_id.lock().await;
        if let Some(id) = *cached {
            return Ok(id);
        }
        let id = match self.api.channel_broadcaster_id(token, &self.slug).await {
            Ok(id) => id,
            Err(ApiError::Unauthorized) => {
                *token = self
                    .tokens
                    .force_refresh()
                    .await
                    .map_err(|error| SendError::Failed(error.to_string()))?;
                self.api
                    .channel_broadcaster_id(token, &self.slug)
                    .await
                    .map_err(map_api_error)?
            }
            Err(error) => return Err(map_api_error(error)),
        };
        *cached = Some(id);
        Ok(id)
    }

    async fn send_inner(&self, text: &ChatText) -> Result<SentMessage, SendError> {
        let mut token = self
            .tokens
            .access_token()
            .await
            .map_err(|error| SendError::Failed(error.to_string()))?;
        let broadcaster_id = self.resolve_broadcaster_id(&mut token).await?;

        let outcome = match self
            .api
            .send_chat(&token, broadcaster_id, text.as_str())
            .await
        {
            Ok(outcome) => outcome,
            Err(ApiError::Unauthorized) => {
                token = self
                    .tokens
                    .force_refresh()
                    .await
                    .map_err(|error| SendError::Failed(error.to_string()))?;
                self.api
                    .send_chat(&token, broadcaster_id, text.as_str())
                    .await
                    .map_err(map_api_error)?
            }
            Err(error) => return Err(map_api_error(error)),
        };

        if outcome.is_sent {
            let id = outcome.message_id.map(MessageId::new);
            log::info!(
                "kick: mensaje enviado ({})",
                id.as_ref().map_or("-", MessageId::as_str)
            );
            Ok(SentMessage { id })
        } else {
            log::warn!("kick: mensaje descartado por la plataforma");
            Err(SendError::Rejected("Kick descartó el mensaje".to_owned()))
        }
    }
}

impl ChatSender for KickChatSender {
    fn platform(&self) -> Platform {
        Platform::Kick
    }

    fn send<'a>(&'a self, text: &'a ChatText) -> SendFuture<'a> {
        Box::pin(self.send_inner(text))
    }
}

fn map_api_error(error: ApiError) -> SendError {
    match error {
        ApiError::Rejected { status, body } => {
            SendError::Rejected(format!("Kick respondió {status}: {body}"))
        }
        ApiError::Network(_) | ApiError::Unauthorized | ApiError::Unreadable(_) => {
            SendError::Failed(error.to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        DEFAULT_SCOPES, KickAppConfig, OAuthClient,
        test_support::{fake_server, free_port},
    };
    use std::sync::Mutex as StdMutex;
    use syon_app::{CredentialError, CredentialStore, Credentials};
    use syon_domain::Timestamp;

    struct MemoryStore(StdMutex<Option<Credentials>>);

    impl CredentialStore for MemoryStore {
        fn load(&self, _platform: Platform) -> Result<Option<Credentials>, CredentialError> {
            Ok(self.0.lock().unwrap().clone())
        }

        fn save(
            &self,
            _platform: Platform,
            credentials: &Credentials,
        ) -> Result<(), CredentialError> {
            *self.0.lock().unwrap() = Some(credentials.clone());
            Ok(())
        }

        fn clear(&self, _platform: Platform) -> Result<(), CredentialError> {
            *self.0.lock().unwrap() = None;
            Ok(())
        }
    }

    fn valid_credentials(access_token: &str) -> Credentials {
        Credentials {
            access_token: access_token.to_owned(),
            refresh_token: Some("rt-0".to_owned()),
            expires_at: Some(Timestamp::from_unix_ms(4_000_000_000_000)),
            scopes: DEFAULT_SCOPES.iter().map(|&s| s.to_owned()).collect(),
        }
    }

    async fn build_sender(base_url: &str, token: &str) -> KickChatSender {
        let mut config = KickAppConfig::new("cid-app", "secret-app");
        config.redirect_uri = format!("http://127.0.0.1:{}/auth/callback", free_port());
        let oauth = OAuthClient::for_tests(config, format!("{base_url}/oauth/token"));
        let store = Arc::new(MemoryStore(StdMutex::new(Some(valid_credentials(token)))));
        let (tokens, _) = TokenManager::try_restore(oauth, store)
            .await
            .unwrap()
            .expect("debería restaurar la sesión");
        KickChatSender::with_api(KickApi::for_tests(base_url), Arc::new(tokens), "mi_canal")
    }

    #[tokio::test]
    async fn resuelve_canal_por_slug_y_reutiliza_broadcaster_id() {
        let channel = r#"{"data":[{"broadcaster_user_id":77,"slug":"mi_canal"}]}"#;
        let sent_one = r#"{"data":{"is_sent":true,"message_id":"k-1"}}"#;
        let sent_two = r#"{"data":{"is_sent":true,"message_id":"k-2"}}"#;
        let server = fake_server(vec![(200, channel), (200, sent_one), (200, sent_two)]).await;

        let sender = build_sender(&server.url(), "tok-kick").await;
        let first = sender
            .send(&ChatText::new("hola uno").unwrap())
            .await
            .unwrap();
        let second = sender
            .send(&ChatText::new("hola dos").unwrap())
            .await
            .unwrap();

        assert_eq!(first.id, Some(MessageId::new("k-1")));
        assert_eq!(second.id, Some(MessageId::new("k-2")));

        let requests = server.requests();
        assert_eq!(requests.len(), 3);
        assert!(requests[0].starts_with("GET /channels?slug=mi_canal "));
        assert!(
            requests[0]
                .to_ascii_lowercase()
                .contains("authorization: bearer tok-kick")
        );
        assert!(requests[1].starts_with("POST /chat "));
        assert!(requests[1].contains(r#""broadcaster_user_id":77"#));
        assert!(requests[1].contains(r#""content":"hola uno""#));
        assert!(requests[1].contains(r#""type":"user""#));
        assert!(requests[2].starts_with("POST /chat "));
    }

    #[tokio::test]
    async fn traduce_is_sent_false_y_errores_sin_exponer_token_ni_texto() {
        let channel = r#"{"data":[{"broadcaster_user_id":77,"slug":"mi_canal"}]}"#;
        let dropped = r#"{"data":{"is_sent":false,"message_id":""}}"#;
        let server = fake_server(vec![(200, channel), (200, dropped)]).await;

        let sender = build_sender(&server.url(), "tok-secreto").await;
        let err = sender
            .send(&ChatText::new("texto privado").unwrap())
            .await
            .unwrap_err();
        assert!(matches!(err, SendError::Rejected(_)));
        assert!(!err.to_string().contains("tok-secreto"));
        assert!(!err.to_string().contains("texto privado"));
    }

    #[tokio::test]
    async fn renueva_el_token_si_kick_responde_401() {
        let channel = r#"{"data":[{"broadcaster_user_id":77,"slug":"mi_canal"}]}"#;
        let refreshed = r#"{"access_token":"tok-nuevo","refresh_token":"rt-1","expires_in":3600,"scope":"user:read channel:read chat:write"}"#;
        let sent = r#"{"data":{"is_sent":true,"message_id":"k-9"}}"#;
        let server = fake_server(vec![
            (200, channel),
            (401, "unauthorized"),
            (200, refreshed),
            (200, sent),
        ])
        .await;

        let sender = build_sender(&server.url(), "tok-viejo").await;
        let outcome = sender.send(&ChatText::new("hola").unwrap()).await.unwrap();
        assert_eq!(outcome.id, Some(MessageId::new("k-9")));

        let requests = server.requests();
        assert_eq!(requests.len(), 4);
        assert!(requests[2].starts_with("POST /oauth/token "));
        assert!(
            requests[3]
                .to_ascii_lowercase()
                .contains("authorization: bearer tok-nuevo")
        );
    }

    #[tokio::test]
    async fn try_restore_sin_sesion_no_abre_navegador() {
        let mut config = KickAppConfig::new("cid", "sec");
        config.redirect_uri = format!("http://127.0.0.1:{}/auth/callback", free_port());
        let oauth = OAuthClient::for_tests(config, "http://127.0.0.1:1/token".into());
        let store = Arc::new(MemoryStore(StdMutex::new(None)));
        assert!(
            TokenManager::try_restore(oauth, store)
                .await
                .unwrap()
                .is_none()
        );
    }
}
