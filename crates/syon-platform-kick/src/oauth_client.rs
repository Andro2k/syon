use crate::{
    callback::wait_for_code,
    config::KickAppConfig,
    error::OAuthError,
    pkce::{Pkce, random_token},
    token::TokenResponse,
};
use reqwest::{Client, Url};
use std::time::{Duration, SystemTime};
use syon_app::Credentials;
use syon_domain::Timestamp;
use tokio::{net::TcpListener, time};

const AUTH_URL: &str = "https://id.kick.com/oauth/authorize";
const TOKEN_URL: &str = "https://id.kick.com/oauth/token";
const CALLBACK_TIMEOUT: Duration = Duration::from_secs(300);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
/// Caracteres del cuerpo de una respuesta de error que se conservan en el mensaje.
const BODY_PREVIEW_CHARS: usize = 200;

/// Cliente OAuth 2.1 con PKCE de Kick: inicio de sesión por navegador con callback local y
/// renovación de tokens. No guarda nada: quien lo usa decide dónde van las credenciales (ver
/// [`TokenManager`](crate::TokenManager)).
pub struct OAuthClient {
    http: Client,
    config: KickAppConfig,
    authorize_url: String,
    token_url: String,
    callback_timeout: Duration,
}

impl OAuthClient {
    /// Crea el cliente. Falla si no se puede inicializar el cliente HTTP (por ejemplo, TLS).
    pub fn new(config: KickAppConfig) -> Result<Self, OAuthError> {
        let http = Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .build()
            .map_err(|error| OAuthError::Network(error.without_url().to_string()))?;
        Ok(Self {
            http,
            config,
            authorize_url: AUTH_URL.to_owned(),
            token_url: TOKEN_URL.to_owned(),
            callback_timeout: CALLBACK_TIMEOUT,
        })
    }

    /// Permisos que este cliente pide al iniciar sesión.
    pub(crate) fn scopes(&self) -> &[String] {
        &self.config.scopes
    }

    fn build_authorize_url(&self, pkce: &Pkce, state: &str) -> Result<String, OAuthError> {
        let scope = self.config.scopes.join(" ");
        Url::parse_with_params(
            &self.authorize_url,
            [
                ("response_type", "code"),
                ("client_id", self.config.client_id.as_str()),
                ("redirect_uri", self.config.redirect_uri.as_str()),
                ("scope", scope.as_str()),
                ("code_challenge", pkce.challenge.as_str()),
                ("code_challenge_method", "S256"),
                ("state", state),
            ],
        )
        .map(String::from)
        .map_err(|_| OAuthError::Authorization("la URL de autorización no es válida".into()))
    }

    /// Inicio de sesión completo. Abre el puerto del callback, entrega a `on_url` el enlace que la
    /// persona debe abrir en su navegador y espera hasta 5 minutos a que autorice.
    pub async fn login(&self, on_url: impl FnOnce(&str)) -> Result<Credentials, OAuthError> {
        let redirect = Url::parse(&self.config.redirect_uri)
            .map_err(|error| OAuthError::InvalidRedirect(error.to_string()))?;
        let host = redirect.host_str().unwrap_or_default().to_owned();
        if !matches!(host.as_str(), "localhost" | "127.0.0.1") {
            return Err(OAuthError::InvalidRedirect(
                "debe apuntar a localhost o 127.0.0.1 para recibir el callback".into(),
            ));
        }
        let port = redirect.port_or_known_default().unwrap_or(80);
        let listener = TcpListener::bind((host.as_str(), port))
            .await
            .map_err(OAuthError::Callback)?;

        let pkce = Pkce::generate();
        let state = random_token(16);
        on_url(&self.build_authorize_url(&pkce, &state)?);

        let code = time::timeout(
            self.callback_timeout,
            wait_for_code(&listener, redirect.path(), &state),
        )
        .await
        .map_err(|_| OAuthError::CallbackTimeout)??;
        let response = self
            .token_request(&[
                ("grant_type", "authorization_code"),
                ("code", &code),
                ("code_verifier", &pkce.verifier),
                ("redirect_uri", &self.config.redirect_uri),
            ])
            .await?;
        Ok(response.into_credentials(now(), None))
    }

    /// Renueva las credenciales con su `refresh_token`. Si Kick no entrega uno nuevo se conserva el
    /// anterior.
    pub async fn refresh(&self, credentials: &Credentials) -> Result<Credentials, OAuthError> {
        let refresh = credentials
            .refresh_token
            .as_deref()
            .ok_or(OAuthError::NoRefreshToken)?;
        let response = self
            .token_request(&[("grant_type", "refresh_token"), ("refresh_token", refresh)])
            .await?;
        Ok(response.into_credentials(now(), Some(refresh)))
    }

    async fn token_request(&self, fields: &[(&str, &str)]) -> Result<TokenResponse, OAuthError> {
        let mut form = vec![
            ("client_id", self.config.client_id.as_str()),
            ("client_secret", self.config.client_secret.as_str()),
        ];
        form.extend_from_slice(fields);
        let response = self
            .http
            .post(&self.token_url)
            .form(&form)
            .send()
            .await
            .map_err(|error| OAuthError::Network(error.without_url().to_string()))?;
        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(OAuthError::TokenRejected {
                status: status.as_u16(),
                body: body.chars().take(BODY_PREVIEW_CHARS).collect(),
            });
        }
        response
            .json()
            .await
            .map_err(|_| OAuthError::Network("la respuesta del token no se pudo leer".into()))
    }
}

/// Hora actual. El dominio no lee el reloj, así que el adaptador la toma aquí.
pub(crate) fn now() -> Timestamp {
    Timestamp::from(SystemTime::now())
}

#[cfg(test)]
impl OAuthClient {
    /// Cliente de prueba: sin proxy, con el endpoint de tokens dado y un callback que caduca pronto.
    pub(crate) fn for_tests(config: KickAppConfig, token_url: String) -> Self {
        Self {
            http: Client::builder().no_proxy().build().unwrap(),
            config,
            authorize_url: AUTH_URL.to_owned(),
            token_url,
            callback_timeout: Duration::from_millis(300),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{fake_server, free_port};
    use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
    use sha2::{Digest, Sha256};
    use std::sync::Mutex;
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpStream,
    };

    const TOKEN_JSON: &str = r#"{"access_token":"A","refresh_token":"R","expires_in":3600,"scope":"user:read chat:write"}"#;

    fn config_with_port(port: u16) -> KickAppConfig {
        let mut config = KickAppConfig::new("cid", "secreto-de-kick");
        config.redirect_uri = format!("http://127.0.0.1:{port}/auth/callback");
        config
    }

    fn query(url: &str, key: &str) -> String {
        Url::parse(url)
            .unwrap()
            .query_pairs()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.into_owned())
            .unwrap_or_default()
    }

    /// Simula el navegador: espera el enlace de autorización y llama al callback con `code` y el
    /// `state` que corresponda (o uno distinto si `wrong_state`).
    async fn browser(
        slot: &Mutex<Option<String>>,
        port: u16,
        wrong_state: bool,
    ) -> (String, String) {
        let url = loop {
            if let Some(url) = slot.lock().unwrap().clone() {
                break url;
            }
            time::sleep(Duration::from_millis(5)).await;
        };
        let state = if wrong_state {
            "falso".to_owned()
        } else {
            query(&url, "state")
        };
        let mut stream = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
        let request =
            format!("GET /auth/callback?code=abc&state={state} HTTP/1.1\r\nHost: x\r\n\r\n");
        stream.write_all(request.as_bytes()).await.unwrap();
        let mut reply = String::new();
        stream.read_to_string(&mut reply).await.unwrap();
        (url, reply)
    }

    #[tokio::test]
    async fn el_login_completo_intercambia_el_code_con_el_verifier_de_pkce() {
        let port = free_port();
        let server = fake_server(vec![(200, TOKEN_JSON)]).await;
        let oauth = OAuthClient::for_tests(config_with_port(port), server.url());
        let slot = Mutex::new(None);

        let (login, (url, reply)) = tokio::join!(
            oauth.login(|u| *slot.lock().unwrap() = Some(u.to_owned())),
            browser(&slot, port, false)
        );

        let credentials = login.unwrap();
        assert_eq!(credentials.access_token, "A");
        assert_eq!(credentials.refresh_token.as_deref(), Some("R"));
        assert_eq!(credentials.scopes, ["user:read", "chat:write"]);
        assert!(reply.contains("200 OK"));

        let requests = server.requests();
        assert_eq!(requests.len(), 1);
        let body = requests[0].split("\r\n\r\n").nth(1).unwrap();
        assert!(body.contains("grant_type=authorization_code") && body.contains("code=abc"));
        let verifier = body
            .split('&')
            .find_map(|pair| pair.strip_prefix("code_verifier="))
            .unwrap();
        let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
        assert_eq!(challenge, query(&url, "code_challenge"));
        assert_eq!(query(&url, "code_challenge_method"), "S256");
        assert_eq!(query(&url, "scope"), "user:read channel:read chat:write");
        assert!(!url.contains("secreto-de-kick"));
    }

    #[tokio::test]
    async fn un_state_distinto_cancela_el_login_sin_pedir_el_token() {
        let port = free_port();
        let server = fake_server(vec![]).await;
        let oauth = OAuthClient::for_tests(config_with_port(port), server.url());
        let slot = Mutex::new(None);

        let (login, (_, reply)) = tokio::join!(
            oauth.login(|u| *slot.lock().unwrap() = Some(u.to_owned())),
            browser(&slot, port, true)
        );

        assert!(matches!(login, Err(OAuthError::Authorization(m)) if m.contains("CSRF")));
        assert!(reply.contains("400 Bad Request"));
        assert!(server.requests().is_empty());
    }

    #[tokio::test]
    async fn sin_autorizacion_el_login_caduca() {
        let oauth =
            OAuthClient::for_tests(config_with_port(free_port()), "http://127.0.0.1:1".into());
        assert!(matches!(
            oauth.login(|_| {}).await,
            Err(OAuthError::CallbackTimeout)
        ));
    }

    #[tokio::test]
    async fn el_callback_debe_ser_local() {
        let mut config = KickAppConfig::new("cid", "s");
        config.redirect_uri = "https://example.com/cb".into();
        let oauth = OAuthClient::for_tests(config, "http://127.0.0.1:1".into());
        assert!(matches!(
            oauth.login(|_| {}).await,
            Err(OAuthError::InvalidRedirect(_))
        ));
    }

    #[tokio::test]
    async fn un_puerto_ocupado_se_informa() {
        let ocupado = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = ocupado.local_addr().unwrap().port();
        let oauth = OAuthClient::for_tests(config_with_port(port), "http://127.0.0.1:1".into());
        assert!(matches!(
            oauth.login(|_| {}).await,
            Err(OAuthError::Callback(_))
        ));
    }

    fn old_credentials(refresh: Option<&str>) -> Credentials {
        Credentials {
            access_token: "viejo".to_owned(),
            refresh_token: refresh.map(str::to_owned),
            expires_at: Some(Timestamp::from_unix_ms(1)),
            scopes: vec![],
        }
    }

    #[tokio::test]
    async fn refresh_envia_el_refresh_token_y_renueva() {
        let server = fake_server(vec![(200, TOKEN_JSON)]).await;
        let oauth = OAuthClient::for_tests(KickAppConfig::new("cid", "s"), server.url());
        let renewed = oauth.refresh(&old_credentials(Some("R0"))).await.unwrap();
        assert_eq!(renewed.access_token, "A");
        let requests = server.requests();
        assert!(requests[0].contains("grant_type=refresh_token"));
        assert!(requests[0].contains("refresh_token=R0"));
    }

    #[tokio::test]
    async fn refresh_conserva_el_refresh_token_si_kick_no_manda_otro() {
        let sin_refresh = r#"{"access_token":"B","expires_in":60}"#;
        let server = fake_server(vec![(200, sin_refresh)]).await;
        let oauth = OAuthClient::for_tests(KickAppConfig::new("cid", "s"), server.url());
        let renewed = oauth.refresh(&old_credentials(Some("R0"))).await.unwrap();
        assert_eq!(renewed.refresh_token.as_deref(), Some("R0"));
    }

    #[tokio::test]
    async fn refresh_sin_refresh_token_falla_sin_llamar_a_kick() {
        let server = fake_server(vec![]).await;
        let oauth = OAuthClient::for_tests(KickAppConfig::new("cid", "s"), server.url());
        assert!(matches!(
            oauth.refresh(&old_credentials(None)).await,
            Err(OAuthError::NoRefreshToken)
        ));
        assert!(server.requests().is_empty());
    }

    #[tokio::test]
    async fn un_rechazo_de_kick_indica_el_codigo_y_no_incluye_secretos() {
        let server = fake_server(vec![(400, r#"{"error":"invalid_grant"}"#)]).await;
        let oauth =
            OAuthClient::for_tests(KickAppConfig::new("cid", "secreto-de-kick"), server.url());
        let error = oauth
            .refresh(&old_credentials(Some("R0")))
            .await
            .err()
            .unwrap();
        assert!(matches!(
            &error,
            OAuthError::TokenRejected { status: 400, .. }
        ));
        let texto = error.to_string();
        assert!(texto.contains("400") && texto.contains("invalid_grant"));
        assert!(!texto.contains("secreto-de-kick") && !texto.contains("R0"));
    }

    #[tokio::test]
    async fn una_respuesta_que_no_es_json_es_un_error_de_red_sin_el_contenido() {
        let server = fake_server(vec![(200, "token-secreto-en-texto")]).await;
        let oauth = OAuthClient::for_tests(KickAppConfig::new("cid", "s"), server.url());
        let error = oauth
            .refresh(&old_credentials(Some("R0")))
            .await
            .err()
            .unwrap();
        assert!(matches!(&error, OAuthError::Network(_)));
        assert!(!error.to_string().contains("token-secreto-en-texto"));
    }
}
