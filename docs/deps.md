# Registro de dependencias

Cada dependencia directa se anota aquí con su motivo, una alternativa y su
riesgo (ADR-0004). Se actualiza en el mismo parche que añade o quita la
dependencia.

## Workspace (raíz)

`xtask` y `syon-domain` usan solo la biblioteca estándar y `syon-app` solo depende de
`syon-domain`. Dependencias directas de `syon-engine` y de `syon-storage-toml`:

| Dependencia | Para qué | Alternativa | Riesgo |
|-------------|----------|-------------|--------|
| tokio 1 (solo `sync`) | Canales acotados del bus de eventos ([ADR-0007](adr/0007-bus-de-eventos.md)) | flume, async-channel | Bajo |
| log 0.4 | Fachada de registro ([ADR-0006](adr/0006-registro-de-diagnostico.md)) | tracing con su puente a `log` | Pre-1.0, pero estable de hecho |
| toml 1 (`std`, `serde`, `parse`, `display`) | Leer y escribir `settings.toml` en `syon-storage-toml` ([ADR-0008](adr/0008-configuracion-toml.md)) | toml_edit, si hace falta conservar comentarios | Bajo; trae `winnow` y los crates `toml_*` y `serde_spanned` (MIT o Apache-2.0) |
| serde 1 (`derive`) | Serializar el formato del archivo; solo `syon-storage-toml` lo usa ([ADR-0008](adr/0008-configuracion-toml.md)) | Escribir el TOML a mano | Bajo; la derivación suma `syn`, `quote` y `proc-macro2` al tiempo de compilación |

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
