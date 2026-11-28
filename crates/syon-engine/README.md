# syon-engine

Ensamblado de SYON: conecta los puertos de [`syon-app`](../syon-app/README.md) con
implementaciones reales y corre sin interfaz. Es el único crate, junto con el binario, que
puede nombrar adaptadores concretos (ver
[ADR-0003](../../docs/adr/0003-arquitectura-hexagonal-workspace.md) y el
[diseño 001](../../docs/designs/001-nucleo-dominio-eventos.md)).

## Qué hace

- `EventBus`: implementa `EventSink` y reparte cada evento a todos los consumidores, cada uno con
  su propio canal acotado de `Arc<Event>` ([ADR-0007](../../docs/adr/0007-bus-de-eventos.md)).
- `SubscriberStats`: nombre, capacidad, eventos en cola y descartados de cada consumidor.
- Automatización de chat: una cola acotada y un consumidor secuencial aplican comandos configurados,
  roles, filtros, deduplicación y límites antes de responder. El eco propio se correlaciona por autor,
  id de envío y texto pendiente, sin registrar contenido de chat.
- `ChatHandle::send_async` permite enviar desde tareas del runtime; `send` conserva el uso síncrono
  desde fuera de él.
- `AppPaths`: carpetas de configuración, datos y caché y las rutas de `settings.toml` y `syon.log`
  ([ADR-0009](../../docs/adr/0009-rutas-de-datos.md)). Es el único lugar que consulta el sistema operativo.

- Registro de diagnóstico ([ADR-0006](../../docs/adr/0006-registro-de-diagnostico.md)): la consola
  muestra `warn` y `syon.log` (16 MB, pasa a `syon.log.1`) el nivel de `[logging].level`, que `SYON_LOG`
  reemplaza. Conserva las últimas 5 ejecuciones, escribe una línea de marca por arranque, pasa cada línea
  por el redactor de secretos y registra los pánicos.

- `Engine`: el arranque ([ADR-0010](../../docs/adr/0010-arranque-del-engine.md)). Lee `settings.toml` con
  [`syon-storage-toml`](../syon-storage-toml/README.md), inicia el registro con el nivel de los ajustes y
  crea el bus. Un archivo de ajustes inválido se aparta como `settings.toml.invalid` y se usan los valores
  por defecto; nunca se guarda nada encima.

Todavía no hay binario que llame a `Engine::start`; el crate `syon` se crea con su fase.

## Qué no hace

- `publish` nunca espera: un consumidor lento pierde sus propios eventos, queda contado y
  registrado, y no frena a quien publica ni a los demás.
- No decide qué hace cada consumidor con un evento; eso es de los módulos que se suscriben.
- `AppPaths` no crea carpetas: las crea quien escribe el archivo.
- `Engine::start` no guarda ajustes al arrancar, ni tras un archivo inválido ni con el archivo ausente, y
  solo puede llamarse una vez por proceso (el registro es global).
- El redactor es una red de seguridad: reconoce secretos por el nombre de su clave y no detecta un valor
  suelto. La regla sigue siendo no registrar secretos.
- No contiene lógica de negocio ni conoce el formato de ninguna plataforma.

## API pública

| Elemento | Para qué sirve |
|----------|----------------|
| `EventBus::new()` | Crea un bus; es barato de clonar y las copias comparten consumidores |
| `EventBus::subscribe(nombre, capacidad)` | Devuelve un `mpsc::Receiver<Arc<Event>>` propio; al soltarlo se retira |
| `EventBus::stats()` | `Vec<SubscriberStats>` de los consumidores activos |
| `impl EventSink for EventBus` | Punto de entrada de los productores |
| `AppPaths::from_system()` | Resuelve las carpetas con las convenciones del sistema; error `PathsError` si no hay carpeta personal |
| `AppPaths::under(raíz)` | Reparte `config`, `data` y `cache` bajo una raíz, sin consultar el sistema (pruebas) |
| `Engine::start(paths, versión)` | Lee los ajustes, inicia el registro y crea el bus; error `StartError` si el registro falla |
| `Engine::paths()`, `settings()`, `bus()` | Accesos a lo que el engine arrancó |
| `init_logging(&AppPaths, &LoggingConfig)` | Instala el registro y el hook de pánico; una sola vez por proceso (`LoggingError::AlreadyInitialized`) |
| `LoggingConfig::from_env(&LoggingSettings, versión)` | Nivel de los ajustes y filtro de `SYON_LOG`; sus campos son públicos para quien no use el entorno |
| `config_dir`, `data_dir`, `cache_dir`, `settings_file`, `log_file` | Accesos a las rutas resueltas |
| `home_dir()` | Carpeta personal (`Some` solo con `from_system`); la usa el registro para ocultarla |
