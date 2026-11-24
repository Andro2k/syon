//! Conexión al feed en tiempo real de Kick (Centrifugo por WebSocket) con reconexión.
//!
//! Flujo no oficial y sujeto a cambios: canal público -> token de invitado ->
//! negociación -> WebSocket (con `Origin: https://kick.com`) -> connect + subscribe.

use std::time::Duration;

use anyhow::{Context, Result, anyhow};
use futures_util::{SinkExt, StreamExt};
use reqwest::Client;
use serde_json::{Value, json};
use tokio::{net::TcpStream, sync::mpsc, time};
use tokio_tungstenite::{
    MaybeTlsStream, WebSocketStream, connect_async,
    tungstenite::{client::IntoClientRequest, http::HeaderValue, protocol::Message},
};
use uuid::Uuid;

use crate::events::{self, Published};

const KICK_API: &str = "https://kick.com/api/v2/channels";
const REALTIME_API: &str = "https://web.kick.com/api/v1/realtime";

const DEFAULT_PING: Duration = Duration::from_secs(25);
const MIN_BACKOFF: Duration = Duration::from_secs(1);
const MAX_BACKOFF: Duration = Duration::from_secs(30);
const TERMINAL_FLOOR: Duration = Duration::from_secs(5);

const CONNECT_ID: u64 = 1;
const FIRST_SUBSCRIBE_ID: u64 = 2;

type Socket = WebSocketStream<MaybeTlsStream<TcpStream>>;

#[derive(Debug, Clone, PartialEq)]
pub struct Channel {
    pub slug: String,
    pub id: u64,
    pub chatroom_id: u64,
}

pub enum FeedItem {
    Event { feed: String, published: Published },
    Status(String),
}

/// Espera creciente entre reconexiones: 1 s, 2 s, 4 s... hasta 30 s.
#[derive(Debug)]
pub struct Backoff {
    next: Duration,
}

impl Backoff {
    pub fn new() -> Self {
        Self { next: MIN_BACKOFF }
    }

    pub fn next_delay(&mut self) -> Duration {
        let delay = self.next;
        self.next = (self.next * 2).min(MAX_BACKOFF);
        delay
    }

    pub fn reset(&mut self) {
        self.next = MIN_BACKOFF;
    }
}

pub async fn resolve_channel(http: &Client, slug: &str) -> Result<Channel> {
    let body: Value = http
        .get(format!("{KICK_API}/{slug}"))
        .send()
        .await?
        .error_for_status()
        .context("Kick no pudo encontrar ese canal")?
        .json()
        .await?;
    channel_from_json(slug, &body)
}

fn channel_from_json(slug: &str, body: &Value) -> Result<Channel> {
    let id = body["id"]
        .as_u64()
        .ok_or_else(|| anyhow!("No encontré el id del canal en la respuesta de Kick"))?;
    let chatroom_id = body["chatroom"]["id"]
        .as_u64()
        .ok_or_else(|| anyhow!("No encontré el id del chatroom en la respuesta de Kick"))?;
    Ok(Channel {
        slug: slug.to_string(),
        id,
        chatroom_id,
    })
}

/// Feeds que un invitado puede leer: chat y moderación, regalos y recompensas,
/// estado del directo y Kicks. Los feeds `private-*` requieren sesión de usuario.
fn feed_names(channel: &Channel) -> Vec<String> {
    vec![
        format!("chatrooms.{}.v2", channel.chatroom_id),
        format!("chatroom_{}", channel.chatroom_id),
        format!("channel.{}", channel.id),
        format!("channel_{}", channel.id),
    ]
}

#[derive(Debug, PartialEq)]
enum Inbound {
    Ping,
    Connected { ping: Duration },
    ConnectFailed(String),
    Subscribed(String),
    SubscribeFailed { feed: String, message: String },
    Publication { feed: String, data: Value },
    Disconnect { code: u16, reason: String },
    Ignored,
}

fn classify(frame: &Value, feeds: &[String]) -> Inbound {
    if frame.as_object().is_some_and(|object| object.is_empty()) {
        return Inbound::Ping;
    }
    if let Some(connect) = frame.get("connect") {
        let ping = connect["ping"]
            .as_u64()
            .map_or(DEFAULT_PING, Duration::from_secs);
        return Inbound::Connected { ping };
    }
    if let Some(push) = frame.get("push") {
        return match push.get("pub") {
            Some(data) => Inbound::Publication {
                feed: push["channel"].as_str().unwrap_or_default().to_string(),
                data: data.clone(),
            },
            None => Inbound::Ignored,
        };
    }
    if let Some(disconnect) = frame.get("disconnect") {
        return Inbound::Disconnect {
            code: disconnect["code"].as_u64().unwrap_or(0) as u16,
            reason: disconnect["reason"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
        };
    }
    let Some(id) = frame["id"].as_u64() else {
        return Inbound::Ignored;
    };
    let error = frame
        .get("error")
        .map(|e| format!("{} {}", e["code"], e["message"].as_str().unwrap_or("")));
    if id == CONNECT_ID {
        return error.map_or(Inbound::Ignored, Inbound::ConnectFailed);
    }
    let feed = id
        .checked_sub(FIRST_SUBSCRIBE_ID)
        .and_then(|index| feeds.get(index as usize))
        .cloned();
    match (feed, error) {
        (Some(feed), Some(message)) => Inbound::SubscribeFailed { feed, message },
        (Some(feed), None) if frame.get("subscribe").is_some() => Inbound::Subscribed(feed),
        _ => Inbound::Ignored,
    }
}

/// Códigos de desconexión que Centrifugo marca como terminales (3500-3999 y 4500-4999).
fn is_terminal(code: u16) -> bool {
    (3500..=3999).contains(&code) || (4500..=4999).contains(&code)
}

enum SessionEnd {
    Closed(Option<u16>),
    ReceiverGone,
}

/// Mantiene la conexión viva hasta que el receptor de `tx` se cierre.
pub async fn run(http: Client, channel: Channel, tx: mpsc::Sender<FeedItem>) {
    let mut backoff = Backoff::new();
    loop {
        let outcome = run_session(&http, &channel, &tx, &mut backoff).await;
        let mut delay = backoff.next_delay();
        let message = match outcome {
            Ok(SessionEnd::ReceiverGone) => return,
            Ok(SessionEnd::Closed(code)) => {
                if code.is_some_and(is_terminal) {
                    delay = delay.max(TERMINAL_FLOOR);
                }
                format!(
                    "Conexión cerrada (código {}).",
                    code.map_or("-".into(), |c| c.to_string())
                )
            }
            Err(error) => format!("Error: {error:#}."),
        };
        if !say(
            &tx,
            format!("{message} Reintentando en {} s...", delay.as_secs()),
        )
        .await
        {
            return;
        }
        time::sleep(delay).await;
    }
}

async fn say(tx: &mpsc::Sender<FeedItem>, text: String) -> bool {
    tx.send(FeedItem::Status(text)).await.is_ok()
}

async fn run_session(
    http: &Client,
    channel: &Channel,
    tx: &mpsc::Sender<FeedItem>,
    backoff: &mut Backoff,
) -> Result<SessionEnd> {
    let client_id = Uuid::new_v4().to_string();
    let token = guest_token(http, &client_id).await?;
    let url = websocket_url(http, channel.id, &client_id).await?;

    let mut request = url.as_str().into_client_request()?;
    request
        .headers_mut()
        .insert("Origin", HeaderValue::from_static("https://kick.com"));
    let (mut socket, _) = connect_async(request)
        .await
        .context("No se pudo abrir el WebSocket de Kick")?;

    let feeds = feed_names(channel);
    send(
        &mut socket,
        json!({ "id": CONNECT_ID, "connect": { "token": token, "name": "chat-server" } }),
    )
    .await?;
    for (index, feed) in feeds.iter().enumerate() {
        let id = FIRST_SUBSCRIBE_ID + index as u64;
        send(
            &mut socket,
            json!({ "id": id, "subscribe": { "channel": feed } }),
        )
        .await?;
    }

    let mut ping = DEFAULT_PING;
    loop {
        let silence_limit = ping * 2 + Duration::from_secs(5);
        let frame = match time::timeout(silence_limit, socket.next()).await {
            Err(_) => {
                return Err(anyhow!(
                    "Kick dejó de responder durante {} s",
                    silence_limit.as_secs()
                ));
            }
            Ok(None) => return Ok(SessionEnd::Closed(None)),
            Ok(Some(frame)) => frame?,
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
                            if !say(tx, format!("Conectado a Kick/{}.", channel.slug)).await {
                                return Ok(SessionEnd::ReceiverGone);
                            }
                        }
                        Inbound::ConnectFailed(message) => {
                            return Err(anyhow!("Kick rechazó la conexión: {message}"));
                        }
                        Inbound::Subscribed(feed) => {
                            if feed == feeds[0] {
                                backoff.reset();
                            }
                            if !say(tx, format!("Suscrito a {feed}")).await {
                                return Ok(SessionEnd::ReceiverGone);
                            }
                        }
                        Inbound::SubscribeFailed { feed, message } => {
                            if !say(tx, format!("No se pudo suscribir a {feed}: {message}")).await {
                                return Ok(SessionEnd::ReceiverGone);
                            }
                        }
                        Inbound::Publication { feed, data } => {
                            if let Some(published) = events::from_publication(&data)
                                && tx.send(FeedItem::Event { feed, published }).await.is_err()
                            {
                                return Ok(SessionEnd::ReceiverGone);
                            }
                        }
                        Inbound::Disconnect { code, reason } => {
                            say(tx, format!("Kick pidió desconectar: {reason}")).await;
                            return Ok(SessionEnd::Closed(Some(code)));
                        }
                        Inbound::Ignored => {}
                    }
                }
            }
            Message::Ping(bytes) => socket.send(Message::Pong(bytes)).await?,
            Message::Close(frame) => {
                return Ok(SessionEnd::Closed(frame.map(|f| u16::from(f.code))));
            }
            _ => {}
        }
    }
}

async fn send(socket: &mut Socket, value: Value) -> Result<()> {
    socket.send(Message::Text(value.to_string().into())).await?;
    Ok(())
}

async fn guest_token(http: &Client, client_id: &str) -> Result<String> {
    let response: Value = http
        .post(format!("{REALTIME_API}/auth/connection"))
        .json(&json!({ "client_id": client_id }))
        .send()
        .await?
        .error_for_status()
        .context("No se pudo pedir el token temporal de Kick")?
        .json()
        .await?;
    response["data"]["token"]
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| anyhow!("Kick no incluyó un token en su respuesta"))
}

async fn websocket_url(http: &Client, channel_id: u64, client_id: &str) -> Result<String> {
    let response: Value = http
        .post(format!("{REALTIME_API}/channels/{channel_id}/chat/connection"))
        .json(&json!({
            "client": { "id": client_id, "type": "web" },
            "capabilities": { "accepted_providers": [{ "provider": "centrifugo" }, { "provider": "pusher" }] }
        }))
        .send()
        .await?
        .error_for_status()
        .context("No se pudo negociar la conexión realtime con Kick")?
        .json()
        .await?;
    centrifugo_url(&response).ok_or_else(|| anyhow!("Kick no ofreció una conexión Centrifugo"))
}

fn centrifugo_url(response: &Value) -> Option<String> {
    response["data"]["connections"]
        .as_array()?
        .iter()
        .find(|connection| connection["provider"] == "centrifugo")
        .and_then(|connection| connection["credentials"]["url"].as_str())
        .map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn feeds() -> Vec<String> {
        feed_names(&Channel {
            slug: "x".into(),
            id: 7,
            chatroom_id: 9,
        })
    }

    #[test]
    fn backoff_duplica_hasta_el_tope_y_se_reinicia() {
        let mut backoff = Backoff::new();
        let secs: Vec<u64> = (0..7).map(|_| backoff.next_delay().as_secs()).collect();
        assert_eq!(secs, [1, 2, 4, 8, 16, 30, 30]);
        backoff.reset();
        assert_eq!(backoff.next_delay().as_secs(), 1);
    }

    #[test]
    fn nombres_de_feeds() {
        assert_eq!(
            feeds(),
            ["chatrooms.9.v2", "chatroom_9", "channel.7", "channel_7"]
        );
    }

    #[test]
    fn clasifica_ping_connect_y_suscripciones() {
        let feeds = feeds();
        assert_eq!(classify(&json!({}), &feeds), Inbound::Ping);
        assert_eq!(
            classify(&json!({ "id": 1, "connect": { "ping": 30 } }), &feeds),
            Inbound::Connected {
                ping: Duration::from_secs(30)
            }
        );
        assert_eq!(
            classify(&json!({ "id": 2, "subscribe": {} }), &feeds),
            Inbound::Subscribed("chatrooms.9.v2".into())
        );
        assert_eq!(
            classify(
                &json!({ "id": 4, "error": { "code": 103, "message": "permission denied" } }),
                &feeds
            ),
            Inbound::SubscribeFailed {
                feed: "channel.7".into(),
                message: "103 permission denied".into()
            }
        );
        assert!(matches!(
            classify(
                &json!({ "id": 1, "error": { "code": 109, "message": "token expired" } }),
                &feeds
            ),
            Inbound::ConnectFailed(_)
        ));
    }

    #[test]
    fn clasifica_publicaciones_y_desconexiones() {
        let feeds = feeds();
        let push =
            json!({ "push": { "channel": "chatroom_9", "pub": { "data": { "event": "X" } } } });
        assert!(
            matches!(classify(&push, &feeds), Inbound::Publication { ref feed, .. } if feed == "chatroom_9")
        );
        assert_eq!(
            classify(
                &json!({ "disconnect": { "code": 3500, "reason": "bye" } }),
                &feeds
            ),
            Inbound::Disconnect {
                code: 3500,
                reason: "bye".into()
            }
        );
        assert_eq!(
            classify(&json!({ "id": 99, "subscribe": {} }), &feeds),
            Inbound::Ignored
        );
    }

    #[test]
    fn codigos_terminales() {
        assert!(is_terminal(3500) && is_terminal(4999));
        assert!(!is_terminal(3000) && !is_terminal(1006));
    }

    #[test]
    fn extrae_canal_y_url_de_las_respuestas() {
        let canal = json!({ "id": 7, "chatroom": { "id": 9 } });
        assert_eq!(
            channel_from_json("x", &canal).unwrap(),
            Channel {
                slug: "x".into(),
                id: 7,
                chatroom_id: 9
            }
        );
        assert!(channel_from_json("x", &json!({ "id": 7 })).is_err());

        let negociacion = json!({ "data": { "connections": [
            { "provider": "pusher", "credentials": { "url": "wss://otro" } },
            { "provider": "centrifugo", "credentials": { "url": "wss://a/connection/websocket" } }
        ] } });
        assert_eq!(
            centrifugo_url(&negociacion).unwrap(),
            "wss://a/connection/websocket"
        );
        assert!(centrifugo_url(&json!({})).is_none());
    }
}
