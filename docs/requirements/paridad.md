# Paridad con MiniKick

Borrador construido a partir del estado conocido de MiniKick; debe validarse
contra su código antes de cerrar F0. Estados: `Pendiente` · `En curso` · `Hecho`.

| Capacidad | En MiniKick | Fase SYON | Estado |
|-----------|-------------|-----------|--------|
| Kick: chat y eventos por WebSocket, OAuth, API pública | Sí | F2 | En curso (eventos, OAuth y feed migrados a `syon-platform-kick`; falta el contraste en vivo y la API pública, F3) |
| Twitch: chat y eventos | Sí | F2 | En curso (OAuth, cliente de Helix, traducción de eventos y conector de EventSub en `syon-platform-twitch`; falta conectarlo al engine, el contraste en vivo y el envío de chat, F3) |
| Chat unificado con 1, 2 o todas las plataformas | Sí | F3 | Pendiente |
| Comandos (incluidos los de moderador) y filtros de spam | Sí | F3 | Pendiente |
| TTS con varios motores (edge-tts, Piper y uno local) | Sí | F4 | Pendiente (motores por decidir) |
| Recompensas, con miniaturas | Sí | F5 | Pendiente |
| Overlays de alertas, música y chat | Sí | F6 | Pendiente |
| Widgets: temporizador y cronómetro, explosión de emotes por plataforma, todos ocultables | Sí | F6 | Pendiente |
| Música: `!sr` con enlaces de YouTube, cola, historial, playlists, vista previa y caché local | Sí | F7 | Pendiente |
| PO token para YouTube | Sí | F7 | Pendiente |
| Webhooks salientes | En diseño (v1.6.9) | F8 | Pendiente |
| Ajustes persistentes en TOML con interfaz de proveedor | Sí | F1 | Hecho (`SettingsStore` en `syon-app`, `syon-storage-toml` y `Engine::start`; hoy solo `[logging]`, el resto se añade con cada fase) |
| Interfaz de escritorio (temas, iconos, i18n) | Sí | F9 | Pendiente |
| Instalador y actualizador | Sí | F10 | Pendiente |
| Registro de diagnóstico en archivo | Sí | F1 | Hecho (`syon.log` de 16 MB con rotación, marca por arranque, redacción de secretos y registro de pánicos, [ADR-0006](../adr/0006-registro-de-diagnostico.md)) |
