# ADR-0010: Arranque del engine y recuperación de ajustes inválidos

- Estado: Aceptada
- Fecha: 2026-11-26

## Contexto

Hasta ahora `syon-engine` ofrece piezas sueltas (bus, rutas, registro) y `syon-storage-toml` guarda los
ajustes, pero nada las une: ningún código lee `settings.toml`, el nivel de log de los ajustes no llega
al registro y no está decidido qué hacer si el archivo es ilegible. El [ADR-0008](0008-configuracion-toml.md)
fija que un archivo inválido nunca se sobrescribe y deja a quien arranca la decisión de qué hacer.
`syon-engine` es el único crate (junto con el binario) que puede nombrar adaptadores concretos
([ADR-0003](0003-arquitectura-hexagonal-workspace.md)), así que la conexión vive allí.

## Decisión

1. **`Engine::start(paths, app_version)`** es el punto de arranque. Hace, en este orden:
   1. Lee `settings.toml` con `TomlSettingsStore` (la ruta sale de `AppPaths`).
   2. Instala el registro con el nivel de los ajustes, `SYON_LOG` y la versión de la aplicación
      (ADR-0006).
   3. Escribe en el registro los avisos de la lectura y crea el `EventBus`.

   El orden es ese porque el nivel del registro sale de los ajustes: los avisos de la lectura se
   guardan y se escriben cuando el registro ya existe. `Engine` ofrece `paths()`, `settings()` y
   `bus()`; no guarda el almacén, porque nada lo usa todavía.
2. **Un problema con los ajustes nunca impide arrancar.**
   - Contenido inválido (`SettingsError::Invalid`): el archivo se aparta renombrándolo a
     `settings.toml.invalid` y se usan los valores por defecto. El apartado sustituye a uno anterior:
     importa el último archivo roto, que es el que la persona acaba de editar. Si el renombrado falla,
     el archivo queda donde estaba.
   - No se puede acceder al archivo (`SettingsError::Io`): se usan los valores por defecto y no se toca
     nada.
   - En ambos casos se registra un aviso (`engine: …`) con el error, que incluye la ruta y la clave.
3. **Al arrancar no se guarda nada.** Ni tras un archivo inválido ni con el archivo ausente: el primer
   `save` lo crea. Así, lo que el usuario escribió sobrevive en el apartado y un fallo de lectura
   transitorio no se convierte en pérdida de datos.
4. **Un fallo del registro sí impide arrancar** (`StartError::Logging`): sin registro no hay forma de
   diagnosticar un reporte. El binario decide cómo avisar. Como el registro es global, un segundo
   `Engine::start` en el mismo proceso devuelve `LoggingError::AlreadyInitialized`.
5. Todavía no hay binario: el crate `syon` se crea con su fase y llamará a `Engine::start` con
   `env!("CARGO_PKG_VERSION")`.

## Alternativas consideradas

| Alternativa | Por qué no |
|-------------|-----------|
| Abortar el arranque ante un archivo inválido | Un error de sintaxis en un archivo editado a mano impediría abrir SYON; el aviso en el log y el apartado bastan |
| Usar los valores por defecto y dejar el archivo roto en su sitio | El siguiente `save` lo sobrescribiría y se perdería lo que escribió el usuario |
| Guardar los valores por defecto sobre el archivo | Mismo problema, de forma inmediata |
| Un apartado con la hora en el nombre | Acumula archivos sin límite y necesita una librería de tiempo; basta con el último roto |
| Iniciar el registro antes de leer los ajustes, con nivel por defecto | El nivel del archivo no sería el de los ajustes en el primer arranque, y habría que cambiarlo después |
| Que `syon-app` reciba el aviso por un puerto de notificaciones | Un puerto sin segundo uso; el registro ya es el canal de avisos |

## Consecuencias

- El nivel de log de `settings.toml` ya tiene efecto, y un archivo inválido deja un aviso con la ruta,
  la clave y dónde quedó el apartado.
- Si el usuario no mira el log, puede no enterarse de que se ignoraron sus ajustes; una notificación en
  la interfaz queda para F9.
- `Engine::start` toca el disco (renombra) antes de que exista el registro; por eso el aviso se
  escribe después.
- Un `save` posterior crea un archivo nuevo con los ajustes del momento; el apartado se conserva hasta
  que otro archivo inválido lo sustituya.
- Las pruebas de `Engine::start` viven en `crates/syon-engine/tests/` (un binario por prueba, porque
  instala el registro global); la lógica de recuperación se prueba por separado con carpetas temporales.
