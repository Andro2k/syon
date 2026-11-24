# ADR-0002: Lenguaje: Rust

- Estado: Aceptada
- Fecha: 2026-11-24

## Contexto

En MiniKick (Python/PySide6) el código se fue acumulando hasta perder claridad
sobre qué hace cada parte, y hubo que dedicar tiempo repetido a corregir
librerías con errores. Ya existe un prototipo funcional en Rust del listener
de Kick (ver `docs/walkthroughs/WT-0.1.0_01.md`).

## Decisión

SYON se escribe en Rust, con un workspace de Cargo (ver ADR-0003).

## Alternativas consideradas

Seguir refactorizando MiniKick en Python. Se descartó por decisión del autor
tras los problemas de mantenibilidad descritos arriba.

## Consecuencias

- El sistema de tipos y la visibilidad por crate hacen cumplir los límites
  entre capas, algo que en MiniKick dependía de disciplina.
- Las piezas sin equivalente maduro en Rust (por ejemplo yt-dlp) se usarán
  como procesos externos con versión fija (ver ADR-0004).
- Las librerías de interfaz en Rust son menos maduras (ver ADR-0005).
