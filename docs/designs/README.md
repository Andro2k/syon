# Documentos de diseño

Un documento por funcionalidad o fase, escrito **antes** de implementarla. Un
diseño no se borra: si cambia, se corrige y se anota el motivo. Las decisiones
que implican adoptar una dependencia o fijar una regla duradera terminan en un
ADR (ver [docs/adr](../adr/README.md)).

Estados: `Propuesta` · `Aceptado` · `Implementado` · `Descartado`.

| Diseño | Título | Fase | Estado |
|--------|--------|------|--------|
| [001](001-nucleo-dominio-eventos.md) | Núcleo: dominio, bus de eventos, configuración y logging | F1 | Implementado |
| [002](002-plataformas-kick-twitch.md) | Plataformas: Kick y Twitch | F2 | Aceptado |
| [003](003-arranque-de-plataformas-y-binario.md) | Arranque de plataformas y binario | F2 | Aceptado |
| [004](004-chat-unificado-y-envio.md) | Chat unificado y envío de mensajes | F3 | Aceptado |
| [005](005-comandos-filtros-antispam.md) | Comandos, filtros y antispam | F3 | Implementado |
| [006](006-tts.md) | TTS con Kokoro: voz local para el chat | F4 | Propuesta |
