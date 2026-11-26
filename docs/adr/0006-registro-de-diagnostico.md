# ADR-0006: Registro de diagnóstico con `log`, `env_logger` y un archivo acotado

- Estado: Aceptada (la fachada `log` se adoptó con el bus de eventos; el archivo, la redacción y el
  hook de pánico se implementaron en el parche 6 de F1)
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

## Detalles de la implementación

Precisiones tomadas al escribir el parche 6; no cambian la decisión.

- **Entrada:** `init_logging(&AppPaths, &LoggingConfig)`. `LoggingConfig::from_env` toma el nivel de
  `[logging].level` y el filtro de `SYON_LOG` (vacío no cuenta). Solo se puede llamar una vez por
  proceso; una segunda llamada devuelve `LoggingError::AlreadyInitialized` sin tocar el archivo.
- **Formato:** `<hora UTC con milisegundos> <NIVEL> [<destino>] <mensaje>`. Las líneas siguientes de un
  mensaje llevan cuatro espacios de sangría, de modo que solo la primera línea de cada registro
  empieza en la columna cero y un texto de chat no puede imitar una marca de arranque.
- **Marca de arranque:** `logging: syon <versión> iniciado`, con el destino `syon::arranque` y nivel
  `info`. Se escribe siempre, aunque el filtro del archivo sea más estricto, y delimita las ejecuciones.
- **Ejecuciones:** al abrir el archivo se conservan las 4 ejecuciones anteriores más recientes entre
  `syon.log.1` y `syon.log` (con la que arranca, 5). Lo que haya antes de la primera marca es el resto de
  una ejecución cuya marca ya se descartó y cuenta como una. La rotación por tamaño ocurre en la escritura:
  si el sistema no deja renombrar, se sigue escribiendo en `syon.log` en lugar de perder líneas.
- **Consola:** siempre `warn`, a `stderr`; `SYON_LOG` solo afecta al archivo.
- **Redacción:** `redact` oculta con `[REDACTADO]` el valor de las claves cuyo nombre termina en `token`,
  `secret`, `password`, `passwd`, `cookie`, `verifier`, `api_key` o `api-key`, además de `authorization` y
  `code` (este solo si el valor tiene 8 o más caracteres, para no ocultar `code=404`), y la credencial que
  sigue a `Bearer`. La carpeta personal pasa a `~`. Reconoce secretos por su clave: un valor suelto sin clave
  no se detecta. Se escribió a mano para no añadir `regex` como dependencia directa.
- **Pánico:** `pánico: el hilo '<hilo>' falló en <archivo:línea>: <mensaje>` y el backtrace si
  `RUST_BACKTRACE` lo habilita; después actúa el hook anterior.
- **Mapeo de niveles:** `syon-engine` traduce `LogLevel` (de `syon-app`) a `log::LevelFilter`.
  `syon-storage-toml` mantiene aparte los nombres del archivo TOML: son dos traducciones con propósitos
  distintos y `syon-app` no puede conocer `log` (diseño 001, sección 10).

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
- Con la marca a nivel `info`, el nivel máximo del archivo nunca baja de `info`, aunque `[logging].level`
  sea `error`; `debug` y `trace` se siguen descartando antes de formatear.
- `redact` y la rotación se prueban con carpetas temporales; la prueba que instala el logger y el hook de
  pánico del proceso vive en `crates/syon-engine/tests/logging.rs`, un binario aparte.
