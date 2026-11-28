# WT-0.4.1_01: aplicar antispam por plataforma de destino

## Contexto

El motor asociaba el intervalo entre envíos y el enfriamiento de respuestas repetidas a la plataforma
donde se recibió el comando. Si este respondía a otra plataforma, esa plataforma de destino no quedaba
protegida por sus límites locales.

## Cambios

- `syon-engine`: cada destino verifica su propio intervalo y enfriamiento de respuesta idéntica. Los
  destinos bloqueados se omiten individualmente; los destinos disponibles aún reciben la respuesta.
- Diseño 005: se precisa que los límites se aplican por plataforma de destino y cómo se tratan los
  comandos con varios destinos.
- Estado del proyecto: se anota la corrección en la fila de F3 parte 2.

## Parche

- `0001-fix-engine-limitar-antispam-por-destino.patch`: corregir los enfriamientos por destino y
  documentar la regla.

## Archivos afectados

- `crates/syon-engine/src/commands.rs`
- `docs/designs/005-comandos-filtros-antispam.md`
- `docs/estado.md`
- `docs/walkthroughs/WT-0.4.1_01.md`

## Verificación

- La prueba `enfriamientos_se_aplican_en_la_plataforma_de_destino` falló antes del cambio y pasó
  después.
- `cargo test -p syon-engine commands::tests --locked`
- `cargo fmt --all --check`
- `cargo clippy --workspace --all-targets --locked -- -D warnings`
- `cargo test --workspace --locked`
- `cargo xtask docs-check`
- `cargo xtask arch-check`
- Reaplicación del parche con `git am` sobre un clon limpio y repetición de las comprobaciones.

## Riesgos conocidos

- La configuración puede seguir rechazando el envío en el servidor; el motor no reintenta esos errores.
- El contraste en vivo de los campos opcionales del feed de Kick sigue pendiente.
