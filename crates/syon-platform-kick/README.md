# syon-platform-kick

Adaptador de Kick (F2, [diseño 002](../../docs/designs/002-plataformas-kick-twitch.md)). Mantiene la
conexión al feed en tiempo real, traduce lo que entrega a eventos del dominio y gestiona el inicio de
sesión OAuth. Migra `events.rs`, `oauth.rs` y `realtime.rs` del prototipo (`prototypes/kick-console`,
[WT-0.1.0_01](../../docs/walkthroughs/WT-0.1.0_01.md)).

## Qué hace

### Eventos

- `event_from_publication(publicación, recibido_en)` interpreta una publicación del feed
  (`push.pub`) y devuelve un `syon_domain::EventKind`, o `None` si no trae el nombre de un evento.
- Traduce chat, borrado de mensaje, ban o timeout, suscripción, regalo de suscripciones, recompensa
  canjeada, total de seguidores y directo iniciado o terminado.
- El dato interno puede ser un objeto JSON o una cadena con JSON; el nombre del evento se acepta con
  o sin espacio de nombres (`App\Events\…`).
- Todo campo se lee de forma tolerante. Un evento desconocido, con campos obligatorios ausentes o con
  un contenido que no es JSON llega como `EventKind::Unknown { name }`, sin su contenido original.
- El chat conserva el id del mensaje, que el prototipo descartaba. Si el feed no trae el id se fabrica
  uno (`autor@milisegundos`) para no perder el mensaje, y un borrado posterior no lo encontrará.
- Si la publicación incluye `sender.user_id` (o `sender.id`, que el feed probablemente manda: por confirmar en vivo) e `identity.badges[].type`, conserva el id de autor y
  reconoce `broadcaster`, `moderator`, `vip` y `subscriber`. El WebSocket actual no tiene contrato
  oficial; los campos ausentes quedan desconocidos o falsos y no autorizan comandos privilegiados.

### Feed en tiempo real ([ADR-0015](../../docs/adr/0015-conexion-al-feed-de-kick.md))

- `KickConnector::start(handle, canal, sumidero)` abre el feed no oficial de Kick (Centrifugo por
  WebSocket) en el runtime del engine y publica `Event { platform: Kick, .. }` en el `EventSink`.
  Implementa `PlatformConnector`: `status()` y `stop()`; soltarlo también lo detiene.
- Se conecta como invitado con un token temporal de Kick y se suscribe a cuatro feeds del canal
  (chat y moderación, regalos y recompensas, estado del directo y Kicks). No usa el OAuth.
- Reconecta sola con espera creciente (1 s a 30 s, `Backoff` de `syon-app`) ante un cierre, un error de
  red o si el servidor calla más de dos pings más 5 s. Un cierre terminal de Centrifugo espera al menos
  5 s. El estado pasa por `Connecting`, `Connected`, `Reconnecting { attempt }` y `Disconnected`.
- `KickChannel::new("slug")` consulta los ids del canal a Kick; con `id` y `chatroom_id` indicados no
  consulta (Kick a veces responde 403 a esa petición).

### OAuth 2.1 ([ADR-0014](../../docs/adr/0014-oauth-de-kick.md))

- `OAuthClient` inicia sesión con PKCE S256 y un callback local, y renueva tokens. No guarda nada.
- `TokenManager` deja lista una sesión: reutiliza las credenciales guardadas, las renueva si van a
  vencer (margen de 60 s) o inicia sesión de nuevo si no hay, faltan permisos o la renovación falla.
  Persiste solo por el puerto `CredentialStore`. `force_refresh` renueva a la fuerza ante un 401.
- Pide `user:read`, `channel:read` y `chat:write` desde el primer inicio de sesión.
- Los errores (`OAuthError`) nunca incluyen tokens, el `client_secret` ni el contenido de una respuesta
  ilegible; `KickAppConfig` implementa `Debug` sin el secreto.

## Qué no hace

- No crea el runtime ni el cliente de las plataformas del engine: recibe el `Handle` y el `EventSink`.
- No conserva el contenido de un evento que no entiende: lo descarta o lo entrega como `Unknown`.
- No lee `.env`, el entorno ni los ajustes: `KickAppConfig` llega por constructor.
- No lee el reloj en los eventos: quien llama pasa la hora de recepción.
- No abre el navegador: entrega el enlace a quien llama (`on_url`).
- No envía mensajes ni llama a la API pública todavía (F3).
- Los nombres de campo del feed, los endpoints de `kick.com` y de `id.kick.com` salen del prototipo y no
  se han contrastado en vivo; el feed y el OAuth se prueban con servidores locales falsos.

## API pública

| Elemento | Para qué sirve |
|----------|----------------|
| `event_from_publication(&Value, Timestamp)` | Traduce una publicación del feed a un `EventKind` |
| `KickConnector::start(&Handle, KickChannel, Arc<dyn EventSink>)` | Abre el feed y lo mantiene; `PlatformConnector` |
| `KickChannel::new(slug)` | Canal que se escucha, con ids opcionales |
| `KickAppConfig::new(client_id, client_secret)` | Datos de la aplicación de Kick, con callback y permisos por defecto |
| `OAuthClient::new(config)` | Cliente OAuth: `login(on_url)` y `refresh(&credenciales)` |
| `TokenManager::start(oauth, almacén, on_url)` | Deja lista la sesión; devuelve su `Source` |
| `TokenManager::access_token()` / `force_refresh()` | Token vigente, renovado si hace falta |
| `OAuthError` / `ConnectError` | Fallos tipados del inicio de sesión y de la preparación del feed |

Dependencias: `syon-app`, `syon-domain`, `serde`, `serde_json`, `reqwest`, `sha2`, `base64`, `rand`,
`tokio-tungstenite`, `futures-util`, `log` y `tokio`.
