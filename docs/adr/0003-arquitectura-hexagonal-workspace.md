# ADR-0003: Arquitectura hexagonal en un workspace de Cargo

- Estado: Aceptada
- Fecha: 2026-11-24

## Contexto

SYON debe igualar las capacidades de MiniKick (plataformas, chat, comandos,
recompensas, TTS, música, overlays, webhooks) sin repetir su problema
principal: mezcla de responsabilidades y código difícil de entender.

## Decisión

Workspace de Cargo con crates pequeños. Las dependencias solo apuntan hacia
`syon-domain`; todo lo externo vive en adaptadores que implementan traits
definidos en `syon-app`.

```
crates/
  syon-domain/          tipos puros (Evento, Mensaje, Comando, Recompensa); sin I/O
  syon-app/             casos de uso y puertos (traits): fuentes de chat, TTS, almacenamiento, medios
  syon-platform-kick/   adaptador de Kick (parte del prototipo)
  syon-platform-twitch/ adaptador de Twitch
  syon-overlay-server/  HTTP + WebSocket local para los overlays de OBS
  syon-engine/          ensambla todo; se ejecuta sin interfaz
ui/                     cliente del engine, reemplazable (ver ADR-0005)
```

Los adaptadores restantes (TTS, música, almacenamiento, webhooks) se crean en
su fase (ver `docs/roadmap.md`), no antes.

El engine funciona sin interfaz: la UI es un cliente intercambiable.

### Prototipo existente

El código de `src/` (prototipo de consola de Kick) se conserva como
referencia ejecutable en `prototypes/kick-console/`, fuera del workspace, y
se migra por partes a `syon-domain` y `syon-platform-kick` en F1-F2. Se
elimina al alcanzar paridad. El traslado se hace en F0, cuando el repositorio
incluya su `Cargo.toml`.

## Alternativas consideradas

- Un solo crate con módulos: más simple al inicio, pero los límites quedan
  solo por convención, justo lo que falló en MiniKick.
- Mover el prototipo directamente a los crates: pierde la referencia
  ejecutable mientras se construyen las piezas nuevas.

## Consecuencias

- Cambiar una librería con errores toca un único adaptador.
- Más crates implican más configuración inicial; se compensa creándolos solo
  cuando una fase los necesita (YAGNI).
- Un test de arquitectura verificará la dirección de dependencias; llega en F1 con el primer crate de dominio, porque antes no hay dependencias entre crates que comprobar.
