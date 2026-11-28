# WT-0.4.2_01: correcciones de la revisión de F3

## Contexto

Revisión del código de los frentes de chat y envío (Gemini) y de comandos, filtros y antispam (Codex),
hecha sobre `origin/master` con el repositorio en verde. No hubo errores graves; sí tres defectos menores
que se corrigen aquí, cada uno en su parche. Los emisores de Kick y Twitch, `ChatHandle` y la entrada de
consola no necesitaron cambios.

## Cambios

- `syon-storage-toml`: la carga rechaza un nombre o alias de comando con espacios o que empieza por el
  prefijo, porque `parse_command` nunca lo encontraría y el comando quedaría muerto sin aviso.
- `syon-engine` (eco propio): con el id del mensaje enviado ya conocido, el eco se reconoce solo por id; el
  texto solo vale mientras el id está pendiente y el mensaje no se sabe ajeno. Antes, quien copiaba la
  respuesta del bot la perdía y borraba el eco registrado.
- `syon-engine` (texto del espectador): `{user}`, `{platform}` y `{args}` se sustituyen en una sola pasada,
  el texto del espectador se reduce a una línea sin caracteres de control y se descarta la respuesta que
  abriría con el prefijo, `/` o `.` sin que la plantilla empiece así. Antes `{args}` permitía que el bot
  enviara `/ban alguien` o un comando para otro bot.
- Diseño 005: dice lo que el código hace (el texto «escapada» no tenía escape detrás).

## Parches

- `0001-docs-estado-anotar-el-frente-de-revisi-n-de-F3.patch`
- `0002-fix-storage-rechazar-nombres-de-comando-que-nunca-se.patch`
- `0003-fix-engine-no-consumir-como-eco-el-mensaje-de-otra-p.patch`
- `0004-fix-engine-tratar-como-texto-plano-lo-que-el-especta.patch`
- `0005-docs-walkthrough-cerrar-la-revisi-n-de-F3.patch`

## Archivos afectados

- `crates/syon-storage-toml/src/format.rs` y `store.rs`
- `crates/syon-engine/src/commands.rs`
- `docs/designs/005-comandos-filtros-antispam.md`
- `docs/estado.md`
- `docs/walkthroughs/WT-0.4.2_01.md`

## Verificación

- Pruebas nuevas: tres casos en `valida_chat_y_no_modifica_el_archivo_invalido`,
  `un_mensaje_ajeno_con_el_mismo_texto_no_consume_el_eco_pendiente`,
  `con_el_id_conocido_el_eco_se_reconoce_solo_por_id`,
  `una_variable_no_se_reinterpreta_y_el_texto_del_usuario_queda_en_una_linea` y
  `el_texto_del_usuario_no_puede_abrir_la_respuesta_con_un_comando`.
- Antes de corregir, pruebas desechables confirmaron el eco consumido por un mensaje ajeno y `{args}`
  enviando `/ban alguien`.
- `./validar.sh` (clippy, pruebas del workspace, `docs-check`, `arch-check` y `fmt`), más `cargo deny` y
  `cargo machete`, sobre un clon limpio de `origin/master` con los parches aplicados con `git am` sin `-3`
  y su propio `target`.

## Riesgos conocidos

- `last_send` se anota al decidir la respuesta, no al enviarla: un envío fallido igualmente gasta el
  intervalo de esa plataforma.
- El consumidor de automatización espera cada envío en serie; un envío lento llena su buffer de 256 y se
  pierden eventos (se cuentan, como manda la regla del bus).
- Solo se revisaron los frentes de chat, comandos y antispam; el resto de F2 y F3 no.
