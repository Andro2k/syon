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
   1. Lee `settings.toml` con `TomlSettingsStore` (la ruta sale de `AppPaths`), **sin tocar el disco**.
   2. Instala el registro con el nivel de los ajustes, `SYON_LOG` y la versión de la aplicación
      (ADR-0006) y crea el runtime asíncrono ([ADR-0011](0011-runtime-asincrono-del-engine.md)).
   3. Atiende el problema de la lectura, si lo hubo (apartar un archivo inválido, punto 2), lo avisa en
      el registro y crea el `EventBus`.

   El orden es ese por dos razones. El nivel del registro sale de los ajustes, así que la lectura va
   primero; y apartar el archivo es lo único que cambia el disco por culpa de los ajustes, así que va
   al final, después de instalar el registro y crear el runtime, que son los pasos que pueden hacer
   fallar el arranque (punto 4). `Engine` ofrece `paths()`, `settings()` y
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
   diagnosticar un reporte, y tampoco se arranca si no se puede crear el runtime (`StartError::Runtime`).
   El binario decide cómo avisar. Como el registro es global, un segundo
   `Engine::start` en el mismo proceso devuelve `LoggingError::AlreadyInitialized`. En todos estos casos
   `start` falla **sin haber cambiado nada en el disco**: el archivo inválido sigue donde estaba y un
   apartado anterior no se sustituye.
5. Todavía no hay binario: el crate `syon` se crea con su fase y llamará a `Engine::start` con
   `env!("CARGO_PKG_VERSION")`.

## Alternativas consideradas

| Alternativa | Por qué no |
|-------------|-----------|
| Abortar el arranque ante un archivo inválido | Un error de sintaxis en un archivo editado a mano impediría abrir SYON; el aviso en el log y el apartado bastan |
| Usar los valores por defecto y dejar el archivo roto en su sitio | El siguiente `save` lo sobrescribiría y se perdería lo que escribió el usuario |
| Guardar los valores por defecto sobre el archivo | Mismo problema, de forma inmediata |
| Un apartado con la hora en el nombre | Acumula archivos sin límite y necesita una librería de tiempo; basta con el último roto |
| Apartar el archivo inválido al leerlo, antes de instalar el registro | Si el registro no se puede instalar (un segundo `start`, una carpeta de datos sin permisos), el archivo ya estaría movido, el aviso se perdería y un apartado anterior se habría sustituido |
| Iniciar el registro antes de leer los ajustes, con nivel por defecto | El nivel del archivo no sería el de los ajustes en el primer arranque, y habría que cambiarlo después |
| Que `syon-app` reciba el aviso por un puerto de notificaciones | Un puerto sin segundo uso; el registro ya es el canal de avisos |

## Consecuencias

- El nivel de log de `settings.toml` ya tiene efecto, y un archivo inválido deja un aviso con la ruta,
  la clave y dónde quedó el apartado.
- Si el usuario no mira el log, puede no enterarse de que se ignoraron sus ajustes; una notificación en
  la interfaz queda para F9.
- `Engine::start` solo toca el disco por culpa de los ajustes cuando el registro ya está instalado, así
  que el aviso se escribe en cuanto se aparta el archivo. Un arranque que falla no deja rastro en la
  carpeta de configuración.
- Un `save` posterior crea un archivo nuevo con los ajustes del momento; el apartado se conserva hasta
  que otro archivo inválido lo sustituya.
- Las pruebas de `Engine::start` viven en `crates/syon-engine/tests/` (un binario por prueba, porque
  instala el registro global), incluidos los dos casos de arranque fallido; la lógica de lectura y
  recuperación se prueba por separado con carpetas temporales.
