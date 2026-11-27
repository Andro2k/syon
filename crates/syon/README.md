# syon

Binario de consola de SYON. Arranca el [`syon-engine`](../syon-engine/README.md), conecta las
plataformas activadas en `settings.toml` e imprime una línea por evento hasta que se pulsa Ctrl+C.
Es el único crate, junto con el engine, que puede nombrar adaptadores (ver
[ADR-0003](../../docs/adr/0003-arquitectura-hexagonal-workspace.md)) y el único que toca el entorno.
Su diseño está en el [diseño 003](../../docs/designs/003-arranque-de-plataformas-y-binario.md).

## Comandos

| Comando | Qué hace |
|---------|----------|
| `syon` o `syon run` | Conecta las plataformas activadas y muestra los eventos; sale con Ctrl+C |
| `syon login <kick\|twitch>` | Deja lista la sesión: imprime el enlace de autorización si hace falta y guarda los tokens |
| `syon --version`, `syon --help` | Versión y uso |

Códigos de salida: 0 correcto, 1 fallo de arranque o sin plataformas activadas, 2 uso incorrecto.
Los fallos de arranque salen por `stderr`, porque puede no haber registro todavía.

## Credenciales de la aplicación

Se incrustan al compilar con las variables `SYON_KICK_CLIENT_ID`, `SYON_KICK_CLIENT_SECRET`,
`SYON_TWITCH_CLIENT_ID` y `SYON_TWITCH_CLIENT_SECRET`
([ADR-0018](../../docs/adr/0018-credenciales-de-aplicacion-incrustadas.md)); compilar sin ellas
funciona. En ejecución, las mismas variables del entorno ganan a lo incrustado. La elección es por
pareja (`client_id` y `client_secret` de la misma procedencia) y una pareja de entorno a medias es un
error. Los mensajes nombran las variables, nunca su valor.

## Salida de eventos

`<hora UTC> <plataforma> <resumen>`, una línea por evento, a `stdout`. El texto lo escribe cualquiera,
así que se eliminan los caracteres de control y los de formato bidireccional antes de imprimirlo. El
formato no es una API estable: la interfaz de F9 lo sustituye.
