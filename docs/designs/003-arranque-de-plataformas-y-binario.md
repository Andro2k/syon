# Diseño 003: arranque de plataformas y binario (cierre de F2)

- Estado: Aceptado (2026-11-27); propuesto el 2026-11-26
- Fase: F2
- Fecha: 2026-11-26

## 1. Objetivo y alcance

Cerrar F2 por el lado de SYON: que existan los ajustes, el cableado del engine y un binario ejecutable que
arranque los conectores y muestre los eventos. Cubre el punto 11 del plan del
[diseño 002](002-plataformas-kick-twitch.md) y un hueco que ese plan no recogía: nadie construía ni
registraba los conectores, y tampoco había de dónde sacar el canal ni las credenciales.

**Dentro:** ajustes de Kick y de Twitch, ruta del archivo de credenciales, credenciales de la aplicación
([ADR-0018](../adr/0018-credenciales-de-aplicacion-incrustadas.md)), arranque y parada de conectores y
`login` en el engine, y el crate `syon`.

**Fuera:** el crate de Twitch, que ya está implementado (`syon-platform-twitch`, puntos 9 y 10 del plan del
diseño 002; [ADR-0016](../adr/0016-oauth-y-api-de-twitch.md) y
[ADR-0017](../adr/0017-conexion-a-eventsub-de-twitch.md)) y aquí solo se cablea; la interfaz gráfica (F9), el
envío de mensajes (F3) y el instalador (F10).

## 2. Punto de partida

| Existe | Falta |
|--------|-------|
| `Engine::start`: ajustes, registro, bus y runtime ([ADR-0010](../adr/0010-arranque-del-engine.md), [ADR-0011](../adr/0011-runtime-asincrono-del-engine.md)) | Nadie construye conectores |
| `Engine::add_connector` y `connection_status` | De dónde sale el canal |
| `KickConnector::start(handle, canal, sink)`: feed no oficial, **sin OAuth** | — |
| `OAuthClient` y `TokenManager` de Kick | Quién los usa y con qué `client_id` y `client_secret` |
| `TwitchConnector::start(handle, Arc<TokenManager>, sink)`, que exige la sesión **ya iniciada** | Quién deja lista esa sesión: `TokenManager::start(oauth, almacén, on_url)` puede pedir un inicio de sesión interactivo |
| `TomlCredentialStore` ([ADR-0012](../adr/0012-almacen-de-credenciales.md)) | Una ruta para `credentials.toml` en `AppPaths` |
| `Settings` con `[logging]` | La sección `[kick]` |
| — | El crate `syon` |

El `EventBus` ya implementa `EventSink`, así que es el destino directo de los conectores.

Para recibir eventos de Kick no hace falta ninguna credencial. Sí hacen falta para iniciar sesión (escribir
en el chat, F3) y para Twitch, cuyo EventSub exige un token de usuario y una sesión iniciada antes de
conectar. Twitch escucha solo el canal del usuario que inicia sesión (el `user_id` que devuelve `validate`).

## 3. Credenciales de la aplicación

Decisión del autor, detallada en el [ADR-0018](../adr/0018-credenciales-de-aplicacion-incrustadas.md): van
incrustadas en el binario, inyectadas al compilar con variables de entorno que nunca se versionan. El
repositorio es público.

```text
SYON_<PLATAFORMA>_CLIENT_ID / _CLIENT_SECRET
        │  option_env! (compilación)      std::env::var (ejecución, gana)
        ▼                                         ▼
   crates/syon  ──  resolve(ejecución, incrustado) ──►  AppCredentials  ──►  Engine
```

Solo el binario toca el entorno. El engine y los adaptadores reciben `AppCredentials` por constructor.

## 4. Ajustes nuevos

```toml
[kick]
enabled = true
channel = "mi_canal"
# Opcionales: con los dos se evita consultar a Kick, que a veces responde 403.
channel_id = 123
chatroom_id = 456
```

- Sección ausente: `enabled = false` y canal vacío. Un archivo de F1 sigue valiendo.
- `enabled = true` sin `channel` no es un fallo de arranque: el engine lo registra como aviso, no crea el
  conector y lo informa en el resultado de `start_platforms`.
- `schema_version` sigue en 1: añadir secciones con valores por defecto es compatible, porque las claves
  desconocidas se ignoran ([ADR-0008](../adr/0008-configuracion-toml.md)).
- Los tipos viven en `syon-app` (`KickSettings`) y el formato en `syon-storage-toml`. `client_id` y
  `client_secret` no van aquí.
- Twitch escucha solo el canal de quien inicia sesión, así que `[twitch]` solo tiene `enabled`: no hay
  `channel`. Se añade con el parche que cablea su conector.

## 5. API del engine

```rust
/// Credenciales de la aplicación de una plataforma. `Debug` no imprime el secreto.
pub struct AppCredentials { pub client_id: String, pub client_secret: String }

/// Las de cada plataforma; `None` si esta compilación no las incluye.
pub struct PlatformCredentials { pub kick: Option<AppCredentials>, pub twitch: Option<AppCredentials> }

impl Engine {
    /// Construye y registra el conector de cada plataforma activada en los ajustes. Devuelve, por
    /// plataforma, si arrancó o por qué no. Kick no necesita credenciales para recibir eventos. Para
    /// Twitch deja lista la sesión antes de conectar: si no hay una válida guardada inicia sesión,
    /// `on_login_url` recibe el enlace y el arranque espera a que la persona autorice. Sin credenciales
    /// de la aplicación, Twitch no arranca y el resultado lo dice.
    pub fn start_platforms(
        &mut self,
        credentials: &PlatformCredentials,
        on_login_url: impl Fn(Platform, &str),
    ) -> Vec<PlatformStart>;

    /// Detiene todos los conectores.
    pub fn stop_platforms(&self);

    /// Inicia sesión OAuth, o reutiliza o renueva la guardada. Bloquea: se llama desde fuera del runtime.
    pub fn login(
        &self,
        platform: Platform,
        credentials: &AppCredentials,
        on_url: impl FnOnce(&str),
    ) -> Result<LoginOutcome, LoginError>;
}

/// Cómo se obtuvo la sesión: `Restored`, `Refreshed` o `NewLogin`.
pub enum LoginOutcome { /* … */ }
```

`LoginOutcome` es un tipo propio del engine para que el binario no vea los tipos de los adaptadores.
`AppPaths::credentials_file()` devuelve `<carpeta de datos>/credentials.toml`: los tokens son estado, no
preferencias, y así quedan junto a `syon.log` y fuera de la carpeta de configuración.

## 6. El binario `syon`

Un crate `crates/syon` que depende de `syon-engine` y de `syon-domain` (para dar formato a los eventos). Tiene el papel `Binary` de `arch-check`.

| Comando | Qué hace |
|---------|----------|
| `syon` (o `syon run`) | `Engine::start`, arranca las plataformas activadas (si Twitch no tiene sesión válida imprime el enlace y espera), imprime los eventos y espera a Ctrl+C |
| `syon login <kick\|twitch>` | Imprime el enlace de autorización, espera el callback y guarda los tokens; sirve para dejar la sesión lista antes de `run` |
| `syon --version` y `--help` | Versión (`CARGO_PKG_VERSION`, que también recibe `Engine::start`) y uso |

- **Sin `clap`.** Son tres comandos; el análisis manual es más corto que la dependencia.
- **Fallos de arranque** (`StartError`): mensaje en `stderr`, porque puede no haber registro todavía, y código
  de salida 1. Sin plataformas activadas: se dice qué archivo editar y sale con 1. Uso incorrecto: 2.
- **Salida de eventos:** una línea por evento, `<hora> <plataforma> <resumen>`, a `stdout`. El formato no es
  una API estable: la interfaz de F9 lo sustituye.
- **Texto de terceros.** Los mensajes de chat los escribe cualquiera, así que se eliminan los caracteres de
  control (incluidas las secuencias de escape de terminal) antes de imprimir. Con prueba.
- **Ctrl+C:** `tokio::signal::ctrl_c` sobre el runtime del engine, y después `stop_platforms`.
- **Credenciales:** `credentials.rs` con la función pura `resolve(ejecución, incrustado)` y los mensajes de
  ausencia. `build.rs` con `rerun-if-env-changed`.
- En Windows se queda con el subsistema de consola hasta F9.

## 7. Dependencias

Ningún crate nuevo. El binario añade a `tokio` la función `signal`, que el workspace no tiene activada
(`default-features = false`); se anota en `docs/deps.md`.

## 8. Plan de parches (nuestra parte)

1. `docs(design)`: este diseño y el ADR-0018.
2. `feat(app)`: `KickSettings` en `Settings` y `[kick]` en `syon-storage-toml`, con pruebas de que un archivo
   sin la sección y uno con claves desconocidas siguen cargando.
3. `feat(engine)`: `AppPaths::credentials_file()`.
4. `feat(engine)`: `AppCredentials`, `start_platforms` y `stop_platforms` (Kick).
5. `feat(engine)`: `Engine::login` (Kick).
6. `feat(syon)`: el crate binario con `run`, `login`, resolución de credenciales, `build.rs` y la salida de
   eventos; entrada en `docs/deps.md`.
7. `feat(engine)` y `feat(syon)`: cablear Twitch (`[twitch] enabled`, la sesión en `start_platforms` y
   `login`). Su crate ya está entregado, así que puede hacerse justo después de Kick.
8. `docs`: `docs/architecture/suscripciones.md` (Kick ahora, Twitch al llegar), tabla de paridad y el
   walkthrough que cierra F2.

## 9. Criterio de salida de esta parte

- `syon --version` imprime la versión, y `syon` sin plataformas activadas lo dice y sale con 1.
- Con `[kick]` activado y un canal, el engine registra el conector y `connection_status()` lo refleja;
  una prueba lo arranca y lo detiene sin esperar a la red.
- Twitch deja la sesión lista antes de conectar y, sin las credenciales de la aplicación, no arranca y lo
  dice.
- Un archivo de ajustes de F1 (sin `[kick]`) y uno con claves desconocidas siguen cargando.
- `resolve` cubre la precedencia (ejecución, incrustado, ausente, en blanco) y el binario compila sin
  ninguna variable definida.
- Ningún valor de credencial aparece en un `Debug`, en un mensaje de error ni en el registro.
- El texto de chat con secuencias de escape sale sin ellas.
- `arch-check` en verde: el binario solo depende del engine y del dominio.
- Contraste en vivo, pendiente del autor: `syon login kick` con la aplicación real.

## 10. Riesgos y pendientes

- **Secreto extraíble del binario** y términos de las plataformas sin verificar
  ([ADR-0018](../adr/0018-credenciales-de-aplicacion-incrustadas.md)).
- **Los callbacks son fijos:** `http://localhost:8080/auth/callback` en Kick y
  `http://localhost:8081/auth/callback` en Twitch, registrados en las aplicaciones del autor. Si otro
  programa ocupa el puerto en la máquina de quien instala, esa plataforma no puede iniciar sesión. Se
  decide antes de F10 si se admite un puerto alternativo (habría que registrarlos).
- Las pruebas del engine con Kick activado llaman a `KickConnector::start`, que intenta conectar con
  `kick.com`. Se detiene de inmediato y la prueba no espera a la red; si resultara frágil habría que
  inyectar una fábrica de conectores, que hoy sería una abstracción sin segundo uso.
- `schema_version` mayor que el soportado: sigue pendiente de decidir (ver `docs/estado.md`).
- El formato de salida en consola es provisional.

## 11. Decisiones

Se propusieron así y el autor las aceptó el 2026-11-27, con los siete parches de código ya aplicados:

1. **Credenciales de la aplicación incrustadas** e inyectadas al compilar: decidido por el autor
   (2026-11-26), con el detalle del ADR-0018.
2. **Tres comandos sin `clap`.**
3. **Kick arranca sin OAuth**; `login` es un comando aparte. Twitch sí necesita una sesión para conectar: si
   no hay una válida, `run` inicia sesión de forma interactiva, y `login` sirve para dejarla lista antes.
4. **`AppCredentials` y `login` viven en el engine**, para que el binario no dependa de los adaptadores.
5. **`credentials.toml` en la carpeta de datos**, no en la de configuración.
6. **`[twitch]` solo tiene `enabled`**: el canal es el del usuario autenticado. Se añade al cablear su
   conector.
