# Registro de dependencias

Cada dependencia directa se anota aquí con su motivo, una alternativa y su
riesgo (ADR-0004). Se actualiza en el mismo parche que añade o quita la
dependencia.

## Workspace (raíz)

`xtask` y `syon-domain` usan solo la biblioteca estándar y `syon-app` solo depende de
`syon-domain`. `syon-engine` depende también de los adaptadores `syon-platform-kick` (arranca su conector e inicia sesión) y `syon-storage-credentials` (guarda los tokens); son crates del workspace y no suman dependencias externas. Dependencias directas de `syon-engine` y de los adaptadores `syon-storage-toml`,
`syon-storage-credentials` y `syon-platform-kick` (este último reutiliza `toml` y `serde`, [ADR-0012](adr/0012-almacen-de-credenciales.md)):

| Dependencia | Para qué | Alternativa | Riesgo |
|-------------|----------|-------------|--------|
| tokio 1 (`sync`; `syon-engine` añade `rt-multi-thread` y `time`) | Canales acotados del bus ([ADR-0007](adr/0007-bus-de-eventos.md)) y runtime asíncrono del engine ([ADR-0011](adr/0011-runtime-asincrono-del-engine.md)) | flume, async-channel; smol | Bajo; las funciones nuevas no suman crates al `Cargo.lock` |
| log 0.4 | Fachada de registro ([ADR-0006](adr/0006-registro-de-diagnostico.md)) | tracing con su puente a `log` | Pre-1.0, pero estable de hecho |
| env_logger 0.11 (funciones por defecto) | Filtros por módulo, `SYON_LOG` y hora de las líneas, en `syon-engine` ([ADR-0006](adr/0006-registro-de-diagnostico.md)) | tracing con `tracing-subscriber` | Pre-1.0; con las funciones por defecto trae `jiff` 1.x, `regex`, `anstream` y `env_filter` (MIT o Apache-2.0, aceptadas por `deny.toml`) |
| etcetera 0.11 | Carpetas de configuración, datos y caché del sistema, solo en `syon-engine` ([ADR-0009](adr/0009-rutas-de-datos.md)) | dirs (arrastra `option-ext`, MPL-2.0) | Pre-1.0; en Linux solo suma `cfg-if`, en Windows `windows-sys` y `windows-link` (MIT o Apache-2.0) |
| toml 1 (`std`, `serde`, `parse`, `display`) | Leer y escribir `settings.toml` en `syon-storage-toml` ([ADR-0008](adr/0008-configuracion-toml.md)) | toml_edit, si hace falta conservar comentarios | Bajo; trae `winnow` y los crates `toml_*` y `serde_spanned` (MIT o Apache-2.0) |
| serde 1 (`derive`) | Serializar el formato del archivo; solo `syon-storage-toml` lo usa ([ADR-0008](adr/0008-configuracion-toml.md)) | Escribir el TOML a mano | Bajo; la derivación suma `syn`, `quote` y `proc-macro2` al tiempo de compilación |
| serde_json 1 | Lectura tolerante de los eventos de las plataformas, en los adaptadores `syon-platform-*` ([ADR-0013](adr/0013-json-tolerante-en-adaptadores.md)) | Structs con `derive(Deserialize)`; simd-json | Bajo; trae `itoa`, `ryu` y `memchr` (MIT o Apache-2.0) |
| reqwest 0.12 (`json`, `rustls-tls`) | Cliente HTTP del OAuth de Kick y, después, de las APIs de las plataformas, en `syon-platform-*` ([ADR-0014](adr/0014-oauth-de-kick.md)) | ureq, hyper | Pre-1.0: actualizar con parche propio; trae `hyper`, `rustls` y `ring` |
| sha2 0.10 | Desafío PKCE S256 en `syon-platform-kick` ([ADR-0014](adr/0014-oauth-de-kick.md)) | — | Pre-1.0 |
| base64 0.22 | Codificación URL-safe sin relleno del PKCE y del `state` | — | Pre-1.0 |
| rand 0.9 | Bytes aleatorios del `verifier` y del `state` | — | Pre-1.0 |
| tokio-tungstenite 0.26 (`rustls-tls-webpki-roots`) | WebSocket del feed de Kick y, después, de EventSub de Twitch ([ADR-0015](adr/0015-conexion-al-feed-de-kick.md)) | async-tungstenite, fastwebsockets | Pre-1.0; comparte `rustls` con `reqwest` (solo `ring`) |
| futures-util 0.3 (`std`, `sink`) | `StreamExt` y `SinkExt` sobre el WebSocket | — | Pre-1.0 |

## prototypes/kick-console

| Dependencia | Para qué | Alternativa | Riesgo |
|-------------|----------|-------------|--------|
| anyhow 1 | Errores con contexto en el binario | thiserror, para errores tipados en librerías | Bajo |
| tokio 1 (`full`) | Runtime asíncrono, canales y temporizadores | smol | Bajo; `full` activa más funciones de las necesarias, afinar al migrar |
| reqwest 0.12 (`json`, `rustls-tls`) | Cliente HTTP de la API y del OAuth | ureq, hyper | Pre-1.0: actualizar con parche propio |
| serde 1 (`derive`) | Estructuras serializables | — | Bajo |
| serde_json 1 | JSON de eventos, API y tokens | — | Bajo |
| tokio-tungstenite 0.26 (`rustls-tls-webpki-roots`) | WebSocket del feed en tiempo real | fastwebsockets | Pre-1.0 |
| futures-util 0.3 | `SinkExt` y `StreamExt` sobre el socket | — | Pre-1.0 |
| uuid 1 (`v4`) | Identificadores de cliente | — | Bajo |
| chrono 0.4 (`clock`) | Hora de la salida por consola | jiff, time | RUSTSEC-2026-0543: sin mantenimiento; ignorado en `deny.toml` hasta migrar |
| dotenvy 0.15 | Carga del `.env` | — | Pre-1.0 |
| sha2 0.10 | Desafío PKCE S256 | — | Pre-1.0 |
| base64 0.22 | Codificación URL-safe del PKCE | — | Pre-1.0 |
| rand 0.9 | `state` y verificador aleatorios | — | Pre-1.0 |

## Auditoría

```text
cargo deny --manifest-path Cargo.toml check
cargo deny --manifest-path prototypes/kick-console/Cargo.toml check
cargo machete .
```

`cargo deny` revisa avisos de seguridad, licencias, versiones duplicadas y
fuentes (`deny.toml`); `cargo machete` detecta dependencias sin uso. Ambas
corren en CI.
