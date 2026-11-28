# syon-domain

Tipos puros del dominio de SYON: los eventos que entregan las plataformas de
streaming, los usuarios y la marca de tiempo. Es la base de la que dependen
todos los demás crates (ver [ADR-0003](../../docs/adr/0003-arquitectura-hexagonal-workspace.md)).

## Qué hace

- Define `Platform`, `User`, `Timestamp`, `Event`, `EventKind`, `ChatMessage`, `ChatRoles` y
  `MessageId` (diseño en [001](../../docs/designs/001-nucleo-dominio-eventos.md), sección 4).
- Convierte un `std::time::SystemTime` en `Timestamp` de forma pura.

## Qué no hace

- No hace I/O, no es asíncrono y no lee el reloj: quien publica un evento llama a
  `SystemTime::now()` y pasa el resultado.
- No tiene dependencias externas ni de otros crates del workspace.
- No conoce el formato de ninguna plataforma: el JSON original se queda en cada adaptador.
- `ChatMessage` conserva el nombre visible, el id opcional de autor, roles normalizados y si se sabe
  que el autor es la cuenta propia.

## API pública

| Tipo | Para qué sirve |
|------|----------------|
| `Platform` | Plataforma de origen (`Kick`, `Twitch`) |
| `User` | Persona que escribe o actúa, por nombre visible |
| `Timestamp` | Milisegundos Unix en UTC, ordenable |
| `MessageId` | Identificador de un mensaje de chat |
| `ChatMessage` | Mensaje de chat con id, autor, identidad opcional, roles y texto |
| `ChatRoles` | Roles conocidos: broadcaster, moderator, vip y subscriber |
| `Event` | Plataforma, hora de recepción y `EventKind` |
| `EventKind` | Qué ocurrió: chat, borrado, ban, suscripción, recompensa, directo... |
