# ADR-0016: OAuth de Twitch con código de autorización, callback local y cliente mínimo de Helix

- Estado: Aceptada
- Fecha: 2026-11-26

## Contexto

Para recibir los eventos de EventSub por WebSocket y, en F3, escribir en el chat, SYON necesita un
token de usuario de Twitch ([diseño 002](../designs/002-plataformas-kick-twitch.md), sección 7; los
WebSockets de EventSub no aceptan tokens de aplicación). La decisión 3 del diseño fija el flujo de
código con callback local, igual que Kick, y la decisión 4 pide los permisos de escritura de chat desde
el primer inicio de sesión. El OAuth de Kick ([ADR-0014](0014-oauth-de-kick.md)) ya resolvió el
patrón; hay que adaptarlo a Twitch, decidir qué ofrece el cliente de la API y qué se duplica.

Los datos de esta decisión (endpoints, parámetros, formato de la respuesta y permisos) salen de la
documentación de Twitch consultada el 2026-11-26; lo que no se pudo leer ese día se marca **sin
contrastar** en las consecuencias.

## Decisión

1. **Flujo de código de autorización con callback local.** El enlace
   `https://id.twitch.tv/oauth2/authorize` lleva `response_type=code`, `client_id`, `redirect_uri`,
   `scope` y un `state` aleatorio; el intercambio y la renovación van a
   `https://id.twitch.tv/oauth2/token` con `client_id` y `client_secret`. No se usa PKCE: el flujo
   documentado por Twitch autentica el intercambio con el secreto. El callback solo acepta un `GET` a
   su ruta con el `state` esperado; cualquier otra petición recibe un 404 y se sigue esperando, hasta
   5 minutos. La dirección por defecto es `http://localhost:8081/auth/callback`: el puerto 8081 evita el
   8080 de Kick, y hay que registrarla en la aplicación de Twitch.
2. **Permisos desde el primer inicio de sesión** (`DEFAULT_SCOPES`), uno por cada necesidad:

   | Permiso | Para qué |
   |---------|----------|
   | `user:read:chat` | `channel.chat.message`, `channel.chat.message_delete` y `channel.chat.notification` |
   | `channel:moderate` | `channel.ban` |
   | `channel:read:redemptions` | `channel.channel_points_custom_reward_redemption.add` |
   | `moderator:read:followers` | `channel.follow` (v2) |
   | `user:write:chat` | Enviar mensajes al chat (F3) |

   `stream.online` y `stream.offline` no exigen permiso. No se piden `user:bot` ni `channel:bot`: solo
   hacen falta con un token de aplicación, y el WebSocket usa el del usuario. Si las credenciales
   guardadas no cubren los permisos pedidos, se inicia sesión de nuevo.
3. **Respuesta de tokens.** Twitch entrega `scope` como una lista JSON; el lector acepta también una
   cadena separada por espacios, y `refresh_token` ausente conserva el anterior.
4. **Renovación** como en Kick: con el `refresh_token` cuando faltan menos de 60 s para vencer, y a la
   fuerza (`force_refresh`) si Twitch responde 401. Twitch indica volver a pedir consentimiento si la
   renovación falla, y es lo que hace el `TokenManager`, con un aviso en el registro.
5. **Separación de responsabilidades.** `OAuthClient` solo habla con Twitch y no guarda nada;
   `TokenManager` decide reutilizar, renovar o iniciar sesión, y persiste solo a través del puerto
   `CredentialStore`. `TwitchAppConfig` llega por constructor: el crate no lee entorno, `.env` ni
   ajustes (de dónde sale el `client_secret` lo fija el [ADR-0018](0018-credenciales-de-aplicacion-incrustadas.md)), y su
   `Debug` no imprime el secreto.
6. **Cliente mínimo de Helix** (`TwitchApi`), con lo imprescindible y sin guardar tokens (cada llamada
   recibe el suyo, con `Authorization` y `Client-Id`):
   - `validate`: `GET https://id.twitch.tv/oauth2/validate`. Devuelve quién autorizó (`user_id`,
     `login`) y los permisos concedidos. Twitch exige validar los tokens de las aplicaciones que
     mantienen una sesión.
   - Resolver el id del canal: SYON escucha el canal de quien inicia sesión, así que el `user_id` de
     `validate` es el `broadcaster_user_id` de todas las condiciones. No hay llamada a `/helix/users`.
   - `create_subscription` y `list_subscriptions`: `POST` y `GET` sobre `/helix/eventsub/subscriptions`.
     La creación lee `cost`, `total_cost` y `max_total_cost` de la respuesta; si faltan quedan en
     `None`, nunca se rellenan con un valor supuesto.
7. **Lectura tolerante** con `serde_json::Value` ([ADR-0013](0013-json-tolerante-en-adaptadores.md)); los
   errores (`OAuthError`, `ApiError`) nunca incluyen tokens, el secreto ni el contenido de una respuesta
   ilegible.
8. **Sin dependencias nuevas:** `reqwest`, `serde`, `serde_json`, `base64`, `rand`, `tokio` y `log` ya
   están en `[workspace.dependencies]`.

## Alternativas consideradas

| Alternativa | Por qué no |
|-------------|-----------|
| Flujo de código de dispositivo (`/oauth2/device`) | No necesita callback local ni secreto, pero la decisión 3 del diseño fija el flujo de código con callback, igual que Kick |
| `twitch_api` y `twitch_oauth2` (crates) | Estructuras rígidas que fallan ante un campo nuevo, contra el [ADR-0013](0013-json-tolerante-en-adaptadores.md), y un árbol de dependencias grande para dos llamadas |
| Un `TokenManager` y un servidor de callback compartidos con Kick en un crate común | El diseño pide no crear un crate compartido: se copia lo mínimo y se decide después (ver consecuencias) |
| Consultar `/helix/users` para el id del canal | Una llamada de más: `validate` ya devuelve el `user_id` |
| Pedir `user:bot` y `channel:bot` | Solo valen con un token de aplicación; el WebSocket exige uno de usuario |

## Consecuencias

- **Duplicación con Kick**, anotada para decidir después si se extrae a un crate común: el servidor de
  callback (`callback.rs`), el reloj (`clock.rs`), `needs_refresh` y `covers` (`token.rs`), el
  `TokenManager` (`session.rs`), la generación del `state` y los servidores falsos de pruebas
  (`test_support.rs`). Lo que cambia es el endpoint, la forma de la respuesta (lista de permisos), la
  ausencia de PKCE y el nombre de la plataforma en `CredentialStore`.
- Hay que registrar la dirección de callback en la aplicación de Twitch y que el puerto esté libre.
- Los tokens viven en `credentials.toml` con los límites del [ADR-0012](0012-almacen-de-credenciales.md).
- El OAuth y la API se prueban sin red, con servidores HTTP locales falsos; falta un inicio de sesión
  real contra `id.twitch.tv`.
- **Sin contrastar** al escribir este ADR: el permiso `moderator:read:followers` de `channel.follow`
  v2 (la condición con `moderator_user_id` sí se leyó) y que Twitch no admita o exija PKCE para este
  flujo.
- Si F3 añade acciones de moderación, pedirá permisos nuevos y obligará a autorizar otra vez: hoy solo
  se piden los de lectura y el de escritura de chat de la decisión 4.
