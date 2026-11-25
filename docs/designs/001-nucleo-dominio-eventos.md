# Diseño 001: núcleo de F1 (dominio, bus de eventos, configuración y logging)

- Estado: Aceptado (2026-11-25)
- Fase: F1
- Fecha: 2026-11-24
- Revisión: 2026-11-25, tras contrastar el diseño con Sonora (sección 16)

## 1. Objetivo y alcance

Construir el núcleo sobre el que se apoyan todas las fases siguientes:

1. Tipos de dominio neutrales a la plataforma (`syon-domain`).
2. Un bus de eventos que reparte lo que publican las plataformas a los módulos
   (comandos, TTS, overlays...) sin que ninguno conozca al otro.
3. Configuración persistente en TOML detrás de un puerto.
4. Registro de diagnóstico en consola y en archivo.
5. La regla de dependencias entre crates, comprobada automáticamente.

Fuera de alcance: adaptadores de Kick y Twitch (F2), roles y comandos (F3),
credenciales y tokens (F2), cualquier interfaz (F9).

## 2. Punto de partida

El prototipo (`prototypes/kick-console`) ya interpreta los eventos de Kick en
`KickEvent`, pero mezcla cosas que el núcleo debe separar:

- Los tipos de evento viven junto al JSON de Kick (`serde_json::Value`).
- El `Chat` no guarda el id del mensaje, aunque `MessageDeleted` lo necesita
  para referirse a él.
- No hay marca de tiempo en el evento: la consola pone la hora al imprimir.
- Un único consumidor recibe los eventos por un canal de 256 posiciones.
- `Settings` mezcla `client_secret` con ajustes; los tokens van a un JSON aparte.

Este diseño toma de ahí lo que sirve y deja fuera lo específico de Kick.

## 3. Crates de F1 y dependencias permitidas

```text
syon-domain      ← syon-app      ← syon-engine
                       ↑               ↑ (ensambla)
                  syon-storage-toml ───┘
```

| Crate | Propósito | Puede depender de |
|-------|-----------|-------------------|
| `syon-domain` | Tipos puros: eventos, usuarios, marca de tiempo. Sin I/O ni async | ningún crate del workspace; ninguna dependencia externa |
| `syon-app` | Puertos (traits) y casos de uso | `syon-domain` |
| `syon-storage-toml` | Adaptador del puerto de ajustes sobre archivos TOML | `syon-domain`, `syon-app` |
| `syon-engine` | Ensambla: implementa el bus, inicia el logging y conecta los adaptadores | todos los anteriores |

Reglas: las flechas solo van hacia la izquierda; un adaptador nunca depende de
otro adaptador; nada depende de `syon-engine` salvo el binario y la UI. Solo
`syon-engine` y el binario nombran adaptadores concretos; el resto ve únicamente
traits y modelos.

## 4. Modelo de dominio

Derivado de `KickEvent`, sin tipos de red y con el id y la hora que faltaban:

```rust
pub enum Platform { Kick, Twitch }

pub struct User { pub name: String }

/// Milisegundos desde el epoch Unix, en UTC. Sin librería de tiempo.
pub struct Timestamp(i64);

pub struct Event {
    pub platform: Platform,
    pub received_at: Timestamp,
    pub kind: EventKind,
}

pub enum EventKind {
    Chat(ChatMessage),
    ChatMessageDeleted { id: MessageId },
    UserBanned { user: User, by: Option<User>, permanent: bool },
    Subscription { user: User, months: u32 },
    GiftedSubs { gifter: User, count: u32 },
    RewardRedeemed { user: User, title: String, input: Option<String> },
    FollowersUpdated { count: u64 },
    StreamStarted { title: Option<String> },
    StreamEnded,
    Unknown { name: String },
}

pub struct ChatMessage { pub id: MessageId, pub author: User, pub text: String }
pub struct MessageId(String);
```

Correspondencia con el prototipo:

| `KickEvent` | `EventKind` | Observación |
|-------------|-------------|-------------|
| `Chat` | `Chat` | Añade `id`; F2 debe extraerlo del feed |
| `MessageDeleted` | `ChatMessageDeleted` | Igual |
| `UserBanned` | `UserBanned` | `permanent = false` es un timeout; la duración no la entrega el prototipo |
| `Subscription` | `Subscription` | Igual |
| `GiftedSubs` | `GiftedSubs` | `usize` pasa a `u32` |
| `Reward` | `RewardRedeemed` | Igual |
| `Followers` | `FollowersUpdated` | Es un total, no un seguidor nuevo; F2 comprueba si el feed oficial entrega el nombre |
| `StreamLive` / `StreamOffline` | `StreamStarted` / `StreamEnded` | Igual |
| `Other { name, data }` | `Unknown { name }` | El JSON original se queda en el adaptador (log de depuración) |

Decisiones de modelado:

- La hora es la de **recepción** en SYON. Si una plataforma entrega la suya, se
  evalúa en F2 y se añade un campo, no se reemplaza este.
- `Timestamp` se crea desde `std::time::SystemTime` con una conversión pura; la
  llamada a `SystemTime::now()` la hace el adaptador, así el dominio no lee el reloj.
- Roles (emisor, moderador, suscriptor) y el id de usuario no se modelan en F1:
  el prototipo no los extrae y los necesitan F3 y F4. Se añadirán al leer los
  feeds reales, no por adelantado.
- `Platform` incluye `Twitch` desde ya porque es parte del producto, aunque su
  adaptador llegue en F2.

## 5. Puertos (en `syon-app`)

```rust
/// Por donde las plataformas entregan eventos. Nunca bloquea.
pub trait EventSink: Send + Sync {
    fn publish(&self, event: Event);
}

/// Ajustes persistentes del usuario.
pub trait SettingsStore: Send + Sync {
    fn load(&self) -> Result<Settings, SettingsError>;
    fn save(&self, settings: &Settings) -> Result<(), SettingsError>;
}
```

`EventSink` tiene al menos tres implementaciones previstas (el bus real, un
colector para pruebas y, en F2, los dos adaptadores como productores que solo
lo conocen a él), lo que justifica el trait. `SettingsStore` se justifica por
las pruebas sin disco y por conservar la separación datos/lógica de MiniKick
(`IConfigProvider` + `TomlConfigProvider`).

## 6. Bus de eventos (en `syon-engine`)

El chat es el centro del sistema: un consumidor lento no debe afectar a los
demás ni a quien publica. Por eso el comportamiento ante lentitud es explícito.

Comportamiento propuesto:

- Cada consumidor se suscribe con un nombre y una capacidad, y recibe su propio
  canal acotado (`tokio::sync::mpsc`) de `Arc<Event>`; el evento no se copia por
  consumidor.
- `publish` usa `try_send` y nunca espera. Si el canal de un consumidor está
  lleno, **descarta el evento nuevo solo para ese consumidor**, incrementa su
  contador de descartes y registra un aviso (como máximo uno por segundo y por
  consumidor).
- No existe una política de bloqueo: detendría a todas las plataformas por culpa
  de un consumidor lento.
- `stats()` devuelve por consumidor: nombre, capacidad, eventos en cola y
  descartados, para diagnóstico y para el logging.
- Un consumidor que no puede perder eventos (comandos, recompensas) se suscribe
  con más capacidad y su contador de descartes se trata como un fallo visible.

Alternativas consideradas:

| Alternativa | Por qué no |
|-------------|-----------|
| `tokio::sync::broadcast` | Más simple, pero un consumidor lento pierde eventos de forma silenciosa para el código (solo se entera con `Lagged`) y una única capacidad sirve a todos por igual |
| Canal con bloqueo (`send().await`) | Un consumidor lento frena a Kick y a Twitch |
| Pipeline secuencial de procesadores | Acopla los módulos entre sí, justo lo que se quiere evitar |

Solo hay una política (descartar el nuevo); una segunda, como descartar el más
antiguo, se añade cuando un consumidor concreto la necesite (YAGNI).

## 7. Configuración
- Archivo `settings.toml` con `schema_version`. F1 define el mecanismo y la
  sección `[logging]`; cada fase añade sus secciones.
- **Preferencias y estado de ejecución van en almacenes distintos.** Las
  preferencias son lo que el usuario cambia a propósito (`settings.toml`); el
  estado de ejecución es lo que la app cambia sola (posición de la ventana,
  contadores de un temporizador). Así arrastrar un panel no reescribe las
  preferencias. F1 solo implementa el primero; el segundo se crea con su primer
  caso de uso.
- Las claves desconocidas se ignoran, para que una versión nueva del archivo no
  rompa una versión anterior de SYON.
- Archivo ausente: se usan los valores por defecto y no se escribe nada hasta
  el primer `save`. Archivo ilegible o con tipos incorrectos: error que indica
  la ruta y la clave, sin sobrescribir el archivo.
- `save` escribe a un archivo temporal en la misma carpeta y lo renombra, para
  no dejar un TOML a medias si el proceso se corta. El guardado diferido
  (debounce) llega con la primera fuente de cambios frecuentes, la UI de F9; en
  F1 `save` es inmediato.
- Los secretos no van aquí. Credenciales y tokens siguen en `.env` y en el
  archivo de tokens (F2); el tipo que contenga un secreto implementa `Debug` a
  mano para ocultarlo.
- No hay migraciones entre versiones de esquema hasta que exista una segunda.

### Ubicación de los datos

- Solo `syon-engine` consulta al sistema operativo con la crate `etcetera` y
  entrega las rutas por constructor a los adaptadores; así las pruebas usan
  carpetas temporales y no hace falta una variable de entorno de ubicación.
- `settings.toml` va en el directorio de configuración; el log y el estado de
  ejecución, en el de datos; la caché, en el de caché. La correspondencia exacta
  en Windows y Linux se fija en el ADR 0009 tras probarla en ambos.
- Se prefiere `etcetera` a `dirs` porque `dirs` arrastra `option-ext`, con
  licencia MPL-2.0 (comprobado con `cargo deny`), y el ADR-0004 exige un ADR
  explícito para cualquier licencia copyleft. `etcetera` es MIT o Apache-2.0 y
  su único paquete adicional en Linux es `cfg-if`.

## 8. Registro de diagnóstico
Se sigue el patrón de Sonora: `log` con `env_logger` y un escritor de archivo
propio, en lugar de `tracing`. Es lo más simple que cubre lo necesario y evita
archivos con fecha y dependencias de tiempo transitivas.

- **Archivo único:** `syon.log` con un tope de 16 MB; al superarlo pasa a
  `syon.log.1` y se empieza otro. Se conservan las últimas 5 ejecuciones y cada
  arranque escribe una línea de marca (`logging: syon <versión> started`) para
  distinguirlas. Son valores iniciales ajustables en el ADR 0006.
- **Dos filtros:** la consola muestra `warn` por defecto; el archivo, más
  detalle por subsistema. `SYON_LOG` reemplaza el filtro del archivo.
- **Redacción:** todo registro pasa por una función `redact` que elimina tokens,
  secretos, códigos de OAuth y la carpeta personal, para que `syon.log` se pueda
  adjuntar a un reporte. Es una red de seguridad: la regla sigue siendo no
  registrar secretos.
- **Pánicos:** un hook de pánico escribe el mensaje, el hilo y el backtrace (si
  `RUST_BACKTRACE` lo habilita) en el log antes del hook por defecto.
- **Mensajes:** con el prefijo del subsistema (`"bus: …"`). Se registran los
  descartes del bus, las reconexiones y los errores de ajustes. El texto
  completo del chat solo aparece en `debug`.
- La inicialización vive en `syon-engine`; los demás crates solo usan las macros
  de `log`.

Coste aceptado: se pierden los spans de `tracing`. Si más adelante hace falta
seguir una tarea asíncrona de extremo a extremo, `tracing` se puede añadir con
su puente a `log` sin tocar los puntos de registro.

## 9. Tiempo
- No se adopta una librería de tiempo directa en F1: el dominio usa `Timestamp`
  y la consola del prototipo conserva `chrono` hasta que se elimine.
- Cuando una capa de presentación necesite horas locales (overlays o UI) se usa
  `jiff`, con su ADR: `env_logger` con sus funciones por defecto ya lo trae
  (jiff 1.0.3, MIT o Unlicense, ya en 1.x), y Sonora también lo usa.
- Se descartan `time` (0.3.55, pre-1.0, requiere Rust 1.88) y `chrono` (sin
  mantenimiento, RUSTSEC-2026-0543).

## 10. Comprobación de la arquitectura
`cargo xtask arch-check` lee `cargo metadata` y falla si:

1. `syon-domain` tiene dependencias externas o de otros crates del workspace.
2. `syon-app` depende de algo distinto de `syon-domain`.
3. Un adaptador depende de otro adaptador o de `syon-engine`.
4. Algún crate, salvo el binario y la UI, depende de `syon-engine`.
5. Algún crate, salvo `syon-engine` y el binario, depende de un adaptador.

Las reglas se expresan como una tabla en `xtask`, y las pruebas incluyen un
ejemplo que debe fallar para cada regla. Se ejecuta en CI junto a `docs-check`.

## 11. Dependencias nuevas
| Dependencia | Crate | Motivo | ADR |
|-------------|-------|--------|-----|
| tokio (solo `sync`) | `syon-engine` | Canales acotados del bus | 0007 |
| log, env_logger | `syon-engine` | Registro a consola y archivo | 0006 |
| toml, serde (`derive`) | `syon-storage-toml` | Leer y escribir `settings.toml` | 0008 |
| thiserror | `syon-app`, `syon-storage-toml` | Errores tipados de los puertos | 0008 |
| etcetera | `syon-engine` | Directorios de configuración, datos y caché | 0009 |
| anyhow | `syon-engine` | Errores con contexto en el ensamblado | 0006 |

Se comprobó que todas las licencias de este conjunto (con `jiff` como
transitiva de `env_logger`) están permitidas por el `deny.toml` actual. Los ADR
0006 a 0009 se escriben en los parches de implementación, con su actualización
de `docs/deps.md`.

## 12. Plan de parches de implementación
1. `feat(domain)`: crate `syon-domain` con los tipos y sus pruebas; el
   workspace pasa a `members = ["crates/*", "xtask"]` y a
   `[workspace.dependencies]`.
2. `feat(app)`: crate `syon-app` con `EventSink` y `SettingsStore`.
3. `feat(engine)`: bus de eventos con descartes y estadísticas, y ADR 0007.
4. `feat(storage)`: `syon-storage-toml` con escritura atómica, y ADR 0008.
5. `feat(engine)`: rutas de datos con `etcetera`, y ADR 0009.
6. `feat(engine)`: logging con archivo acotado, redacción y hook de pánico, y
   ADR 0006.
7. `build(xtask)`: `arch-check` y su paso en CI.
8. `docs`: `docs/deps.md` y walkthrough `WT-0.2.0_01`.

## 13. Criterio de salida de F1

- `syon-domain` compila sin dependencias y sus pruebas pasan.
- El bus entrega a varios consumidores, descarta con contador y nunca bloquea al
  publicador (prueba con un consumidor detenido).
- `settings.toml` hace ida y vuelta, tolera claves desconocidas y no sobrescribe
  un archivo corrupto.
- `syon.log` se crea con tope y rotación a `syon.log.1`, marca cada arranque,
  registra los pánicos y `redact` elimina tokens y secretos (con pruebas).
- `cargo xtask arch-check` falla con cada una de las violaciones de la sección 10.
- Todo en verde en CI, incluido `cargo deny`.

## 14. Decisiones abiertas
1. **Bus:** se mantiene la propuesta (canales por consumidor, descartar el nuevo
   y contar). En lo revisado de Sonora el estado vive en entidades de GPUI, así
   que no hay un contraste directo: SYON necesita un engine sin interfaz para
   servir los overlays.
2. **Logging a archivo:** resuelta. Un único `syon.log` con escritor propio,
   como Sonora (sección 8).
3. **Ubicación de los datos:** resuelta. Directorios del sistema con `etcetera`
   y rutas por constructor (sección 7).
4. **Roles y id de usuario:** se difieren a F3 y F4.

El autor aceptó el diseño el 2026-11-25; la implementación sigue el plan de la
sección 12, un parche por vez.

## 15. Riesgos

- Los campos del feed no oficial de Kick pueden cambiar; el id del mensaje y el
  nombre del seguidor no están contrastados en vivo (ver `WT-0.1.0_01`).
- Descartar eventos bajo saturación pierde información; la mitigación es el
  contador, el aviso y la capacidad por consumidor, no evitar el descarte.
- Cuatro crates nuevos de golpe cuestan configuración; el resto se crea en su
  fase (ADR-0003).
- El escritor de log propio es código que mantener (unas decenas de líneas);
  se acota con pruebas de rotación y de redacción.
- `redact` no puede garantizar que ningún secreto se cuele: es una red de
  seguridad, no una autorización para registrarlos.
- La correspondencia de `etcetera` con las carpetas de Windows no está probada
  todavía; se verifica en el parche 5.

## 16. Referencias

- [Sonora](https://github.com/sonorahq/sonora) (GPL-3.0-or-later), solo como
  referencia de diseño: logging con archivo acotado y redacción, preferencias
  separadas del estado de ejecución, `webview` propio para el PO token (F7) y
  dependencias centralizadas en el workspace. No se copia código.
