use crate::error::ApiError;
use reqwest::{Client, RequestBuilder, Response, StatusCode};
use serde_json::{Value, json};
use std::time::Duration;

const API_BASE: &str = "https://api.kick.com/public/v1";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
/// Caracteres del cuerpo de una respuesta de error que se conservan en el mensaje.
const BODY_PREVIEW_CHARS: usize = 200;

/// Respuesta de Kick al enviar un mensaje de chat (`POST /chat`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SentChat {
    /// Identificador del mensaje si Kick lo incluye.
    pub message_id: Option<String>,
    /// Si Kick aceptó el mensaje.
    pub is_sent: bool,
}

/// Cliente mínimo de la API pública oficial de Kick (`api.kick.com/public/v1`).
#[derive(Clone)]
pub struct KickApi {
    http: Client,
    base_url: String,
}

impl KickApi {
    /// Crea el cliente HTTP con el tiempo de espera por defecto.
    pub fn new() -> Result<Self, ApiError> {
        let http = Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .build()
            .map_err(|error| ApiError::Network(error.to_string()))?;
        Ok(Self {
            http,
            base_url: API_BASE.to_owned(),
        })
    }

    /// Consulta el `broadcaster_user_id` de un canal por su `slug` (`GET /channels?slug=`).
    pub async fn channel_broadcaster_id(&self, token: &str, slug: &str) -> Result<u64, ApiError> {
        let request = self
            .http
            .get(format!("{}/channels", self.base_url))
            .bearer_auth(token)
            .query(&[("slug", slug)]);
        let response = read_json(request).await?;
        let first = response["data"]
            .as_array()
            .and_then(|items| items.first())
            .ok_or_else(|| ApiError::Unreadable("falta data[0] en /channels de Kick".into()))?;
        first["broadcaster_user_id"]
            .as_u64()
            .ok_or_else(|| ApiError::Unreadable("falta broadcaster_user_id en /channels".into()))
    }

    /// Envía un mensaje de chat al canal `broadcaster_user_id` (`POST /chat`).
    pub async fn send_chat(
        &self,
        token: &str,
        broadcaster_user_id: u64,
        content: &str,
    ) -> Result<SentChat, ApiError> {
        let request = self
            .http
            .post(format!("{}/chat", self.base_url))
            .bearer_auth(token)
            .json(&json!({
                "broadcaster_user_id": broadcaster_user_id,
                "content": content,
                "type": "user",
            }));
        let response = read_json(request).await?;
        let data = &response["data"];
        if !data.is_object() {
            return Err(ApiError::Unreadable(
                "falta data en la respuesta de chat de Kick".into(),
            ));
        }
        let is_sent = data["is_sent"].as_bool().unwrap_or(true);
        let message_id = data["message_id"]
            .as_str()
            .filter(|id| !id.is_empty())
            .map(str::to_owned);
        Ok(SentChat {
            message_id,
            is_sent,
        })
    }

    #[cfg(test)]
    pub(crate) fn for_tests(base_url: &str) -> Self {
        Self {
            http: Client::builder().no_proxy().build().unwrap(),
            base_url: base_url.to_owned(),
        }
    }
}

async fn read_json(request: RequestBuilder) -> Result<Value, ApiError> {
    let response = request
        .send()
        .await
        .map_err(|error| ApiError::Network(error.without_url().to_string()))?;
    let status = response.status();
    if status == StatusCode::UNAUTHORIZED {
        return Err(ApiError::Unauthorized);
    }
    if !status.is_success() {
        return Err(rejected(response).await);
    }
    response
        .json()
        .await
        .map_err(|_| ApiError::Unreadable("no es JSON".into()))
}

async fn rejected(response: Response) -> ApiError {
    let status = response.status().as_u16();
    let body = response.text().await.unwrap_or_default();
    ApiError::Rejected {
        status,
        body: body.chars().take(BODY_PREVIEW_CHARS).collect(),
    }
}
