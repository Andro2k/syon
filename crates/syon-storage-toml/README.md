# syon-storage-toml

Adaptador del puerto `SettingsStore` de [`syon-app`](../syon-app/README.md) sobre un archivo
`settings.toml`. Es el único crate que conoce el formato TOML y `serde`; traduce desde y hacia el
modelo plano `Settings` (ver [ADR-0008](../../docs/adr/0008-configuracion-toml.md) y el
[diseño 001](../../docs/designs/001-nucleo-dominio-eventos.md), secciones 5 y 7).

## Qué hace

- Implementa `SettingsStore` con `TomlSettingsStore`.
- `load`: archivo ausente devuelve `Settings::default()` sin escribir nada; las claves ausentes
  toman su valor por defecto y las desconocidas se ignoran.
- `load` con contenido ilegible (TOML roto, no UTF-8, tipo incorrecto o nivel desconocido) devuelve
  `SettingsError::Invalid` con la ruta y, cuando se conoce, la clave (`logging.level: …`). Nunca
  escribe, así que el archivo no se sobrescribe.
- `save`: crea la carpeta si falta, escribe un temporal en la misma carpeta, lo sincroniza y lo
  renombra sobre el destino. Si falla, el archivo anterior queda intacto y el temporal se borra.
- Conserva `schema_version` y `[logging].level` en la ida y vuelta.

## Qué no hace

- No decide dónde vive el archivo: la ruta llega por constructor; `syon-engine` la resuelve (ADR 0009).
- No guarda el estado de ejecución, solo preferencias, ni secretos.
- No conserva comentarios ni el orden del archivo al guardar, y no migra entre versiones de esquema.
- No decide qué hacer ante un `Invalid`: quien llama no debe guardar valores por defecto encima.
- No usa `thiserror`: los errores son los de `syon-app`.

## API pública

| Elemento | Para qué sirve |
|----------|----------------|
| `TomlSettingsStore::new(ruta)` | Crea el adaptador para ese archivo; no toca el disco |
| `TomlSettingsStore::path()` | Ruta del archivo de ajustes |
| `impl SettingsStore` | `load` y `save` según el contrato del puerto |

Dependencias: `syon-app`, `toml` y `serde` (con `derive`).
