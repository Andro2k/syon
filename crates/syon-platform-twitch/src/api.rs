use crate::error::ApiError;
use reqwest::{Client, RequestBuilder, Response, StatusCode};
use serde_json::{Value, json};
use std::time::Duration;

const VALIDATE_URL: &str = "https://id.twitch.tv/oauth2/validate";
const HELIX_URL: &str = "https://api.twitch.tv/helix";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
/// Caracteres del cuerpo de una respuesta de error que se conservan en el mensaje.
const BODY_PREVIEW_CHARS: usize = 200;

/// Qué dice Twitch de un token de usuario (respuesta de `/oauth2/validate`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenInfo {
    /// Aplicación a la que se concedió el token.
    pub client_id: String,
    /// Nombre de usuario (login) de quien autorizó.
    pub login: String,
    /// Id de quien autorizó: es el id de su canal y la condición de las suscripciones.
    pub user_id: String,
    /// Permisos que el token tiene concedidos.
    pub scopes: Vec<String>,
}

/// Suscripción de EventSub que se pide crear: tipo, versión y condición (pares nombre y valor).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubscriptionRequest {
    /// Nombre del tipo, por ejemplo `channel.chat.message`.
    pub kind: &'static str,
    /// Versión del tipo.
    pub version: &'static str,
    /// Parámetros de la condición, por ejemplo `broadcaster_user_id`.
    pub condition: Vec<(&'static str, String)>,
}

/// Resultado de crear una suscripción. Los números salen de la respuesta de Twitch y son `None` si
/// la respuesta no los trae: nunca se rellenan con un valor supuesto.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreatedSubscription {
    /// Id de la suscripción.
    pub id: String,
    /// Estado que informa Twitch (`enabled` para WebSocket).
    pub status: String,
    /// Cuánto cuenta esta suscripción contra el límite de la aplicación.
    pub cost: Option<u64>,
    /// Suma del coste de todas las suscripciones de la aplicación.
    pub total_cost: Option<u64>,
    /// Límite de coste de la aplicación.
    pub max_total_cost: Option<u64>,
}

/// Respuesta de Twitch al enviar un mensaje al chat (`POST /helix/chat/messages`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SentChat {
    /// Identificador del mensaje asignado por Twitch, si lo devuelve.
    pub message_id: Option<String>,
    /// Si Twitch aceptó el mensaje en el chat; cuando es `false`, `drop_reason` explica el motivo.
    pub is_sent: bool,
    /// Motivo del descarte cuando `is_sent` es `false`.
    pub drop_reason: Option<String>,
}

/// Una suscripción de la lista que devuelve Twitch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListedSubscription {
    /// Id de la suscripción.
    pub id: String,
    /// Nombre del tipo.
    pub kind: String,
    /// Versión del tipo.
    pub version: String,
    /// Estado de la suscripción (`enabled`, `websocket_disconnected`, `authorization_revoked`...).
    pub status: String,
    /// Coste de esta suscripción, si la respuesta lo trae.
    pub cost: Option<u64>,
}

/// Una página de la lista de suscripciones.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubscriptionList {
    /// Suscripciones de esta página.
    pub subscriptions: Vec<ListedSubscription>,
    /// Suma del coste de todas las suscripciones, si la respuesta lo trae.
    pub total_cost: Option<u64>,
    /// Límite de coste de la aplicación, si la respuesta lo trae.
    pub max_total_cost: Option<u64>,
    /// Cursor de la página siguiente; `None` si no hay más.
    pub next_cursor: Option<String>,
}

/// Cliente mínimo de la API de Twitch: validar un token y crear o listar suscripciones de EventSub.
/// No guarda tokens: cada llamada recibe el que debe usar (ver [`TokenManager`](crate::TokenManager)).
#[derive(Debug, Clone)]
pub struct TwitchApi {
    http: Client,
    client_id: String,
    validate_url: String,
    helix_url: String,
}

impl TwitchApi {
    /// Crea el cliente para la aplicación `client_id`. Falla si no se puede inicializar el cliente
    /// HTTP (por ejemplo, TLS).
    pub fn new(client_id: impl Into<String>) -> Result<Self, ApiError> {
        let http = Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .build()
            .map_err(|error| ApiError::Network(error.without_url().to_string()))?;
        Ok(Self {
            http,
            client_id: client_id.into(),
            validate_url: VALIDATE_URL.to_owned(),
            helix_url: HELIX_URL.to_owned(),
        })
    }

    /// Valida el token y devuelve a quién pertenece y qué permisos tiene. Un token vencido o
    /// revocado es [`ApiError::Unauthorized`]; uno de aplicación (sin usuario) es `Unreadable`.
    pub async fn validate(&self, token: &str) -> Result<TokenInfo, ApiError> {
        let request = self
            .http
            .get(&self.validate_url)
            .header("Authorization", format!("OAuth {token}"));
        let body = read_json(request).await?;
        let text = |key: &str| body[key].as_str().filter(|s| !s.is_empty());
        let user_id = text("user_id")
            .ok_or_else(|| ApiError::Unreadable("el token no pertenece a un usuario".into()))?;
        Ok(TokenInfo {
            client_id: text("client_id").unwrap_or_default().to_owned(),
            login: text("login").unwrap_or_default().to_owned(),
            user_id: user_id.to_owned(),
            scopes: body["scopes"]
                .as_array()
                .map(|list| {
                    list.iter()
                        .filter_map(|s| s.as_str().map(str::to_owned))
                        .collect()
                })
                .unwrap_or_default(),
        })
    }

    /// Crea una suscripción de EventSub por WebSocket para la sesión `session_id` (la del mensaje de
    /// bienvenida). Lee `cost`, `total_cost` y `max_total_cost` de la respuesta.
    pub async fn create_subscription(
        &self,
        token: &str,
        request: &SubscriptionRequest,
        session_id: &str,
    ) -> Result<CreatedSubscription, ApiError> {
        let condition: serde_json::Map<String, Value> = request
            .condition
            .iter()
            .map(|(key, value)| ((*key).to_owned(), Value::String(value.clone())))
            .collect();
        let body = json!({
            "type": request.kind,
            "version": request.version,
            "condition": condition,
            "transport": { "method": "websocket", "session_id": session_id },
        });
        let response = read_json(
            self.helix(
                token,
                self.http
                    .post(format!("{}/eventsub/subscriptions", self.helix_url)),
            )
            .json(&body),
        )
        .await?;
        let created = response["data"]
            .get(0)
            .ok_or_else(|| ApiError::Unreadable("la respuesta no incluye la suscripción".into()))?;
        Ok(CreatedSubscription {
            id: created["id"]
                .as_str()
                .ok_or_else(|| ApiError::Unreadable("la suscripción no trae id".into()))?
                .to_owned(),
            status: created["status"].as_str().unwrap_or_default().to_owned(),
            cost: created["cost"].as_u64(),
            total_cost: response["total_cost"].as_u64(),
            max_total_cost: response["max_total_cost"].as_u64(),
        })
    }

    /// Lista las suscripciones de EventSub. `after` es el cursor de la página anterior; sin él se
    /// pide la primera.
    pub async fn list_subscriptions(
        &self,
        token: &str,
        after: Option<&str>,
    ) -> Result<SubscriptionList, ApiError> {
        let mut request = self.helix(
            token,
            self.http
                .get(format!("{}/eventsub/subscriptions", self.helix_url)),
        );
        if let Some(cursor) = after {
            request = request.query(&[("after", cursor)]);
        }
        let response = read_json(request).await?;
        let text = |value: &Value, key: &str| value[key].as_str().unwrap_or_default().to_owned();
        Ok(SubscriptionList {
            subscriptions: response["data"]
                .as_array()
                .map(|list| {
                    list.iter()
                        .filter_map(|item| {
                            Some(ListedSubscription {
                                id: item["id"].as_str()?.to_owned(),
                                kind: text(item, "type"),
                                version: text(item, "version"),
                                status: text(item, "status"),
                                cost: item["cost"].as_u64(),
                            })
                        })
                        .collect()
                })
                .unwrap_or_default(),
            total_cost: response["total_cost"].as_u64(),
            max_total_cost: response["max_total_cost"].as_u64(),
            next_cursor: response["pagination"]["cursor"].as_str().map(str::to_owned),
        })
    }

    /// Envía un mensaje al chat de `broadcaster_id` en nombre de `sender_id`.
    pub async fn send_chat_message(
        &self,
        token: &str,
        broadcaster_id: &str,
        sender_id: &str,
        message: &str,
    ) -> Result<SentChat, ApiError> {
        let request = self
            .helix(
                token,
                self.http.post(format!("{}/chat/messages", self.helix_url)),
            )
            .json(&json!({
                "broadcaster_id": broadcaster_id,
                "sender_id": sender_id,
                "message": message,
            }));
        let response = read_json(request).await?;
        let entry = response["data"]
            .as_array()
            .and_then(|list| list.first())
            .ok_or_else(|| {
                ApiError::Unreadable("falta data[0] en la respuesta de chat de Twitch".into())
            })?;
        let is_sent = entry["is_sent"].as_bool().ok_or_else(|| {
            ApiError::Unreadable("falta is_sent en la respuesta de chat de Twitch".into())
        })?;
        let message_id = entry["message_id"]
            .as_str()
            .filter(|id| !id.is_empty())
            .map(str::to_owned);
        let drop_reason = if entry["drop_reason"].is_object() {
            entry["drop_reason"]["message"]
                .as_str()
                .filter(|s| !s.is_empty())
                .or_else(|| {
                    entry["drop_reason"]["code"]
                        .as_str()
                        .filter(|s| !s.is_empty())
                })
                .map(str::to_owned)
        } else {
            None
        };
        Ok(SentChat {
            message_id,
            is_sent,
            drop_reason,
        })
    }

    /// Añade las cabeceras que Helix pide en cada llamada.
    fn helix(&self, token: &str, request: RequestBuilder) -> RequestBuilder {
        request
            .header("Authorization", format!("Bearer {token}"))
            .header("Client-Id", &self.client_id)
    }
}

/// Envía la petición y devuelve el JSON de la respuesta. Los errores no incluyen la URL ni el
/// contenido de una respuesta ilegible.
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

#[cfg(test)]
impl TwitchApi {
    /// Cliente de prueba: sin proxy y con las direcciones del servidor local falso.
    pub(crate) fn for_tests(client_id: &str, base_url: &str) -> Self {
        Self {
            http: Client::builder().no_proxy().build().unwrap(),
            client_id: client_id.to_owned(),
            validate_url: format!("{base_url}/oauth2/validate"),
            helix_url: format!("{base_url}/helix"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::fake_server;

    const VALID: &str = r#"{"client_id":"cid","login":"ana","scopes":["user:read:chat","channel:moderate"],"user_id":"141981764","expires_in":5520838}"#;
    const CREATED: &str = r#"{"data":[{"id":"sub-1","status":"enabled","type":"channel.chat.message","version":"1","condition":{"broadcaster_user_id":"1337","user_id":"1337"},"transport":{"method":"websocket","session_id":"S1"},"created_at":"2023-11-06T18:11:47.492253549Z","cost":0}],"total":1,"total_cost":0,"max_total_cost":10}"#;

    fn request() -> SubscriptionRequest {
        SubscriptionRequest {
            kind: "channel.chat.message",
            version: "1",
            condition: vec![
                ("broadcaster_user_id", "1337".to_owned()),
                ("user_id", "1337".to_owned()),
            ],
        }
    }

    fn json_body(raw_request: &str) -> Value {
        serde_json::from_str(raw_request.split("\r\n\r\n").nth(1).unwrap()).unwrap()
    }

    #[tokio::test]
    async fn validate_devuelve_el_usuario_y_los_permisos_y_envia_el_token() {
        let server = fake_server(vec![(200, VALID)]).await;
        let api = TwitchApi::for_tests("cid", &server.url());
        let info = api.validate("TOKEN-SECRETO").await.unwrap();
        assert_eq!(info.user_id, "141981764");
        assert_eq!(info.login, "ana");
        assert_eq!(info.scopes, ["user:read:chat", "channel:moderate"]);
        let sent = server.requests().remove(0);
        assert!(sent.starts_with("GET /oauth2/validate "));
        assert!(sent.contains("authorization: OAuth TOKEN-SECRETO"));
    }

    #[tokio::test]
    async fn un_token_rechazado_es_unauthorized_y_no_lo_repite() {
        let server = fake_server(vec![(
            401,
            r#"{"status":401,"message":"invalid access token"}"#,
        )])
        .await;
        let api = TwitchApi::for_tests("cid", &server.url());
        let error = api.validate("TOKEN-SECRETO").await.unwrap_err();
        assert_eq!(error, ApiError::Unauthorized);
        assert!(!error.to_string().contains("TOKEN-SECRETO"));
    }

    #[tokio::test]
    async fn un_token_de_aplicacion_sin_usuario_es_ilegible() {
        let server = fake_server(vec![(
            200,
            r#"{"client_id":"cid","login":null,"scopes":[],"user_id":null,"expires_in":10}"#,
        )])
        .await;
        let api = TwitchApi::for_tests("cid", &server.url());
        assert!(matches!(
            api.validate("T").await,
            Err(ApiError::Unreadable(_))
        ));
    }

    #[tokio::test]
    async fn crear_una_suscripcion_envia_la_condicion_y_la_sesion_y_lee_los_costes() {
        let server = fake_server(vec![(202, CREATED)]).await;
        let api = TwitchApi::for_tests("cid", &server.url());
        let created = api
            .create_subscription("TOKEN-SECRETO", &request(), "S1")
            .await
            .unwrap();
        assert_eq!(
            created,
            CreatedSubscription {
                id: "sub-1".into(),
                status: "enabled".into(),
                cost: Some(0),
                total_cost: Some(0),
                max_total_cost: Some(10),
            }
        );
        let sent = server.requests().remove(0);
        assert!(sent.starts_with("POST /helix/eventsub/subscriptions "));
        assert!(sent.contains("authorization: Bearer TOKEN-SECRETO"));
        assert!(sent.contains("client-id: cid"));
        assert_eq!(
            json_body(&sent),
            json!({
                "type": "channel.chat.message",
                "version": "1",
                "condition": { "broadcaster_user_id": "1337", "user_id": "1337" },
                "transport": { "method": "websocket", "session_id": "S1" }
            })
        );
    }

    #[tokio::test]
    async fn los_costes_ausentes_quedan_en_none_en_lugar_de_inventarse() {
        let server = fake_server(vec![(
            202,
            r#"{"data":[{"id":"sub-1","status":"enabled"}]}"#,
        )])
        .await;
        let api = TwitchApi::for_tests("cid", &server.url());
        let created = api
            .create_subscription("T", &request(), "S1")
            .await
            .unwrap();
        assert_eq!(created.cost, None);
        assert_eq!(created.total_cost, None);
        assert_eq!(created.max_total_cost, None);
    }

    #[tokio::test]
    async fn un_rechazo_indica_el_codigo_y_el_motivo_sin_el_token() {
        let server = fake_server(vec![(
            403,
            r#"{"error":"Forbidden","status":403,"message":"subscription missing proper authorization"}"#,
        )])
        .await;
        let api = TwitchApi::for_tests("cid", &server.url());
        let error = api
            .create_subscription("TOKEN-SECRETO", &request(), "S1")
            .await
            .unwrap_err();
        assert!(matches!(&error, ApiError::Rejected { status: 403, .. }));
        let text = error.to_string();
        assert!(text.contains("403") && text.contains("missing proper authorization"));
        assert!(!text.contains("TOKEN-SECRETO"));
    }

    #[tokio::test]
    async fn una_respuesta_sin_suscripcion_o_sin_json_es_ilegible_sin_su_contenido() {
        let server = fake_server(vec![(202, r#"{"data":[]}"#), (202, "texto-secreto")]).await;
        let api = TwitchApi::for_tests("cid", &server.url());
        assert!(matches!(
            api.create_subscription("T", &request(), "S1").await,
            Err(ApiError::Unreadable(_))
        ));
        let error = api
            .create_subscription("T", &request(), "S1")
            .await
            .unwrap_err();
        assert!(matches!(&error, ApiError::Unreadable(_)));
        assert!(!error.to_string().contains("texto-secreto"));
    }

    #[tokio::test]
    async fn listar_devuelve_las_suscripciones_los_costes_y_el_cursor() {
        let listing = r#"{"data":[
            {"id":"a","status":"enabled","type":"stream.online","version":"1","cost":1},
            {"id":"b","status":"websocket_disconnected","type":"channel.follow","version":"2","cost":0},
            {"status":"sin-id"}
        ],"total":2,"total_cost":1,"max_total_cost":10,"pagination":{"cursor":"SIGUE"}}"#;
        let server = fake_server(vec![
            (200, listing),
            (200, r#"{"data":[],"pagination":{}}"#),
        ])
        .await;
        let api = TwitchApi::for_tests("cid", &server.url());

        let first = api.list_subscriptions("T", None).await.unwrap();
        assert_eq!(first.subscriptions.len(), 2);
        assert_eq!(first.subscriptions[0].kind, "stream.online");
        assert_eq!(first.subscriptions[1].status, "websocket_disconnected");
        assert_eq!(first.total_cost, Some(1));
        assert_eq!(first.max_total_cost, Some(10));
        assert_eq!(first.next_cursor.as_deref(), Some("SIGUE"));

        let second = api.list_subscriptions("T", Some("SIGUE")).await.unwrap();
        assert!(second.subscriptions.is_empty() && second.next_cursor.is_none());
        let requests = server.requests();
        assert!(requests[0].starts_with("GET /helix/eventsub/subscriptions "));
        assert!(requests[1].starts_with("GET /helix/eventsub/subscriptions?after=SIGUE "));
    }
}
