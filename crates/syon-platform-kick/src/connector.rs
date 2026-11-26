use crate::{
    clock::now,
    error::ConnectError,
    events::event_from_publication,
    frames::{
        CONNECT_ID, Channel, DEFAULT_PING, FIRST_SUBSCRIBE_ID, Inbound, centrifugo_url,
        channel_from_json, classify, feed_names, is_terminal,
    },
};
use futures_util::{SinkExt, StreamExt};
use reqwest::Client;
use serde_json::{Value, json};
use std::{
    sync::{
        Arc, Mutex, PoisonError,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use syon_app::{Backoff, ConnectionStatus, EventSink, PlatformConnector};
use syon_domain::{Event, EventKind, Platform};
use tokio::{net::TcpStream, runtime::Handle, task::AbortHandle, time};
use tokio_tungstenite::{
    MaybeTlsStream, WebSocketStream, connect_async,
    tungstenite::{client::IntoClientRequest, http::HeaderValue, protocol::Message},
};

const KICK_API: &str = "https://kick.com/api/v2/channels";
const REALTIME_API: &str = "https://web.kick.com/api/v1/realtime";
/// Espera mínima ante un cierre terminal de Centrifugo.
const TERMINAL_FLOOR: Duration = Duration::from_secs(5);
/// Margen que se suma a dos intervalos de ping antes de dar la conexión por muerta.
const SILENCE_SLACK: Duration = Duration::from_secs(5);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

type Socket = WebSocketStream<MaybeTlsStream<TcpStream>>;

/// Canal de Kick que se escucha.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KickChannel {
    /// Nombre del canal en la dirección de Kick (`kick.com/<slug>`).
    pub slug: String,
    /// Id numérico del canal. Con `chatroom_id` evita consultar a Kick, que a veces bloquea esa
    /// consulta (403).
    pub id: Option<u64>,
    /// Id numérico del chat del canal.
    pub chatroom_id: Option<u64>,
}

impl KickChannel {
    /// Canal cuyos ids se consultan a Kick al conectar.
    pub fn new(slug: impl Into<String>) -> Self {
        Self {
            slug: slug.into(),
            id: None,
            chatroom_id: None,
        }
    }

    /// Los ids si se indicaron los dos.
    fn known(&self) -> Option<Channel> {
        Some(Channel {
            slug: self.slug.clone(),
            id: self.id?,
            chatroom_id: self.chatroom_id?,
        })
    }
}

/// Direcciones de la API de Kick; se cambian en las pruebas.
#[derive(Debug, Clone)]
pub(crate) struct Endpoints {
    pub(crate) channel_api: String,
    pub(crate) realtime_api: String,
}

impl Default for Endpoints {
    fn default() -> Self {
        Self {
            channel_api: KICK_API.to_owned(),
            realtime_api: REALTIME_API.to_owned(),
        }
    }
}

/// Tiempos de la conexión; se acortan en las pruebas.
#[derive(Debug, Clone)]
pub(crate) struct Timing {
    /// Divide las esperas entre reconexiones (1 en producción).
    pub(crate) delay_divisor: u32,
    /// Silencio tras el cual se da la conexión por muerta; `None` usa dos pings más un margen.
    pub(crate) silence_limit: Option<Duration>,
}

impl Default for Timing {
    fn default() -> Self {
        Self {
            delay_divisor: 1,
            silence_limit: None,
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
    http: Client,
    channel: KickChannel,
    sink: Arc<dyn EventSink>,
    shared: Arc<Shared>,
    endpoints: Endpoints,
    timing: Timing,
}

/// Conexión al feed en tiempo real de Kick. Publica en un [`EventSink`] los eventos del canal y
/// reconecta sola con espera creciente hasta que se la detiene (con [`PlatformConnector::stop`] o
/// al soltarla).
///
/// El feed es el no oficial de kick.com (ADR-0014 y diseño 002): no usa las credenciales de OAuth y
/// puede cambiar o bloquear clientes.
pub struct KickConnector {
    shared: Arc<Shared>,
    task: AbortHandle,
}

impl KickConnector {
    /// Arranca la conexión en el runtime de `handle` y vuelve enseguida: el estado empieza en
    /// `Connecting` y los fallos de red se reintentan por su cuenta.
    pub fn start(
        handle: &Handle,
        channel: KickChannel,
        sink: Arc<dyn EventSink>,
    ) -> Result<Self, ConnectError> {
        Self::start_with(
            handle,
            channel,
            sink,
            Endpoints::default(),
            Timing::default(),
        )
    }

    pub(crate) fn start_with(
        handle: &Handle,
        channel: KickChannel,
        sink: Arc<dyn EventSink>,
        endpoints: Endpoints,
        timing: Timing,
    ) -> Result<Self, ConnectError> {
        let http = Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .build()
            .map_err(|error| ConnectError::Http(error.without_url().to_string()))?;
        let shared = Arc::new(Shared {
            status: Mutex::new(ConnectionStatus::Connecting),
            stopped: AtomicBool::new(false),
        });
        let context = Context {
            http,
            channel,
            sink,
            shared: Arc::clone(&shared),
            endpoints,
            timing,
        };
        let task = handle.spawn(run(context)).abort_handle();
        Ok(Self { shared, task })
    }
}

impl PlatformConnector for KickConnector {
    fn platform(&self) -> Platform {
        Platform::Kick
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

impl Drop for KickConnector {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Mantiene la conexión viva: sesión tras sesión, con espera creciente entre ellas.
async fn run(ctx: Context) {
    let mut backoff = Backoff::new();
    let mut resolved = ctx.channel.known();
    loop {
        let outcome = session(&ctx, &mut resolved, &mut backoff).await;
        let mut delay = backoff.next_delay();
        let reason = match outcome {
            Ok(code) => {
                if code.is_some_and(is_terminal) {
                    delay = delay.max(TERMINAL_FLOOR);
                }
                format!(
                    "conexión cerrada (código {})",
                    code.map_or_else(|| "-".to_owned(), |c| c.to_string())
                )
            }
            Err(error) => error,
        };
        let delay = delay / ctx.timing.delay_divisor.max(1);
        ctx.shared.set(ConnectionStatus::Reconnecting {
            attempt: backoff.attempt(),
        });
        log::warn!("kick: {reason}; reintentando en {delay:?}");
        time::sleep(delay).await;
    }
}

/// Una conexión completa: resuelve el canal si hace falta, negocia, se suscribe y publica hasta que
/// se cierra. Devuelve el código de cierre, si lo hubo.
async fn session(
    ctx: &Context,
    resolved: &mut Option<Channel>,
    backoff: &mut Backoff,
) -> Result<Option<u16>, String> {
    let channel = match resolved {
        Some(channel) => channel.clone(),
        None => {
            let channel = resolve_channel(ctx).await?;
            *resolved = Some(channel.clone());
            channel
        }
    };
    let client_id = new_client_id();
    let token = guest_token(ctx, &client_id).await?;
    let url = websocket_url(ctx, channel.id, &client_id).await?;

    let mut request = url
        .as_str()
        .into_client_request()
        .map_err(|error| format!("la dirección del WebSocket no es válida: {error}"))?;
    request
        .headers_mut()
        .insert("Origin", HeaderValue::from_static("https://kick.com"));
    let (mut socket, _) = connect_async(request)
        .await
        .map_err(|error| format!("no se pudo abrir el WebSocket: {error}"))?;

    let feeds = feed_names(&channel);
    send(
        &mut socket,
        json!({ "id": CONNECT_ID, "connect": { "token": token, "name": "syon" } }),
    )
    .await?;
    for (index, feed) in (0u64..).zip(&feeds) {
        send(
            &mut socket,
            json!({ "id": FIRST_SUBSCRIBE_ID + index, "subscribe": { "channel": feed } }),
        )
        .await?;
    }

    let mut ping = DEFAULT_PING;
    loop {
        let silence_limit = ctx.timing.silence_limit.unwrap_or(ping * 2 + SILENCE_SLACK);
        let frame = match time::timeout(silence_limit, socket.next()).await {
            Err(_) => {
                return Err(format!(
                    "Kick dejó de responder durante {} s",
                    silence_limit.as_secs()
                ));
            }
            Ok(None) => return Ok(None),
            Ok(Some(frame)) => frame.map_err(|error| format!("error del WebSocket: {error}"))?,
        };
        match frame {
            Message::Text(text) => {
                for line in text.lines() {
                    let Ok(value) = serde_json::from_str::<Value>(line) else {
                        continue;
                    };
                    match classify(&value, &feeds) {
                        Inbound::Ping => send(&mut socket, json!({})).await?,
                        Inbound::Connected { ping: seconds } => {
                            ping = seconds;
                            ctx.shared.set(ConnectionStatus::Connected);
                            log::info!("kick: conectado a {}", channel.slug);
                        }
                        Inbound::ConnectFailed(message) => {
                            return Err(format!("Kick rechazó la conexión: {message}"));
                        }
                        Inbound::Subscribed(feed) => {
                            if feed == feeds[0] {
                                backoff.reset();
                            }
                            log::debug!("kick: suscrito a {feed}");
                        }
                        Inbound::SubscribeFailed { feed, message } => {
                            log::warn!("kick: no se pudo suscribir a {feed}: {message}");
                        }
                        Inbound::Publication { data, .. } => publish(ctx, &data),
                        Inbound::Disconnect { code, reason } => {
                            log::warn!("kick: el servidor pidió desconectar: {reason}");
                            return Ok(Some(code));
                        }
                        Inbound::Ignored => {}
                    }
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

/// Traduce una publicación y la entrega al sumidero. Una que no se entiende se descarta.
fn publish(ctx: &Context, data: &Value) {
    let received_at = now();
    let Some(kind) = event_from_publication(data, received_at) else {
        return;
    };
    if let EventKind::Unknown { name } = &kind {
        log::debug!("kick: evento no reconocido: {name}");
    }
    ctx.sink.publish(Event {
        platform: Platform::Kick,
        received_at,
        kind,
    });
}

async fn send(socket: &mut Socket, value: Value) -> Result<(), String> {
    socket
        .send(Message::Text(value.to_string().into()))
        .await
        .map_err(|error| format!("error del WebSocket: {error}"))
}

async fn resolve_channel(ctx: &Context) -> Result<Channel, String> {
    let url = format!("{}/{}", ctx.endpoints.channel_api, ctx.channel.slug);
    let body: Value = get_json(&ctx.http, &url, "Kick no pudo encontrar ese canal").await?;
    channel_from_json(&ctx.channel.slug, &body)
}

async fn guest_token(ctx: &Context, client_id: &str) -> Result<String, String> {
    let url = format!("{}/auth/connection", ctx.endpoints.realtime_api);
    let response = post_json(
        &ctx.http,
        &url,
        &json!({ "client_id": client_id }),
        "no se pudo pedir el token temporal de Kick",
    )
    .await?;
    response["data"]["token"]
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| "Kick no incluyó un token en su respuesta".to_owned())
}

async fn websocket_url(ctx: &Context, channel_id: u64, client_id: &str) -> Result<String, String> {
    let url = format!(
        "{}/channels/{channel_id}/chat/connection",
        ctx.endpoints.realtime_api
    );
    let response = post_json(
        &ctx.http,
        &url,
        &json!({
            "client": { "id": client_id, "type": "web" },
            "capabilities": { "accepted_providers": [{ "provider": "centrifugo" }, { "provider": "pusher" }] }
        }),
        "no se pudo negociar la conexión en tiempo real con Kick",
    )
    .await?;
    centrifugo_url(&response).ok_or_else(|| "Kick no ofreció una conexión Centrifugo".to_owned())
}

/// `GET` que devuelve JSON. El mensaje de error no incluye la URL.
async fn get_json(http: &Client, url: &str, what: &str) -> Result<Value, String> {
    let response = http
        .get(url)
        .send()
        .await
        .and_then(reqwest::Response::error_for_status)
        .map_err(|error| format!("{what}: {}", error.without_url()))?;
    response
        .json()
        .await
        .map_err(|error| format!("{what}: respuesta ilegible ({})", error.without_url()))
}

/// `POST` con cuerpo JSON que devuelve JSON. El mensaje de error no incluye la URL.
async fn post_json(http: &Client, url: &str, body: &Value, what: &str) -> Result<Value, String> {
    let response = http
        .post(url)
        .json(body)
        .send()
        .await
        .and_then(reqwest::Response::error_for_status)
        .map_err(|error| format!("{what}: {}", error.without_url()))?;
    response
        .json()
        .await
        .map_err(|error| format!("{what}: respuesta ilegible ({})", error.without_url()))
}

/// Identificador de cliente con forma de UUID v4, hecho con bytes aleatorios.
fn new_client_id() -> String {
    let mut bytes = [0u8; 16];
    rand::fill(&mut bytes);
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    format!(
        "{}-{}-{}-{}-{}",
        &hex[0..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..32]
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{FakeServer, fake_server};
    use syon_domain::User;
    use tokio::net::TcpListener;
    use tokio_tungstenite::{
        accept_async,
        tungstenite::protocol::{CloseFrame, frame::coding::CloseCode},
    };

    type ServerSocket = WebSocketStream<TcpStream>;

    const TOKEN: &str = r#"{"data":{"token":"T"}}"#;
    const CHANNEL: &str = r#"{"id":7,"chatroom":{"id":9}}"#;

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

    fn leak(text: String) -> &'static str {
        Box::leak(text.into_boxed_str())
    }

    /// Respuestas de la negociación HTTP para `sessions` conexiones, que apuntan al WebSocket local.
    fn negotiation(port: u16, sessions: usize, with_channel: bool) -> Vec<(u16, &'static str)> {
        let connection = leak(format!(
            r#"{{"data":{{"connections":[{{"provider":"centrifugo","credentials":{{"url":"ws://127.0.0.1:{port}"}}}}]}}}}"#
        ));
        let mut responses = Vec::new();
        if with_channel {
            responses.push((200, CHANNEL));
        }
        for _ in 0..sessions {
            responses.push((200, TOKEN));
            responses.push((200, connection));
        }
        responses
    }

    fn endpoints(server: &FakeServer) -> Endpoints {
        Endpoints {
            channel_api: format!("{}/api/v2/channels", server.url()),
            realtime_api: format!("{}/realtime", server.url()),
        }
    }

    fn fast() -> Timing {
        Timing {
            delay_divisor: 1000,
            silence_limit: None,
        }
    }

    fn start(
        channel: KickChannel,
        server: &FakeServer,
        timing: Timing,
    ) -> (KickConnector, Arc<Collector>) {
        let sink = Arc::new(Collector::default());
        let connector = KickConnector::start_with(
            &Handle::current(),
            channel,
            sink.clone(),
            endpoints(server),
            timing,
        )
        .unwrap();
        (connector, sink)
    }

    async fn next_json(socket: &mut ServerSocket) -> Value {
        loop {
            match socket.next().await.unwrap().unwrap() {
                Message::Text(text) => return serde_json::from_str(&text).unwrap(),
                _ => continue,
            }
        }
    }

    /// Acepta una conexión y hace el saludo de Centrifugo: `connect` y los cuatro `subscribe`.
    async fn handshake(listener: &TcpListener) -> ServerSocket {
        let (stream, _) = listener.accept().await.unwrap();
        let mut socket = accept_async(stream).await.unwrap();
        let connect = next_json(&mut socket).await;
        assert_eq!(connect["connect"]["token"], "T");
        let reply = json!({ "id": 1, "connect": { "ping": 25 } });
        socket
            .send(Message::Text(reply.to_string().into()))
            .await
            .unwrap();
        for index in 0..4u64 {
            let subscribe = next_json(&mut socket).await;
            assert_eq!(subscribe["id"], FIRST_SUBSCRIBE_ID + index);
            let reply = json!({ "id": FIRST_SUBSCRIBE_ID + index, "subscribe": {} });
            socket
                .send(Message::Text(reply.to_string().into()))
                .await
                .unwrap();
        }
        socket
    }

    fn publication(event: &str, data: Value) -> Message {
        let frame = json!({ "push": { "channel": "chatroom_9", "pub": { "data": { "event": event, "data": data.to_string() } } } });
        Message::Text(frame.to_string().into())
    }

    async fn listener() -> (TcpListener, u16) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        (listener, port)
    }

    #[tokio::test]
    async fn conecta_resuelve_el_canal_y_publica_los_eventos_en_el_sumidero() {
        let (listener, port) = listener().await;
        let server = fake_server(negotiation(port, 1, true)).await;
        let (connector, sink) = start(KickChannel::new("ana"), &server, fast());
        assert_eq!(connector.status(), ConnectionStatus::Connecting);
        assert_eq!(connector.platform(), Platform::Kick);

        let mut socket = handshake(&listener).await;
        let chat = json!({ "id": "m-1", "content": "hola", "sender": { "username": "ana" } });
        socket
            .send(publication("App\\Events\\ChatMessageEvent", chat))
            .await
            .unwrap();

        eventually("el chat llega al sumidero", || sink.kinds().len() == 1).await;
        let EventKind::Chat(message) = &sink.kinds()[0] else {
            panic!("debía ser un chat");
        };
        assert_eq!(
            (message.id.as_str(), message.text.as_str()),
            ("m-1", "hola")
        );
        assert_eq!(message.author, User::new("ana"));
        assert_eq!(sink.0.lock().unwrap()[0].platform, Platform::Kick);
        eventually("estado conectado", || {
            connector.status() == ConnectionStatus::Connected
        })
        .await;

        let requests = server.requests();
        assert!(
            requests[0].starts_with("GET /api/v2/channels/ana "),
            "{}",
            requests[0]
        );
        assert!(requests[1].starts_with("POST /realtime/auth/connection "));
        assert!(requests[2].starts_with("POST /realtime/channels/7/chat/connection "));
    }

    #[tokio::test]
    async fn con_los_ids_indicados_no_consulta_el_canal() {
        let (listener, port) = listener().await;
        let server = fake_server(negotiation(port, 1, false)).await;
        let channel = KickChannel {
            slug: "ana".into(),
            id: Some(7),
            chatroom_id: Some(9),
        };
        let (_connector, _sink) = start(channel, &server, fast());
        let _socket = handshake(&listener).await;
        assert!(server.requests()[0].starts_with("POST /realtime/auth/connection "));
    }

    #[tokio::test]
    async fn tras_un_cierre_terminal_reconecta_y_sigue_publicando() {
        let (listener, port) = listener().await;
        let server = fake_server(negotiation(port, 2, true)).await;
        let (connector, sink) = start(KickChannel::new("ana"), &server, fast());

        let mut first = handshake(&listener).await;
        eventually("conectado", || {
            connector.status() == ConnectionStatus::Connected
        })
        .await;
        let close = CloseFrame {
            code: CloseCode::Library(4500),
            reason: "bye".into(),
        };
        first.close(Some(close)).await.unwrap();

        let mut second = handshake(&listener).await;
        let sub = json!({ "username": "luis", "months": 2 });
        second
            .send(publication("SubscriptionEvent", sub))
            .await
            .unwrap();
        eventually("llega el evento de la segunda sesión", || {
            sink.kinds()
                == [EventKind::Subscription {
                    user: User::new("luis"),
                    months: 2,
                }]
        })
        .await;
        let tokens = server
            .requests()
            .iter()
            .filter(|r| r.starts_with("POST /realtime/auth/connection"))
            .count();
        assert_eq!(tokens, 2);
        // El canal se resolvió una sola vez.
        let gets = server
            .requests()
            .iter()
            .filter(|r| r.starts_with("GET "))
            .count();
        assert_eq!(gets, 1);
    }

    #[tokio::test]
    async fn si_el_servidor_calla_da_la_conexion_por_muerta_y_reconecta() {
        let (listener, port) = listener().await;
        let server = fake_server(negotiation(port, 2, true)).await;
        let timing = Timing {
            delay_divisor: 1000,
            silence_limit: Some(Duration::from_millis(200)),
        };
        let (_connector, _sink) = start(KickChannel::new("ana"), &server, timing);

        let _mudo = handshake(&listener).await;
        let _segundo = time::timeout(Duration::from_secs(5), handshake(&listener))
            .await
            .expect("debía reconectar tras el silencio");
    }

    #[tokio::test]
    async fn los_fallos_de_negociacion_se_reintentan_y_se_refleja_el_estado() {
        let (listener, port) = listener().await;
        let mut responses = vec![(200, CHANNEL), (500, "{}")];
        responses.extend(negotiation(port, 1, false));
        let server = fake_server(responses).await;
        let (connector, _sink) = start(KickChannel::new("ana"), &server, fast());

        eventually("estado de reconexión", || {
            matches!(connector.status(), ConnectionStatus::Reconnecting { .. })
                || connector.status() == ConnectionStatus::Connected
        })
        .await;
        let _socket = handshake(&listener).await;
        eventually("conectado tras el reintento", || {
            connector.status() == ConnectionStatus::Connected
        })
        .await;
    }

    #[tokio::test]
    async fn stop_deja_el_estado_desconectado_y_no_se_publica_nada_mas() {
        let (listener, port) = listener().await;
        let server = fake_server(negotiation(port, 1, true)).await;
        let (connector, sink) = start(KickChannel::new("ana"), &server, fast());
        let mut socket = handshake(&listener).await;
        eventually("conectado", || {
            connector.status() == ConnectionStatus::Connected
        })
        .await;

        connector.stop();
        connector.stop();
        assert_eq!(connector.status(), ConnectionStatus::Disconnected);

        let sub = json!({ "username": "tarde", "months": 1 });
        let _ = socket.send(publication("SubscriptionEvent", sub)).await;
        time::sleep(Duration::from_millis(100)).await;
        assert!(sink.kinds().is_empty());
        assert_eq!(connector.status(), ConnectionStatus::Disconnected);
    }

    #[tokio::test]
    async fn soltar_el_conector_lo_detiene() {
        let (listener, port) = listener().await;
        let server = fake_server(negotiation(port, 1, true)).await;
        let (connector, sink) = start(KickChannel::new("ana"), &server, fast());
        let mut socket = handshake(&listener).await;
        drop(connector);
        let sub = json!({ "username": "tarde", "months": 1 });
        let _ = socket.send(publication("SubscriptionEvent", sub)).await;
        time::sleep(Duration::from_millis(100)).await;
        assert!(sink.kinds().is_empty());
    }

    #[tokio::test]
    async fn abrir_un_wss_no_entra_en_panico_por_falta_de_proveedor_criptografico() {
        let (listener, port) = listener().await;
        tokio::spawn(async move {
            // Acepta y suelta la conexión: el cliente ya habrá preparado su configuración TLS.
            let _ = listener.accept().await;
        });
        let result = connect_async(format!("wss://127.0.0.1:{port}")).await;
        assert!(result.is_err());
    }

    #[test]
    fn el_id_de_cliente_tiene_forma_de_uuid_v4_y_cambia() {
        let id = new_client_id();
        let parts: Vec<&str> = id.split('-').collect();
        assert_eq!(
            parts.iter().map(|p| p.len()).collect::<Vec<_>>(),
            [8, 4, 4, 4, 12]
        );
        assert!(parts[2].starts_with('4'));
        assert!(matches!(parts[3].as_bytes()[0], b'8' | b'9' | b'a' | b'b'));
        assert_ne!(id, new_client_id());
    }

    #[test]
    fn los_ids_conocidos_se_usan_solo_si_estan_los_dos() {
        let mut channel = KickChannel::new("ana");
        assert_eq!(channel.known(), None);
        channel.id = Some(7);
        assert_eq!(channel.known(), None);
        channel.chatroom_id = Some(9);
        assert_eq!(
            channel.known(),
            Some(Channel {
                slug: "ana".into(),
                id: 7,
                chatroom_id: 9
            })
        );
    }
}
