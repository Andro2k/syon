# WT-0.4.2_02: los comandos no respondían en Kick

## Contexto

El 2026-11-28 el autor contrastó en vivo Kick y Twitch en Windows (`syon login`, `syon run`, envío por
consola y el comando `!ping`/`!p` de `settings.toml`). Resultado:

- Los dos inicios de sesión se renovaron, las dos plataformas conectaron y Twitch reportó 8 de 8
  suscripciones con coste 0 (total de la aplicación 0 de 10).
- El envío desde la consola llegó a Kick y a Twitch, a ambas o a una sola con `@kick` y `@twitch`, y se
  vieron los mensajes de cuentas ajenas en las dos plataformas.
- `!p hola` desde otra cuenta de Twitch respondió, una vez por mensaje. Desde otra cuenta de Kick no hubo
  respuesta, y el registro no mostraba ningún intento de envío ni error.

## Causa

La automatización excluye todo chat sin `author_id` o sin `is_self` conocido. El traductor de Kick solo
leía `sender.user_id`, el campo del webhook oficial; el diseño 005 ya avisaba de que el feed WebSocket podía
traer otro. Lo más probable, aunque falta confirmarlo con una publicación real, es que el feed mande
`sender.id`. Además, el motor contaba esas exclusiones solo cada 100 y en nivel debug, así que no quedó
ninguna pista en el registro.

## Cambios

- `syon-platform-kick`: `author_id` se lee de `sender.user_id` y, si falta, de `sender.id`.
- `syon-engine`: el primer chat excluido por autoría desconocida se registra como aviso, sin texto ni
  usuario.
- Diseño 005 y README del crate de Kick: ajustados, con la corrección marcada como por confirmar en vivo.

## Parches

Se numeran desde `0001` porque el autor ya aplicó la tanda de WT-0.4.2_01.

- `0001-docs-estado-reabrir-la-fila-de-revisi-n-de-F3-por-el.patch`
- `0002-fix-kick-leer-el-id-del-autor-de-sender.id-adem-s-de.patch`
- `0003-fix-engine-avisar-la-primera-vez-que-un-chat-se-excl.patch`
- `0004-docs-walkthrough-cerrar-el-hallazgo-de-Kick-en-vivo.patch`

## Archivos afectados

- `crates/syon-platform-kick/src/events.rs` y `README.md`
- `crates/syon-engine/src/commands.rs`
- `docs/designs/005-comandos-filtros-antispam.md`
- `docs/estado.md`
- `docs/walkthroughs/WT-0.4.2_02.md`

## Verificación

- Prueba nueva `chat_del_feed_con_sender_id_conoce_al_autor`.
- `./validar.sh` más `cargo deny` y `cargo machete` sobre un clon limpio de `origin/master` con los parches
  aplicados con `git am` sin `-3` y su propio `target`.
- **Pendiente, a cargo del autor:** repetir `!p hola` desde otra cuenta de Kick con estos parches. Si sigue
  sin responder, el registro debe traer el aviso nuevo, y hará falta una publicación real del feed (con los
  textos y nombres tapados) para ver sus campos.

## Riesgos conocidos

- El conector de Kick compara el autor con `channel_id` de los ajustes para decidir `is_self`, pero el id
  de un canal no tiene por qué ser el id del usuario que autorizó la sesión. Con `sender.id` leído, un
  mensaje propio escrito en el chat de Kick puede quedar como ajeno. Las respuestas del propio bot siguen
  cubiertas por la correlación de ecos. El arreglo correcto es obtener el id del usuario autorizado de la
  API de Kick; queda para otro paso y su diseño.
- El `settings.toml` del contraste tiene `channel_id = 123456`, que parece un valor de ejemplo. Si no es el
  id real, además de afectar a lo anterior, la conexión del chat no lo comprueba.
