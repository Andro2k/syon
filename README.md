# SYON

Aplicación de escritorio para streamers escrita en Rust: vincula plataformas
de streaming y reacciona a su chat y eventos con comandos, recompensas, voz,
música y overlays. Es la reconstrucción de MiniKick.

Estado: F0 completa (base del repositorio). El prototipo de consola de Kick
vive en [prototypes/kick-console](prototypes/kick-console/README.md) y está
descrito en [WT-0.1.0_01](docs/walkthroughs/WT-0.1.0_01.md).

## Comprobaciones locales

```text
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo xtask docs-check
```

La auditoría de dependencias se describe en [docs/deps.md](docs/deps.md). Las reglas de
arquitectura, estilo y entrega están en [AGENTS.md](AGENTS.md).

## Documentación

- [Visión](docs/00-vision.md)
- [Roadmap](docs/roadmap.md)
- [Estado actual y traspaso](docs/estado.md)
- [Paridad con MiniKick](docs/requirements/paridad.md)
- [Decisiones de arquitectura (ADR)](docs/adr/README.md)
- [Registro de dependencias](docs/deps.md)
- [Walkthroughs](docs/walkthroughs/)
