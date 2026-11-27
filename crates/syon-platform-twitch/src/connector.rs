use crate::{
    api::{CreatedSubscription, TwitchApi},
    clock::now,
    error::{ApiError, ConnectError},
    events::event_from_notification,
    messages::{Dedup, Inbound, classify},
    session::TokenManager,
    subscriptions::subscription_requests,
};
use futures_util::{SinkExt, StreamExt};
use serde_json::Value;
use std::{
    future::Future,
    sync::{
        Arc, Mutex, PoisonError,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use syon_app::{Backoff, ConnectionStatus, EventSink, PlatformConnector};
use syon_domain::{Event, EventKind, Platform};
use tokio::{net::TcpStream, runtime::Handle, task::AbortHandle, time};
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async, tungstenite::Message};

const EVENTSUB_URL: &str = "wss://eventsub.wss.twitch.tv/ws";
/// Margen que se suma al plazo de silencio de Twitch antes de dar la conexión por muerta.
const KEEPALIVE_MARGIN: Duration = Duration::from_secs(5);
/// Plazo para abrir el WebSocket y recibir su mensaje de bienvenida.
const WELCOME_TIMEOUT: Duration = Duration::from_secs(15);
/// Cuánto se sigue leyendo la conexión vieja tras abrir la nueva en un cambio de conexión.
const DRAIN_TIMEOUT: Duration = Duration::from_secs(2);

type Socket = WebSocketStream<MaybeTlsStream<TcpStream>>;

/// Tiempos de la conexión; se acortan en las pruebas.
#[derive(Debug, Clone)]
pub(crate) struct Timing {
    /// Divide las esperas entre reconexiones (1 en producción).
    pub(crate) delay_divisor: u32,
    /// Margen sobre el plazo de silencio de Twitch.
    pub(crate) keepalive_margin: Duration,
    /// Silencio tras el cual se da la conexión por muerta; `None` usa el plazo anunciado por Twitch
    /// más el margen.
    pub(crate) silence_limit: Option<Duration>,
    /// Plazo para recibir la bienvenida.
    pub(crate) welcome_timeout: Duration,
    /// Plazo para vaciar la conexión vieja en un cambio de conexión.
    pub(crate) drain_timeout: Duration,
}

impl Default for Timing {
    fn default() -> Self {
        Self {
            delay_divisor: 1,
            keepalive_margin: KEEPALIVE_MARGIN,
            silence_limit: None,
            welcome_timeout: WELCOME_TIMEOUT,
            drain_timeout: DRAIN_TIMEOUT,
        }
    }
}

/// Estado compartido entre el conector y su tarea.
struct Shared {
    status: Mutex<ConnectionStatus>,
    stopped: AtomicBool,
}

impl Shared {
    /// Cambia el estado, salvo que ya se pidiera parar: una tarea en pleno `abort` no debe pisar
    /// `Disconnected`.
    fn set(&self, status: ConnectionStatus) {
        if !self.stopped.load(Ordering::SeqCst) {
            *self.status.lock().unwrap_or_else(PoisonError::into_inner) = status;
        }
    }

    fn get(&self) -> ConnectionStatus {
        *self.status.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// Todo lo que necesita la tarea que mantiene la conexión.
struct Context {
    api: TwitchApi,
    tokens: Arc<TokenManager>,
    sink: Arc<dyn EventSink>,
    shared: Arc<Shared>,
    eventsub_url: String,
    timing: Timing,
}

/// Conexión a EventSub de Twitch por WebSocket. Crea las ocho suscripciones, publica en un
/// [`EventSink`] los eventos del canal y reconecta sola con espera creciente hasta que se la detiene
/// (con [`PlatformConnector::stop`] o al soltarla).
///
/// Usa el token de usuario del [`TokenManager`] (ADR-0016) y atiende los mensajes de EventSub
/// (ADR-0017): bienvenida, keepalive y cambio de conexión con `session_reconnect`.
pub struct TwitchConnector {
    shared: Arc<Shared>,
    task: AbortHandle,
}

impl TwitchConnector {
    /// Arranca la conexión en el runtime de `handle` y vuelve enseguida: el estado empieza en
    /// `Connecting` y los fallos de red se reintentan por su cuenta. `tokens` debe tener la sesión ya
    /// iniciada ([`TokenManager::start`]).
    pub fn start(
        handle: &Handle,
        tokens: Arc<TokenManager>,
        sink: Arc<dyn EventSink>,
    ) -> Result<Self, ConnectError> {
        let api = TwitchApi::new(tokens.client_id())
            .map_err(|error| ConnectError::Http(error.to_string()))?;
        Ok(Self::start_with(
            handle,
            api,
            tokens,
            sink,
            EVENTSUB_URL.to_owned(),
            Timing::default(),
        ))
    }

    pub(crate) fn start_with(
        handle: &Handle,
        api: TwitchApi,
        tokens: Arc<TokenManager>,
        sink: Arc<dyn EventSink>,
        eventsub_url: String,
        timing: Timing,
    ) -> Self {
        let shared = Arc::new(Shared {
            status: Mutex::new(ConnectionStatus::Connecting),
            stopped: AtomicBool::new(false),
        });
        let context = Context {
            api,
            tokens,
            sink,
            shared: Arc::clone(&shared),
            eventsub_url,
            timing,
        };
        let task = handle.spawn(run(context)).abort_handle();
        Self { shared, task }
    }
}

impl PlatformConnector for TwitchConnector {
    fn platform(&self) -> Platform {
        Platform::Twitch
    }

    fn status(&self) -> ConnectionStatus {
        self.shared.get()
    }

    fn stop(&self) {
        self.shared.stopped.store(true, Ordering::SeqCst);
        *self
            .shared
            .status
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = ConnectionStatus::Disconnected;
        self.task.abort();
    }
}

impl Drop for TwitchConnector {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Mantiene la conexión viva: sesión tras sesión, con espera creciente entre ellas.
async fn run(ctx: Context) {
    let mut backoff = Backoff::new();
    loop {
        let reason = match session(&ctx, &mut backoff).await {
            Ok(code) => format!(
                "conexión cerrada (código {})",
                code.map_or_else(|| "-".to_owned(), |c| c.to_string())
            ),
            Err(error) => error,
        };
        let delay = backoff.next_delay() / ctx.timing.delay_divisor.max(1);
        ctx.shared.set(ConnectionStatus::Reconnecting {
            attempt: backoff.attempt(),
        });
        log::warn!("twitch: {reason}; reintentando en {delay:?}");
        time::sleep(delay).await;
    }
}

/// Una conexión completa: valida el token, abre el WebSocket, crea las suscripciones y publica hasta
/// que se cierra. Devuelve el código de cierre, si lo hubo.
async fn session(ctx: &Context, backoff: &mut Backoff) -> Result<Option<u16>, String> {
    let info = with_token(ctx, |token| async move { ctx.api.validate(&token).await }).await?;
    let (socket, welcome) = open(&ctx.eventsub_url, &ctx.timing).await?;

    let requests = subscription_requests(&info.user_id);
    let mut created = Vec::new();
    for request in &requests {
        let session_id = welcome.session_id.as_str();
        let result = with_token(ctx, |token| async move {
            ctx.api
                .create_subscription(&token, request, session_id)
                .await
        })
        .await;
        match result {
            Ok(subscription) => created.push(subscription),
            Err(error) => log::warn!("twitch: no se pudo crear {}: {error}", request.kind),
        }
    }
    if created.is_empty() {
        return Err("no se pudo crear ninguna suscripción".to_owned());
    }
    log::info!(
        "twitch: conectado a {} con {}",
        info.login,
        cost_summary(&created, requests.len())
    );
    ctx.shared.set(ConnectionStatus::Connected);
    backoff.reset();

    read_events(ctx, socket, welcome.keepalive).await
}

/// Lee los mensajes hasta que la conexión se cierra o falla. En un `session_reconnect` abre la
/// conexión nueva antes de soltar la vieja.
async fn read_events(
    ctx: &Context,
    mut socket: Socket,
    mut keepalive: Duration,
) -> Result<Option<u16>, String> {
    let mut dedup = Dedup::default();
    loop {
        let silence_limit = ctx
            .timing
            .silence_limit
            .unwrap_or(keepalive + ctx.timing.keepalive_margin);
        let frame = match time::timeout(silence_limit, socket.next()).await {
            Err(_) => {
                return Err(format!(
                    "Twitch dejó de responder durante {} s",
                    silence_limit.as_secs()
                ));
            }
            Ok(None) => return Ok(None),
            Ok(Some(frame)) => frame.map_err(|error| format!("error del WebSocket: {error}"))?,
        };
        match frame {
            Message::Text(text) => {
                let Ok(value) = serde_json::from_str::<Value>(&text) else {
                    continue;
                };
                match classify(&value) {
                    Inbound::Notification => deliver(ctx, &mut dedup, &value),
                    Inbound::Reconnect { url } => {
                        log::info!("twitch: Twitch pidió cambiar de conexión");
                        let (new_socket, welcome) = open(&url, &ctx.timing).await?;
                        // La conexión vieja recibe eventos hasta que la nueva dé la bienvenida:
                        // se vacía antes de soltarla.
                        let mut old = std::mem::replace(&mut socket, new_socket);
                        drain(ctx, &mut dedup, &mut old).await;
                        keepalive = welcome.keepalive;
                    }
                    Inbound::Revocation {
                        subscription_type,
                        status,
                    } => {
                        return Err(format!(
                            "Twitch canceló la suscripción {subscription_type} ({status})"
                        ));
                    }
                    Inbound::Keepalive | Inbound::Welcome { .. } | Inbound::Ignored => {}
                }
            }
            Message::Ping(bytes) => socket
                .send(Message::Pong(bytes))
                .await
                .map_err(|error| format!("error del WebSocket: {error}"))?,
            Message::Close(frame) => return Ok(frame.map(|f| u16::from(f.code))),
            _ => {}
        }
    }
}

/// Lee lo que quede en la conexión vieja, hasta que se cierre o pase `drain_timeout`, y publica sus
/// notificaciones.
async fn drain(ctx: &Context, dedup: &mut Dedup, socket: &mut Socket) {
    let drained = time::timeout(ctx.timing.drain_timeout, async {
        while let Some(Ok(frame)) = socket.next().await {
            match frame {
                Message::Text(text) => {
                    if let Ok(value) = serde_json::from_str::<Value>(&text)
                        && classify(&value) == Inbound::Notification
                    {
                        deliver(ctx, dedup, &value);
                    }
                }
                Message::Close(_) => break,
                _ => {}
            }
        }
    })
    .await;
    if drained.is_err() {
        log::debug!("twitch: la conexión vieja no se cerró a tiempo; se suelta");
    }
}

/// Datos de la bienvenida que necesita el resto de la sesión.
struct Welcome {
    session_id: String,
    keepalive: Duration,
}

/// Abre el WebSocket en `url` y espera su mensaje de bienvenida. La dirección de un
/// `session_reconnect` se usa tal cual y no se escribe en el registro ni en los errores.
async fn open(url: &str, timing: &Timing) -> Result<(Socket, Welcome), String> {
    let (mut socket, _) = time::timeout(timing.welcome_timeout, connect_async(url))
        .await
        .map_err(|_| "se agotó el tiempo al abrir el WebSocket".to_owned())?
        .map_err(|_| "no se pudo abrir el WebSocket".to_owned())?;
    let welcome = time::timeout(timing.welcome_timeout, async {
        loop {
            match socket.next().await {
                None => return Err("Twitch cerró la conexión antes de saludar".to_owned()),
                Some(Err(error)) => return Err(format!("error del WebSocket: {error}")),
                Some(Ok(Message::Text(text))) => {
                    let Ok(value) = serde_json::from_str::<Value>(&text) else {
                        continue;
                    };
                    if let Inbound::Welcome {
                        session_id,
                        keepalive,
                    } = classify(&value)
                    {
                        return Ok(Welcome {
                            session_id,
                            keepalive,
                        });
                    }
                }
                Some(Ok(Message::Close(_))) => {
                    return Err("Twitch cerró la conexión antes de saludar".to_owned());
                }
                Some(Ok(_)) => {}
            }
        }
    })
    .await
    .map_err(|_| "Twitch no envió la bienvenida a tiempo".to_owned())??;
    Ok((socket, welcome))
}

/// Traduce una notificación y la entrega al sumidero, salvo que sea una entrega repetida.
fn deliver(ctx: &Context, dedup: &mut Dedup, message: &Value) {
    if dedup.is_repeat(message) {
        log::debug!("twitch: notificación repetida descartada");
        return;
    }
    let Some(kind) = event_from_notification(message) else {
        return;
    };
    if let EventKind::Unknown { name } = &kind {
        log::debug!("twitch: evento no reconocido: {name}");
    }
    ctx.sink.publish(Event {
        platform: Platform::Twitch,
        received_at: now(),
        kind,
    });
}

/// Ejecuta una llamada a la API con el token vigente. Si Twitch responde 401, renueva el token y
/// reintenta una vez.
async fn with_token<T, F, Fut>(ctx: &Context, call: F) -> Result<T, String>
where
    F: Fn(String) -> Fut,
    Fut: Future<Output = Result<T, ApiError>>,
{
    let token = ctx
        .tokens
        .access_token()
        .await
        .map_err(|error| error.to_string())?;
    match call(token).await {
        Err(ApiError::Unauthorized) => {
            log::debug!("twitch: el token fue rechazado; se renueva");
            let renewed = ctx
                .tokens
                .force_refresh()
                .await
                .map_err(|error| error.to_string())?;
            call(renewed).await.map_err(|error| error.to_string())
        }
        other => other.map_err(|error| error.to_string()),
    }
}

/// Resumen para el registro: cuántas suscripciones se crearon y su coste. Lo que Twitch no informa
/// se muestra como `?`, nunca como un número supuesto.
fn cost_summary(created: &[CreatedSubscription], requested: usize) -> String {
    let show = |value: Option<u64>| value.map_or_else(|| "?".to_owned(), |v| v.to_string());
    let costs: Option<Vec<u64>> = created.iter().map(|c| c.cost).collect();
    let own = costs.map(|list| list.iter().sum::<u64>());
    // Los totales de la aplicación vienen en cada respuesta; la última es la más reciente.
    let last = created.last();
    format!(
        "{} de {requested} suscripciones (coste {}; total de la aplicación {} de {})",
        created.len(),
        show(own),
        show(last.and_then(|c| c.total_cost)),
        show(last.and_then(|c| c.max_total_cost)),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        config::{DEFAULT_SCOPES, TwitchAppConfig},
        oauth_client::OAuthClient,
        subscriptions::SUBSCRIPTIONS,
        test_support::{FakeServer, fake_server},
    };
    use serde_json::json;
    use std::collections::HashMap;
    use syon_app::{CredentialError, CredentialStore, Credentials};
    use syon_domain::{ChatMessage, MessageId, Timestamp, User};
    use tokio::net::TcpListener;
    use tokio_tungstenite::accept_async;

    type ServerSocket = WebSocketStream<TcpStream>;

    const VALID: &str =
        r#"{"client_id":"cid","login":"ana","scopes":[],"user_id":"1337","expires_in":5000}"#;
    const CREATED: &str = r#"{"data":[{"id":"s","status":"enabled","type":"t","version":"1","cost":0}],"total":1,"total_cost":0,"max_total_cost":10}"#;
    const FORBIDDEN: &str =
        r#"{"error":"Forbidden","status":403,"message":"missing authorization"}"#;
    const TOKEN_JSON: &str = r#"{"access_token":"NUEVO","refresh_token":"R2","expires_in":3600,"scope":[],"token_type":"bearer"}"#;

    /// Almacén en memoria con credenciales ya guardadas.
    struct MemoryStore(Mutex<HashMap<&'static str, Credentials>>);

    impl CredentialStore for MemoryStore {
        fn load(&self, _: Platform) -> Result<Option<Credentials>, CredentialError> {
            Ok(self.0.lock().unwrap().get("twitch").cloned())
        }

        fn save(&self, _: Platform, credentials: &Credentials) -> Result<(), CredentialError> {
            self.0.lock().unwrap().insert("twitch", credentials.clone());
            Ok(())
        }

        fn clear(&self, _: Platform) -> Result<(), CredentialError> {
            self.0.lock().unwrap().remove("twitch");
            Ok(())
        }
    }

    /// Sesión ya iniciada con `token` vigente y todos los permisos.
    async fn tokens(token_url: &str, token: &str) -> Arc<TokenManager> {
        let credentials = Credentials {
            access_token: token.to_owned(),
            refresh_token: Some("R1".to_owned()),
            expires_at: Some(Timestamp::from_unix_ms(
                now().as_unix_ms().saturating_add(3_600_000),
            )),
            scopes: DEFAULT_SCOPES.iter().map(|s| (*s).to_owned()).collect(),
        };
        let store = Arc::new(MemoryStore(Mutex::new(HashMap::from([(
            "twitch",
            credentials,
        )]))));
        let oauth = OAuthClient::for_tests(TwitchAppConfig::new("cid", "s"), token_url.to_owned());
        let (manager, _) = TokenManager::start(oauth, store, |_| {}).await.unwrap();
        Arc::new(manager)
    }

    /// Sumidero que guarda lo publicado.
    #[derive(Default)]
    struct Collector(Mutex<Vec<Event>>);

    impl Collector {
        fn kinds(&self) -> Vec<EventKind> {
            self.0
                .lock()
                .unwrap()
                .iter()
                .map(|e| e.kind.clone())
                .collect()
        }
    }

    impl EventSink for Collector {
        fn publish(&self, event: Event) {
            self.0.lock().unwrap().push(event);
        }
    }

    /// Espera hasta 5 s a que se cumpla `condition`.
    async fn eventually(what: &str, condition: impl Fn() -> bool) {
        for _ in 0..1000 {
            if condition() {
                return;
            }
            time::sleep(Duration::from_millis(5)).await;
        }
        panic!("no se cumplió a tiempo: {what}");
    }

    /// Esperas muy cortas para que las pruebas de reconexión no tarden.
    fn fast() -> Timing {
        Timing {
            delay_divisor: 1000,
            ..Timing::default()
        }
    }

    /// Respuestas de la API para `sessions` conexiones completas: validar y crear las ocho.
    fn api_responses(sessions: usize) -> Vec<(u16, &'static str)> {
        let mut responses = Vec::new();
        for _ in 0..sessions {
            responses.push((200, VALID));
            responses.extend(std::iter::repeat_n((202, CREATED), SUBSCRIPTIONS.len()));
        }
        responses
    }

    async fn listener() -> (TcpListener, String) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("ws://{}", listener.local_addr().unwrap());
        (listener, url)
    }

    async fn accept(listener: &TcpListener) -> ServerSocket {
        let (stream, _) = listener.accept().await.unwrap();
        accept_async(stream).await.unwrap()
    }

    async fn send(socket: &mut ServerSocket, value: Value) {
        // Si el cliente ya cerró, no hay nada útil que hacer con el error.
        let _ = socket.send(Message::Text(value.to_string().into())).await;
    }

    /// Espera hasta que el cliente cierre la conexión.
    async fn hold(socket: &mut ServerSocket) {
        while socket.next().await.is_some() {}
    }

    fn welcome(session: &str) -> Value {
        json!({
            "metadata": { "message_type": "session_welcome" },
            "payload": { "session": { "id": session, "status": "connected", "keepalive_timeout_seconds": 10, "reconnect_url": null } }
        })
    }

    fn notification(id: &str, subscription_type: &str, event: Value) -> Value {
        json!({
            "metadata": { "message_id": id, "message_type": "notification", "subscription_type": subscription_type },
            "payload": { "subscription": { "type": subscription_type }, "event": event }
        })
    }

    fn chat(id: &str, text: &str) -> Value {
        notification(
            id,
            "channel.chat.message",
            json!({ "message_id": id, "chatter_user_name": "Ana", "message": { "text": text } }),
        )
    }

    fn chat_kind(id: &str, text: &str) -> EventKind {
        EventKind::Chat(ChatMessage {
            id: MessageId::new(id),
            author: User::new("Ana"),
            text: text.to_owned(),
        })
    }

    struct Fixture {
        connector: TwitchConnector,
        sink: Arc<Collector>,
        http: FakeServer,
    }

    async fn start(responses: Vec<(u16, &'static str)>, ws_url: &str, timing: Timing) -> Fixture {
        let http = fake_server(responses).await;
        let sink = Arc::new(Collector::default());
        let connector = TwitchConnector::start_with(
            &Handle::current(),
            TwitchApi::for_tests("cid", &http.url()),
            tokens(&http.url(), "TOKEN").await,
            sink.clone(),
            ws_url.to_owned(),
            timing,
        );
        Fixture {
            connector,
            sink,
            http,
        }
    }

    #[tokio::test]
    async fn crea_las_ocho_suscripciones_con_la_sesion_y_publica_sin_repetir() {
        let (listener, url) = listener().await;
        let server = tokio::spawn(async move {
            let mut ws = accept(&listener).await;
            send(&mut ws, welcome("S1")).await;
            send(
                &mut ws,
                json!({ "metadata": { "message_type": "session_keepalive" } }),
            )
            .await;
            send(&mut ws, chat("n1", "hola")).await;
            send(&mut ws, chat("n1", "hola")).await;
            send(
                &mut ws,
                notification(
                    "n2",
                    "stream.offline",
                    json!({ "broadcaster_user_id": "1337" }),
                ),
            )
            .await;
            hold(&mut ws).await;
        });
        let fx = start(api_responses(1), &url, Timing::default()).await;
        assert_eq!(fx.connector.platform(), Platform::Twitch);
        assert_eq!(fx.connector.status(), ConnectionStatus::Connecting);

        eventually("dos eventos publicados", || fx.sink.kinds().len() == 2).await;

        assert_eq!(
            fx.sink.kinds(),
            [chat_kind("n1", "hola"), EventKind::StreamEnded]
        );
        assert_eq!(fx.connector.status(), ConnectionStatus::Connected);
        let requests = fx.http.requests();
        assert_eq!(requests.len(), 1 + SUBSCRIPTIONS.len());
        assert!(requests[0].starts_with("GET /oauth2/validate "));
        assert!(requests[0].contains("authorization: OAuth TOKEN"));
        for (spec, request) in SUBSCRIPTIONS.iter().zip(&requests[1..]) {
            assert!(request.starts_with("POST /helix/eventsub/subscriptions "));
            assert!(request.contains("authorization: Bearer TOKEN"));
            let body: Value =
                serde_json::from_str(request.split("\r\n\r\n").nth(1).unwrap()).unwrap();
            assert_eq!(body["type"], spec.kind);
            assert_eq!(body["version"], spec.version);
            assert_eq!(body["condition"]["broadcaster_user_id"], "1337");
            assert_eq!(body["transport"]["method"], "websocket");
            assert_eq!(body["transport"]["session_id"], "S1");
        }
        server.abort();
    }

    #[tokio::test]
    async fn si_falla_una_suscripcion_las_demas_siguen_y_queda_conectado() {
        let (listener, url) = listener().await;
        let server = tokio::spawn(async move {
            let mut ws = accept(&listener).await;
            send(&mut ws, welcome("S1")).await;
            hold(&mut ws).await;
        });
        let mut responses = vec![(200, VALID), (403, FORBIDDEN)];
        responses.extend(std::iter::repeat_n((202, CREATED), SUBSCRIPTIONS.len() - 1));
        let fx = start(responses, &url, Timing::default()).await;

        eventually("conectado", || {
            fx.connector.status() == ConnectionStatus::Connected
        })
        .await;
        server.abort();
    }

    #[tokio::test]
    async fn si_no_se_crea_ninguna_suscripcion_reintenta_con_el_estado_reconnecting() {
        let (listener, url) = listener().await;
        let server = tokio::spawn(async move {
            let mut ws = accept(&listener).await;
            send(&mut ws, welcome("S1")).await;
            hold(&mut ws).await;
        });
        let mut responses = vec![(200, VALID)];
        responses.extend(std::iter::repeat_n((403, FORBIDDEN), SUBSCRIPTIONS.len()));
        let fx = start(responses, &url, fast()).await;

        eventually("reconectando", || {
            matches!(fx.connector.status(), ConnectionStatus::Reconnecting { attempt } if attempt >= 1)
        })
        .await;
        assert!(fx.sink.kinds().is_empty());
        server.abort();
    }

    #[tokio::test]
    async fn el_silencio_de_la_conexion_la_da_por_muerta_y_vuelve_a_conectar() {
        let (listener, url) = listener().await;
        let connections = Arc::new(Mutex::new(0u32));
        let counter = Arc::clone(&connections);
        let server = tokio::spawn(async move {
            loop {
                let mut ws = accept(&listener).await;
                *counter.lock().unwrap() += 1;
                send(&mut ws, welcome("S1")).await;
                tokio::spawn(async move { hold(&mut ws).await });
            }
        });
        let timing = Timing {
            silence_limit: Some(Duration::from_millis(100)),
            ..fast()
        };
        let fx = start(api_responses(2), &url, timing).await;

        eventually("una segunda conexión", || {
            *connections.lock().unwrap() >= 2
        })
        .await;
        // Cada conexión volvió a crear las ocho suscripciones con su propia sesión.
        eventually("las suscripciones de la segunda", || {
            fx.http.requests().len() == 2 * (1 + SUBSCRIPTIONS.len())
        })
        .await;
        server.abort();
    }

    #[tokio::test]
    async fn una_suscripcion_cancelada_reinicia_la_conexion() {
        let (listener, url) = listener().await;
        let connections = Arc::new(Mutex::new(0u32));
        let counter = Arc::clone(&connections);
        let server = tokio::spawn(async move {
            loop {
                let mut ws = accept(&listener).await;
                let first = {
                    let mut count = counter.lock().unwrap();
                    *count += 1;
                    *count == 1
                };
                send(&mut ws, welcome("S1")).await;
                if first {
                    send(
                        &mut ws,
                        json!({
                            "metadata": { "message_type": "revocation" },
                            "payload": { "subscription": { "type": "channel.ban", "status": "authorization_revoked" } }
                        }),
                    )
                    .await;
                }
                tokio::spawn(async move { hold(&mut ws).await });
            }
        });
        let fx = start(api_responses(2), &url, fast()).await;

        eventually("una segunda conexión", || {
            *connections.lock().unwrap() >= 2
        })
        .await;
        eventually("conectado otra vez", || {
            fx.connector.status() == ConnectionStatus::Connected
        })
        .await;
        server.abort();
    }

    #[tokio::test]
    async fn en_un_session_reconnect_abre_la_nueva_antes_de_cerrar_la_vieja_sin_perder_eventos() {
        let (listener, url) = listener().await;
        let server_url = url.clone();
        let server = tokio::spawn(async move {
            let mut old = accept(&listener).await;
            send(&mut old, welcome("S1")).await;
            send(&mut old, chat("n0", "antes")).await;
            send(
                &mut old,
                json!({
                    "metadata": { "message_type": "session_reconnect" },
                    "payload": { "session": { "id": "S1", "status": "reconnecting", "keepalive_timeout_seconds": null, "reconnect_url": server_url } }
                }),
            )
            .await;
            // El cliente abre la nueva conexión mientras la vieja sigue abierta.
            let mut new = accept(&listener).await;
            send(&mut new, welcome("S1")).await;
            // Un evento que la conexión vieja entrega hasta que se cierra.
            send(&mut old, chat("n1", "en vuelo")).await;
            let _ = old.close(None).await;
            send(&mut new, chat("n2", "despues")).await;
            hold(&mut new).await;
        });
        let fx = start(api_responses(1), &url, Timing::default()).await;

        eventually("tres eventos publicados", || fx.sink.kinds().len() == 3).await;

        assert_eq!(
            fx.sink.kinds(),
            [
                chat_kind("n0", "antes"),
                chat_kind("n1", "en vuelo"),
                chat_kind("n2", "despues")
            ]
        );
        assert_eq!(fx.connector.status(), ConnectionStatus::Connected);
        // Las suscripciones pasan a la sesión nueva solas: no se vuelve a suscribir.
        assert_eq!(fx.http.requests().len(), 1 + SUBSCRIPTIONS.len());
        server.abort();
    }

    #[tokio::test]
    async fn si_la_conexion_nueva_falla_en_un_cambio_se_reconecta_desde_cero() {
        let (listener, url) = listener().await;
        let connections = Arc::new(Mutex::new(0u32));
        let counter = Arc::clone(&connections);
        let server = tokio::spawn(async move {
            // Primera conexión: pide cambiar a una dirección donde no escucha nadie.
            let mut first = accept(&listener).await;
            *counter.lock().unwrap() += 1;
            send(&mut first, welcome("S1")).await;
            send(
                &mut first,
                json!({
                    "metadata": { "message_type": "session_reconnect" },
                    "payload": { "session": { "id": "S1", "reconnect_url": "ws://127.0.0.1:1" } }
                }),
            )
            .await;
            hold(&mut first).await;
            loop {
                let mut ws = accept(&listener).await;
                *counter.lock().unwrap() += 1;
                send(&mut ws, welcome("S2")).await;
                tokio::spawn(async move { hold(&mut ws).await });
            }
        });
        let fx = start(api_responses(2), &url, fast()).await;

        eventually("una segunda sesión", || *connections.lock().unwrap() >= 2).await;
        eventually("conectado otra vez", || {
            fx.connector.status() == ConnectionStatus::Connected
        })
        .await;
        server.abort();
    }

    #[tokio::test]
    async fn sin_bienvenida_a_tiempo_reintenta() {
        let (listener, url) = listener().await;
        let server = tokio::spawn(async move {
            let mut ws = accept(&listener).await;
            hold(&mut ws).await;
        });
        let timing = Timing {
            welcome_timeout: Duration::from_millis(100),
            ..fast()
        };
        let fx = start(api_responses(1), &url, timing).await;

        eventually("reconectando", || {
            matches!(fx.connector.status(), ConnectionStatus::Reconnecting { .. })
        })
        .await;
        server.abort();
    }

    #[tokio::test]
    async fn un_401_renueva_el_token_y_reintenta_la_llamada() {
        let (listener, url) = listener().await;
        let server = tokio::spawn(async move {
            let mut ws = accept(&listener).await;
            send(&mut ws, welcome("S1")).await;
            hold(&mut ws).await;
        });
        let token_server = fake_server(vec![(200, TOKEN_JSON)]).await;
        let mut responses = vec![(401, r#"{"status":401,"message":"invalid access token"}"#)];
        responses.extend(api_responses(1));
        let http = fake_server(responses).await;
        let sink = Arc::new(Collector::default());
        let connector = TwitchConnector::start_with(
            &Handle::current(),
            TwitchApi::for_tests("cid", &http.url()),
            tokens(&token_server.url(), "VIEJO").await,
            sink,
            url,
            Timing::default(),
        );

        eventually("conectado", || {
            connector.status() == ConnectionStatus::Connected
        })
        .await;
        let requests = http.requests();
        assert!(requests[0].contains("authorization: OAuth VIEJO"));
        assert!(requests[1].contains("authorization: OAuth NUEVO"));
        assert!(requests[2].contains("authorization: Bearer NUEVO"));
        server.abort();
    }

    #[tokio::test]
    async fn parar_deja_el_estado_en_disconnected_y_es_idempotente() {
        let (listener, url) = listener().await;
        let server = tokio::spawn(async move {
            let mut ws = accept(&listener).await;
            send(&mut ws, welcome("S1")).await;
            hold(&mut ws).await;
        });
        let fx = start(api_responses(1), &url, Timing::default()).await;
        eventually("conectado", || {
            fx.connector.status() == ConnectionStatus::Connected
        })
        .await;

        fx.connector.stop();
        fx.connector.stop();

        assert_eq!(fx.connector.status(), ConnectionStatus::Disconnected);
        time::sleep(Duration::from_millis(50)).await;
        assert_eq!(fx.connector.status(), ConnectionStatus::Disconnected);
        server.abort();
    }

    #[tokio::test]
    async fn un_error_de_red_deja_reintentando_hasta_parar() {
        let fx = start(vec![], "ws://127.0.0.1:1", fast()).await;
        eventually("reconectando", || {
            matches!(fx.connector.status(), ConnectionStatus::Reconnecting { .. })
        })
        .await;
        drop(fx.connector);
    }

    fn created(cost: Option<u64>, total: Option<u64>, max: Option<u64>) -> CreatedSubscription {
        CreatedSubscription {
            id: "s".into(),
            status: "enabled".into(),
            cost,
            total_cost: total,
            max_total_cost: max,
        }
    }

    #[test]
    fn el_resumen_de_costes_suma_lo_informado_y_marca_con_interrogacion_lo_que_falta() {
        let known = [
            created(Some(0), Some(0), Some(10)),
            created(Some(1), Some(1), Some(10)),
        ];
        assert_eq!(
            cost_summary(&known, 8),
            "2 de 8 suscripciones (coste 1; total de la aplicación 1 de 10)"
        );
        let unknown = [created(Some(0), None, None), created(None, None, None)];
        assert_eq!(
            cost_summary(&unknown, 8),
            "2 de 8 suscripciones (coste ?; total de la aplicación ? de ?)"
        );
    }
}
