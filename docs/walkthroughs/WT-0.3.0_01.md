# WT-0.3.0_01: plataformas conectadas al engine y binario `syon` (cierre de F2)

## Contexto

F2 tenía entregados los crates de las plataformas: `syon-platform-kick` (eventos, OAuth y feed) y
`syon-platform-twitch` (OAuth, Helix y EventSub), según el [diseño 002](../designs/002-plataformas-kick-twitch.md).
Faltaba el cableado: nadie construía ni registraba los conectores, no había de dónde sacar el canal ni
las credenciales de la aplicación, y no existía un binario que arrancara el engine. El
[diseño 003](../designs/003-arranque-de-plataformas-y-binario.md) cubre ese hueco y el punto 11 del
plan del diseño 002. Con este cierre SYON se puede ejecutar desde la consola: conecta las plataformas
activadas en `settings.toml` e imprime los eventos. No hay interfaz (F9), ni envío de mensajes (F3), ni
instalador (F10).

El diseño 003 sigue en «Propuesta»: sus siete parches de código se aplicaron antes de aceptarlo.

## Cambios por capa

**Aplicación** (`syon-app`): `KickSettings` (`enabled`, `channel`, `channel_id`, `chatroom_id`) y
`TwitchSettings` (`enabled`) dentro de `Settings`. Por defecto ambas plataformas están desactivadas.

**Adaptador de ajustes** (`syon-storage-toml`): las secciones `[kick]` y `[twitch]`. Los ids opcionales
de Kick no se escriben si no están definidos. Un archivo de F1 sin las secciones y uno con claves
desconocidas siguen cargando, y `schema_version` se queda en 1 ([ADR-0008](../adr/0008-configuracion-toml.md)).

**Engine** (`syon-engine`):

- `AppPaths::credentials_file()`: `<carpeta de datos>/credentials.toml`, junto a `syon.log`, porque los
  tokens son estado y no preferencias.
- `AppCredentials` (con `Debug` a mano que no imprime el secreto) y `PlatformCredentials`
  ([ADR-0018](../adr/0018-credenciales-de-aplicacion-incrustadas.md)). El engine no lee el entorno.
- `Engine::start_platforms(credenciales, on_login_url)`: construye, arranca y registra el conector de
  cada plataforma activada y devuelve un `PlatformStart` por plataforma, con el motivo si no arrancó
  (`MissingChannel`, `MissingCredentials`, `AlreadyRunning`, `Login`, `Connect`). Kick no pide credenciales
  ni enlace. Twitch deja lista la sesión antes de conectar: reutiliza la guardada, la renueva o, si no
  hay una válida, llama a `on_login_url` y espera a que la persona autorice. `stop_platforms` detiene
  todos los conectores.
- `Engine::login(plataforma, credenciales, on_url)`: deja lista la sesión de Kick o de Twitch y devuelve
  `LoginOutcome` (`Restored`, `Refreshed`, `NewLogin`). `LoginOutcome` y `LoginError` son tipos del
  engine, para que el binario no vea los de los adaptadores. Ningún mensaje lleva tokens ni secretos.
- El engine pasa a depender de `syon-platform-kick`, `syon-platform-twitch` y `syon-storage-credentials`,
  todos crates del workspace.

**Binario** (`syon`, papel `Binary` en `arch-check`): `syon [run]`, `syon login <kick|twitch>`,
`--version` y `--help`, sin `clap`. `run` arranca las plataformas activadas, imprime una línea por
evento (`<hora UTC> <plataforma> <resumen>`) y se detiene con Ctrl+C; sin plataformas activadas dice qué
archivo editar y sale con 1. Códigos de salida: 0, 1 (fallo de arranque) y 2 (uso incorrecto). Las
credenciales de la aplicación se incrustan al compilar con `option_env!` y las variables de entorno en
ejecución las reemplazan; `build.rs` declara `rerun-if-env-changed`. El texto de los eventos pasa por un
filtro que quita los caracteres de control y los de formato bidireccional.

**Documentación**: [`docs/architecture/suscripciones.md`](../architecture/suscripciones.md) con el
presupuesto de conexiones y suscripciones de cada plataforma (criterio de salida de F2 del roadmap), la
tabla de paridad, `docs/deps.md` (`tokio` con `rt` y `signal` para el binario), el README del crate
`syon`, `AGENTS.md` y `docs/estado.md`.

## Parches

Aplicados con `git am`, en este orden, sobre `master`. Los dos primeros son del diseño; el resto son los
parches 2 a 8 de su plan (el punto 9 es este cierre).

1. `6efd022` `docs(design)`: el diseño 003 y el ADR-0018; `91b2486` `docs(twitch)`: apunta al ADR-0018.
2. `04f00bd` `feat(app)`: la sección `[kick]` en los ajustes.
3. `a5763d7` `feat(engine)`: `AppPaths::credentials_file`.
4. `b7882aa` `feat(engine)`: `start_platforms` y `stop_platforms` para Kick, y `AppCredentials`.
5. `ee79974` `feat(engine)`: `Engine::login` para Kick.
6. `635d6f9` `feat(syon)`: el binario con `run` y `login`.
7. `fd1c04c` `feat(app)`: la sección `[twitch]` en los ajustes.
8. `288f3d1` `feat(engine)`: Twitch en `start_platforms` y `login`, y en el binario.
9. `docs`: este walkthrough, el presupuesto de suscripciones y el cierre de `docs/estado.md`.

Además, `c090def` `feat(validar)`: un script para validar dependencias y ejecutar pruebas, del autor; no
forma parte del plan del diseño 003.

## Archivos afectados

- Nuevos: `crates/syon/` (`build.rs`, `src/main.rs`, `cli.rs`, `commands.rs`, `credentials.rs`,
  `events.rs`, `tests/cli.rs`, `README.md`), `crates/syon-engine/src/` (`credentials.rs`, `login.rs`,
  `platforms.rs`), `crates/syon-engine/tests/` (`engine_login.rs`, `engine_platforms.rs`,
  `engine_twitch.rs`), `docs/designs/003-arranque-de-plataformas-y-binario.md`,
  `docs/adr/0018-credenciales-de-aplicacion-incrustadas.md`, `docs/architecture/suscripciones.md` y este
  archivo.
- Modificados: `Cargo.toml` y `Cargo.lock` (miembros y dependencias; el lock suma `errno` y
  `signal-hook-registry` por la función `signal` de `tokio`), `crates/syon-app/src/settings.rs` y
  `lib.rs`, `crates/syon-storage-toml/src/format.rs` y `store.rs`, `crates/syon-engine/src/` (`engine.rs`,
  `paths.rs`, `connectors.rs`, `lib.rs`, `Cargo.toml`), `docs/deps.md`, `docs/estado.md`,
  `docs/requirements/paridad.md`, `docs/adr/README.md`, `AGENTS.md`.
- Sin cambios: `prototypes/kick-console/` y los crates `syon-platform-kick` y `syon-platform-twitch`.

## Verificación ejecutada

Sobre un clon limpio de `origin/master` en `288f3d1` con este parche aplicado con `git am` (sin `-3`),
con `target` propio y Rust 1.91.1 (paquetes de Ubuntu, sin rustup), en Linux:

- `cargo fmt --all --check` y `cargo clippy --workspace --all-targets --locked -- -D warnings`, sin
  avisos.
- `cargo test --workspace --locked`: 332 pruebas aprobadas, 0 fallidas (`syon` 23, `syon-app` 15,
  `syon-domain` 10, `syon-engine` 80, `syon-platform-kick` 59, `syon-platform-twitch` 78,
  `syon-storage-credentials` 15, `syon-storage-toml` 25, `xtask` 27).
- `cargo xtask docs-check` y `cargo xtask arch-check` sin problemas (9 crates).
- `cargo deny check` sobre `Cargo.toml` y sobre `prototypes/kick-console/Cargo.toml`: avisos, bans,
  licencias y fuentes en verde. `cargo machete .`: sin dependencias sin uso.
- A mano: `syon run` con `[kick]` activado y sin red arranca, reintenta la conexión y sale con código 0 al
  recibir Ctrl+C.

Criterio de salida del diseño 003 (sección 9):

| Criterio | Prueba |
|----------|--------|
| `syon --version` imprime la versión; sin plataformas activadas lo dice y sale con 1 | `tests/cli.rs` de `syon` |
| Con `[kick]` activado el engine registra el conector, `connection_status()` lo refleja y se detiene sin esperar a la red | `engine_platforms.rs` |
| Twitch deja la sesión lista antes de conectar y, sin las credenciales, no arranca y lo dice | `engine_twitch.rs` y el caso de Twitch en `tests/cli.rs` |
| Un archivo de F1 y uno con claves desconocidas siguen cargando | `store.rs` de `syon-storage-toml` |
| `resolve` cubre precedencia, ausencia y valores en blanco; el binario compila sin variables | `credentials::tests` de `syon` |
| Ningún valor de credencial aparece en un `Debug`, en un error ni en el registro | `credentials::tests`, `engine_login.rs`, `engine_twitch.rs` |
| El texto de chat con secuencias de escape sale sin ellas | `events::tests` de `syon` |
| `arch-check` en verde | 9 crates sin problemas |
| Contraste en vivo con la aplicación real | **Pendiente, a cargo del autor** |

## Riesgos conocidos

- **Contraste en vivo pendiente.** Ningún inicio de sesión ni evento real se ha visto: `syon login kick`,
  `syon login twitch` y el tráfico real de ambas plataformas necesitan la aplicación del autor, el
  navegador y la red. Tampoco se ha leído el coste real de EventSub frente al `max_total_cost` de 10
  ([suscripciones](../architecture/suscripciones.md)). Hasta entonces F2 no debe darse por cerrada.
- **Windows sin verificar.** Todo se ejecutó en Linux. Las pruebas de `run` del binario solo se compilan
  en Unix (redirigen las carpetas con variables XDG) y el manejo de Ctrl+C no se ha visto en Windows.
- **Secreto extraíble del binario** y términos de Kick y de Twitch sin verificar
  ([ADR-0018](../adr/0018-credenciales-de-aplicacion-incrustadas.md)).
- **Callbacks fijos** en `localhost:8080` (Kick) y `localhost:8081` (Twitch): si otro programa ocupa el
  puerto, esa plataforma no puede iniciar sesión.
- **El arranque de Twitch bloquea hasta autorizar.** `start_platforms` arranca Kick primero y después
  espera la autorización de Twitch; por cómo está escrito, un Ctrl+C durante esa espera termina el
  proceso sin pasar por `stop_platforms`. No se probó en ejecución.
- **Credenciales por pareja.** El ADR-0018 dice que cada variable de entorno gana a la incrustada; el
  binario decide por pareja (`client_id` y `client_secret`) y trata una pareja de entorno a medias como
  error, para no mezclar dos aplicaciones. Conviene reflejarlo en el ADR al aceptarlo.
- **El binario depende también de `syon-domain`**, no solo de `syon-engine`, para dar formato a los
  eventos; `arch-check` lo admite y el diseño 003 se corrigió.
- **Salida de eventos provisional**; la sustituye la interfaz de F9. La prueba del engine con Kick
  activado llama a `KickConnector::start`, que intenta conectar con `kick.com` y se detiene de inmediato;
  si resultara frágil habría que inyectar una fábrica de conectores.
- **`schema_version` mayor que el soportado**: sigue sin decidirse si rechazar o avisar
  ([ADR-0008](../adr/0008-configuracion-toml.md)).
