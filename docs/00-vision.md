# Visión de SYON

SYON es una aplicación de escritorio para streamers que reconstruye desde cero
las capacidades de MiniKick en Rust: vincular plataformas de streaming,
unificar el chat y reaccionar a sus eventos con comandos, recompensas, voz
(TTS), música y overlays.

## Problemas que resuelve respecto a MiniKick

- El código crecía sin que quedara claro qué hacía cada parte.
- Había que corregir librerías con errores de forma recurrente.

## Objetivos

1. Paridad funcional con MiniKick (ver [paridad](requirements/paridad.md)).
2. Cada parte con un propósito único y documentado.
3. Dependencias pocas, fijas y reemplazables (ADR-0004).
4. Un engine que corre sin interfaz y una UI intercambiable (ADR-0003).

## No objetivos (por ahora)

- Cumplir paridad de golpe: se avanza por rebanadas verticales (ver [roadmap](roadmap.md)).
- Elegir ya el motor de voz: se decidirá en la fase de TTS con su propio ADR.

## Principios de arquitectura

1. Inversión de dependencias: interfaces antes que implementaciones concretas;
   los proveedores se pueden cambiar sin tocar la lógica de negocio.
2. Separación de responsabilidades: datos, lógica y presentación en capas distintas.
3. Alta cohesión: cada crate o módulo tiene un único propósito.
4. DRY: lógica repetida centralizada; si la duplicación es mínima, se prefiere la legibilidad.
5. Abstracción justificada (YAGNI): solo se abstrae cuando el patrón de repetición es claro.

## Cómo se trabaja

- Cada funcionalidad empieza con un documento de diseño en `docs/designs/`.
- Los cambios se entregan como parches atómicos (`git am`) con Conventional Commits.
- Cada cambio completo cierra con un walkthrough en `docs/walkthroughs/`.
- Las decisiones se registran en `docs/adr/`.
