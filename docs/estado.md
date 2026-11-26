# Estado del proyecto

Última actualización: 2026-11-26. Se actualiza en el mismo parche que completa un paso del
plan (regla 7 de [AGENTS.md](../AGENTS.md)), para que cualquier persona o IA pueda retomar
el trabajo sin depender de una conversación.

## Dónde estamos

- Fase: **F1 (núcleo) cerrada** con el [walkthrough WT-0.2.0_01](walkthroughs/WT-0.2.0_01.md); el
  [diseño 001](designs/001-nucleo-dominio-eventos.md) está implementado en sus ocho parches. Falta
  confirmar en la pestaña Actions que la CI, incluido el trabajo de Windows, pasa (ver abajo).
- Hecho: F0 ([WT-0.1.0_02](walkthroughs/WT-0.1.0_02.md)) y F1: `syon-domain`, `syon-app`, el bus de
  eventos, las rutas de datos y el registro en `syon-engine`, `syon-storage-toml`, `cargo xtask
  arch-check` y su paso en la CI.
- Parche de conexión: `Engine::start` lee los ajustes con `syon-storage-toml`, aparta un archivo inválido
  como `settings.toml.invalid` sin guardar nada encima, inicia el registro con el nivel de los ajustes y
  crea el bus ([ADR-0010](adr/0010-arranque-del-engine.md)).
- Siguiente: **F2 (plataformas)** en curso. El [diseño 002](designs/002-plataformas-kick-twitch.md) está
  aceptado (2026-11-26) y se implementa con su plan de la sección 12, un parche por vez. Parches hechos: 1
  (diseño), 2 (aceptación) y 3 (`EventKind::Followed`). Sigue el 4: `feat(app)` con `PlatformConnector`,
  `ConnectionStatus`, `Backoff` y `CredentialStore`.

## Pendiente de decidir o verificar

- Los límites de Twitch del diseño 002 (300 suscripciones por conexión, 3 conexiones, `max_total_cost` de
  10 con token de usuario, coste 0 con permiso del usuario) salen de documentación y foros consultados el
  2026-11-26; el plazo de bienvenida y el coste real se confirman en vivo al implementar.
- Motores de TTS: se eligen en F4 con su ADR.
- Interfaz: GPUI directo ([ADR-0005](adr/0005-ui-gpui-directo.md)); el spike de F9 valida los criterios.
- La prueba de `AppPaths` en Windows (`%APPDATA%`, `%LOCALAPPDATA%`) solo corre en el trabajo de Windows de la CI: confirmar que pasó al aplicar el parche 5.
- `chrono` sigue en el prototipo con su aviso ignorado en `deny.toml`; se va con el prototipo.
- Confirmar en la pestaña Actions que el job de Windows pasa, en particular la prueba de `AppPaths`
  que solo se compila allí y la de guardados simultáneos del almacén TOML.
- Decisión del autor: rechazar o avisar al guardar si el `schema_version` del archivo es mayor que
  `SCHEMA_VERSION`. Hoy un `save` de una versión vieja reescribe el archivo sin las claves que no
  conoce y deja su versión (ADR 0008, consecuencias).
- Confirmar en el job de Windows de la CI las pruebas nuevas del parche 6: la rotación renombra
  `syon.log` mientras sigue abierto (en Windows pasa por `MoveFileEx` con reemplazo) y la poda reescribe
  archivos con un temporal; en Linux pasan 25 de 25 ejecuciones seguidas.
- Ningún binario llama todavía a `Engine::start`: las pruebas de `crates/syon-engine/tests/` son su
  único uso. Al crear el crate `syon`, pasarle `env!("CARGO_PKG_VERSION")` y decidir cómo avisa un
  `StartError`.
- Si el usuario no mira el log, no se entera de que se ignoró un `settings.toml` inválido; una
  notificación en la interfaz queda para F9 (ADR 0010).
- Decisión del autor: «últimas 5 ejecuciones» se implementó de forma literal (se podan las marcas más
  antiguas entre `syon.log.1` y `syon.log` al arrancar, con la actual son 5). Si bastaba con el tope de
  tamaño, la poda (`prune_runs` en `log_file.rs`) se puede quitar sin tocar el resto.

## Cómo retomar el trabajo

1. `git clone https://github.com/Andro2k/syon`, o `git pull` si ya existe, y `git log --oneline`
   para ver qué parches están aplicados.
2. Leer [AGENTS.md](../AGENTS.md), este archivo y la sección 12 del diseño 001.
3. Recrear el entorno si falta Rust 1.91.1:
   - Con rustup: `rustup toolchain install` (lee `rust-toolchain.toml`).
   - En Ubuntu sin rustup: `apt-get install -y cargo-1.91 rustc-1.91 rust-1.91-clippy rustfmt-1.91
     build-essential pkg-config` y `export PATH=/usr/lib/rust-1.91/bin:$PATH`.
   - `cargo-deny` (verificado con 0.22.1) y `cargo-machete` (verificado con 0.10.1): binarios de
     las versiones publicadas en GitHub de EmbarkStudios/cargo-deny y bnjbvr/cargo-machete.
4. Ejecutar las comprobaciones de AGENTS.md antes de cualquier cambio, para partir de verde.
5. Entregar según AGENTS.md. Si el autor todavía no aplicó la tanda anterior, la nueva se numera a
   continuación y se indica cuáles aplicar; los parches se verifican siempre sobre un clon limpio
   de `origin/master`.

Notas del entorno: la API anónima de GitHub devuelve 403 al superar su límite de consultas, así
que para ver el estado de la CI hay que abrir la pestaña Actions del repositorio.

## Prompt de traspaso

Para pasar el trabajo a otra IA, pegar esto y completar la última línea:

```text
Continúa el proyecto SYON (https://github.com/Andro2k/syon). Haz git clone, lee AGENTS.md,
docs/estado.md y la sección 12 de docs/designs/001-nucleo-dominio-eventos.md, y revisa con git log
qué parches están aplicados. Respeta las reglas de arquitectura y de entrega de AGENTS.md: parches
atómicos de git format-patch, verificados sobre un clon limpio, con la autoría y la coautoría
indicadas. Actualiza docs/estado.md al terminar cada paso. Tarea concreta: <qué sigue>.
```
