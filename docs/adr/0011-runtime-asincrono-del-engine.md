# ADR-0011: Runtime asíncrono propiedad del engine

- Estado: Aceptada
- Fecha: 2026-11-26

## Contexto

F2 trae la primera red de SYON: un WebSocket y llamadas HTTP por plataforma, con reconexión y
temporizadores. Hasta ahora el workspace solo usa `tokio` con la función `sync` para los canales del
bus ([ADR-0007](0007-bus-de-eventos.md)). Hay que decidir quién crea el runtime, porque el engine debe
poder correr sin interfaz ([ADR-0003](0003-arquitectura-hexagonal-workspace.md)) y los puertos de
`syon-app` no deben depender de `async` ([diseño 002](../designs/002-plataformas-kick-twitch.md),
sección 4).

## Decisión

1. **El engine crea y es dueño del runtime** de `tokio` (multihilo, dos hilos de trabajo, nombre
   `syon-worker`) dentro de `Engine::start`. Si no se puede crear, `StartError::Runtime`.
2. **Los adaptadores no crean runtimes.** Reciben un `tokio::runtime::Handle` (`Engine::runtime_handle`)
   y lanzan en él sus tareas. Los puertos de `syon-app` siguen siendo síncronos y no esperan red.
3. **El engine registra los conectores** (`Engine::add_connector`) para consultar su estado
   (`connection_status`) y, al destruirse, les pide detenerse antes de destruir el runtime.
4. **Cada crate declara las funciones de `tokio` que usa.** La entrada del workspace sigue con solo
   `sync`; `syon-engine` añade `rt-multi-thread` y `time`, y cada adaptador añadirá las suyas (`net`,
   `macros`...). Cargo las une. Nadie activa `full`.

## Alternativas consideradas

| Alternativa | Por qué no |
|-------------|-----------|
| `#[tokio::main]` en el binario | El engine no podría arrancarse desde pruebas ni desde otro cliente (UI, overlays) sin repetir el runtime |
| Un runtime por adaptador | Hilos duplicados y tres sitios donde cerrar ordenadamente |
| `async fn` en los puertos de `syon-app` | Los traits con `async fn` no son compatibles con `dyn`, y el crate necesitaría un runtime o `futures` |
| Otro runtime (`smol`, `async-std`) | El prototipo ya usa `tokio` y `tokio-tungstenite`, y el bus ya depende de sus canales |
| Activar `full` en el workspace | Compila funciones que SYON no usa; el prototipo ya lo marcó como pendiente de afinar |

## Consecuencias

- Los adaptadores se prueban con un runtime de prueba propio, sin arrancar el engine.
- El número de hilos es una constante; si la carga lo pide se pasa a los ajustes.
- `Engine::start` sigue pudiendo llamarse una sola vez por proceso (registro global), así que las
  pruebas que lo usan viven en `crates/syon-engine/tests/`.
- Detener un conector es una petición: la tarea termina por su cuenta, y al destruir el runtime las
  tareas que sigan vivas se cancelan.
