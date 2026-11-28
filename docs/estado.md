# Estado del proyecto

Última actualización: 2026-11-28. Se actualiza en el mismo parche que completa un paso del
plan (regla 7 de [AGENTS.md](../AGENTS.md)), para que cualquier persona o IA pueda retomar
el trabajo sin depender de una conversación.

## Dónde estamos

- Fase: **F1 (núcleo) cerrada** con el [walkthrough WT-0.2.0_01](walkthroughs/WT-0.2.0_01.md); el
  [diseño 001](designs/001-nucleo-dominio-eventos.md) está implementado en sus ocho parches y la CI,
  incluido el trabajo de Windows, pasa en verde (confirmado por el autor el 2026-11-26).
- Hecho: F0 ([WT-0.1.0_02](walkthroughs/WT-0.1.0_02.md)) y F1: `syon-domain`, `syon-app`, el bus de
  eventos, las rutas de datos y el registro en `syon-engine`, `syon-storage-toml`, `cargo xtask
  arch-check` y su paso en la CI.
- Parche de conexión: `Engine::start` lee los ajustes con `syon-storage-toml`, aparta un archivo inválido
  como `settings.toml.invalid` sin guardar nada encima, inicia el registro con el nivel de los ajustes y
  crea el bus ([ADR-0010](adr/0010-arranque-del-engine.md)).
- Rutas: desde el 2026-11-27 todo cuelga de `~/.SYON/{config,data,cache}` en Linux y Windows, con la
  carpeta personal de `std::env::home_dir` y sin `etcetera` ([ADR-0020](adr/0020-carpeta-personal-punto-syon.md),
  que reemplaza al ADR-0009). No hay migración automática: quien tenga archivos en `%APPDATA%\SYON` o en
  `~/.config/syon` y `~/.local/share/syon` los mueve a mano.
- Siguiente: **F2 (plataformas)** en curso. El [diseño 002](designs/002-plataformas-kick-twitch.md) está
  aceptado (2026-11-26) y se implementa con su plan de la sección 12, un parche por vez. Hecho: el diseño,
  `EventKind::Followed`, los puertos de `syon-app` (`PlatformConnector`, `ConnectionStatus`, `Backoff` y
  `CredentialStore`), el runtime de `tokio` y el registro de conectores en `syon-engine`
  ([ADR-0011](adr/0011-runtime-asincrono-del-engine.md)), `syon-storage-credentials`
  ([ADR-0012](adr/0012-almacen-de-credenciales.md)), `syon-platform-kick` con la traducción de eventos
  (`serde_json`, [ADR-0013](adr/0013-json-tolerante-en-adaptadores.md)), el OAuth 2.1 con PKCE
  (`reqwest`, [ADR-0014](adr/0014-oauth-de-kick.md)) y el feed en tiempo real con reconexión y
  `KickConnector` (`tokio-tungstenite`, [ADR-0015](adr/0015-conexion-al-feed-de-kick.md)), y
  `syon-platform-twitch` (puntos 9 y 10): OAuth y cliente de Helix
  ([ADR-0016](adr/0016-oauth-y-api-de-twitch.md)), traducción de eventos, tabla de las ocho suscripciones
  y conexión a EventSub con `TwitchConnector` ([ADR-0017](adr/0017-conexion-a-eventsub-de-twitch.md)).
  El punto 11 (el binario) y el cableado de los conectores se hicieron con el diseño 003.
- Aceptado el 2026-11-27: el [diseño 003](designs/003-arranque-de-plataformas-y-binario.md), que
  cubre el punto 11 y el hueco que el plan no recogía (ajustes de Kick, arranque de conectores en el
  engine y el binario `syon`). El autor decidió el 2026-11-26 que las credenciales de la aplicación van
  incrustadas en el binario y aceptó el [ADR-0018](adr/0018-credenciales-de-aplicacion-incrustadas.md) el 2026-11-27.
  Los siete parches de código del plan del 003 están aplicados: `[kick]` en los ajustes (04f00bd),
  `AppPaths::credentials_file()` (a5763d7), `AppCredentials`, `start_platforms` y `stop_platforms`
  (b7882aa), `Engine::login` (ee79974), el binario `syon` con `run` y `login` (635d6f9), `[twitch]`
  en los ajustes (fd1c04c) y el cableado de Twitch en el engine y el binario (288f3d1). El parche 9
  cierra F2 por el lado de SYON con `docs/architecture/suscripciones.md`, la tabla de paridad y el
  [walkthrough WT-0.3.0_01](walkthroughs/WT-0.3.0_01.md).

- F3 empieza con el [diseño 004](designs/004-chat-unificado-y-envio.md), aceptado el 2026-11-27: chat
  unificado y envío de mensajes a Kick y Twitch. Los comandos, filtros y antispam van en otro diseño.
  Sus cinco parches de código están implementados (`syon-app`, `syon-platform-twitch`,
  `syon-platform-kick`, `syon-engine` y `syon`); queda el contraste en vivo de envío a ambas
  plataformas a cargo del autor.

## Frentes de trabajo

Quien empieza un frente lo anota aquí **en su primer parche** y solo edita su propia fila; así varias
personas o IAs pueden trabajar a la vez sin pisarse. Al terminar, la fila pasa a «Hecho» con el hash del
parche aplicado. La columna «Responsable» la rellena quien coordina; vacía significa que nadie lo tomó.

| Frente | Estado | Crates y archivos que toca | Responsable |
|--------|--------|----------------------------|-------------|
| Chat unificado y envío ([diseño 004](designs/004-chat-unificado-y-envio.md)) | Implementado (5 parches de código); falta contraste en vivo de envío | `syon-app`, `syon-platform-twitch`, `syon-platform-kick`, `syon-engine`, `syon` | Gemini |
| Comandos, filtros y antispam (F3, parte 2) | Implementado en 5 parches de código; límites corregidos para aplicarse por destino; falta confirmar en vivo la forma de los campos opcionales del feed de Kick | `syon-domain`, `syon-app`, `syon-platform-kick`, `syon-platform-twitch`, `syon-storage-toml`, `syon-engine` | Codex |
| Contraste en vivo de Kick y Twitch (F2) | Pendiente: `syon login` de ambas, eventos reales y la línea de coste de EventSub | Nada de código; con la aplicación real | Autor |
| Job de Windows de la CI con `syon` y los crates de plataformas | Pendiente de confirmar | Nada | Autor |
| TTS (F4) | Solo Kokoro por decisión del autor (2026-11-27): ADR-0021 y [diseño 006](designs/006-tts.md) corregidos, pendientes de aceptación | Solo `docs/adr/` (0021), `docs/designs/` (006), la fila de TTS de `docs/requirements/paridad.md` y mi fila de este archivo; sin código hasta que se acepten | Claude |
| Revisión de F3 (correcciones menores tras revisar los frentes de chat, comandos y antispam) | Hecho ([WT-0.4.2_01](walkthroughs/WT-0.4.2_01.md)): eco propio, nombres de comando inalcanzables y texto del usuario en `{args}` y `{user}`; falta que el autor aplique los parches y anote los hashes | `syon-engine/src/commands.rs`, `syon-storage-toml/src/format.rs`, `docs/designs/005-comandos-filtros-antispam.md` y mi fila de este archivo | Claude |

**Archivos que varios frentes tocan** y donde hay que tener más cuidado: `Cargo.toml` y `Cargo.lock`
(regenerar con `cargo check --workspace --offline`, nunca `cargo update`; si hay conflicto, descartar el
lock y regenerarlo), `docs/estado.md`, `docs/deps.md`, `docs/requirements/paridad.md`, los índices de
`docs/adr/` y `docs/designs/`, `crates/syon-engine/src/engine.rs` y `lib.rs`, y
`crates/syon/src/commands.rs`. Antes de exportar, `git fetch` y `git rebase origin/master`.

## Pendiente de decidir o verificar

- Contraste en vivo con la aplicación real, a cargo del autor: `syon login kick`, `syon login twitch`,
  eventos reales de ambas plataformas y la línea de coste de EventSub en `syon.log`
  ([suscripciones](architecture/suscripciones.md)). F2 no se da por cerrada hasta verlo.
- Confirmar el job de Windows de la CI con el crate `syon` y los crates de plataformas, que solo se han
  ejecutado en Linux.
- Antes de publicar (F10): verificar los términos de Kick y de Twitch sobre secretos de aplicación
  incrustados y, para Twitch, si un cliente público con flujo de dispositivo evita el secreto
  ([ADR-0018](adr/0018-credenciales-de-aplicacion-incrustadas.md)).
- Los límites de Twitch del diseño 002 (300 suscripciones por conexión, 3 conexiones, `max_total_cost` de
  10 con token de usuario, coste 0 con permiso del usuario) salen de documentación y foros consultados el
  2026-11-26; el plazo de bienvenida y el coste real se confirman en vivo al implementar.
- Motor de TTS: el autor decidió empezar solo con Kokoro. El [ADR-0021](adr/0021-motores-de-tts-como-procesos-externos.md) lo propone ejecutado con `sherpa-onnx-offline-tts` como proceso externo; falta aceptarlo, y confirmar en vivo los argumentos del programa, el paquete de modelo y la calidad del español, de lo que depende el diseño 006.
- Interfaz: GPUI directo ([ADR-0005](adr/0005-ui-gpui-directo.md)); el spike de F9 valida los criterios.
- `chrono` sigue en el prototipo con su aviso ignorado en `deny.toml`; se va con el prototipo.
- Los permisos del archivo de credenciales no se comprueban en Windows (solo en Unix): confirmarlo al
  iniciar sesión por primera vez.
- Aceptar o corregir el [ADR-0019](adr/0019-guardado-con-esquema-mas-nuevo.md): `save` rechaza guardar si el
  `schema_version` del archivo o de los ajustes es mayor que `SCHEMA_VERSION` (la decisión estaba abierta; se
  eligió no perder claves). Ningún código de producción llama aún a `save`; la interfaz de F9 deberá mostrar
  el error.
- El feed de Kick solo se probó con servidores HTTP y WebSocket locales falsos: falta contrastarlo en vivo
  (nombres de campo, endpoints de `web.kick.com` y `kick.com/api/v2`, el 403 de la consulta del canal
  y la forma real de las publicaciones).
- El OAuth de Kick solo se probó con servidores locales falsos: falta un inicio de sesión real contra
  `id.kick.com` (endpoints, permisos y puerto de callback `8080`, que debe estar registrado en la aplicación
  de Kick).
- Si el usuario no mira el log, no se entera de que se ignoró un `settings.toml` inválido; una
  notificación en la interfaz queda para F9 (ADR 0010).
- Decisión del autor: «últimas 5 ejecuciones» se implementó de forma literal (se podan las marcas más
  antiguas entre `syon.log.1` y `syon.log` al arrancar, con la actual son 5). Si bastaba con el tope de
  tamaño, la poda (`prune_runs` en `log_file.rs`) se puede quitar sin tocar el resto.

## Cómo retomar el trabajo

1. `git clone https://github.com/Andro2k/syon`, o `git pull` si ya existe, y `git log --oneline`
   para ver qué parches están aplicados.
2. Leer [AGENTS.md](../AGENTS.md), este archivo completo y el diseño de tu frente en `docs/designs/` (su
   sección «Plan de parches»). Si el repositorio contradice lo que te dijeron, manda el repositorio.
3. Recrear el entorno si falta Rust 1.91.1:
   - Con rustup: `rustup toolchain install` (lee `rust-toolchain.toml`).
   - En Ubuntu sin rustup: `apt-get install -y cargo-1.91 rustc-1.91 rust-1.91-clippy rustfmt-1.91
     build-essential pkg-config` y `export PATH=/usr/lib/rust-1.91/bin:$PATH`.
   - `cargo-deny` (verificado con 0.22.1) y `cargo-machete` (verificado con 0.10.1): binarios de
     las versiones publicadas en GitHub de EmbarkStudios/cargo-deny y bnjbvr/cargo-machete.
4. Partir de verde: `./validar.sh` (ver el README) ejecuta las comprobaciones de AGENTS.md y las de
   dependencias. Para entregar, repetirlo sobre un clon limpio de `origin/master` con los parches aplicados
   con `git am` sin `-3` y con su propio `target`, nunca compartido con otra copia.
5. Entregar según AGENTS.md. El autor aplica los parches con otro hash, así que antes de cada tanda hay
   que hacer `git fetch` y `git reset --hard origin/master`, y la nueva tanda se numera desde `0001`, salvo
   que el autor pida otra numeración. Si el autor todavía no aplicó la tanda anterior, la nueva se numera a
   continuación y se indica cuáles aplicar.

Convenciones que ya se han corregido más de una vez:

- Los walkthroughs se llaman `WT-<versión>_<NN>.md` con la versión de la fase: F0 `0.1.0`, F1 `0.2.0`,
  F2 `0.3.0`.
- Una prueba que instala estado global del proceso (`Engine::start` instala el registro) va en su propio
  binario de `crates/<crate>/tests/`, con una sola función de prueba.
- Los datos de las plataformas se contrastan con su documentación oficial; lo que salga de otras fuentes se
  marca «por confirmar en vivo» en el diseño. Las credenciales de la aplicación se resuelven solo en el
  binario ([ADR-0018](adr/0018-credenciales-de-aplicacion-incrustadas.md)) y todo cuelga de `~/.SYON`
  ([ADR-0020](adr/0020-carpeta-personal-punto-syon.md)).
- Un parche que adelanta código de un diseño sin aceptar lo dice en el mensaje final y en este archivo.
- En el entorno de Ubuntu sin rustup, `sh` no admite `set -o pipefail` (usar `bash`), y cada `target`
  completo pesa varios GB: borrar los de copias anteriores antes de compilar otra.

Notas del entorno: la API anónima de GitHub devuelve 403 al superar su límite de consultas, así
que para ver el estado de la CI hay que abrir la pestaña Actions del repositorio.

## Prompt de traspaso

Para pasar el trabajo a otra IA, pegar esto y completar la última línea. Lo que cambia con el tiempo vive
en este archivo y no en el prompt, así que no hace falta actualizarlo a mano:

```text
Continúa el proyecto SYON (https://github.com/Andro2k/syon). Haz git clone (o git pull --ff-only si ya existe) y lee, en este orden: AGENTS.md, docs/estado.md completo (incluidos «Frentes de trabajo» y «Cómo retomar el trabajo») y el diseño de tu frente en docs/designs/, con su «Plan de parches». Revisa con git log qué parches están aplicados: el autor los aplica con otro hash, así que parte siempre de origin/master.
Antes de empezar, anota tu frente en la tabla de docs/estado.md y no toques los archivos de otros frentes.
Entrega parches atómicos de git format-patch, validados con ./validar.sh y repetidos sobre un clon limpio,  con la autoría y la coautoría de AGENTS.md. Actualiza docs/estado.md al terminar cada paso. Si el repositorio contradice este mensaje, manda el repositorio. Tarea concreta: <frente o paso>.
```
