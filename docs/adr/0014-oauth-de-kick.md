# ADR-0014: OAuth 2.1 de Kick con PKCE, callback local y cliente HTTP `reqwest`

- Estado: Aceptada
- Fecha: 2026-11-26

## Contexto

Para leer el canal y, en F3, escribir en el chat, SYON necesita tokens de usuario de Kick
([diseño 002](../designs/002-plataformas-kick-twitch.md), secciones 5 y 6). El prototipo
(`prototypes/kick-console/src/oauth.rs`, [WT-0.1.0_01](../walkthroughs/WT-0.1.0_01.md)) ya resuelve el
flujo; hay que migrarlo respetando la separación de capas y el almacén de credenciales
([ADR-0012](0012-almacen-de-credenciales.md)), y decidir las dependencias que añade.

## Decisión

1. **Flujo de código de autorización con PKCE S256**, con `state` aleatorio y un callback en un puerto
   local (`localhost` o `127.0.0.1`, por defecto `http://localhost:8080/auth/callback`). El callback solo
   acepta un `GET` a su ruta con el `state` esperado; cualquier otra petición recibe un 404 y se sigue
   esperando. Se espera hasta 5 minutos.
2. **Permisos desde el primer inicio de sesión:** `user:read`, `channel:read` y `chat:write`, para no
   volver a autorizar en F3. Si las credenciales guardadas no cubren los permisos pedidos, se inicia sesión
   de nuevo.
3. **Renovación** con el `refresh_token` cuando faltan menos de 60 s para vencer, y a la fuerza cuando
   Kick responde 401 (`force_refresh`). Si Kick no entrega un `refresh_token` nuevo se conserva el
   anterior. Si la renovación falla se pide un inicio de sesión nuevo, y queda un aviso en el registro.
4. **Separación de responsabilidades:** `OAuthClient` solo habla con Kick y no guarda nada;
   `TokenManager` decide reutilizar, renovar o iniciar sesión, y persiste solo a través del puerto
   `CredentialStore`. `KickAppConfig` llega por constructor: el adaptador no lee `.env`, el entorno ni
   los ajustes (lo decide el engine o el binario). Su `client_secret` no se imprime ni se guarda.
5. **El tipo del cliente HTTP no sale del crate:** `OAuthClient::new` construye su propio cliente. Los
   errores son un enum tipado (`OAuthError`) cuyos mensajes nunca incluyen tokens, el secreto ni el
   contenido de una respuesta ilegible.
6. **Dependencias nuevas** (versiones en `[workspace.dependencies]`):
   - `reqwest` 0.12 sin funciones por defecto, con `json` y `rustls-tls`: cliente HTTP (y, más adelante,
     de la API de las plataformas). TLS con `rustls`, sin depender de OpenSSL del sistema.
   - `sha2` 0.10 y `base64` 0.22: el desafío PKCE.
   - `rand` 0.9: bytes aleatorios del `verifier` y del `state`.

## Alternativas consideradas

| Alternativa | Por qué no |
|-------------|-----------|
| `oauth2` (crate) | Añade un modelo de tipos y un cliente de más para un flujo de dos peticiones que ya está probado en el prototipo |
| `ureq` o `hyper` directo en lugar de `reqwest` | `ureq` es bloqueante y SYON ya usa `tokio`; `hyper` exige más código para formularios y JSON |
| `reqwest` con `native-tls` | Depende de OpenSSL o de la pila del sistema, lo que complica la distribución en Windows (F10) |
| Servidor de callback con una librería HTTP | Para una única petición de navegador bastan unas líneas sobre `TcpListener` |
| Pedir los permisos de escritura en F3 | Obligaría a autorizar otra vez a quien ya inició sesión |

## Consecuencias

- Hace falta registrar en la aplicación de Kick la dirección de callback y que el puerto esté libre; si
  está ocupado el error lo indica.
- Los tokens viven en `credentials.toml` con los límites del ADR-0012.
- El OAuth se prueba sin red, con servidores HTTP locales falsos y el flujo completo de principio a fin
  (callback, intercambio del `code` y verificación del desafío PKCE). Falta contrastarlo con Kick real.
- `reqwest` es pre-1.0 y arrastra `rustls`, `hyper` y `ring`; las actualizaciones van en un parche propio.
- Los endpoints (`id.kick.com`) y los permisos son los del prototipo y no se han contrastado en vivo.
