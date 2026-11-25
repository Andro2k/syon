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

Llegarán en F1 las rutas de datos, el logging con archivo acotado y la conexión de
`syon-storage-toml`.

## Qué no hace

- `publish` nunca espera: un consumidor lento pierde sus propios eventos, queda contado y
  registrado, y no frena a quien publica ni a los demás.
- No decide qué hace cada consumidor con un evento; eso es de los módulos que se suscriben.
- No contiene lógica de negocio ni conoce el formato de ninguna plataforma.

## API pública

| Elemento | Para qué sirve |
|----------|----------------|
| `EventBus::new()` | Crea un bus; es barato de clonar y las copias comparten consumidores |
| `EventBus::subscribe(nombre, capacidad)` | Devuelve un `mpsc::Receiver<Arc<Event>>` propio; al soltarlo se retira |
| `EventBus::stats()` | `Vec<SubscriberStats>` de los consumidores activos |
| `impl EventSink for EventBus` | Punto de entrada de los productores |
