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
| [0006](0006-registro-de-diagnostico.md) | Registro de diagnóstico con `log`, `env_logger` y un archivo acotado | Aceptada (se implementa en F1, parche 6) |
| [0007](0007-bus-de-eventos.md) | Bus de eventos con un canal acotado por consumidor | Aceptada |

Para crear uno nuevo: copiar [0000-plantilla.md](0000-plantilla.md), usar el
siguiente número libre y añadir la fila a esta tabla en el mismo commit.
