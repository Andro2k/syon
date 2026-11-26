# Estado del proyecto

Última actualización: 2026-11-25. Se actualiza en el mismo parche que completa un paso del
plan (regla 7 de [AGENTS.md](../AGENTS.md)), para que cualquier persona o IA pueda retomar
el trabajo sin depender de una conversación.

## Dónde estamos

- Fase: **F1** (núcleo), con el [diseño 001](designs/001-nucleo-dominio-eventos.md) aceptado.
- Hecho: F0 completa ([WT-0.1.0_02](walkthroughs/WT-0.1.0_02.md)); F1 parche 1 (`syon-domain`),
  parche 2 (`syon-app`), parche 3 (bus de eventos en `syon-engine`, ADR 0006 y 0007), parche 4
  (`syon-storage-toml` con escritura atómica, [ADR-0008](adr/0008-configuracion-toml.md)) y parche 5
  (rutas de datos con `etcetera` en `syon-engine`, [ADR-0009](adr/0009-rutas-de-datos.md)) y parche 6
  (logging en `syon-engine`: `syon.log` de 16 MB y `syon.log.1`, últimas 5 ejecuciones, marca por
  arranque, redactor de secretos y hook de pánico, [ADR-0006](adr/0006-registro-de-diagnostico.md); se
  entregó en dos parches: la carpeta personal en `AppPaths` y el registro).
- Siguiente: F1 parche 7, `build(xtask)`: `arch-check` y su paso en CI (sección 10 del diseño).
- Después: el walkthrough `WT-0.2.0_01` (parche 8), que cierra F1.

## Pendiente de decidir o verificar

- Motores de TTS: se eligen en F4 con su ADR.
- Interfaz: GPUI directo ([ADR-0005](adr/0005-ui-gpui-directo.md)); el spike de F9 valida los criterios.
- La prueba de `AppPaths` en Windows (`%APPDATA%`, `%LOCALAPPDATA%`) solo corre en el trabajo de Windows de la CI: confirmar que pasó al aplicar el parche 5.
- Tras un `SettingsError::Invalid`, el engine no debe guardar valores por defecto sobre el archivo; lo decide el parche que conecte `syon-storage-toml` con el engine.
- `chrono` sigue en el prototipo con su aviso ignorado en `deny.toml`; se va con el prototipo.
- Confirmar en la pestaña Actions que el job de Windows pasa, en particular la prueba de `AppPaths`
  que solo se compila allí y la de guardados simultáneos del almacén TOML.
- Decisión del autor: rechazar o avisar al guardar si el `schema_version` del archivo es mayor que
  `SCHEMA_VERSION`. Hoy un `save` de una versión vieja reescribe el archivo sin las claves que no
  conoce y deja su versión (ADR 0008, consecuencias).
- Al conectar el engine con `SettingsStore`: tras un `SettingsError::Invalid` no se debe guardar los
  valores por defecto sobre el archivo; hay que renombrar el roto antes de continuar, con su prueba.
- Confirmar en el job de Windows de la CI las pruebas nuevas del parche 6: la rotación renombra
  `syon.log` mientras sigue abierto (en Windows pasa por `MoveFileEx` con reemplazo) y la poda reescribe
  archivos con un temporal; en Linux pasan 25 de 25 ejecuciones seguidas.
- El logging aún no se conecta a nada: ningún binario llama a `init_logging`. Lo hace el parche que
  conecte `syon-engine` con `SettingsStore` (leer `[logging].level`, pasar la versión de la app).
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
