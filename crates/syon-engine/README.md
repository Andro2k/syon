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
- `AppPaths`: carpetas de configuración, datos y caché y las rutas de `settings.toml` y `syon.log`
  ([ADR-0009](../../docs/adr/0009-rutas-de-datos.md)). Es el único lugar que consulta el sistema operativo.

Llegarán en F1 el logging con archivo acotado y la conexión de
[`syon-storage-toml`](../syon-storage-toml/README.md).

## Qué no hace

- `publish` nunca espera: un consumidor lento pierde sus propios eventos, queda contado y
  registrado, y no frena a quien publica ni a los demás.
- No decide qué hace cada consumidor con un evento; eso es de los módulos que se suscriben.
- `AppPaths` no crea carpetas: las crea quien escribe el archivo.
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
| `config_dir`, `data_dir`, `cache_dir`, `settings_file`, `log_file` | Accesos a las rutas resueltas |
| `home_dir()` | Carpeta personal (`Some` solo con `from_system`); la usa el registro para ocultarla |
