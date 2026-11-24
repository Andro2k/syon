# kick-console (prototipo)

Prototipo de consola que escucha el chat y los eventos de un canal de Kick y
envía mensajes con OAuth 2.1. Es una referencia ejecutable fuera del
workspace (ADR-0003): su código se migra por partes a `syon-domain` y
`syon-platform-kick` en F1-F2 y la carpeta se elimina al alcanzar paridad.

Descripción técnica completa: [WT-0.1.0_01](../../docs/walkthroughs/WT-0.1.0_01.md).

## Uso

Desde esta carpeta:

```text
cp .env.example .env        # completar KICK_CLIENT_ID y KICK_CLIENT_SECRET
cargo run -- <canal> --raw  # solo escuchar
cargo run -- --login        # iniciar sesión OAuth
```

`.env` se busca en la carpeta actual y en las superiores, así que también
funciona un `.env` en la raíz del repositorio. El archivo de tokens
(`.kick_tokens.json`) se crea en la carpeta desde la que se ejecuta.
