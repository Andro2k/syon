# ADR-0006: Registro de diagnóstico con `log`, `env_logger` y un archivo acotado

- Estado: Aceptada (la fachada `log` se adopta con el bus de eventos; el archivo, la redacción
  y el hook de pánico se implementan en el parche 6 de F1)
- Fecha: 2026-11-25

## Contexto

SYON necesita un registro que se pueda adjuntar a un reporte sin filtrar credenciales y que
ocupe un espacio acotado. El bus de eventos ya emite avisos (consumidor saturado), así que la
fachada de registro se necesita antes que el resto de la implementación. Sonora, un proyecto
de referencia con una arquitectura parecida, resuelve esto con `log`, `env_logger` y un
escritor de archivo propio.

## Decisión

1. Todos los crates registran con las macros de `log`; solo `syon-engine` inicializa el registro.
2. Implementación en `syon-engine`: `env_logger` más un escritor propio con un único
   `syon.log` de 16 MB, que al llenarse pasa a `syon.log.1`; se conservan las últimas 5
   ejecuciones y cada arranque escribe una línea de marca. Son valores iniciales.
3. Dos filtros: la consola muestra `warn`; el archivo usa `[logging].level` y la variable
   `SYON_LOG` lo reemplaza.
4. Toda línea pasa por `redact`, que elimina tokens, secretos, códigos de OAuth y la carpeta
   personal. Es una red de seguridad: la regla sigue siendo no registrar secretos.
5. Un hook de pánico escribe el mensaje, el hilo y el backtrace en el log.
6. Mensajes en español con el prefijo del subsistema (`"bus: …"`).

## Alternativas consideradas

- `tracing` con `tracing-subscriber` y `tracing-appender`: aporta spans, pero rota por día
  (archivos `syon.log.AAAA-MM-DD`, menos cómodos para adjuntar) y trae `time` como dependencia
  transitiva (verificado). Si más adelante hace falta seguir una tarea asíncrona de extremo a
  extremo, se puede añadir con su puente a `log` sin tocar los puntos de registro.

## Consecuencias

- Se pierden los spans de `tracing`.
- El escritor de archivo es código propio que mantener; se acota con pruebas de rotación y de
  redacción.
- `env_logger` con sus funciones por defecto trae `jiff` y `regex` (verificado); `jiff` es la
  librería de tiempo prevista para la presentación (diseño 001, sección 9).
- `log` es una dependencia 0.x; se registra en `docs/deps.md`.
