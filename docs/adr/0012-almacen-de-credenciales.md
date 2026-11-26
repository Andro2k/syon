# ADR-0012: Almacén de credenciales en un archivo TOML con permisos restringidos

- Estado: Aceptada
- Fecha: 2026-11-26

## Contexto

Kick y Twitch entregan tokens OAuth que SYON debe conservar entre ejecuciones
([diseño 002](../designs/002-plataformas-kick-twitch.md), sección 5). Son secretos: no pueden ir en
`settings.toml` ([ADR-0008](0008-configuracion-toml.md)), ni al repositorio, ni al registro
([ADR-0006](0006-registro-de-diagnostico.md)). Hay que elegir dónde y en qué formato.

## Decisión

1. **Puerto `CredentialStore`** en `syon-app` (`load`, `save`, `clear` por plataforma) y adaptador
   `syon-storage-credentials` con `TomlCredentialStore`. La ruta llega por constructor; la resuelve
   `syon-engine` con `AppPaths`.
2. **Un archivo `credentials.toml`** con una sección por plataforma, separado de los ajustes. Reutiliza
   `toml` y `serde`, que el workspace ya tiene: no suma ninguna dependencia nueva.
3. **Permisos:** en Unix, `0600` desde la creación del temporal. En Windows se hereda la ACL de la
   carpeta del perfil del usuario.
4. **Escritura atómica** (temporal, `sync_all`, `rename`), serializada en el proceso. Cambiar una
   plataforma conserva las demás secciones.
5. **Un archivo inválido nunca se sobrescribe.** `load`, `save` y `clear` devuelven
   `CredentialError::Invalid`; no se apartan ni se reemplazan, porque borrar un token por un error de
   formato obligaría a iniciar sesión de nuevo sin motivo.
6. **Ningún secreto en errores ni `Debug`.** `Credentials` implementa `Debug` a mano y el mensaje de
   error de `toml`, que incluye un fragmento del texto, se sustituye por el mensaje y la línea.

## Alternativas consideradas

| Alternativa | Por qué no |
|-------------|-----------|
| Llavero del sistema (`keyring`) | Añade una dependencia con comportamiento distinto por sistema y dificulta las pruebas; se reconsidera si hace falta protección más fuerte |
| Cifrar el archivo con una clave local | Sin una clave fuera del propio archivo no protege de quien lo lee; añade dependencias de criptografía |
| Guardar los tokens en `settings.toml` | Mezcla preferencias con secretos y obliga a proteger todo el archivo |
| JSON como el prototipo (`.kick_tokens.json`) | Sumaría `serde_json` a este crate solo por el formato; TOML ya está en el workspace |

## Consecuencias

- Quien pueda leer la carpeta del usuario puede leer los tokens: la protección es la de los permisos
  del sistema, no un cifrado.
- Si el usuario edita el archivo y lo rompe, hay que corregirlo o borrarlo a mano; el error indica la
  ruta y la línea.
- En Windows los permisos no se verifican desde SYON; queda pendiente probarlo en la CI y al usarlo.
- Cambiar a un llavero toca un único adaptador, porque el engine solo ve el puerto.
