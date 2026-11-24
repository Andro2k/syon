# WT-0.1.0_01: prototipo de consola con eventos en tiempo real y OAuth

## Contexto

`chat-server` solo imprimía mensajes de chat de un canal de Kick, sin reconexión ni credenciales. El objetivo es un prototipo en Rust, solo de consola y sin tocar MiniKick, que escuche el chat y los eventos del canal por WebSocket y use las credenciales OAuth de la API de Kick tomadas de un `.env`. Servirá de referencia para un futuro motor en MiniKick.

## Cambios por capa

**Presentación** (`console.rs`, `input.rs`): salida con hora, etiqueta y color por tipo de evento; `NO_COLOR` desactiva los colores; `--raw` imprime el JSON original; los emotes `[emote:ID:nombre]` se muestran como `:nombre:`. Con sesión OAuth, cada línea escrita se envía al chat.

**Lógica** (`events.rs`, `oauth.rs`): interpretación tolerante de eventos (chat, borrados, bans, subs, regalos, recompensas, seguidores, directo en vivo/terminado) con `Other` para lo desconocido. OAuth 2.1 con PKCE S256, `state` aleatorio, callback local validado, renovación con margen de 60 s y nuevo login si faltan scopes.

**Acceso a datos y red** (`realtime.rs`, `api.rs`, `config.rs`): WebSocket Centrifugo con cuatro feeds, backoff de 1 a 30 s, reconexión con client id nuevo, espera mínima de 5 s ante cierres terminales (3500-3999, 4500-4999) y vigilancia de silencio. Cliente de la API pública (`GET /users`, `GET /channels`, `POST /chat`) con un reintento tras renovar el token ante un 401. Configuración desde `.env` y entorno.

## Parches

1. `feat(realtime)`: módulos events, console y realtime; reconexión y feeds de eventos.
2. `feat(config)`: carga de `.env` e ids de canal manuales (`KICK_CHANNEL_ID`, `KICK_CHATROOM_ID`).
3. `feat(oauth)`: credenciales OAuth desde `.env`, login PKCE, tokens persistentes y `--login`.
4. `feat(api)`: cliente de la API pública y envío de chat desde la consola.
5. `docs`: README y este walkthrough.

## Archivos afectados

- Nuevos: `src/{events,console,realtime,config,oauth,api,input}.rs`, `.env.example`, `docs/walkthroughs/WT-0.1.0_01.md`.
- Modificados: `src/main.rs`, `Cargo.toml`, `.gitignore`, `README.md`.

## Verificación ejecutada

- `cargo build`, `cargo clippy --all-targets` (0 avisos) y `cargo fmt`.
- `cargo test`: 35 pruebas aprobadas, 0 fallidas.
- Ejecución del binario sin argumentos, con `--login` sin `.env` y con un canal sin acceso a kick.com: los mensajes de error son claros y no exponen credenciales.
- Pendiente, porque el entorno de desarrollo no llega a `kick.com`, `id.kick.com` ni `api.kick.com`: primer arranque real con `cargo run -- <canal> --raw` y `cargo run -- <canal> --login`. Los nombres de campos de los eventos no se han contrastado en vivo.

## Riesgos conocidos

- El feed y los endpoints de `kick.com` no son oficiales y pueden cambiar o bloquear clientes (403); `KICK_CHANNEL_ID` y `KICK_CHATROOM_ID` permiten saltarse la consulta bloqueada, pero no el handshake del WebSocket.
- Los scopes pedidos deben estar habilitados en la app de Kick, o la autorización falla con `invalid_scope`.
- Los eventos oficiales de Kick se entregan por webhooks (URL HTTPS pública); no se implementan en este prototipo.
