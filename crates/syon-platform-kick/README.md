# syon-platform-kick

Adaptador de Kick (F2, [diseño 002](../../docs/designs/002-plataformas-kick-twitch.md)). Por ahora solo
traduce lo que entrega el feed en tiempo real a eventos del dominio; el OAuth y la conexión llegan
en los parches siguientes de la fase. Migra `events.rs` del prototipo
(`prototypes/kick-console`, [WT-0.1.0_01](../../docs/walkthroughs/WT-0.1.0_01.md)).

## Qué hace

- `event_from_publication(publicación, recibido_en)` interpreta una publicación del feed
  (`push.pub`) y devuelve un `syon_domain::EventKind`, o `None` si no trae el nombre de un evento.
- Traduce chat, borrado de mensaje, ban o timeout, suscripción, regalo de suscripciones, recompensa
  canjeada, total de seguidores y directo iniciado o terminado.
- El dato interno puede ser un objeto JSON o una cadena con JSON; el nombre del evento se acepta con
  o sin espacio de nombres (`App\Events\…`).
- Todo campo se lee de forma tolerante. Un evento desconocido, con campos obligatorios ausentes o con
  un contenido que no es JSON llega como `EventKind::Unknown { name }`, sin su contenido original.
- El chat conserva el id del mensaje, que el prototipo descartaba; así un borrado puede referirse a
  él. Si el feed no trae el id se fabrica uno (`autor@milisegundos`) para no perder el mensaje, y un
  borrado posterior no lo encontrará.
- Los números enormes se saturan a `u32` en lugar de fallar.

## Qué no hace

- No abre conexiones, no autentica y no publica en el bus: eso llega en los parches de OAuth y del
  feed.
- No lee el reloj: quien llama pasa la hora de recepción (el dominio no la consulta).
- No conserva el contenido original de un evento desconocido.
- Los nombres de campo salen del prototipo y del feed no oficial; no se han contrastado en vivo.

## API pública

| Elemento | Para qué sirve |
|----------|----------------|
| `event_from_publication(&Value, Timestamp)` | Traduce una publicación del feed a un `EventKind` |

Dependencias: `syon-domain` y `serde_json`.
