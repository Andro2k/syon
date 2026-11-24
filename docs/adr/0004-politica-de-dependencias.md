# ADR-0004: Política de dependencias

- Estado: Aceptada
- Fecha: 2026-11-24

## Contexto

En MiniKick hubo que acomodar librerías con errores de forma recurrente. En
SYON las dependencias deben ser pocas, justificadas y reemplazables.

## Decisión

1. `Cargo.lock` se versiona y las actualizaciones van en un parche propio
   `chore(deps)`.
2. Toolchain fijada en `rust-toolchain.toml`.
3. Antes de adoptar una dependencia relevante se escribe un ADR corto y, si
   hay duda, un spike descartable.
4. Toda dependencia externa que toque I/O va detrás de un trait de
   `syon-app` y dentro de un adaptador.
5. Lo que dependa de Python o de un navegador (yt-dlp, generación de PO
   token, algunos motores de voz) se ejecuta como proceso externo con
   versión fija, también detrás de un trait.
6. Auditoría automática: `cargo-deny` (licencias, avisos de seguridad,
   fuentes) y `cargo-machete` (dependencias sin uso).
7. Cada dependencia directa se registra en `docs/deps.md` con motivo,
   alternativa y riesgo.
8. Las licencias copyleft requieren un ADR explícito antes de añadirse.

## Alternativas consideradas

Aceptar versiones flotantes y corregir al romperse: es el comportamiento que
se quiere evitar.

## Consecuencias

- Adoptar una librería cuesta más al principio, pero las roturas quedan
  acotadas a un adaptador.
- `deny.toml` y `docs/deps.md` se crean junto al `Cargo.toml` en F0.
