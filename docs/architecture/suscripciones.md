# Presupuesto de suscripciones por plataforma

Cuántas conexiones y suscripciones abre SYON en cada plataforma y contra qué límites conocidos se
mide. Cierra el criterio de salida de F2 del [roadmap](../roadmap.md) y concreta la sección 9 del
[diseño 002](../designs/002-plataformas-kick-twitch.md). Hoy cada plataforma escucha **un solo canal**:
el de `[kick] channel` en Kick y el de quien inicia sesión en Twitch
([diseño 003](../designs/003-arranque-de-plataformas-y-binario.md)).

## Resumen

| Plataforma | Conexiones | Suscripciones con un canal | Límite conocido | Estado del dato |
|------------|-----------|----------------------------|-----------------|-----------------|
| Kick (feed no oficial) | 1 WebSocket | 4 feeds | Sin límite documentado | Sale del código y del prototipo, no de documentación de Kick |
| Twitch (EventSub por WebSocket) | 1 WebSocket | 8 suscripciones | 300 activas por conexión, 3 conexiones por usuario y cliente, `max_total_cost` de 10 con token de usuario | Límites tomados de la documentación de Twitch; el coste real **sin confirmar en vivo** |

## Kick

El feed no usa OAuth ([ADR-0015](../adr/0015-conexion-al-feed-de-kick.md)): `KickConnector` abre una
sola conexión y se suscribe a cuatro feeds de lectura pública (`feed_names` en
`crates/syon-platform-kick/src/frames.rs`):

| Feed | Contenido |
|------|-----------|
| `chatrooms.{id}.v2` | Chat y moderación |
| `chatroom_{id}` | Regalos y recompensas |
| `channel.{id}` | Estado del directo |
| `channel_{id}` | Seguidores y Kicks |

Los feeds `private-*` exigen sesión de usuario y no se piden. Kick no documenta límites para este canal,
así que no hay un techo que medir; la cuenta es una conexión y cuatro feeds por canal.

## Twitch

`TwitchConnector` abre una conexión de EventSub y crea las ocho suscripciones de `SUBSCRIPTIONS`
(`crates/syon-platform-twitch/src/subscriptions.rs`), todas sobre el canal de quien inició sesión
([ADR-0017](../adr/0017-conexion-a-eventsub-de-twitch.md)):

| Suscripción | Versión | Permiso del token | Se traduce a |
|-------------|---------|-------------------|--------------|
| `channel.chat.message` | 1 | `user:read:chat` | `Chat` |
| `channel.chat.message_delete` | 1 | `user:read:chat` | `ChatMessageDeleted` |
| `channel.chat.notification` | 1 | `user:read:chat` | `Subscription`, `GiftedSubs` |
| `channel.ban` | 1 | `channel:moderate` | `UserBanned` |
| `channel.channel_points_custom_reward_redemption.add` | 1 | `channel:read:redemptions` | `RewardRedeemed` |
| `channel.follow` | 2 | `moderator:read:followers` | `Followed` |
| `stream.online` | 1 | ninguno | `StreamStarted` |
| `stream.offline` | 1 | ninguno | `StreamEnded` |

Con un canal se usan 8 de las 300 suscripciones activas que admite una conexión y 1 de las 3
conexiones por usuario y cliente.

**Pendiente de confirmar en vivo.** El coste de cada suscripción depende de que la persona haya
autorizado la aplicación con el permiso correspondiente; las muestras de la documentación traen
`cost: 0` en esos casos, pero no se ha visto con la aplicación real. El conector lee `cost`,
`total_cost` y `max_total_cost` de la respuesta al crear cada suscripción y, al conectar, los escribe en
el registro (`syon.log`, nivel `info`) en una línea como `twitch: conectado a <canal> con 8 de 8
suscripciones (coste …; total de la aplicación … de …)`. Esa línea es el dato que falta para dar por
cerrado el presupuesto de Twitch frente al `max_total_cost` de 10.

## Con varios canales (F3 en adelante)

Aritmética sobre los números de arriba, sin verificar contra las plataformas:

- Kick: 4 feeds por canal en la misma conexión, es decir `4 × N` para `N` canales.
- Twitch: 8 suscripciones por canal, `8 × N`. Quien limita antes no es el tope de 300 por conexión sino
  el `max_total_cost` de 10, y solo si las suscripciones tienen coste; por eso la línea del registro
  anterior decide cuántos canales caben.

Hoy Twitch escucha solo el canal del usuario autenticado, así que `N` vale 1 y no hay que decidir nada
más hasta que una fase posterior admita otros canales.
