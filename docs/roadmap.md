# Roadmap

Cada fase se hace como rebanada vertical y sigue el mismo ciclo: documento de
diseño en `docs/designs/NNN-*.md`, parches atómicos y walkthrough de cierre.
Una fase se cierra solo cuando se cumple su criterio de salida.

| Fase | Contenido | Criterio de salida |
|------|-----------|--------------------|
| F0 | Base del repo: workspace, toolchain, rustfmt/clippy, `deny.toml`, `docs/deps.md`, test de arquitectura, CI, traslado del prototipo | `cargo build`, `clippy`, `fmt` y `test` en verde en CI; `docs/` completo |
| F1 | Núcleo: tipos de dominio, bus de eventos, configuración y logging | Dominio sin dependencias de I/O y con pruebas |
| F2 | Plataformas: migrar Kick desde el prototipo y añadir Twitch; presupuesto de suscripciones por plataforma | Ambas plataformas entregan eventos al engine; `docs/architecture/` documenta cuántas suscripciones abre cada una |
| F3 | Chat unificado (1, 2 o todas las plataformas), comandos, filtros y antispam | Mismo flujo de chat que MiniKick con pruebas por plataforma |
| F4 | TTS | Motor(es) elegidos mediante ADR y detrás de un trait |
| F5 | Recompensas | Paridad con MiniKick |
| F6 | Servidor de overlays (alertas, chat, widgets) | Overlays visibles en OBS |
| F7 | Música y playlists | Paridad con MiniKick |
| F8 | Webhooks salientes | Entrega con reintentos |
| F9 | Interfaz con GPUI (spike previo, ver ADR-0005) | Criterios del spike cumplidos |
| F10 | Distribución: instalador y actualizador | Instalación y actualización probadas en Windows |

## Decisiones pendientes

- Motor(es) de TTS: se decide en F4.
- Elección definitiva de la UI: se valida en F9 (ADR-0005).
