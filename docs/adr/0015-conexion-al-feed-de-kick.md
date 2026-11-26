# ADR-0015: Conexión al feed de Kick con `tokio-tungstenite` y un conector abortable

- Estado: Aceptada
- Fecha: 2026-11-26

## Contexto

El [diseño 002](../designs/002-plataformas-kick-twitch.md) (sección 6 y decisión 1) elige recibir los
eventos de Kick por el feed en tiempo real no oficial (Centrifugo por WebSocket), el del prototipo
(`prototypes/kick-console/src/realtime.rs`), porque los webhooks oficiales exigen una URL pública.
Hay que decidir cómo se integra ese feed con el engine ([ADR-0011](0011-runtime-asincrono-del-engine.md))
y qué dependencias añade.

## Decisión

1. **`KickConnector` implementa `PlatformConnector`.** `KickConnector::start(handle, canal, sumidero)`
   lanza una tarea en el runtime del engine y vuelve enseguida; el estado empieza en `Connecting` y los
   fallos de red se reintentan solos. Publica `Event { platform: Kick, .. }` en el `EventSink` que recibe.
2. **El feed no usa el OAuth.** Se conecta como invitado con un token temporal que pide a Kick; el
   inicio de sesión sirve para la API pública (F3).
3. **Reconexión:** espera creciente con `Backoff` (1 s, 2 s... hasta 30 s), que vuelve a 1 s al
   suscribirse al primer feed. Un cierre terminal de Centrifugo (códigos 3500-3999 y 4500-4999) espera
   al menos 5 s. Si el servidor calla más de dos intervalos de ping más 5 s, la conexión se da por
   muerta. Los ids del canal se consultan una vez y se conservan; si se indican `id` y `chatroom_id` no
   se consulta (Kick a veces bloquea esa petición con un 403).
4. **Estado:** `Connecting` al arrancar, `Connected` cuando el servidor acepta la conexión,
   `Reconnecting { attempt }` mientras espera y `Disconnected` tras `stop`.
5. **Parar es abortar la tarea.** `stop` (y soltar el conector) marca el estado `Disconnected`, impide
   que la tarea lo cambie y la aborta; es idempotente. No hay una señal de parada aparte.
6. **Lógica pura aparte de la red:** `frames.rs` clasifica los mensajes y lee las negociaciones sin
   hacer red; el conector solo une eso con el socket.
7. **Dependencias nuevas** (versiones en `[workspace.dependencies]`):
   - `tokio-tungstenite` 0.26 con `rustls-tls-webpki-roots`: el WebSocket.
   - `futures-util` 0.3 (`std` y `sink`): `StreamExt::next` y `SinkExt::send` sobre ese WebSocket.
8. **Un solo proveedor criptográfico de `rustls`** (`ring`), el que ya trae `reqwest`. Una prueba abre
   un `wss://` para detectar que `aws-lc-rs` se haya colado: en ese caso `rustls` entraría en pánico al
   crear la configuración TLS.
9. **El identificador de cliente se genera con `rand`** con forma de UUID v4, sin añadir el crate `uuid`.

## Alternativas consideradas

| Alternativa | Por qué no |
|-------------|-----------|
| Webhooks oficiales de Kick | Exigen una URL HTTPS pública, que no encaja en una aplicación de escritorio sin túnel |
| `async-tungstenite` o `fastwebsockets` | `tokio-tungstenite` es el que usa el prototipo y el que Twitch EventSub también necesitará |
| Señal de parada con `tokio::select!` | Obliga a activar `macros` en el código de producción y a repartir el punto de espera; abortar basta |
| El crate `uuid` solo para el identificador | Una dependencia más por 16 bytes con dos máscaras |
| Un `Notify` o `watch` para el estado | `ConnectionStatus` es `Copy` y se lee desde un método síncrono: un `Mutex` es suficiente |

## Consecuencias

- Abortar cierra el socket sin enviar un cierre WebSocket: para un lector de solo lectura no importa.
- El feed es no oficial: puede cambiar, pedir más cabeceras o bloquear clientes. Los nombres de campo y
  los endpoints no se han contrastado en vivo; las pruebas usan un servidor HTTP y uno WebSocket
  Centrifugo locales y falsos.
- Un evento que no se entiende se descarta o llega como `Unknown`; el registro de nivel `debug` dice
  cuál.
- Twitch EventSub reutilizará `tokio-tungstenite` y `futures-util`.
