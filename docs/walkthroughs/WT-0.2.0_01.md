# WT-0.2.0_01: núcleo de SYON (F1)

## Contexto

F0 dejó el workspace, las herramientas de calidad y la CI, pero ningún crate de SYON: el único código
era el prototipo de consola de Kick, fuera del workspace. F1 construye el núcleo sobre el que se
apoyan las fases siguientes, siguiendo el [diseño 001](../designs/001-nucleo-dominio-eventos.md): los
tipos del dominio, los puertos, un bus de eventos que nunca bloquea, la configuración en TOML, las
rutas de datos, el registro de diagnóstico y la comprobación automática de la dirección de
dependencias. No hay interfaz ni plataformas todavía, y el prototipo no cambia.

## Cambios por capa

**Dominio** (`syon-domain`): `Platform`, `User`, `Timestamp`, `MessageId`, `ChatMessage`, `Event` y
`EventKind`. Sin dependencias, sin I/O, sin asincronía y sin leer el reloj: quien publica un evento
pasa la hora.

**Aplicación** (`syon-app`): los puertos `EventSink` (por donde las plataformas entregan eventos; no
bloquea) y `SettingsStore` (`load` y `save`), con `SettingsError` (`Io` e `Invalid`, siempre con la
ruta) y el modelo plano `Settings`, `LoggingSettings` y `LogLevel`. Solo depende de `syon-domain`; no
usa `serde` ni `thiserror`.

**Engine** (`syon-engine`), que corre sin interfaz:

- `EventBus` ([ADR-0007](../adr/0007-bus-de-eventos.md)): un canal acotado de `Arc<Event>` por
  consumidor; `publish` usa `try_send` y, si la cola está llena, descarta el evento nuevo solo para ese
  consumidor, lo cuenta y avisa en el log como máximo una vez por segundo. `stats()` informa de la cola
  y los descartes de cada uno.
- `AppPaths` ([ADR-0009](../adr/0009-rutas-de-datos.md)): carpetas de configuración, datos y caché con
  `etcetera` (no `dirs`, que arrastra `option-ext` con MPL-2.0), rutas de `settings.toml` y `syon.log` y
  la carpeta personal. Es el único lugar que consulta el sistema operativo; `AppPaths::under` reparte
  las carpetas bajo una raíz para las pruebas.
- Registro ([ADR-0006](../adr/0006-registro-de-diagnostico.md)): `log` con `env_logger`; consola en
  `warn` y `syon.log` con el nivel de los ajustes (`SYON_LOG` lo reemplaza). El archivo tiene un tope de
  16 MB y pasa a `syon.log.1`, conserva las últimas 5 ejecuciones y escribe una línea de marca por
  arranque. Cada línea pasa por el redactor de secretos, escrito a mano, y un hook de pánico registra el
  hilo, el lugar y el backtrace.

**Adaptador de ajustes** (`syon-storage-toml`, [ADR-0008](../adr/0008-configuracion-toml.md)):
implementa `SettingsStore` sobre `settings.toml` con un formato propio del adaptador (`syon-app` no
conoce `serde`). Archivo ausente da los valores por defecto sin escribir nada; las claves desconocidas
se ignoran; un archivo ilegible o con tipos incorrectos da `SettingsError::Invalid` con la ruta y la
clave y no se sobrescribe; el guardado es atómico (temporal en la misma carpeta y `rename`) y los
guardados simultáneos del proceso se serializan.

**Herramientas y CI** (`xtask`, `.github/workflows/ci.yml`): `cargo xtask arch-check` lee
`cargo metadata` y falla ante cualquiera de las cinco reglas de la sección 10 del diseño, expresadas en
una tabla con una prueba que debe fallar por cada regla. Usa un lector de JSON propio en lugar de
`serde_json`, así que `xtask` sigue usando solo la biblioteca estándar. La CI ejecuta el paso junto a
`docs-check` y añade `cargo-deny` sobre el workspace raíz; `deny.toml` admite dependencias por ruta
sin versión (los crates no se publican) y licencias permitidas que solo usa uno de los dos proyectos.

**Documentación**: diseño 001 aceptado; ADR 0006 a 0009 con su índice; `docs/deps.md` con una fila por
cada dependencia nueva (`tokio`, `log`, `env_logger`, `etcetera`, `toml`, `serde`); README de cada
crate; `AGENTS.md` y `docs/estado.md`, que permite retomar el trabajo sin depender de una conversación.

## Parches

Aplicados con `git am`, en este orden, sobre `master`.

1. `5524123` `feat(domain)`: crate `syon-domain`; el workspace pasa a `members = ["crates/*", "xtask"]`.
2. `df8186d` `feat(app)`: crate `syon-app` con `EventSink` y `SettingsStore`.
3. `678b1d0` `feat(engine)`: crate `syon-engine` con el bus de eventos; ADR 0006 y 0007.
4. `2a3c1c6` `feat(storage)`: crate `syon-storage-toml` y ADR 0008; seguido de `340cedf` `fix(storage)`,
   que serializa los guardados simultáneos (con 8 hilos fallaban 350 de 400).
5. `c92cd3e` `feat(engine)`: `AppPaths` con `etcetera` y ADR 0009.
6. `2aae256` `feat(engine)` (carpeta personal en `AppPaths`) y `63f7398` `feat(engine)` (registro con
   archivo acotado, redacción y hook de pánico).
7. `6795e97` `build(xtask)`: `arch-check` y su paso en la CI.
8. `docs`: este walkthrough y el cierre de `docs/estado.md`.

Además: `56cac9d` y `786c38d` (`docs(design)`: aceptación del diseño 001, sin `thiserror`) y `fddd845`
(`docs(estado)`: `docs/estado.md` con el estado y el prompt de traspaso).

Después de este cierre: `7e2446b` `feat(engine)`: `Engine::start` y el
[ADR-0010](../adr/0010-arranque-del-engine.md), que conectan los ajustes TOML con el registro y el
bus. No es uno de los ocho parches del diseño 001, por eso no figura arriba ni en los archivos
afectados ni en la verificación, que describe el estado en `6795e97`.

## Archivos afectados

- Nuevos: `crates/syon-domain/`, `crates/syon-app/`, `crates/syon-engine/` (`event_bus.rs`,
  `subscriber.rs`, `paths.rs`, `logging.rs`, `log_file.rs`, `redact.rs`, `panic_hook.rs` y
  `tests/logging.rs`), `crates/syon-storage-toml/` (`format.rs`, `store.rs`), `xtask/src/arch_check.rs`,
  `xtask/src/json.rs`, `docs/designs/001-nucleo-dominio-eventos.md`, `docs/adr/0006` a `0009`,
  `docs/estado.md` y este archivo.
- Modificados: `Cargo.toml` y `Cargo.lock` (miembros del workspace y dependencias del workspace),
  `.github/workflows/ci.yml`, `deny.toml`, `docs/deps.md`, `docs/adr/README.md`, `AGENTS.md`,
  `README.md` y `xtask/src/main.rs`.
- Sin cambios: `prototypes/kick-console/`.

## Verificación ejecutada

Sobre `origin/master` en `6795e97`, con el árbol sin cambios y Rust 1.91.1 (paquetes de Ubuntu, sin
rustup):

- `cargo fmt --all --check` y `cargo clippy --workspace --all-targets --locked -- -D warnings`, sin
  avisos.
- `cargo test --workspace --locked`: 117 pruebas aprobadas, 0 fallidas (`syon-domain` 9, `syon-app` 7,
  `syon-engine` 57, `syon-storage-toml` 17, `xtask` 27). En el prototipo, con `--locked`: 35
  aprobadas.
- `cargo xtask docs-check` y `cargo xtask arch-check` sin problemas (5 crates). Con `log` añadido a
  `syon-domain` a propósito, `arch-check` falla con la regla 1 y código de salida 1.
- `cargo deny check` sobre `Cargo.toml` y sobre `prototypes/kick-console/Cargo.toml`: avisos, bans,
  licencias y fuentes en verde. `cargo machete .`: sin dependencias sin uso.

Criterio de salida de F1 (diseño 001, sección 13):

| Criterio | Prueba |
|----------|--------|
| `syon-domain` sin dependencias y con pruebas | `arch-check` regla 1; 9 pruebas |
| El bus entrega a varios consumidores, descarta con contador y no bloquea | `entrega_cada_evento_a_todos_los_consumidores_sin_copiarlo`, `un_consumidor_detenido_descarta_y_cuenta_sin_afectar_a_otro_ni_bloquear` |
| `settings.toml` hace ida y vuelta, ignora claves desconocidas y no sobrescribe lo corrupto | `guardar_y_cargar_devuelve_lo_mismo_para_cada_nivel`, `las_claves_desconocidas_se_ignoran`, `un_*_es_invalid_y_no_se_sobrescribe` |
| `syon.log` con tope y rotación, marca por arranque, pánicos registrados y `redact` | `log_file::tests`, `logging::tests`, `redact::tests` y `tests/logging.rs` |
| `arch-check` falla con cada violación de la sección 10 | una prueba por regla en `arch_check::tests` |
| Todo en verde en la CI, incluido `cargo deny` | Pendiente de confirmar en Actions (ver riesgos) |

Pendiente: la CI no se ejecuta en el entorno de desarrollo. Los comandos de arriba son los mismos que
ejecuta, pero el resultado en Actions, y en particular en Windows, no se ha visto.

## Riesgos conocidos

- **Windows sin verificar.** La prueba de `AppPaths` (`%APPDATA%` y `%LOCALAPPDATA%`) solo se compila en
  Windows; la rotación renombra `syon.log` mientras sigue abierto y la poda reescribe archivos con un
  temporal; los guardados simultáneos del almacén TOML dependen de la plataforma. Hay que confirmar el
  trabajo de Windows de la CI antes de dar F1 por cerrada.
- **Ningún binario arranca el engine todavía.** La conexión de los ajustes con el registro se hizo
  después de este cierre, en `Engine::start` ([ADR-0010](../adr/0010-arranque-del-engine.md)), con las
  dos garantías que pedía este riesgo: tras un `SettingsError::Invalid` no se guardan valores por defecto
  sobre el archivo (se aparta como `settings.toml.invalid`) y la versión de la aplicación llega a la marca
  de arranque. Falta el crate `syon`, que es quien llamará a `Engine::start`.
- **Versión de esquema.** Un `save` de una versión vieja reescribe el archivo sin las claves que no
  conoce y deja su versión (ADR 0008). Pendiente decidir si rechazar o avisar cuando `schema_version` es
  mayor que `SCHEMA_VERSION`.
- **«Últimas 5 ejecuciones» se implementó de forma literal.** Al arrancar se podan las marcas más
  antiguas entre `syon.log.1` y `syon.log`. Si bastaba con el tope de tamaño, la poda (`prune_runs`)
  puede quitarse sin tocar el resto.
- **`redact` es una red de seguridad.** Reconoce secretos por el nombre de su clave y no detecta un valor
  suelto; la regla sigue siendo no registrarlos.
- **Dependencias.** `env_logger` se usa con sus funciones por defecto, que traen `jiff` y `regex`; `jiff`
  es la librería de tiempo prevista para la interfaz. `etcetera` y `env_logger` son pre-1.0.
- **`arch-check` rechaza crates sin papel.** Un crate cuyo nombre no encaje en `role_of` (por ejemplo,
  los de overlays) falla hasta que se decida su papel y se añada a la tabla. Cuentan también las
  dependencias de desarrollo.
- **Descarte bajo saturación.** El bus pierde información cuando un consumidor se satura; la mitigación
  es el contador, el aviso y la capacidad por consumidor, no evitar el descarte.
- **Feed no oficial de Kick.** Los campos que el prototipo lee del feed pueden cambiar (ver
  [WT-0.1.0_01](WT-0.1.0_01.md)); F2 los contrasta al migrar la plataforma.
- `chrono` sigue en el prototipo con su aviso ignorado en `deny.toml` hasta que se elimine el
  prototipo.
