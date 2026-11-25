# SYON

Aplicación de escritorio para streamers escrita en Rust: vincula plataformas de
streaming y reacciona a su chat y eventos con comandos, recompensas, voz, música
y overlays. Reconstruye MiniKick con paridad funcional.

Workspace de Cargo, edición 2024, resolver 3. Las reglas de este archivo valen
para cualquier persona o herramienta que cambie el repositorio.

## Dónde está cada cosa

- [Visión](docs/00-vision.md), [roadmap](docs/roadmap.md) y
  [paridad con MiniKick](docs/requirements/paridad.md): qué se construye y en qué orden.
- [ADR](docs/adr/README.md): decisiones ya tomadas. No se contradicen sin otro ADR.
- [Diseños](docs/designs/README.md): se escriben antes del código de cada fase.
- [Walkthroughs](docs/walkthroughs/): cierre documentado de cada cambio.
- [Registro de dependencias](docs/deps.md).
- `prototypes/kick-console/`: referencia ejecutable del listener de Kick. Fuera del
  workspace, con su propio `Cargo.lock`; solo se toca para migrarlo a los crates y
  se elimina al alcanzar paridad.

## Disposición de crates

Estado: F1 en curso. `syon-domain` ya existe; los otros tres se crean en los parches de
implementación del [diseño 001](docs/designs/001-nucleo-dominio-eventos.md).

```text
crates/
  syon-domain/          tipos puros: eventos, usuarios, marca de tiempo (sin I/O ni async)
  syon-app/             puertos (traits) y casos de uso
  syon-storage-toml/    adaptador de ajustes sobre TOML
  syon-engine/          ensambla todo: bus, rutas, logging; corre sin interfaz
xtask/                  tareas de mantenimiento (cargo xtask)
```

Los crates de plataformas, TTS, música, overlays y la UI se crean en su fase, no
antes (ver [ADR-0003](docs/adr/0003-arquitectura-hexagonal-workspace.md)).

La dirección de dependencias es estricta. No se crea una arista de vuelta:

```text
syon-engine → adaptadores → syon-app → syon-domain
```

- `syon-domain` no tiene dependencias externas ni del workspace.
- `syon-app` solo depende de `syon-domain`.
- Un adaptador no depende de otro adaptador ni de `syon-engine`.
- Solo `syon-engine` y el binario nombran adaptadores concretos; el resto ve traits y modelos.
- La UI es un cliente del engine y nunca contiene lógica de negocio.

`cargo xtask arch-check` hará cumplir esto en CI desde F1.

## Reglas

- **Diseño antes que código.** Cada fase empieza con un documento en `docs/designs/`.
  Una decisión duradera o una dependencia nueva lleva su ADR.
- **Inversión de dependencias.** La lógica conoce traits, no implementaciones. Cambiar
  una librería con errores toca un único adaptador.
- **Separación de responsabilidades.** Datos, lógica y presentación en capas distintas;
  sin consultas, cálculos y formato en el mismo componente.
- **Alta cohesión.** Cada crate o módulo tiene un propósito; lo que cruza dos, se divide.
- **DRY sin excesos.** La lógica repetida se centraliza; con una duplicación de dos o
  tres líneas simples se prefiere la legibilidad.
- **Abstracción justificada (YAGNI).** Un trait o un crate nuevo necesita un patrón de
  repetición claro o más de una implementación real.
- **El bus nunca bloquea.** `EventSink::publish` no espera; un consumidor lento pierde
  sus propios eventos, queda contado y registrado, y no frena a las plataformas.
- **Dependencias.** Versiones en `[workspace.dependencies]` y `Cargo.lock` versionado.
  Un crate joven se fija exacto (`=x.y.z`) con un comentario que lo explique. Toda
  dependencia directa nueva: ADR, fila en `docs/deps.md` y `cargo deny` en verde. Las
  licencias copyleft requieren un ADR explícito. Lo que dependa de Python o de un
  navegador corre como proceso externo con versión fija, detrás de un trait.
- **Secretos.** Nunca en el repositorio ni en el log. Los tipos que contienen un secreto
  implementan `Debug` a mano. `.env` y los archivos de tokens están en `.gitignore`.
- **Sin `unsafe`.** El workspace lo prohíbe.
- **Pruebas.** Cada crate nuevo trae las pruebas de su criterio de salida, en
  `#[cfg(test)] mod tests` al final del archivo que cubren.
- **Código generado.** Se regenera, no se edita a mano.

## Estilo de código

- Identificadores en inglés; documentación, comentarios y mensajes de log y de error
  en español, como el prototipo.
- Cada tipo y cada función cuyo comportamiento no sea evidente por su nombre lleva un
  comentario `///` de una o dos frases: para qué sirve y qué debe saber quien lo llama.
- Orden de un módulo: `use`, constantes, tipos, `impl`, funciones privadas al final.
- Errores tipados de los puertos: enums propios con `Display` y `Error` escritos a mano,
  sin `thiserror`; `anyhow::Result` en los bordes con `.context("no se pudo …")` en
  minúscula.
- Los logs llevan el prefijo del subsistema: `"bus: …"`, `"ajustes: …"`.
- Clippy sin avisos con `--all-targets`; `#[allow]` solo con un comentario que lo justifique.

## Comprobaciones

Antes de entregar cualquier cambio de código:

```text
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo xtask docs-check
```

Para dependencias: `cargo deny --manifest-path prototypes/kick-console/Cargo.toml check`
y `cargo machete .` (ver [docs/deps.md](docs/deps.md)). La CI ejecuta lo mismo en
Ubuntu y Windows.

## Flujo de trabajo y entregas

1. Antes de empezar, `git pull` en la rama activa.
2. Los cambios se entregan como archivos `.patch` de `git format-patch`, numerados
   `0001-<slug>.patch`, `0002-<slug>.patch`, listos para `git am 00##.patch`.
3. Autor de cada commit: `Andro2k <kirito100crack@gmail.com>`, con la coautoría en el
   mensaje:

   ```text
   Co-authored-by: Claude (https://github.com/claude) <noreply@anthropic.com>
   ```

4. Cada parche es atómico, pasa las comprobaciones de arriba antes de exportarse y
   sigue Conventional Commits: `tipo(scope): descripción` en imperativo y minúscula,
   con cuerpo cuando explica el motivo.
5. Scopes en uso: `repo`, `workspace`, `deps`, `ci`, `docs`, `design`, `adr`, `agents`,
   `readme`, `walkthrough`, `xtask`, `toolchain`, `prototype`, y por crate (`domain`,
   `app`, `engine`, `storage`) cuando existan.
6. Al cerrar un cambio o un conjunto de parches se escribe
   `docs/walkthroughs/WT-<versión>_<NN>.md` con contexto, cambios por capa, parches,
   archivos afectados, verificación ejecutada y riesgos conocidos.
7. Se actualizan en el mismo parche que los provoca la tabla de paridad, `docs/deps.md`
   y el índice de ADR o de diseños que corresponda.

## Referencias externas

[Sonora](https://github.com/sonorahq/sonora) se usa como referencia de diseño. Su
licencia es GPL-3.0-or-later: se toman ideas, no código.
