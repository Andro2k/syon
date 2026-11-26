# Diseño 002: plataformas de F2 (Kick y Twitch)

- Estado: Aceptado (2026-11-26)
- Fase: F2
- Fecha: 2026-11-26
- Revisión: 2026-11-26, el trabajo de Kick de la sección 12 se divide en tres parches (eventos, OAuth y feed)
  porque juntos serían demasiado grandes para revisarlos como un cambio atómico.

## 1. Objetivo y alcance

Hacer que Kick y Twitch entreguen eventos al engine por el puerto `EventSink`, sin que el
resto de SYON conozca nada de ninguna de las dos:

1. Un adaptador por plataforma (`syon-platform-kick`, `syon-platform-twitch`) que traduce lo
   que recibe a `syon_domain::Event`.
2. Autenticación OAuth de cada plataforma y almacén de credenciales detrás de un puerto.
3. Reconexión con espera creciente y un estado de conexión que el engine puede consultar.
4. Un presupuesto de suscripciones por plataforma, documentado en
   `docs/architecture/suscripciones.md` (criterio de salida de F2 del [roadmap](../roadmap.md)).

Fuera de alcance: enviar mensajes al chat y moderar (F3, que es quien los necesita), el chat
unificado y los comandos (F3), TTS, overlays y cualquier interfaz. El binario `syon` solo se crea
para llamar a `Engine::start` y mantener las plataformas conectadas.

## 2. Punto de partida

- `prototypes/kick-console` ya resuelve Kick de extremo a extremo (WT-0.1.0_01), pero mezcla JSON,
  red, OAuth y consola, y sus nombres de campo no están contrastados en vivo.
- Los tipos de F1 ya existen: `Event`, `EventKind`, `Platform::{Kick, Twitch}` y el puerto
  `EventSink`. El dominio no lee el reloj: el adaptador pasa `received_at`.
- `syon-app` solo depende de `syon-domain`, no usa `async` y la dirección de dependencias la
  hace cumplir `cargo xtask arch-check`; los nombres `syon-platform-*` ya se reconocen como
  adaptadores.
- El workspace solo tiene `tokio` con la función `sync`. F2 es la primera fase con red.

## 3. Crates y dependencias

```text
syon-engine → syon-platform-kick   → syon-app → syon-domain
            → syon-platform-twitch → syon-app → syon-domain
            → syon-storage-*       → syon-app → syon-domain
```

- `syon-platform-kick` y `syon-platform-twitch` dependen solo de `syon-app` y `syon-domain`. Ninguno
  depende del otro ni del engine. El código que ambos necesitan y es lógica pura (la espera
  creciente) vive en `syon-app`, no en un tercer crate.
- Un adaptador de credenciales vive en un crate `syon-storage-*` (ver sección 5).
- Solo `syon-engine` nombra los adaptadores concretos y los arranca.

## 4. Puertos nuevos (en `syon-app`)

```rust
/// Una conexión de plataforma que el engine arranca y vigila.
pub trait PlatformConnector: Send + Sync {
    fn platform(&self) -> Platform;
    fn status(&self) -> ConnectionStatus;
    fn stop(&self);
}

pub enum ConnectionStatus { Disconnected, Connecting, Connected, Reconnecting { attempt: u32 } }
```

- Son métodos síncronos a propósito: el trait es compatible con `dyn`, `syon-app` no necesita
  runtime y cada adaptador lanza sus propias tareas en el runtime que le pasa el engine.
- Hay dos implementaciones reales desde el primer día, lo que justifica el trait (regla de
  abstracción de [AGENTS.md](../../AGENTS.md)).
- `Backoff` (1 s, 2 s, 4 s... hasta 30 s, con `reset`) se mueve del prototipo a `syon-app` tal cual,
  porque es pura y la usan los dos adaptadores.
- `CredentialStore` (`load`/`save`/`clear` por plataforma) con un error tipado propio, como
  `SettingsStore`. Los tipos que contienen un token implementan `Debug` a mano y no se imprimen
  nunca (regla de secretos).

No se añade un puerto de envío de chat: F3 lo diseña cuando haya un consumidor real.

## 5. Credenciales

- Los tokens no van en `settings.toml` ni en el repositorio. Se guardan en un archivo propio
  dentro de la carpeta de datos que ya resuelve `AppPaths` (ADR-0009), con permisos restringidos al
  usuario en Unix; en Windows se confía en la carpeta del perfil del usuario.
- El adaptador es `syon-storage-credentials` (formato JSON o TOML, escritura atómica como el almacén
  de ajustes). Su ADR cubre el formato y las alternativas, entre ellas el llavero del sistema
  (decisión 2).
- El `client_secret` de Kick es un secreto de la aplicación, no de la persona: no va en `settings.toml`
  (que se edita a mano y se comparte al reportar un problema) ni en el archivo de tokens. De dónde
  viene queda por decidir (sección 14, pendiente 6).

## 6. Kick

Dos caminos:

| Camino | Qué da | Coste |
|--------|--------|-------|
| Feed en tiempo real no oficial (Centrifugo por WebSocket), el del prototipo | Chat y eventos sin infraestructura pública | Puede cambiar o bloquear clientes (403) |
| Eventos oficiales por webhook | Contrato estable y firmado | Exige una URL HTTPS pública: no encaja en una aplicación de escritorio sin túnel |

**Propuesta:** feed no oficial para recibir; API pública y OAuth 2.1 con PKCE (oficiales) para
autenticar, como en el prototipo. Los webhooks quedan como alternativa futura si el feed
deja de funcionar. Se migra por módulos del prototipo:

| Prototipo | Destino |
|-----------|---------|
| `events.rs` (interpretación tolerante) | `syon-platform-kick`, traducción a `EventKind` |
| `realtime.rs` (WebSocket y reconexión) | `syon-platform-kick`; `Backoff` va a `syon-app` |
| `oauth.rs`, `api.rs` | `syon-platform-kick`; tokens por `CredentialStore` |
| `console.rs`, `input.rs`, `config.rs` | no se migran: presentación y `.env` |

## 7. Twitch

EventSub por WebSocket con token de usuario (OAuth, flujo por determinar, decisión 3). No
requiere URL pública. Eventos previstos y su traducción:

| Suscripción de Twitch | `EventKind` |
|-----------------------|-------------|
| `channel.chat.message` | `Chat` |
| `channel.chat.message_delete` | `ChatMessageDeleted` |
| `channel.ban` | `UserBanned` |
| `channel.chat.notification` (suscripciones y regalos) | `Subscription`, `GiftedSubs` |
| `channel.channel_points_custom_reward_redemption.add` | `RewardRedeemed` |
| `channel.follow` (v2) | `Followed` (nuevo, ver sección 8) |
| `stream.online` / `stream.offline` | `StreamStarted` / `StreamEnded` |

Comportamiento de la conexión: recibir el mensaje de bienvenida, crear las suscripciones con el
`session_id` por la API REST, atender los keepalive y, ante un mensaje `session_reconnect`, abrir la
conexión nueva antes de cerrar la vieja.

## 8. Cambio en el dominio

Twitch notifica cada seguidor nuevo; Kick, en el feed, el total. `FollowersUpdated { count }` no
representa lo primero. Propuesta: añadir `EventKind::Followed { user: User }` (aditivo; ningún
consumidor existente se rompe) y mantener `FollowersUpdated`. Cada adaptador emite el que su
plataforma entrega.

## 9. Presupuesto de suscripciones

| Plataforma | Conexiones | Suscripciones | Límite conocido |
|------------|-----------|---------------|-----------------|
| Kick (feed no oficial) | 1 WebSocket | 4 feeds por canal (`chatrooms.{id}.v2`, `chatroom_{id}`, `channel.{id}`, `channel_{id}`) | Sin límite documentado |
| Twitch (EventSub WebSocket) | 1 | 8 por canal (sección 7) | 300 activas por conexión; 3 conexiones por usuario y cliente; `max_total_cost` de 10 con token de usuario |

- Con un solo canal propio se usa una fracción de cada límite. El presupuesto es lo que
  `docs/architecture/suscripciones.md` documenta, con la cuenta por canal para cuando F3 permita
  varios.
- El coste de cada suscripción de Twitch depende de que el usuario haya autorizado la aplicación con
  el permiso (scope) correspondiente; las muestras de la documentación traen `cost: 0` en esos
  casos. Esto **debe confirmarse en vivo** leyendo `cost` y `max_total_cost` de la respuesta al
  crear cada una, y registrar el total al arrancar.
- Los datos de Kick salen del prototipo y de bibliotecas de terceros, no de documentación
  oficial contrastada.

## 10. Runtime asíncrono

El engine crea un runtime de `tokio` y pasa su `Handle` a cada adaptador; los adaptadores no
crean runtimes propios. `tokio` en el workspace pasa de `sync` a añadir `rt-multi-thread`, `net`,
`time` y `macros` (ADR). El `EventSink::publish` sigue sin bloquear.

## 11. Dependencias nuevas (cada una con ADR y fila en `docs/deps.md`)

`reqwest` (HTTP, rustls), `tokio-tungstenite` (WebSocket), `serde_json`, `futures-util`, `uuid`, `sha2`,
`base64` y `rand` (PKCE), todas ya probadas en el prototipo. Se fijan al migrar y se afinan las
funciones de `tokio` (el prototipo usa `full`). `dotenvy`, `chrono` y `anyhow` no se migran.

## 12. Plan de parches de implementación

1. `docs(design)`: este diseño, su fila en el índice y `docs/estado.md`.
2. `feat(domain)`: `EventKind::Followed`.
3. `feat(app)`: `PlatformConnector`, `ConnectionStatus`, `Backoff` y `CredentialStore`.
4. `feat(engine)`: runtime de `tokio` y registro de conectores, con su ADR.
5. `feat(storage)`: `syon-storage-credentials` y su ADR.
6. `feat(kick)`: crate con la traducción de eventos, `serde_json` y su ADR.
7. `feat(kick)`: OAuth 2.1 con PKCE y renovación de tokens por `CredentialStore`.
8. `feat(kick)`: feed en tiempo real con reconexión y `PlatformConnector`.
9. `feat(twitch)`: OAuth y cliente de la API.
10. `feat(twitch)`: EventSub por WebSocket.
11. `feat(syon)`: binario que llama a `Engine::start` y decide cómo avisa un `StartError`.
12. `docs`: `docs/architecture/suscripciones.md`, tabla de paridad, `docs/deps.md` y el walkthrough.

## 13. Criterio de salida de F2

- Kick y Twitch publican en el bus los eventos de la sección 7 y el feed de Kick, con pruebas por
  plataforma sobre cargas de ejemplo (sin red).
- Una caída de red hace reconectar con espera creciente; `status()` refleja cada estado.
- Ningún token aparece en el log ni en un `Debug`.
- `docs/architecture/suscripciones.md` documenta las suscripciones de cada plataforma.
- Contraste en vivo, pendiente del autor: nombres de campo de Kick y `cost` real de Twitch.
- Todo en verde en CI, incluidos `cargo deny` y `arch-check`.

## 14. Decisiones

El autor pidió aplicar la fase el 2026-11-26 sin objetar el diseño; se toman las propuestas de este
documento como decididas y se revisan si algo cambia:

1. **Kick:** feed no oficial para recibir; OAuth y API públicas para autenticar (sección 6). Los
   webhooks quedan como alternativa futura.
2. **Tokens:** archivo propio en la carpeta de datos con permisos restringidos en Unix (sección 5).
   El llavero del sistema se reconsidera si hace falta.
3. **OAuth de Twitch:** flujo de código con callback local, igual que Kick, para compartir el
   mismo patrón y no sumar un segundo flujo al código.
4. **Permisos de escritura:** se piden desde el primer inicio de sesión para no obligar a
   autorizar de nuevo en F3.
5. **Seguidores:** `Followed` junto a `FollowersUpdated` (sección 8).

**Pendiente, no cubierto por la aceptación:**

6. **Dónde vive el `client_secret` de Kick:** variable de entorno, como hacía el prototipo con `.env`
   (sin migrar `dotenvy`), o una entrada de aplicación en el almacén de credenciales. `Settings` no
   tiene ese campo y no debe tenerlo (el diseño 001 ya señala como defecto del prototipo que lo
   mezclara con los ajustes). Una aplicación de escritorio distribuida no puede guardar un secreto de
   aplicación de forma confidencial; si Kick lo exige con PKCE, hay que decidir qué se acepta. El OAuth ya
   recibe el valor en `KickAppConfig::new(client_id, client_secret)` y el ADR-0014 deja que lo decida el
   engine o el binario: debe resolverse antes de conectar esa configuración.

## 15. Riesgos

- El feed y los endpoints no oficiales de Kick pueden cambiar o bloquear clientes.
- Los límites y costes de Twitch pueden cambiar; la fuente de verdad es la respuesta de la API.
- Dos autenticaciones distintas aumentan la superficie de secretos; mitigación: un único almacén
  y `Debug` a mano en todos los tipos con tokens.
- Sin acceso a las plataformas desde el entorno de desarrollo, las pruebas usan cargas de ejemplo y la
  verificación en vivo queda para el autor.

## 16. Referencias

- [Diseño 001](001-nucleo-dominio-eventos.md), [ADR-0003](../adr/0003-arquitectura-hexagonal-workspace.md)
  y [WT-0.1.0_01](../walkthroughs/WT-0.1.0_01.md).
- Documentación de EventSub de Twitch (`dev.twitch.tv/docs/eventsub`) y API pública de Kick.
