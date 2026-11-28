use crate::{api::TwitchApi, error::ApiError, session::TokenManager};
use std::sync::Arc;
use syon_app::{ChatSender, ChatText, SendError, SendFuture, SentMessage};
use syon_domain::{MessageId, Platform};
use tokio::sync::Mutex;

/// Envía mensajes al chat de Twitch mediante Helix (`POST /helix/chat/messages`) usando el
/// [`TokenManager`] compartido con el conector.
///
/// Resuelve el `user_id` de quien autorizó la sesión en el primer envío y lo reutiliza en los
/// siguientes. Nunca registra el texto del mensaje ni los tokens.
pub struct TwitchChatSender {
    api: TwitchApi,
    tokens: Arc<TokenManager>,
    user_id: Mutex<Option<String>>,
}

impl TwitchChatSender {
    /// Crea el emisor para la aplicación configurada en `tokens`.
    pub fn new(tokens: Arc<TokenManager>) -> Result<Self, ApiError> {
        let api = TwitchApi::new(tokens.client_id())?;
        Ok(Self::with_api(api, tokens))
    }

    pub(crate) fn with_api(api: TwitchApi, tokens: Arc<TokenManager>) -> Self {
        Self {
            api,
            tokens,
            user_id: Mutex::new(None),
        }
    }

    async fn resolve_user_id(&self, token: &mut String) -> Result<String, SendError> {
        let mut cached = self.user_id.lock().await;
        if let Some(id) = cached.as_ref() {
            return Ok(id.clone());
        }
        let info = match self.api.validate(token).await {
            Ok(info) => info,
            Err(ApiError::Unauthorized) => {
                *token = self
                    .tokens
                    .force_refresh()
                    .await
                    .map_err(|error| SendError::Failed(error.to_string()))?;
                self.api.validate(token).await.map_err(map_api_error)?
            }
            Err(error) => return Err(map_api_error(error)),
        };
        if info.user_id.is_empty() {
            return Err(SendError::Failed(
                "Twitch no devolvió el user_id de la sesión".to_owned(),
            ));
        }
        *cached = Some(info.user_id.clone());
        Ok(info.user_id)
    }

    async fn send_inner(&self, text: &ChatText) -> Result<SentMessage, SendError> {
        let mut token = self
            .tokens
            .access_token()
            .await
            .map_err(|error| SendError::Failed(error.to_string()))?;
        let user_id = self.resolve_user_id(&mut token).await?;

        let outcome = match self
            .api
            .send_chat_message(&token, &user_id, &user_id, text.as_str())
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
                    .send_chat_message(&token, &user_id, &user_id, text.as_str())
                    .await
                    .map_err(map_api_error)?
            }
            Err(error) => return Err(map_api_error(error)),
        };

        if outcome.is_sent {
            let id = outcome.message_id.map(MessageId::new);
            log::info!(
                "twitch: mensaje enviado ({})",
                id.as_ref().map_or("-", MessageId::as_str)
            );
            Ok(SentMessage { id })
        } else {
            let reason = outcome
                .drop_reason
                .unwrap_or_else(|| "mensaje descartado por Twitch".to_owned());
            log::warn!("twitch: mensaje rechazado: {reason}");
            Err(SendError::Rejected(reason))
        }
    }
}

impl ChatSender for TwitchChatSender {
    fn platform(&self) -> Platform {
        Platform::Twitch
    }

    fn send<'a>(&'a self, text: &'a ChatText) -> SendFuture<'a> {
        Box::pin(self.send_inner(text))
    }
}

fn map_api_error(error: ApiError) -> SendError {
    match error {
        ApiError::Rejected { status, body } => {
            SendError::Rejected(format!("Twitch respondió {status}: {body}"))
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
        OAuthClient, TwitchAppConfig,
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
            scopes: crate::DEFAULT_SCOPES
                .iter()
                .map(|&s| s.to_owned())
                .collect(),
        }
    }

    async fn build_sender(base_url: &str, token: &str) -> TwitchChatSender {
        let mut config = TwitchAppConfig::new("cid-app", "secret-app");
        config.redirect_uri = format!("http://127.0.0.1:{}/auth/callback", free_port());
        let oauth = OAuthClient::for_tests(config, format!("{base_url}/oauth2/token"));
        let store = Arc::new(MemoryStore(StdMutex::new(Some(valid_credentials(token)))));
        let (tokens, _) = TokenManager::start(oauth, store, |_| {}).await.unwrap();
        TwitchChatSender::with_api(TwitchApi::for_tests("cid-app", base_url), Arc::new(tokens))
    }

    #[tokio::test]
    async fn envia_mensaje_resolviendo_y_reutilizando_el_user_id() {
        let valid = r#"{"client_id":"cid-app","login":"ana","scopes":["user:write:chat"],"user_id":"141981764","expires_in":3600}"#;
        let sent_one = r#"{"data":[{"message_id":"msg-1","is_sent":true,"drop_reason":null}]}"#;
        let sent_two = r#"{"data":[{"message_id":"msg-2","is_sent":true,"drop_reason":null}]}"#;
        let server = fake_server(vec![(200, valid), (200, sent_one), (200, sent_two)]).await;

        let sender = build_sender(&server.url(), "tok-secreto").await;
        let first = sender
            .send(&ChatText::new("hola uno").unwrap())
            .await
            .unwrap();
        let second = sender
            .send(&ChatText::new("hola dos").unwrap())
            .await
            .unwrap();

        assert_eq!(first.id, Some(MessageId::new("msg-1")));
        assert_eq!(second.id, Some(MessageId::new("msg-2")));

        let requests = server.requests();
        assert_eq!(requests.len(), 3);
        assert!(requests[0].starts_with("GET /oauth2/validate "));
        assert!(requests[1].starts_with("POST /helix/chat/messages "));
        assert!(
            requests[1]
                .to_ascii_lowercase()
                .contains("authorization: bearer tok-secreto")
        );
        assert!(
            requests[1]
                .to_ascii_lowercase()
                .contains("client-id: cid-app")
        );
        assert!(requests[1].contains(r#""broadcaster_id":"141981764""#));
        assert!(requests[1].contains(r#""sender_id":"141981764""#));
        assert!(requests[1].contains(r#""message":"hola uno""#));
        assert!(requests[2].starts_with("POST /helix/chat/messages "));
    }

    #[tokio::test]
    async fn traduce_drop_reason_cuando_is_sent_es_false() {
        let valid = r#"{"client_id":"cid-app","login":"ana","scopes":["user:write:chat"],"user_id":"141981764","expires_in":3600}"#;
        let dropped = r#"{"data":[{"message_id":"","is_sent":false,"drop_reason":{"code":"automod_held","message":"Retenido por AutoMod"}}]}"#;
        let server = fake_server(vec![(200, valid), (200, dropped)]).await;

        let sender = build_sender(&server.url(), "tok-secreto").await;
        let err = sender
            .send(&ChatText::new("texto bloqueado").unwrap())
            .await
            .unwrap_err();
        assert_eq!(err, SendError::Rejected("Retenido por AutoMod".into()));
        assert!(!err.to_string().contains("tok-secreto"));
        assert!(!err.to_string().contains("texto bloqueado"));
    }

    #[tokio::test]
    async fn renueva_el_token_si_twitch_responde_401() {
        let valid = r#"{"client_id":"cid-app","login":"ana","scopes":["user:write:chat"],"user_id":"141981764","expires_in":3600}"#;
        let refreshed = r#"{"access_token":"tok-nuevo","refresh_token":"rt-1","expires_in":3600,"scope":["user:write:chat"]}"#;
        let sent = r#"{"data":[{"message_id":"msg-9","is_sent":true,"drop_reason":null}]}"#;
        let server = fake_server(vec![
            (200, valid),
            (401, "unauthorized"),
            (200, refreshed),
            (200, sent),
        ])
        .await;

        let sender = build_sender(&server.url(), "tok-viejo").await;
        let outcome = sender.send(&ChatText::new("hola").unwrap()).await.unwrap();
        assert_eq!(outcome.id, Some(MessageId::new("msg-9")));

        let requests = server.requests();
        assert_eq!(requests.len(), 4);
        assert!(requests[2].starts_with("POST /oauth2/token "));
        assert!(
            requests[3]
                .to_ascii_lowercase()
                .contains("authorization: bearer tok-nuevo")
        );
    }
}
