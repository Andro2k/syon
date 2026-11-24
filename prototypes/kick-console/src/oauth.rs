//! OAuth 2.1 con PKCE para Kick: login por navegador con callback local,
//! tokens guardados en disco y renovación automática.

use std::{
    collections::HashMap,
    io::Write,
    path::PathBuf,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result, anyhow, bail};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use reqwest::{Client, Url};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    sync::Mutex,
    time,
};

use crate::config::Settings;

const AUTH_URL: &str = "https://id.kick.com/oauth/authorize";
const TOKEN_URL: &str = "https://id.kick.com/oauth/token";
const REFRESH_SKEW_SECS: u64 = 60;
const CALLBACK_TIMEOUT: Duration = Duration::from_secs(300);

pub fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs())
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Tokens {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_at: u64,
    pub scope: String,
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
    refresh_token: Option<String>,
    expires_in: u64,
    scope: Option<String>,
}

impl Tokens {
    fn from_response(response: TokenResponse, now: u64, previous_refresh: Option<&str>) -> Self {
        Self {
            access_token: response.access_token,
            refresh_token: response
                .refresh_token
                .or_else(|| previous_refresh.map(str::to_string)),
            expires_at: now + response.expires_in,
            scope: response.scope.unwrap_or_default(),
        }
    }

    pub fn needs_refresh(&self, now: u64) -> bool {
        now + REFRESH_SKEW_SECS >= self.expires_at
    }

    pub fn seconds_left(&self, now: u64) -> u64 {
        self.expires_at.saturating_sub(now)
    }

    /// Si Kick no informó los scopes concedidos no se puede comprobar, así que se asumen válidos.
    fn covers(&self, requested: &[String]) -> bool {
        if self.scope.is_empty() {
            return true;
        }
        let granted: Vec<&str> = self.scope.split_whitespace().collect();
        requested
            .iter()
            .all(|scope| granted.contains(&scope.as_str()))
    }
}

pub struct Pkce {
    pub verifier: String,
    pub challenge: String,
}

impl Pkce {
    pub fn generate() -> Self {
        Self::from_verifier(random_token(32))
    }

    fn from_verifier(verifier: String) -> Self {
        let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
        Self {
            verifier,
            challenge,
        }
    }
}

fn random_token(bytes: usize) -> String {
    let mut buffer = vec![0u8; bytes];
    rand::fill(&mut buffer[..]);
    URL_SAFE_NO_PAD.encode(buffer)
}

/// Persistencia de los tokens en un archivo JSON local (permisos 0600 en Unix).
pub struct TokenStore {
    path: PathBuf,
}

impl TokenStore {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn load(&self) -> Result<Option<Tokens>> {
        match std::fs::read_to_string(&self.path) {
            Ok(json) => serde_json::from_str(&json).map(Some).with_context(|| {
                format!("El archivo de tokens {} está dañado", self.path.display())
            }),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => {
                Err(error).with_context(|| format!("No se pudo leer {}", self.path.display()))
            }
        }
    }

    pub fn save(&self, tokens: &Tokens) -> Result<()> {
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options
            .open(&self.path)
            .with_context(|| format!("No se pudo escribir {}", self.path.display()))?;
        file.write_all(serde_json::to_string_pretty(tokens)?.as_bytes())?;
        Ok(())
    }
}

pub struct OAuthClient {
    http: Client,
    settings: Settings,
}

impl OAuthClient {
    pub fn new(http: Client, settings: Settings) -> Self {
        Self { http, settings }
    }

    fn authorize_url(&self, pkce: &Pkce, state: &str) -> Result<Url> {
        Ok(Url::parse_with_params(
            AUTH_URL,
            [
                ("response_type", "code"),
                ("client_id", self.settings.client_id.as_str()),
                ("redirect_uri", self.settings.redirect_uri.as_str()),
                ("scope", self.settings.scopes.join(" ").as_str()),
                ("code_challenge", pkce.challenge.as_str()),
                ("code_challenge_method", "S256"),
                ("state", state),
            ],
        )?)
    }

    /// Login completo: `on_url` recibe el enlace que la persona debe abrir en su navegador.
    pub async fn login(&self, on_url: impl FnOnce(&Url)) -> Result<Tokens> {
        let redirect = Url::parse(&self.settings.redirect_uri)
            .context("KICK_REDIRECT_URI no es una URL válida")?;
        let host = redirect.host_str().unwrap_or_default().to_string();
        if !matches!(host.as_str(), "localhost" | "127.0.0.1") {
            bail!(
                "KICK_REDIRECT_URI debe apuntar a localhost o 127.0.0.1 para recibir el callback"
            );
        }
        let port = redirect.port_or_known_default().unwrap_or(80);
        let listener = TcpListener::bind((host.as_str(), port))
            .await
            .with_context(|| {
                format!("No se pudo abrir el callback en {host}:{port} (¿puerto ocupado?)")
            })?;

        let pkce = Pkce::generate();
        let state = random_token(16);
        on_url(&self.authorize_url(&pkce, &state)?);

        let code = time::timeout(
            CALLBACK_TIMEOUT,
            wait_for_code(&listener, redirect.path(), &state),
        )
        .await
        .map_err(|_| anyhow!("Se agotó el tiempo esperando la autorización en el navegador"))??;
        let response = self
            .token_request(&[
                ("grant_type", "authorization_code"),
                ("code", &code),
                ("code_verifier", &pkce.verifier),
                ("redirect_uri", &self.settings.redirect_uri),
            ])
            .await?;
        Ok(Tokens::from_response(response, unix_now(), None))
    }

    pub async fn refresh(&self, tokens: &Tokens) -> Result<Tokens> {
        let refresh = tokens
            .refresh_token
            .as_deref()
            .ok_or_else(|| anyhow!("No hay refresh token guardado"))?;
        let response = self
            .token_request(&[("grant_type", "refresh_token"), ("refresh_token", refresh)])
            .await?;
        Ok(Tokens::from_response(response, unix_now(), Some(refresh)))
    }

    async fn token_request(&self, fields: &[(&str, &str)]) -> Result<TokenResponse> {
        let mut form = vec![
            ("client_id", self.settings.client_id.as_str()),
            ("client_secret", self.settings.client_secret.as_str()),
        ];
        form.extend_from_slice(fields);
        let response = self.http.post(TOKEN_URL).form(&form).send().await?;
        let status = response.status();
        if !status.is_success() {
            let body: String = response
                .text()
                .await
                .unwrap_or_default()
                .chars()
                .take(200)
                .collect();
            bail!("Kick respondió {status} al pedir el token: {body}");
        }
        Ok(response.json().await?)
    }
}

#[derive(Debug, PartialEq)]
enum Callback {
    Code(String),
    Failed(String),
    Unrelated,
}

fn parse_callback(request: &str, expected_path: &str, expected_state: &str) -> Callback {
    let target = request.lines().next().and_then(|line| {
        let mut parts = line.split_whitespace();
        (parts.next() == Some("GET"))
            .then(|| parts.next())
            .flatten()
    });
    let Some(url) = target.and_then(|t| Url::parse("http://localhost").ok()?.join(t).ok()) else {
        return Callback::Unrelated;
    };
    if url.path() != expected_path {
        return Callback::Unrelated;
    }
    let params: HashMap<String, String> = url.query_pairs().into_owned().collect();
    if let Some(error) = params.get("error") {
        let detail = params.get("error_description").map_or("", String::as_str);
        return Callback::Failed(format!("Kick rechazó la autorización: {error} {detail}"));
    }
    if params.get("state").map(String::as_str) != Some(expected_state) {
        return Callback::Failed("El parámetro state no coincide (posible CSRF)".into());
    }
    match params.get("code") {
        Some(code) => Callback::Code(code.clone()),
        None => Callback::Failed("La respuesta de Kick no incluye el code".into()),
    }
}

async fn wait_for_code(listener: &TcpListener, path: &str, state: &str) -> Result<String> {
    loop {
        let (mut stream, _) = listener.accept().await?;
        let mut buffer = vec![0u8; 4096];
        let read = stream.read(&mut buffer).await?;
        let (status, message, outcome) =
            match parse_callback(&String::from_utf8_lossy(&buffer[..read]), path, state) {
                Callback::Code(code) => (
                    "200 OK",
                    "Listo, ya puedes cerrar esta pestaña.",
                    Some(Ok(code)),
                ),
                Callback::Failed(reason) => (
                    "400 Bad Request",
                    "No se pudo autorizar. Revisa la consola.",
                    Some(Err(reason)),
                ),
                Callback::Unrelated => ("404 Not Found", "No encontrado.", None),
            };
        let body = format!("<!doctype html><meta charset=utf-8><p>{message}</p>");
        let reply = format!(
            "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        stream.write_all(reply.as_bytes()).await.ok();
        match outcome {
            Some(Ok(code)) => return Ok(code),
            Some(Err(reason)) => bail!(reason),
            None => continue,
        }
    }
}

/// De dónde salió la sesión actual (para informarlo en consola).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Source {
    Restored,
    Refreshed,
    NewLogin,
}

pub struct TokenManager {
    oauth: OAuthClient,
    store: TokenStore,
    tokens: Mutex<Tokens>,
}

impl TokenManager {
    /// Reutiliza los tokens guardados, los renueva o abre un login nuevo según haga falta.
    pub async fn start(
        oauth: OAuthClient,
        store: TokenStore,
        on_url: impl FnOnce(&Url),
    ) -> Result<(Self, Source)> {
        let now = unix_now();
        let stored = store
            .load()?
            .filter(|tokens| tokens.covers(&oauth.settings.scopes));
        let (tokens, source) = match stored {
            Some(tokens) if !tokens.needs_refresh(now) => (tokens, Source::Restored),
            Some(tokens) => match oauth.refresh(&tokens).await {
                Ok(renewed) => (renewed, Source::Refreshed),
                Err(_) => (oauth.login(on_url).await?, Source::NewLogin),
            },
            None => (oauth.login(on_url).await?, Source::NewLogin),
        };
        store.save(&tokens)?;
        Ok((
            Self {
                oauth,
                store,
                tokens: Mutex::new(tokens),
            },
            source,
        ))
    }

    /// Token de acceso vigente; lo renueva (y lo guarda) si está por vencer.
    pub async fn access_token(&self) -> Result<String> {
        let mut tokens = self.tokens.lock().await;
        if tokens.needs_refresh(unix_now()) {
            let renewed = self.renew(&tokens).await?;
            *tokens = renewed;
        }
        Ok(tokens.access_token.clone())
    }

    /// Renueva sin mirar la fecha: para cuando Kick responde 401 a un token que parecía válido.
    pub async fn force_refresh(&self) -> Result<String> {
        let mut tokens = self.tokens.lock().await;
        let renewed = self.renew(&tokens).await?;
        *tokens = renewed;
        Ok(tokens.access_token.clone())
    }

    async fn renew(&self, current: &Tokens) -> Result<Tokens> {
        let renewed = self.oauth.refresh(current).await?;
        self.store.save(&renewed)?;
        Ok(renewed)
    }

    pub async fn snapshot(&self) -> Tokens {
        self.tokens.lock().await.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    fn tokens(expires_at: u64, scope: &str) -> Tokens {
        Tokens {
            access_token: "a".into(),
            refresh_token: Some("r".into()),
            expires_at,
            scope: scope.into(),
        }
    }

    #[test]
    fn pkce_cumple_el_vector_de_la_rfc_7636() {
        let pkce = Pkce::from_verifier("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk".into());
        assert_eq!(
            pkce.challenge,
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
    }

    #[test]
    fn el_verifier_generado_es_valido_y_distinto_cada_vez() {
        let generados: HashSet<String> = (0..20).map(|_| Pkce::generate().verifier).collect();
        assert_eq!(generados.len(), 20);
        assert!(generados.iter().all(|v| (43..=128).contains(&v.len())));
    }

    #[test]
    fn expiracion_con_margen_y_scopes() {
        let t = tokens(1_000, "user:read chat:write");
        assert!(!t.needs_refresh(900));
        assert!(t.needs_refresh(940));
        assert_eq!(t.seconds_left(1_500), 0);
        assert!(t.covers(&["chat:write".to_string()]));
        assert!(!t.covers(&["channel:read".to_string()]));
        assert!(tokens(1, "").covers(&["cualquiera".to_string()]));
    }

    #[test]
    fn la_respuesta_conserva_el_refresh_token_anterior_si_no_llega_uno_nuevo() {
        let response = TokenResponse {
            access_token: "n".into(),
            refresh_token: None,
            expires_in: 100,
            scope: None,
        };
        let t = Tokens::from_response(response, 50, Some("viejo"));
        assert_eq!(
            (t.expires_at, t.refresh_token.as_deref()),
            (150, Some("viejo"))
        );
    }

    #[test]
    fn interpreta_el_callback() {
        let ok = "GET /auth/callback?code=abc&state=xyz HTTP/1.1\r\nHost: localhost\r\n\r\n";
        assert_eq!(
            parse_callback(ok, "/auth/callback", "xyz"),
            Callback::Code("abc".into())
        );
        assert!(matches!(
            parse_callback(ok, "/auth/callback", "otro"),
            Callback::Failed(_)
        ));
        assert_eq!(
            parse_callback("GET /favicon.ico HTTP/1.1\r\n\r\n", "/auth/callback", "xyz"),
            Callback::Unrelated
        );
        assert_eq!(
            parse_callback(
                "POST /auth/callback HTTP/1.1\r\n\r\n",
                "/auth/callback",
                "xyz"
            ),
            Callback::Unrelated
        );
        let denegado = "GET /auth/callback?error=access_denied&state=xyz HTTP/1.1\r\n\r\n";
        assert!(
            matches!(parse_callback(denegado, "/auth/callback", "xyz"), Callback::Failed(m) if m.contains("access_denied"))
        );
    }

    #[test]
    fn la_url_de_autorizacion_lleva_pkce_y_scopes() {
        let settings = Settings {
            client_id: "cid".into(),
            client_secret: "secreto".into(),
            redirect_uri: "http://localhost:8080/auth/callback".into(),
            scopes: vec!["user:read".into(), "chat:write".into()],
            token_file: ".kick_tokens.json".into(),
        };
        let oauth = OAuthClient::new(Client::new(), settings);
        let url = oauth
            .authorize_url(&Pkce::from_verifier("v".into()), "st")
            .unwrap()
            .to_string();
        assert!(url.starts_with("https://id.kick.com/oauth/authorize?"));
        assert!(url.contains("code_challenge_method=S256") && url.contains("state=st"));
        assert!(url.contains("scope=user%3Aread+chat%3Awrite") && !url.contains("secreto"));
    }

    #[test]
    fn el_almacen_guarda_y_recupera_los_tokens() {
        let path = std::env::temp_dir().join(format!("kick_tokens_{}.json", uuid::Uuid::new_v4()));
        let store = TokenStore::new(path.clone());
        assert_eq!(store.load().unwrap(), None);
        let t = tokens(123, "user:read");
        store.save(&t).unwrap();
        assert_eq!(store.load().unwrap(), Some(t));
        std::fs::write(&path, "no es json").unwrap();
        assert!(store.load().is_err());
        std::fs::remove_file(path).ok();
    }
}
