# syon-platform-twitch

Adaptador de Twitch (F2, [diseño 002](../../docs/designs/002-plataformas-kick-twitch.md)). Gestiona el
inicio de sesión OAuth, ofrece un cliente mínimo de la API de Helix y traduce los eventos de EventSub.
Sigue la estructura de [`syon-platform-kick`](../syon-platform-kick/README.md), del que no depende. La
conexión por WebSocket llega en el parche siguiente.

## Qué hace

### Eventos

- `event_from_notification(&Value)` interpreta un mensaje `notification` de EventSub y devuelve un
  `syon_domain::EventKind`, o `None` si el mensaje no es una notificación (bienvenida, keepalive...) o
  si no debe publicarse.
- Traduce las ocho suscripciones: `channel.chat.message` (`Chat`), `channel.chat.message_delete`
  (`ChatMessageDeleted`), `channel.ban` (`UserBanned`), `channel.chat.notification` (`Subscription` y
  `GiftedSubs`), `channel.channel_points_custom_reward_redemption.add` (`RewardRedeemed`),
  `channel.follow` (`Followed`) y `stream.online` y `stream.offline` (`StreamStarted` y `StreamEnded`).
- Detalles de la traducción: una suscripción nueva cuenta un mes y una renovación sus meses
  acumulados; un regalo a la comunidad cuenta su total y las suscripciones sueltas dentro de él no se
  repiten (`None`); un regalo anónimo usa el nombre `Anónimo`; `stream.online` no trae título.
- Todo campo se lee de forma tolerante ([ADR-0013](../../docs/adr/0013-json-tolerante-en-adaptadores.md)).
  Un evento desconocido o con campos obligatorios ausentes llega como `EventKind::Unknown { name }`,
  con el nombre de la suscripción (y `:tipo_de_aviso` en los avisos de chat), sin su contenido.

### OAuth ([ADR-0016](../../docs/adr/0016-oauth-y-api-de-twitch.md))

- `OAuthClient` inicia sesión con el flujo de código de autorización y un callback local
  (`http://localhost:8081/auth/callback` por defecto), y renueva tokens. No guarda nada y no abre el
  navegador: entrega el enlace a quien llama (`on_url`).
- `TokenManager` deja lista una sesión: reutiliza las credenciales guardadas, las renueva si van a
  vencer (margen de 60 s) o inicia sesión de nuevo si no hay, faltan permisos o la renovación falla.
  Persiste solo por el puerto `CredentialStore`. `force_refresh` renueva a la fuerza ante un 401.
- Pide desde el primer inicio de sesión los permisos de lectura de las ocho suscripciones
  (`user:read:chat`, `channel:moderate`, `channel:read:redemptions`, `moderator:read:followers`) y el de
  escritura de chat (`user:write:chat`).
- Los errores (`OAuthError`) nunca incluyen tokens, el `client_secret` ni el contenido de una respuesta
  ilegible; `TwitchAppConfig` implementa `Debug` sin el secreto.

### Cliente de la API ([ADR-0016](../../docs/adr/0016-oauth-y-api-de-twitch.md))

- `TwitchApi` hace solo tres cosas, y no guarda tokens (cada llamada recibe el suyo, con las cabeceras
  `Authorization` y `Client-Id`):
  - `validate(token)` consulta `https://id.twitch.tv/oauth2/validate` y devuelve quién autorizó y sus
    permisos. El `user_id` es el id del canal que se escucha.
  - `create_subscription(token, petición, session_id)` crea una suscripción de EventSub por WebSocket y
    devuelve `cost`, `total_cost` y `max_total_cost` tal como los entrega Twitch (`None` si faltan).
  - `list_subscriptions(token, cursor)` lista una página de suscripciones con su estado y coste.
- Un 401 es `ApiError::Unauthorized`, para que quien llama renueve el token y reintente. Ningún error
  incluye el token ni el contenido de una respuesta ilegible.

## Qué no hace

- No lee `.env`, el entorno ni los ajustes: `TwitchAppConfig` llega por constructor. De dónde sale el
  `client_secret` lo decide el autor (pendiente 6 del diseño).
- No abre el navegador ni crea el runtime del engine.
- No envía mensajes ni modera (F3). No consulta otros canales: solo resuelve el del usuario que inició
  sesión.
- Los endpoints de `id.twitch.tv`, el formato de la respuesta y los permisos salen de la documentación de
  Twitch y no se han contrastado en vivo; el OAuth y la API se prueban con servidores locales falsos. El permiso
  `moderator:read:followers` está sin contrastar (ver el ADR).

## API pública

| Elemento | Para qué sirve |
|----------|----------------|
| `event_from_notification(&Value)` | Traduce una notificación de EventSub a un `EventKind` |
| `TwitchAppConfig::new(client_id, client_secret)` | Datos de la aplicación de Twitch, con callback y permisos por defecto |
| `OAuthClient::new(config)` | Cliente OAuth: `login(on_url)`, `refresh(&credenciales)` y `client_id()` |
| `TokenManager::start(oauth, almacén, on_url)` | Deja lista la sesión; devuelve su `Source` |
| `TokenManager::access_token()` / `force_refresh()` / `client_id()` | Token vigente, renovado si hace falta |
| `TwitchApi::new(client_id)` | Cliente de Helix: `validate`, `create_subscription` y `list_subscriptions` |
| `TokenInfo`, `SubscriptionRequest`, `CreatedSubscription`, `SubscriptionList` | Datos que entran y salen de la API |
| `OAuthError` / `ApiError` | Fallos tipados del inicio de sesión y de la API |
| `DEFAULT_REDIRECT_URI` / `DEFAULT_SCOPES` | Callback y permisos por defecto |

Dependencias: `syon-app`, `syon-domain`, `serde`, `serde_json`, `reqwest`, `base64`, `rand`, `log` y `tokio`.
