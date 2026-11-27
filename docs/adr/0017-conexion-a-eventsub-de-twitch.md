# ADR-0017: Conexión a EventSub de Twitch por WebSocket con cambio de conexión sin pérdidas

- Estado: Aceptada
- Fecha: 2026-11-26

## Contexto

El [diseño 002](../designs/002-plataformas-kick-twitch.md) (sección 7) elige EventSub por WebSocket con
token de usuario para recibir los eventos de Twitch sin una URL pública, con ocho suscripciones por
canal, y la sección 9 pide leer `cost` y `max_total_cost` de cada respuesta. El [ADR-0016](0016-oauth-y-api-de-twitch.md)
ya resolvió el token y el cliente de la API. Falta decidir cómo se mantiene la conexión, qué se
suscribe y cómo se integra con el engine ([ADR-0011](0011-runtime-asincrono-del-engine.md)).

Los datos salen de la documentación de Twitch consultada el 2026-11-26. Lo que no se pudo leer ese día
se marca **sin contrastar** en la tabla y en las consecuencias.

## Decisión

1. **`TwitchConnector` implementa `PlatformConnector`.** `TwitchConnector::start(handle, tokens, sumidero)`
   lanza una tarea en el runtime del engine y vuelve enseguida. Recibe un `TokenManager` con la sesión ya
   iniciada: quién inicia sesión y de dónde sale el `client_secret` no es cosa del conector (pendiente 6
   del diseño). Publica `Event { platform: Twitch, .. }` en el `EventSink`.
2. **Una sesión** (`session`): valida el token (su `user_id` es el canal), abre
   `wss://eventsub.wss.twitch.tv/ws`, espera el mensaje de bienvenida, crea las suscripciones con su
   `session_id` y lee mensajes hasta que la conexión se cierra o falla. Si Twitch responde 401 a una
   llamada, renueva el token (`force_refresh`) y reintenta una vez.
3. **Suscripciones** (tabla de abajo, `SUBSCRIPTIONS`). Si una falla se registra y las demás siguen; si
   no se crea ninguna, la sesión falla y se reintenta. Con cada respuesta se lee `cost`; al terminar se
   registra el total (`twitch: conectado a <login> con 8 de 8 suscripciones (coste X; total de la
   aplicación Y de Z)`). Lo que Twitch no informa se muestra como `?`. La verificación en vivo del coste
   real la hace el autor.
4. **Keepalive.** Se da la conexión por muerta si pasa el plazo `keepalive_timeout_seconds` de la
   bienvenida (10 s si falta) más 5 s de margen sin ningún mensaje. Los `Ping` de WebSocket se contestan.
5. **`session_reconnect`: la conexión nueva se abre antes de soltar la vieja.** Ante el mensaje se abre
   `reconnect_url` tal cual (no se modifica ni se escribe en el registro), se espera su bienvenida, se
   sigue leyendo la conexión vieja hasta 2 s o hasta que se cierre, publicando lo que traiga, y entonces se
   cambia a la nueva. Las suscripciones pasan solas a la sesión nueva: no se vuelve a suscribir. Si la
   conexión nueva falla, se reconecta desde cero. Mientras tanto el estado sigue `Connected`.
6. **Reconexión** con `Backoff` de `syon-app` (1 s, 2 s... 30 s), que vuelve a 1 s al crear las
   suscripciones. Un cierre, un error, el silencio o una suscripción cancelada (`revocation`, por
   ejemplo `authorization_revoked`) terminan la sesión y empiezan otra, que crea las suscripciones de
   nuevo.
7. **Estado:** `Connecting` al arrancar, `Connected` cuando las suscripciones están creadas,
   `Reconnecting { attempt }` mientras espera y `Disconnected` tras `stop`.
8. **Parar es abortar la tarea**, como en Kick ([ADR-0015](0015-conexion-al-feed-de-kick.md)): `stop` (y
   soltar el conector) marca `Disconnected`, impide que la tarea cambie el estado y la aborta; es idempotente.
9. **Entregas repetidas.** Se recuerdan los últimos 128 `message_id` y se descarta una notificación ya
   vista (por ejemplo, la que llegara por las dos conexiones durante un cambio).
10. **Lógica pura aparte de la red:** `messages.rs` clasifica los mensajes y descarta repetidos,
    `events.rs` traduce y `subscriptions.rs` es la tabla; el conector solo los une con el socket.
11. **Sin dependencias nuevas:** `tokio-tungstenite` y `futures-util` ya están por el
    [ADR-0015](0015-conexion-al-feed-de-kick.md).

### Suscripciones

Todas por `POST /helix/eventsub/subscriptions` con `transport.method = websocket`, que exige un token de
usuario. En un canal propio, `broadcaster_user_id`, `user_id` y `moderator_user_id` son el id de quien
inició sesión.

| Suscripción | Versión | Condición | Permiso requerido | `EventKind` |
|-------------|---------|-----------|-------------------|-------------|
| `channel.chat.message` | 1 | `broadcaster_user_id`, `user_id` | `user:read:chat` | `Chat` |
| `channel.chat.message_delete` | 1 | `broadcaster_user_id`, `user_id` | `user:read:chat` | `ChatMessageDeleted` |
| `channel.chat.notification` | 1 | `broadcaster_user_id`, `user_id` | `user:read:chat` | `Subscription`, `GiftedSubs` |
| `channel.ban` | 1 | `broadcaster_user_id` | `channel:moderate` | `UserBanned` |
| `channel.channel_points_custom_reward_redemption.add` | 1 | `broadcaster_user_id` | `channel:read:redemptions` | `RewardRedeemed` |
| `channel.follow` | 2 | `broadcaster_user_id`, `moderator_user_id` | `moderator:read:followers` (**sin contrastar**) | `Followed` |
| `stream.online` | 1 | `broadcaster_user_id` | ninguno | `StreamStarted` (sin título) |
| `stream.offline` | 1 | `broadcaster_user_id` (**sin contrastar**, por analogía con `stream.online`) | ninguno | `StreamEnded` |

Traducción de los avisos de chat: `sub` es una suscripción de 1 mes; `resub`, sus meses acumulados;
`community_sub_gift`, un regalo con su total; `sub_gift` suelto, un regalo de 1. Cada `sub_gift` que
pertenece a un regalo a la comunidad no se publica, porque ese aviso ya lo cuenta. Un regalo anónimo usa
el nombre `Anónimo`. Los demás avisos (`raid`, `announcement`...) llegan como
`Unknown { name: "channel.chat.notification:<tipo>" }`.

## Alternativas consideradas

| Alternativa | Por qué no |
|-------------|-----------|
| Webhooks o conduits de EventSub | Exigen una URL HTTPS pública y un token de aplicación, que no encajan en una aplicación de escritorio |
| Volver a suscribir al cambiar de conexión | Dejaría un hueco sin eventos y gastaría suscripciones; Twitch traslada las existentes a la sesión nueva |
| Cerrar la conexión vieja nada más abrir la nueva | Pierde los eventos que la vieja entregue hasta la bienvenida de la nueva |
| `tokio::select!` para leer las dos conexiones a la vez | Obliga a activar `macros` en producción; vaciar la vieja con un plazo basta |
| Chat por IRC | Es otra conexión y otro protocolo; el chat de EventSub ya trae los avisos de suscripciones |
| Reintentar las suscripciones fallidas sin reconectar | Un fallo de permisos o de límite no se arregla solo; reconectar con espera creciente evita un bucle apretado |

## Consecuencias

- **Duplicación con Kick**, para decidir después si se extrae: el estado compartido (`Shared`), los
  `Timing`, el bucle `run` con `Backoff`, `stop`/`Drop` abortables y los ayudantes de las pruebas
  (`eventually`, `Collector`). Lo propio de Twitch (mensajes, suscripciones, cambio de conexión) no se
  comparte.
- Se prueba con un servidor HTTP y uno WebSocket de EventSub locales y falsos, incluido el cambio de
  conexión con un evento en vuelo. Falta una conexión real: el nombre de los campos, el `cost` y el
  `max_total_cost` reales, y el permiso de `channel.follow`.
- Twitch limita a 3 conexiones con suscripciones por usuario y aplicación, a 300 suscripciones por
  conexión y a un `max_total_cost` de 10 con token de usuario; con los permisos concedidos el coste es 0
  y con ocho suscripciones sobra margen. Reconectar con una conexión vieja sin cerrar podría acercarse al
  límite de conexiones: la tarea suelta la vieja al cambiar.
- Si faltan permisos (por ejemplo, el token se guardó antes de ampliarlos), Twitch rechaza esas
  suscripciones: se registra una por una y las demás funcionan. Si el usuario revoca la aplicación, la
  validación o la renovación fallan y se reintenta cada 30 s con un aviso en el registro; ofrecer un
  inicio de sesión nuevo es cosa de quien arranca el conector.
- **Sin contrastar:** el descarte por `message_id` (cada mensaje lo trae en `metadata`), la forma de
  `stream.online` y `stream.offline` (solo se asume que no traen título), y si `stream.online` avisa de
  repeticiones o estrenos (`type`) además de directos.
- En un chat compartido llegan también los mensajes de los demás canales de la sesión; hoy se publican
  como chat normal sin distinguir su origen. Los mensajes con `/me` llegan con su texto tal cual.
