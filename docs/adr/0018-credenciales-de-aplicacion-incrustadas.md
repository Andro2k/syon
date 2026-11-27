# ADR-0018: Credenciales de la aplicación incrustadas en el binario en tiempo de compilación

- Estado: Propuesta (la decisión de fondo la tomó el autor el 2026-11-26; falta revisar los detalles)
- Fecha: 2026-11-26

## Contexto

Kick envía el `client_secret` en cada petición de token, tanto al canjear el código como al renovar
([ADR-0014](0014-oauth-de-kick.md)), y Twitch lo exigirá con el flujo de código del
[diseño 002](../designs/002-plataformas-kick-twitch.md). Ese secreto es de la aplicación, no de la persona:
no puede ir en `settings.toml`, que se edita a mano y se comparte al reportar un problema, ni en el archivo
de tokens ([ADR-0012](0012-almacen-de-credenciales.md)). El diseño 001 ya señala como defecto del
prototipo que mezclara `client_secret` con los ajustes.

El prototipo lo leía de `.env` (`KICK_CLIENT_ID` y `KICK_CLIENT_SECRET`), cosa razonable para quien lo
desarrolla y inviable para quien lo instala con el instalador de F10. Quedaba pendiente decidir de dónde
sale ([diseño 002](../designs/002-plataformas-kick-twitch.md), sección 14, punto 6).

Dos hechos acotan la decisión:

- **El repositorio es público.** El secreto no puede estar en el código fuente ni en un archivo versionado.
- **El feed de Kick no usa OAuth** ([ADR-0015](0015-conexion-al-feed-de-kick.md)). Para recibir eventos de
  Kick no hace falta ninguna credencial; sí hacen falta para iniciar sesión (escribir en el chat, F3) y
  para Twitch, cuyo EventSub exige un token de usuario.

## Decisión

El autor decidió **incrustar en el binario las credenciales de su aplicación** de Kick y de Twitch, de modo
que quien instala SYON no tenga que registrar nada. El detalle:

1. **Se inyectan al compilar, nunca en git.** El crate `syon` (el binario) es el único que toca el entorno.
   Lee con `option_env!` las variables `SYON_KICK_CLIENT_ID`, `SYON_KICK_CLIENT_SECRET`,
   `SYON_TWITCH_CLIENT_ID` y `SYON_TWITCH_CLIENT_SECRET`. `.gitignore` ya excluye `.env` y `.env.*`.
2. **Un `build.rs` del binario** declara `cargo:rerun-if-env-changed` para las cuatro variables. Sin él,
   Cargo reutilizaría un binario compilado con otro valor.
3. **Precedencia al arrancar:** la variable de entorno en ejecución (mismo nombre) pasa por delante del valor
   incrustado, para que quien desarrolla use su propia aplicación de prueba. Si no hay ninguno, esa
   plataforma no puede iniciar sesión y se explica con un mensaje que nombra las variables, no su valor. Un
   valor vacío o en blanco cuenta como ausente.
4. **El engine y los adaptadores no leen el entorno.** Reciben las credenciales por constructor, como ya
   hace `KickAppConfig::new(client_id, client_secret)`. El engine define su propio tipo `AppCredentials`
   (con `Debug` a mano que no imprime el secreto), para que el binario no dependa de los adaptadores.
5. **Compilar sin las variables funciona.** La CI normal y quien contribuye compilan y prueban sin ellas.
   En la publicación (F10) llegan como secretos de GitHub Actions solo al trabajo de release, no al de las
   solicitudes de cambio.
6. **No hay almacén de credenciales de aplicación.** `CredentialStore` sigue guardando solo los tokens de
   usuario.
7. **Nunca se registra el valor.** `redact` ya oculta `client_secret`; además el registro solo dice si una
   credencial está presente o ausente.

## Alternativas consideradas

| Alternativa | Motivo para no elegirla |
|-------------|-------------------------|
| Cada streamer registra su aplicación y escribe su secreto, guardado en el almacén de credenciales | No elegida por el autor. Evita un secreto compartido que se pueda extraer; a cambio, cada persona debe registrar una aplicación en cada plataforma y habría que ampliar el puerto de credenciales para guardar datos de la aplicación |
| Solo variable de entorno o `.env`, como el prototipo | No elegida por el autor: no sirve con un instalador |
| Un servidor propio que guarde el secreto y haga el canje de tokens | Exige operar y mantener un servicio; fuera del alcance de una aplicación de escritorio |

## Consecuencias

- **El secreto es extraíble del binario.** Las cadenas quedan legibles y ofuscarlas no cambia eso. Si se
  filtra, afecta a todas las instalaciones: hay que rotarlo en el panel de cada plataforma y publicar una
  versión nueva; hasta que se actualice, el inicio de sesión de las instalaciones antiguas falla.
- **No está verificado que los términos de Kick y de Twitch permitan un secreto en una aplicación
  distribuida.** Hay que revisarlos antes de F10. Para Twitch, según recuerdo, existe un tipo de cliente
  público con flujo de dispositivo que no usa secreto; contrastarlo con su documentación podría eliminar
  el riesgo en esa plataforma.
- Todas las instalaciones comparten la cuota y los límites de tasa de la aplicación del autor.
- Cambiar una credencial exige publicar una versión nueva: no hay forma de actualizarla en caliente.
- La ruta con credenciales solo se ejerce de verdad con las variables puestas. Las pruebas cubren la
  precedencia y la ausencia con una función pura; el inicio de sesión real queda para el contraste en vivo
  del autor.
- Queda sin tocar `settings.toml`: no recibe `client_id` ni secretos.
