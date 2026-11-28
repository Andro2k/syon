# Diseño 004: chat unificado y envío de mensajes (primera parte de F3)

- Estado: Aceptado (2026-11-27); propuesto el 2026-11-27
- Fase: F3
- Fecha: 2026-11-27

## 1. Objetivo y alcance

Que SYON pueda **escribir** en el chat de Kick y de Twitch, a una plataforma o a varias a la vez, y que el
chat de ambas se vea como un solo flujo. Es la primera parte de F3; el [roadmap](../roadmap.md) la
completa con comandos, filtros y antispam, que van en otro diseño.

**Dentro:** el puerto de envío en `syon-app`, su implementación en los dos adaptadores, la API del
engine para enviar, y la entrada por consola del binario `syon`.

**Fuera:** comandos (también los de moderador), filtros y antispam; respuestas a un mensaje, anuncios,
susurros y moderación; roles y insignias en `ChatMessage`; el filtro por plataforma de la interfaz (F9).

**Leer ya está resuelto.** Los dos conectores publican `EventKind::Chat` en el bus con la plataforma de
origen, así que el chat unificado de lectura es el flujo del bus. Elegir qué plataformas se ven es cosa de
quien consume (la consola imprime todas; la interfaz de F9 filtrará). Este diseño no cambia la lectura.

## 2. Punto de partida

- El [diseño 002](002-plataformas-kick-twitch.md) dejó fuera el envío a propósito: «F3 lo diseña cuando
  haya un consumidor real» (sección 4), y pidió los permisos de escritura desde el primer inicio de
  sesión. Ya se piden: `chat:write` en Kick y `user:write:chat` en Twitch (`DEFAULT_SCOPES` de cada
  adaptador).
- `PlatformConnector` es solo de lectura y no espera a la red. No hay ningún cliente de la API pública de
  Kick en los crates; solo existe en el prototipo `prototypes/kick-console` (`KickApi::send_chat`,
  `KickApi::channel`).
- Twitch ya tiene `ApiClient` (Helix) con `validate` y `create_subscription`, y su `TokenManager` se
  comparte con el conector.
- Kick **arranca sin OAuth** ([diseño 003](003-arranque-de-plataformas-y-binario.md), decisión 3):
  `start_platforms` no abre ninguna sesión para Kick.

## 3. Qué ofrece cada plataforma

| | Twitch | Kick |
|---|--------|------|
| Petición | `POST https://api.twitch.tv/helix/chat/messages` | `POST /chat` de la API pública |
| Cuerpo | `broadcaster_id`, `sender_id`, `message`; opcional `reply_parent_message_id` | `broadcaster_user_id`, `content`, `type` (`"user"`); opcional `reply_to_message_id` |
| Cabeceras | `Authorization: Bearer` y `Client-Id` | `Authorization: Bearer` |
| Permiso | `user:write:chat` del usuario que escribe (`user:bot` solo aplica con token de aplicación) | `chat:write` |
| Límite de texto | 500 caracteres | Sin confirmar (se asume el mismo) |
| Respuesta | `message_id`, `is_sent` y `drop_reason` (`code` y `message`, por ejemplo `automod_held`) | `message_id` e `is_sent` |
| Ritmo | 20 mensajes cada 30 s si quien escribe no es el emisor, moderador ni VIP; el bucket de esta API es aparte | Sin confirmar |
| Id del canal | `sender_id` y `broadcaster_id` son el `user_id` de quien inició sesión (lo da `validate`) | `broadcaster_user_id` sale de `GET /channels?slug=` (permiso `channel:read`, que ya se pide) |

**Procedencia de los datos.** Los de Twitch salen de su documentación oficial (guía de envío y
recepción de mensajes, migración desde IRC y resumen de límites de chat), consultada el 2026-11-27. Los de
Kick salen del prototipo del repositorio y de clientes de terceros en `docs.rs`; **no se leyó la
documentación oficial de Kick**, así que el límite de texto, el ritmo y el significado de `type` quedan
por confirmar en vivo.

## 4. Puerto de envío en `syon-app`

Sin dependencias nuevas: `syon-app` sigue siendo solo `std`. Enviar espera a la red, así que a diferencia
de `PlatformConnector` el método es asíncrono, con un futuro de `std` en una caja para que el trait siga
siendo un objeto (`Box<dyn ChatSender>`).

```rust
/// Texto ya validado: sin espacios sobrantes, no vacío y de 500 caracteres como máximo.
pub struct ChatText(String);
impl ChatText { pub fn new(text: &str) -> Result<Self, SendError>; pub fn as_str(&self) -> &str; }

/// Lo que devuelve una plataforma al aceptar un mensaje.
pub struct SentMessage { pub id: Option<MessageId> }

pub enum SendError {
    Empty,                  // el texto queda vacío al recortarlo
    TooLong { max: usize }, // pasa de 500 caracteres
    NotRunning,             // la plataforma no está activada o no tiene envío disponible
    NotLoggedIn,            // no hay una sesión guardada: `syon login <plataforma>`
    Rejected(String),       // la plataforma lo descartó (automod, permisos, límite de ritmo...)
    Failed(String),         // red u otro fallo; sin tokens ni secretos
}

pub type SendFuture<'a> = Pin<Box<dyn Future<Output = Result<SentMessage, SendError>> + Send + 'a>>;

pub trait ChatSender: Send + Sync {
    fn platform(&self) -> Platform;
    fn send<'a>(&'a self, text: &'a ChatText) -> SendFuture<'a>;
}
```

`ChatText::new` es la única validación y vive en el puerto, para que las dos plataformas y el engine
apliquen la misma regla. Cuenta caracteres (`chars()`), no bytes.

## 5. Adaptadores

- **`syon-platform-twitch`:** `ApiClient::send_chat_message` y `TwitchChatSender`. Comparte el
  `TokenManager` del conector (la renovación del token ya está resuelta). El `user_id` se obtiene con
  `validate` en el primer envío y se guarda. Un `is_sent: false` se convierte en `Rejected` con el
  `message` del `drop_reason`.
- **`syon-platform-kick`:** cliente mínimo de la API pública (`GET /channels?slug=` y `POST /chat`),
  `KickChatSender`, y una forma de **recuperar la sesión sin abrir el navegador**: hoy
  `TokenManager::start` inicia sesión si no hay una guardada, y enviar no debe hacerlo. El
  `broadcaster_user_id` se resuelve por el slug del canal en el primer envío y se guarda; no se reutiliza
  `channel_id` de los ajustes, porque no consta que sea el id de usuario del emisor que pide `POST /chat`.
- Ninguno registra el texto del mensaje. Se registra plataforma, resultado e id.

## 6. API del engine

```rust
pub struct SendResult { pub platform: Platform, pub outcome: Result<SentMessage, SendError> }

impl Engine {
    /// Plataformas con envío disponible ahora.
    pub fn chat_platforms(&self) -> Vec<Platform>;
    /// Manejador clonable que se puede usar desde otro hilo.
    pub fn chat_handle(&self) -> ChatHandle;
}

#[derive(Clone)]
pub struct ChatHandle { /* senders + Handle del runtime */ }
impl ChatHandle {
    /// Envía `text` a cada plataforma de `platforms`, en paralelo, y devuelve un resultado por
    /// plataforma en el orden pedido (sin repetidas). Bloquea: se llama desde fuera del runtime.
    pub fn send(&self, platforms: &[Platform], text: &str) -> Vec<SendResult>;
}
```

- **Registro de senders**, como el de conectores. `start_platforms` registra el sender de Twitch junto con
  su conector, con el mismo `TokenManager`. Para Kick lo registra solo si hay una sesión guardada que se
  pueda recuperar sin navegador; si no, no hay sender y `send` devuelve `NotLoggedIn` (no `NotRunning`,
  porque la plataforma sí está activada).
- Enviar **nunca** abre el navegador ni pide un inicio de sesión: la sesión se deja lista con
  `syon login` o al arrancar Twitch.
- `ChatText::new` se aplica una vez, antes de tocar la red, así que un texto inválido falla igual en todas
  las plataformas.
- Sin reintentos automáticos y sin limitador local: un reintento puede duplicar un mensaje que sí salió, y
  el límite de ritmo lo informa la plataforma como `Rejected`.

## 7. El binario `syon`

`syon run` empieza a leer la entrada estándar en un hilo propio (sin sumar la función `io-std` de
`tokio`) y manda cada línea con un `ChatHandle`:

- Sin prefijo, la línea va a **todas** las plataformas con envío disponible (`chat_platforms`).
- Con prefijos `@kick` y/o `@twitch` al principio, solo a las indicadas (`@kick @twitch hola` equivale a
  sin prefijo si ambas están disponibles). Una línea vacía, o solo con prefijos, se ignora.
- Cada resultado se imprime: `<plataforma>: enviado.` o `<plataforma>: no se pudo enviar: <motivo>`, el
  fallo por `stderr`. Los mensajes enviados deben volver por el flujo de eventos y salir también como
  chat; en Kick es lo que hacía el prototipo, en Twitch hay que confirmarlo.
- Si la entrada estándar se cierra (tubería o sin terminal), `run` sigue funcionando hasta Ctrl+C.
- Un hilo bloqueado leyendo la entrada no impide salir: el proceso termina al volver de `run`.

## 8. Dependencias

Ninguna nueva. `reqwest` y `serde_json` ya los usan los adaptadores; `syon-app` no suma nada. Si se
prefiere un `async fn` en el trait en lugar del futuro en caja, dejaría de ser un objeto y habría que
decidirlo con un ADR; por eso se propone el futuro en caja.

## 9. Plan de parches

Uno por vez, cada uno verde por sí solo y con `docs/estado.md` al día.

1. `docs(design)`: este diseño.
2. `feat(app)`: `ChatText`, `SentMessage`, `SendError`, `ChatSender` y sus pruebas.
3. `feat(twitch)`: `send_chat_message` y `TwitchChatSender`, con pruebas contra un servidor local.
4. `feat(kick)`: recuperar la sesión sin navegador, cliente mínimo de la API pública y `KickChatSender`.
5. `feat(engine)`: registro de senders, `chat_platforms`, `chat_handle` y `ChatHandle::send`, cableados en
   `start_platforms`.
6. `feat(syon)`: la entrada por consola con los prefijos `@kick` y `@twitch`.

## 10. Criterio de salida de esta parte

| Criterio | Prueba |
|----------|--------|
| `ChatText` recorta, rechaza lo vacío y lo de más de 500 caracteres contando caracteres | pruebas de `syon-app` |
| Twitch envía con `broadcaster_id`, `sender_id` y `Client-Id` correctos y traduce `drop_reason` | servidor local en `syon-platform-twitch` |
| Kick resuelve el canal por slug, envía y traduce errores de la API | servidor local en `syon-platform-kick` |
| Sin sesión guardada, Kick devuelve `NotLoggedIn` y no abre el navegador | prueba del engine |
| `ChatHandle::send` a varias plataformas devuelve un resultado por plataforma y un fallo no afecta al otro | prueba del engine con senders falsos |
| Ningún resultado, error ni registro contiene un token, el secreto de la aplicación ni el texto enviado | pruebas de los adaptadores y del engine |
| La entrada por consola interpreta los prefijos y ignora las líneas vacías | pruebas de `syon` |
| Envío real a Kick y a Twitch con la aplicación del autor | **Pendiente, a cargo del autor** |

## 11. Riesgos y pendientes

- **Kick sin documentación oficial leída.** El límite de texto, el ritmo y `type` salen del prototipo y de
  clientes de terceros. Se confirman en vivo.
- **Sesión de Kick sin el permiso `chat:write`.** El gestor de tokens de Kick no comprueba los permisos al
  recuperar una sesión (la prueba `engine_login.rs` restaura una sin permisos), así que una guardada antes
  de pedirlos fallaría al enviar con el error de la plataforma. `syon login kick` la vuelve a crear; un
  cambio del gestor para comprobarlos queda como mejora.
- **Eco en Twitch sin confirmar.** No consta que los mensajes propios vuelvan por `channel.chat.message`.
  Si no volvieran, la consola solo mostraría la confirmación de envío.
- **Bucles cuando lleguen los comandos.** Si un comando responde por el chat y la respuesta vuelve como
  evento, puede dispararse a sí mismo. Es una restricción del diseño de comandos: ignorar los mensajes
  propios.
- **Un solo canal por plataforma.** Twitch envía al canal de quien inició sesión y Kick al del ajuste
  `channel`; escribir en otros canales no está previsto.
- **El texto se envía tal cual.** Una línea que empiece por `/` se manda como texto; los comandos de
  barra de cada plataforma no se interpretan.

## 12. Decisiones a aceptar

1. Puerto asíncrono con futuro en caja de `std`, sin dependencias nuevas (sección 4).
2. Enviar nunca abre el navegador: sin sesión, `NotLoggedIn` y `syon login` (sección 6).
3. Sin reintentos ni limitador local; el rechazo de la plataforma se muestra (sección 6).
4. Límite común de 500 caracteres, contados como caracteres (sección 4).
5. Fuera de esta parte: respuestas, anuncios, susurros y moderación (sección 1).
6. El texto de los mensajes no se registra (sección 5).
7. Entrada por consola con los prefijos `@kick` y `@twitch`, y todas las plataformas por defecto
   (sección 7).
8. Leer no cambia: el chat unificado es el flujo del bus y el filtro por plataforma es del consumidor
   (sección 1).
