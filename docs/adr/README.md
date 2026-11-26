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
| [0009](0009-rutas-de-datos.md) | Rutas de datos con `etcetera`, resueltas solo en `syon-engine` | Aceptada |
| [0010](0010-arranque-del-engine.md) | Arranque del engine y recuperación de ajustes inválidos | Aceptada |
| [0011](0011-runtime-asincrono-del-engine.md) | Runtime asíncrono propiedad del engine | Aceptada |
| [0012](0012-almacen-de-credenciales.md) | Almacén de credenciales en un archivo TOML con permisos restringidos | Aceptada |
| [0013](0013-json-tolerante-en-adaptadores.md) | JSON tolerante con `serde_json::Value` en los adaptadores de plataforma | Aceptada |
| [0014](0014-oauth-de-kick.md) | OAuth 2.1 de Kick con PKCE, callback local y cliente HTTP `reqwest` | Aceptada |

Para crear uno nuevo: copiar [0000-plantilla.md](0000-plantilla.md), usar el
siguiente número libre y añadir la fila a esta tabla en el mismo commit.
