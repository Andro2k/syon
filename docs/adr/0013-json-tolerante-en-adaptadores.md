# ADR-0013: JSON tolerante con `serde_json::Value` en los adaptadores de plataforma

- Estado: Aceptada
- Fecha: 2026-11-26

## Contexto

Los adaptadores de Kick y Twitch reciben JSON cuya forma no controlan: el feed de Kick no es oficial y
cambia sin aviso ([diseño 002](../designs/002-plataformas-kick-twitch.md), sección 15), y los eventos de
Twitch añaden campos con el tiempo. El prototipo ya resolvió esto leyendo cada campo por su ruta y
degradando a un evento desconocido ante cualquier ausencia. Hace falta una librería JSON y una
regla para usarla ([ADR-0004](0004-politica-de-dependencias.md)).

## Decisión

1. **`serde_json` 1** como dependencia directa de los adaptadores de plataforma, centralizada en
   `[workspace.dependencies]`. No la usan `syon-domain`, `syon-app` ni `syon-engine`.
2. **Lectura tolerante con `serde_json::Value`** en la frontera de cada plataforma: se busca cada campo
   por su ruta y, si falta o tiene otro tipo, el evento se degrada a `EventKind::Unknown` en lugar de
   fallar. No se derivan structs rígidos para las cargas de eventos.
3. **El tipo `Value` no sale del adaptador** salvo en la función de entrada que reciba el
   contenido ya decodificado; el dominio y los puertos no lo conocen.
4. **Los secretos no se deserializan a structs con `Debug` derivado**: se mantiene la regla de
   AGENTS.md.

## Alternativas consideradas

| Alternativa | Por qué no |
|-------------|-----------|
| Structs con `#[derive(Deserialize)]` por evento | Un campo ausente o renombrado haría fallar todo el evento; la lectura por ruta degrada solo lo que falta |
| `simd-json` o `sonic-rs` | Más rápidos, pero el volumen de un chat no lo necesita y suman dependencias |
| `miniserde` o `nanoserde` | Menos ecosistema y sin la lectura por ruta sobre un `Value` genérico |
| Interpretar el texto a mano | Código propio que mantener y fácil de equivocar con escapes y números |

## Consecuencias

- Un cambio de nombre de campo en una plataforma degrada el evento a `Unknown` sin romper el resto; hay
  que mirar el registro para notarlo.
- Las pruebas de cada adaptador usan cargas de ejemplo (`json!`) sin red.
- `serde_json` trae `itoa`, `ryu` y `memchr`, de licencia MIT o Apache-2.0, aceptadas por `deny.toml`.
