# ADR-0005: Interfaz: GPUI directo desde Rust

- Estado: Aceptada (a validar con un spike en F9)
- Fecha: 2026-11-24

## Contexto

Los overlays son páginas web servidas a OBS, así que el engine puede correr
sin interfaz y la UI es un cliente intercambiable (ADR-0003). Se evaluó
GPUI, el framework de interfaz de Zed.

## Decisión

La interfaz de escritorio usará **GPUI directamente desde Rust**.

## Alternativas consideradas

- **GPUIX** (bindings de Node/Bun para GPUI con React o Solid): descartada.
  Añade un runtime y npm, además de un fork de Zed como dependencia
  indirecta, y obliga a cruzar Rust, JS y GPUI para hablar con un engine que
  ya está en Rust.
- Tauri, Slint, iced, egui y Qt vía cxx-qt: quedan como respaldo si el spike
  de F9 no valida GPUI.

## Consecuencias

Riesgos conocidos:

- La API es pre-1.0 y cambia con frecuencia entre versiones.
- Existen varias distribuciones independientes del mismo código (`gpui-pre`,
  `gpui-box`, `bezel-gpui`); mezclarlas rompe los tipos. Habrá que fijar una
  sola distribución o revisión.
- La documentación es escasa; se aprende del código de Zed.
- Hasta donde se sabe no trae webview, por lo que la vista previa de
  overlays dentro de la app debe verificarse en el spike.

Criterios del spike de F9:

1. Estabilidad de la API entre las últimas versiones.
2. Soporte en Windows y Linux.
3. Lista virtualizada para el chat y campos de texto reales.
4. Fuentes (Meslo Nerd Font) e iconos.
5. Vista previa de overlays.
6. Empaquetado con instalador.
