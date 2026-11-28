# WT-0.4.0_01: comandos, filtros y antispam

## Contexto

El diseño 005 completa la segunda parte de F3. Las plataformas publican `ChatMessage` normalizados y
el engine responde mediante `ChatSender`, sin ejecutar red dentro de `EventSink::publish`. Kick y
Twitch devuelven eco de los mensajes enviados; por eso los ecos propios e inciertos se excluyen antes
de evaluar reglas. Los roles y la identidad que no se puedan leer se conservan como desconocidos o
falsos.

## Cambios por capa

- `syon-domain`: `ChatMessage` incluye id de autor opcional, roles normalizados y `is_self` triestado.
- Adaptadores: Twitch deriva roles de badges y compara el id de chat con la identidad validada;
  Kick lee solo los campos opcionales conocidos y compara el autor con el id del canal configurado.
- `syon-app` y `syon-storage-toml`: modelo de comandos, destinos, filtros y límites en `[chat]`, con
  defaults inertes, validación explícita y compatibilidad de archivos sin esa sección.
- `syon-engine`: parser literal con alias y argumentos, autorización acumulativa, filtros, ventanas
  de duplicados y enfriamiento, correlación acotada de ecos y worker secuencial en una suscripción
  acotada. `ChatHandle::send_async` mantiene el envío fuera del camino de publicación. El engine
  cuenta autores/autorías desconocidos y registra solo un diagnóstico agregado cada 100 omisiones.

## Parches

El `origin/master` usado como base no contiene todavía el código del envío de chat de F3 parte 1,
aunque `docs/estado.md` lo registra como implementado. Los parches 0001–0006 reconstruyen esos
prerrequisitos presentes en la rama de trabajo; 0007 contiene la propuesta de diseño 005. Se incluyen
para que la serie completa aplique desde `origin/master` en un clon limpio.

1. `0001-docs-design-aceptar-el-dise-o-004-y-tomar-el-frente-.patch`: diseño 004 y traspaso del frente.
2. `0002-feat-app-a-adir-puerto-ChatSender-y-tipos-de-env-o.patch`: puerto de envío.
3. `0003-feat-twitch-a-adir-send_chat_message-y-TwitchChatSen.patch`: sender de Twitch.
4. `0004-feat-kick-a-adir-try_restore-KickApi-y-KickChatSende.patch`: sender de Kick.
5. `0005-feat-engine-a-adir-registro-de-emisores-de-chat-y-Ch.patch`: registro y `ChatHandle`.
6. `0006-feat-syon-a-adir-entrada-de-chat-por-consola-con-pre.patch`: entrada manual de consola.
7. `0007-docs-design-proponer-comandos-filtros-y-antispam.patch`: propuesta del diseño 005.
8. `0008-docs-design-aceptar-dise-o-de-comandos-y-antispam.patch`: aceptación e implementación del frente.
9. `0009-feat-domain-a-adir-identidad-y-roles-de-chat.patch`: identidad y traducción de roles.
10. `0010-feat-app-modelar-ajustes-de-automatizaci-n-de-chat.patch`: modelos de aplicación.
11. `0011-feat-storage-persistir-ajustes-de-comandos-y-filtros.patch`: TOML compatible y validado.
12. `0012-feat-engine-ejecutar-comandos-y-filtrar-ecos-de-chat.patch`: políticas, worker y envío asíncrono.
13. `0013-docs-walkthrough-cerrar-comandos-filtros-y-antispam.patch`: este walkthrough, paridad y estado.
14. `0014-feat-engine-contar-chat-con-autoria-desconocida.patch`: contador agregado sin contenido de chat.

## Archivos afectados

`crates/syon-domain`, `crates/syon-app`, `crates/syon-platform-kick`, `crates/syon-platform-twitch`,
`crates/syon-storage-toml`, `crates/syon-engine`, `docs/designs/005-comandos-filtros-antispam.md`,
`docs/requirements/paridad.md` y `docs/estado.md`.

## Verificación

- `cargo fmt --all --check`
- `cargo clippy --workspace --all-targets --locked -- -D warnings`
- `cargo test --workspace --locked`
- `cargo xtask docs-check`
- `cargo xtask arch-check`
- Reaplicación con `git am` y repetición de estas comprobaciones en un clon limpio.

Las cinco comprobaciones terminaron con código 0 en la rama de trabajo y en el clon limpio. El clon
partió de `217ee533f629fddbfb13598e7af267c6bf6c4bfe`, recibió los 14 parches con `git am` y usó un
`CARGO_TARGET_DIR` independiente.

`./validar.sh` no se pudo invocar en este host Windows porque el ejecutable `bash.exe` está denegado;
sus cinco comprobaciones requeridas se ejecutaron manualmente. `cargo-deny` y `cargo-machete` son
opcionales en el script y no forman parte de esta lista de comprobaciones.

## Riesgos conocidos

- Kick usa el WebSocket de la página y no el webhook oficial. Los campos opcionales de id y badges
  son tolerantes, pero deben confirmarse contra un evento real antes de confiar en roles privilegiados.
- La automatización descarta autores con identidad o autoría desconocida para evitar bucles. Ese
  diagnóstico no incluye nombres, texto, ids ni credenciales.
- Los límites locales son conservadores y no reemplazan los límites aplicados por cada plataforma.
