# Registro de decisiones de arquitectura (ADR)

Una decisión por archivo, escrita **antes** de implementarla. Los ADR no se
borran: si una decisión cambia, se crea otro ADR que la reemplaza y el
anterior pasa a `Reemplazada`.

Estados: `Propuesta` · `Aceptada` · `Reemplazada por ADR-NNNN` · `Descartada`.

| ADR | Título | Estado |
|-----|--------|--------|
| [0001](0001-nombre-syon.md) | Nombre del proyecto: SYON | Aceptada |
| [0002](0002-lenguaje-rust.md) | Lenguaje: Rust | Aceptada |
| [0003](0003-arquitectura-hexagonal-workspace.md) | Arquitectura hexagonal en un workspace de Cargo | Aceptada |
| [0004](0004-politica-de-dependencias.md) | Política de dependencias | Aceptada |
| [0005](0005-ui-gpui-directo.md) | Interfaz: GPUI directo desde Rust | Aceptada (a validar en F9) |
| [0006](0006-registro-de-diagnostico.md) | Registro de diagnóstico con `log`, `env_logger` y un archivo acotado | Aceptada |
| [0007](0007-bus-de-eventos.md) | Bus de eventos con un canal acotado por consumidor | Aceptada |
| [0008](0008-configuracion-toml.md) | Configuración en TOML, preferencias separadas del estado y formato propio del adaptador | Aceptada |
| [0009](0009-rutas-de-datos.md) | Rutas de datos con `etcetera`, resueltas solo en `syon-engine` | Reemplazada por ADR-0020 |
| [0010](0010-arranque-del-engine.md) | Arranque del engine y recuperación de ajustes inválidos | Aceptada |
| [0011](0011-runtime-asincrono-del-engine.md) | Runtime asíncrono propiedad del engine | Aceptada |
| [0012](0012-almacen-de-credenciales.md) | Almacén de credenciales en un archivo TOML con permisos restringidos | Aceptada |
| [0013](0013-json-tolerante-en-adaptadores.md) | JSON tolerante con `serde_json::Value` en los adaptadores de plataforma | Aceptada |
| [0014](0014-oauth-de-kick.md) | OAuth 2.1 de Kick con PKCE, callback local y cliente HTTP `reqwest` | Aceptada |
| [0015](0015-conexion-al-feed-de-kick.md) | Conexión al feed de Kick con `tokio-tungstenite` y un conector abortable | Aceptada |
| [0016](0016-oauth-y-api-de-twitch.md) | OAuth de Twitch con código de autorización, callback local y cliente mínimo de Helix | Aceptada |
| [0017](0017-conexion-a-eventsub-de-twitch.md) | Conexión a EventSub de Twitch por WebSocket con cambio de conexión sin pérdidas | Aceptada |
| [0018](0018-credenciales-de-aplicacion-incrustadas.md) | Credenciales de la aplicación incrustadas en el binario en tiempo de compilación | Aceptada |
| [0019](0019-guardado-con-esquema-mas-nuevo.md) | Un `save` no reescribe ajustes de un esquema más nuevo | Propuesta |
| [0020](0020-carpeta-personal-punto-syon.md) | Una sola carpeta `~/.SYON` para configuración, datos y caché | Aceptada |
| [0021](0021-motores-de-tts-como-procesos-externos.md) | Kokoro como único motor de TTS, como proceso externo detrás de un puerto | Propuesta |

Para crear uno nuevo: copiar [0000-plantilla.md](0000-plantilla.md), usar el
siguiente número libre y añadir la fila a esta tabla en el mismo commit.
