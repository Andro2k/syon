# ADR-0001: Nombre del proyecto: SYON

- Estado: Aceptada
- Fecha: 2026-11-24

## Contexto

La reconstrucción de MiniKick es un proyecto nuevo y necesita su propia
identidad. El nombre debía ser formal, evitar la palabra "bot" y las marcas
de las plataformas (Kick, Twitch) y no sonar a copia de herramientas
existentes.

## Decisión

El proyecto se llama **SYON**. Repositorio: `Andro2k/syon`. Binario: `syon`.
Los crates del workspace llevan el prefijo `syon-` (por ejemplo `syon-domain`).

## Alternativas consideradas

Switchboard, Talkback, Tally, Cuesheet, Simulcast y Showrunner. Se
descartaron por elección del autor.

## Consecuencias

- No se verificó disponibilidad de marca ni de nombres en crates.io. Queda
  pendiente comprobar `syon-*` antes de publicar cualquier crate.
- El prefijo `syon-` evita colisiones de nombres de crate por el nombre corto.
