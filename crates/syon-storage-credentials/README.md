# syon-storage-credentials

Adaptador del puerto `CredentialStore` de [`syon-app`](../syon-app/README.md) sobre un archivo
`credentials.toml`. Guarda los tokens OAuth de cada plataforma, separados de los ajustes (ver
[ADR-0012](../../docs/adr/0012-almacen-de-credenciales.md) y el
[diseño 002](../../docs/designs/002-plataformas-kick-twitch.md), sección 5).

## Qué hace

- Implementa `CredentialStore` con `TomlCredentialStore`: `load`, `save` y `clear` por plataforma.
- El archivo tiene `schema_version` y una sección por plataforma (`[kick]`, `[twitch]`). Solo el
  `access_token` es obligatorio; las claves desconocidas se ignoran.
- `save` y `clear` leen el archivo, cambian la sección de esa plataforma y lo reescriben con un
  temporal en la misma carpeta que se sincroniza y se renombra. Las demás secciones no se tocan y, si
  falla, el archivo anterior queda intacto. Los guardados del proceso se serializan.
- En Unix el archivo queda con permisos `0600` desde antes de escribir el primer byte.
- Un archivo ilegible (TOML roto, no UTF-8, tipo incorrecto) devuelve `CredentialError::Invalid` con
  la ruta y la línea, **sin copiar el contenido**, y nunca se sobrescribe: ni `save` ni `clear` lo
  tocan.

## Qué no hace

- No decide dónde vive el archivo: la ruta llega por constructor; `syon-engine` la resuelve.
- No cifra los tokens: confía en los permisos del archivo y de la carpeta del usuario. En Windows no
  cambia las ACL; hereda las de la carpeta del perfil.
- No renueva tokens ni conoce OAuth: eso es de cada adaptador de plataforma.
- No imprime tokens: ni `Debug` ni los errores los incluyen.

## API pública

| Elemento | Para qué sirve |
|----------|----------------|
| `TomlCredentialStore::new(ruta)` | Crea el adaptador para ese archivo; no toca el disco |
| `TomlCredentialStore::path()` | Ruta del archivo de credenciales |
| `impl CredentialStore` | `load`, `save` y `clear` según el contrato del puerto |

Dependencias: `syon-app`, `syon-domain`, `toml` y `serde` (con `derive`).
