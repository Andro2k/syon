# WT-0.1.0_02: base del repositorio (F0)

## Contexto

El repositorio tenía el prototipo de consola de Kick en `src/`, un
`.gitignore` reducido a tres líneas y ninguna herramienta de calidad. F0 deja
la base para construir SYON por fases: prototipo trasladado, workspace de
Cargo, comprobaciones automáticas de documentación y dependencias, y CI. No
cambia el comportamiento del prototipo.

## Cambios por capa

**Repositorio y toolchain**: `.gitignore` completo (secretos, logs, editores;
`target/` sin ancla para cubrir `prototypes/`), `.gitattributes` con fines de
línea LF y `rust-toolchain.toml` fijado en Rust 1.91 con rustfmt y clippy.

**Prototipo** (`prototypes/kick-console/`): traslado sin cambios de código de
`src/`, `Cargo.toml`, `Cargo.lock` y `.env.example`, con README de uso. Queda
fuera del workspace con su propio lock (ADR-0003).

**Workspace y herramientas** (`Cargo.toml`, `xtask/`): workspace virtual
(resolver 3, edición 2024, `unsafe_code` prohibido, `clippy::all` como aviso)
y `cargo xtask docs-check`, que falla ante enlaces relativos rotos, un ADR
fuera del índice o un crate sin README. Sin dependencias externas.

**Dependencias** (`deny.toml`, `docs/deps.md`): auditoría con `cargo-deny`
(avisos, licencias, fuentes, duplicados) y registro de las dependencias
directas del prototipo con motivo, alternativa y riesgo.

**CI** (`.github/workflows/ci.yml`): `check` en Ubuntu y Windows (fmt, clippy
con `-D warnings`, pruebas y `docs-check`, sobre workspace y prototipo) y
`audit` (`cargo-deny` y `cargo-machete`).

## Parches

1. `chore(repo)`: toolchain, fines de línea y `.gitignore`.
2. `refactor(prototype)`: traslado del prototipo a `prototypes/kick-console/`.
3. `build(workspace)`: workspace y `xtask docs-check`; el test de dirección de
   dependencias pasa de F0 a F1.
4. `chore(deps)`: `url` 2.5.9 por RUSTSEC-2026-0592.
5. `build(deps)`: `deny.toml` y `docs/deps.md`.
6. `ci`: flujo de GitHub Actions.
7. `docs`: este walkthrough y el README.

## Archivos afectados

- Nuevos: `.gitattributes`, `rust-toolchain.toml`, `Cargo.toml`, `Cargo.lock`,
  `.cargo/config.toml`, `xtask/`, `deny.toml`, `docs/deps.md`,
  `.github/workflows/ci.yml`, `prototypes/kick-console/README.md`,
  `docs/walkthroughs/WT-0.1.0_02.md`.
- Movidos: `src/`, `Cargo.toml`, `Cargo.lock` y `.env.example` a
  `prototypes/kick-console/`.
- Modificados: `.gitignore`, `docs/roadmap.md`,
  `docs/adr/0003-arquitectura-hexagonal-workspace.md`, `README.md`,
  `prototypes/kick-console/Cargo.toml` (`publish = false`) y
  `prototypes/kick-console/Cargo.lock` (`url` y `form_urlencoded`).

## Verificación ejecutada

- Rust 1.91.1 (paquetes de Ubuntu, sin rustup). En el workspace:
  `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked
  -- -D warnings` y `cargo test --workspace --locked` (3 pruebas de
  `docs-check`), sin avisos.
- En el prototipo, con `--locked`: `cargo test` (35 aprobadas, 0 fallidas),
  `cargo clippy -D warnings` y `cargo fmt --check`, antes del traslado y
  después de él y de la actualización de `url`.
- `cargo xtask docs-check` sin problemas; con un enlace roto añadido a propósito
  falla con código 1.
- `cargo deny check` sobre el prototipo: avisos, licencias, fuentes y bans en
  verde; quedan avisos de versiones duplicadas (`base64`, `getrandom`, `syn`,
  `webpki-roots`) que no bloquean.
- `cargo machete .`: sin dependencias sin uso.
- Los siete parches aplican con `git am` sobre un clon limpio de `master`
  (9b4af92) y repiten estas comprobaciones.
- Pendiente: el flujo de CI no se ha ejecutado (no hay Actions en el entorno de
  desarrollo); se valida solo la sintaxis YAML. Tampoco se ha probado en Windows.

## Riesgos conocidos

- `chrono` no tiene mantenimiento (RUSTSEC-2026-0543); el aviso está ignorado
  con motivo en `deny.toml` y se sustituirá al migrar el prototipo.
- Los nombres de entradas del flujo de `cargo-deny-action` y `install-action`
  deben confirmarse en la primera ejecución real de CI.
- `rust-toolchain.toml` fija la 1.91: si el entorno local usa otra versión,
  rustup descargará esa toolchain la primera vez.
- Si ya existe un `.env` local en la raíz del repositorio, sigue funcionando
  (se busca en carpetas superiores); el archivo de tokens se crea en la carpeta
  desde la que se ejecuta el prototipo.
