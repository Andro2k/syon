# Estado del proyecto

Última actualización: 2026-11-27. Se actualiza en el mismo parche que completa un paso del
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
  Sigue el punto 11 (el binario) y el cableado de los conectores.
- Propuesto, pendiente de aceptar: el [diseño 003](designs/003-arranque-de-plataformas-y-binario.md), que
  cubre el punto 11 y el hueco que el plan no recogía (ajustes de Kick, arranque de conectores en el
  engine y el binario `syon`). El autor decidió el 2026-11-26 que las credenciales de la aplicación van
  incrustadas en el binario ([ADR-0018](adr/0018-credenciales-de-aplicacion-incrustadas.md)).
  Parche 2 del plan del 003 hecho (2026-11-27): `KickSettings` en `syon-app` y la sección `[kick]` en
  `syon-storage-toml` (aplicado como 04f00bd); un archivo de F1 sin la sección sigue cargando. Parche 3
  hecho: `AppPaths::credentials_file()` (aplicado como a5763d7). Parche 4 hecho: `AppCredentials`,
  `PlatformCredentials`, `Engine::start_platforms` y `stop_platforms` para Kick (sin las credenciales ni el
  enlace de inicio de sesión, que llegan con Twitch en el parche 7). Siguen los parches 5 a 8 de ese plan.

## Pendiente de decidir o verificar

- Aceptar o corregir el [diseño 003](designs/003-arranque-de-plataformas-y-binario.md) (decisiones de su
  sección 11); los parches 2 a 4 se adelantaron sin aceptarlo, así que conviene decidirlo antes de los parches 5 a 8.
- Antes de publicar (F10): verificar los términos de Kick y de Twitch sobre secretos de aplicación
  incrustados y, para Twitch, si un cliente público con flujo de dispositivo evita el secreto
  ([ADR-0018](adr/0018-credenciales-de-aplicacion-incrustadas.md)).
- Los límites de Twitch del diseño 002 (300 suscripciones por conexión, 3 conexiones, `max_total_cost` de
  10 con token de usuario, coste 0 con permiso del usuario) salen de documentación y foros consultados el
  2026-11-26; el plazo de bienvenida y el coste real se confirman en vivo al implementar.
- Motores de TTS: se eligen en F4 con su ADR.
- Interfaz: GPUI directo ([ADR-0005](adr/0005-ui-gpui-directo.md)); el spike de F9 valida los criterios.
- `chrono` sigue en el prototipo con su aviso ignorado en `deny.toml`; se va con el prototipo.
- Los permisos del archivo de credenciales no se comprueban en Windows (solo en Unix): confirmarlo al
  iniciar sesión por primera vez.
- Decisión del autor: rechazar o avisar al guardar si el `schema_version` del archivo es mayor que
  `SCHEMA_VERSION`. Hoy un `save` de una versión vieja reescribe el archivo sin las claves que no
  conoce y deja su versión (ADR 0008, consecuencias).
- El feed de Kick solo se probó con servidores HTTP y WebSocket locales falsos: falta contrastarlo en vivo
  (nombres de campo, endpoints de `web.kick.com` y `kick.com/api/v2`, el 403 de la consulta del canal
  y la forma real de las publicaciones).
- El OAuth de Kick solo se probó con servidores locales falsos: falta un inicio de sesión real contra
  `id.kick.com` (endpoints, permisos y puerto de callback `8080`, que debe estar registrado en la aplicación
  de Kick).
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
