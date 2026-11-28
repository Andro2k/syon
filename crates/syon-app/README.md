# syon-app

Puertos (traits) y casos de uso de SYON. Define lo que el núcleo necesita del
exterior sin saber cómo se obtiene: por dónde entran los eventos y dónde se
guardan los ajustes. Solo depende de [`syon-domain`](../syon-domain/README.md)
(ver [ADR-0003](../../docs/adr/0003-arquitectura-hexagonal-workspace.md) y el
[diseño 001](../../docs/designs/001-nucleo-dominio-eventos.md), sección 5).

## Qué hace

- `EventSink`: puerto por el que las plataformas entregan eventos.
- `SettingsStore` y `SettingsError`: puerto de lectura y escritura de ajustes.
- `Settings`, `LoggingSettings`, `ChatSettings` y `ChatCommand`: modelo plano de ajustes de registro,
  conexiones, comandos, filtros por usuario/texto, destinos y antispam (enfriamiento de comando,
  respuesta idéntica y ritmo por plataforma). No serializa ni conoce TOML.

## Qué no hace

- No implementa ningún puerto: el bus vive en `syon-engine` y el formato TOML en
  `syon-storage-toml`.
- No conoce ningún formato de archivo ni de red; no usa `serde`.
- No es asíncrono: `EventSink::publish` es una llamada que nunca espera.
- No tiene dependencias externas.

## API pública

| Elemento | Para qué sirve |
|----------|----------------|
| `EventSink` | `publish(&self, Event)`; no bloquea y se comparte entre hilos |
| `SettingsStore` | `load` y `save` de los ajustes del usuario |
| `SettingsError` | `Io` (acceso al archivo) o `Invalid` (contenido inválido), con la ruta |
| `Settings` | Versión de esquema y secciones de ajustes, incluida la configuración inerte del chat |
| `LoggingSettings` | Nivel del archivo de log (`LogLevel`, por defecto `Info`) |
