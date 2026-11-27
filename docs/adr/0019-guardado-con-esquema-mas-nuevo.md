# ADR-0019: Un `save` no reescribe ajustes de un esquema más nuevo

- Estado: Propuesta (el pendiente era una decisión del autor; se eligió la opción que no pierde datos)
- Fecha: 2026-11-27

## Contexto

El [ADR-0008](0008-configuracion-toml.md) aceptó que una versión anterior de SYON que guardara un
`settings.toml` escrito por una versión más nueva perdiera las claves que no conoce, y dejó abierta la
decisión de rechazar o avisar en ese caso. `load` ya ignora las claves desconocidas a propósito, así que
un archivo más nuevo se lee sin problema; el daño llega al guardar, porque `save` reescribe el archivo
entero. Hoy ningún código de producción llama a `SettingsStore::save`; lo hará la interfaz (F9).

## Decisión

`TomlSettingsStore::save` se niega a escribir cuando el `schema_version` del archivo en disco, o el de
los ajustes que recibe, es mayor que `SCHEMA_VERSION`. Devuelve `SettingsError::Invalid` con un mensaje
en español que nombra ambas versiones, y no toca el archivo ni crea ninguno. `load` no cambia. Un
archivo ausente, ilegible o sin `schema_version` no bloquea el guardado (de lo ilegible se ocupa el
arranque, [ADR-0010](0010-arranque-del-engine.md)).

## Alternativas consideradas

| Alternativa | Por qué no |
|-------------|-----------|
| Solo avisar en el log y guardar | El aviso no evita perder las claves; el usuario no suele mirar el log (ADR-0010) |
| Rechazar también en `load` | Impediría arrancar con un archivo más nuevo que, por lo demás, se lee bien |
| Variante nueva `SettingsError::NewerSchema` | Cambia el puerto de `syon-app` y obliga a tocar a quien lo trata; `Invalid` ya cubre «el contenido no es utilizable» |
| Conservar las claves desconocidas al guardar (`toml_edit`) | Es la solución de fondo, pero añade una dependencia; se reevaluará en F9 (ADR-0008) |

## Consecuencias

- Quien baja de versión conserva su archivo, a costa de no poder guardar ajustes con la versión vieja
  hasta que borre o corrija `schema_version`. La interfaz de F9 deberá mostrar el error.
- Sustituye la consecuencia del ADR-0008 sobre la pérdida de claves por un guardado rechazado.
- Si llega una segunda versión de esquema, la migración tendrá su ADR y revisará esta regla.
