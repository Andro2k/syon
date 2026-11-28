#!/usr/bin/env bash
# Ejecuta las comprobaciones del repositorio y resume el resultado de cada una.
#
# Uso: ./validar.sh [directorio]      (por defecto, el directorio actual)
#
# Es una ayuda local, no sustituye a la CI: el formato, clippy y las pruebas del prototipo
# `prototypes/kick-console` solo los ejecuta la CI. Las comprobaciones ligeras (formato, auditoría de
# dependencias y dependencias sin usar) van en segundo plano mientras corren las pesadas. `cargo-deny`
# y `cargo-machete` son opcionales: si no están instalados se omiten, y el resumen lo dice.
#
# Cada copia de trabajo debe usar su propio directorio `target`: compartirlo entre copias del mismo
# repositorio puede reutilizar artefactos de otro código y dar resultados falsos.
set -uo pipefail
cd "${1:-.}" || exit 1
out=$(mktemp -d)
fails=0
SECONDS=0
have() { command -v "$1" >/dev/null 2>&1; }

# 1) Dependencias en línea una sola vez (rápido si ya están en caché); después, sin red.
cargo fetch --locked -q 2>"$out/fetch" || { echo "FALLA  fetch"; tail -3 "$out/fetch"; exit 1; }
[ -f prototypes/kick-console/Cargo.toml ] \
    && cargo fetch --locked -q --manifest-path prototypes/kick-console/Cargo.toml 2>>"$out/fetch"
export CARGO_NET_OFFLINE=true

# 2) Ligeras en segundo plano (no compilan). Cada una con su PID, para conservar su código de salida.
names=(); pids=()
bg() { local name=$1; shift; "$@" >"$out/$name" 2>&1 & names+=("$name"); pids+=("$!"); }
bg fmt cargo fmt --all --check
if have cargo-deny; then
    bg deny cargo deny --manifest-path Cargo.toml check
    [ -f prototypes/kick-console/Cargo.toml ] \
        && bg deny-prototipo cargo deny --manifest-path prototypes/kick-console/Cargo.toml check
else
    echo "omitido  deny (falta cargo-deny)"
fi
if have cargo-machete; then bg machete cargo machete .; else echo "omitido  machete (falta cargo-machete)"; fi

# 3) Pesadas en serie con fail-fast: si clippy falla no se pierde tiempo en las pruebas.
#    Las tareas de xtask son baratas y se ejecutan siempre.
step() {
    local name=$1; shift
    if "$@" >"$out/$name" 2>&1; then
        echo "ok       $name"
    else
        echo "FALLA    $name"; tail -8 "$out/$name" | sed 's/^/           /'
        fails=$((fails + 1)); return 1
    fi
}
{ step clippy cargo clippy --workspace --all-targets --locked -- -D warnings \
    && step test cargo test --workspace --locked; } || true
step docs-check cargo xtask docs-check || true
step arch-check cargo xtask arch-check || true

# 4) Recoger las ligeras con su estado real.
for i in "${!names[@]}"; do
    if wait "${pids[$i]}"; then
        echo "ok       ${names[$i]}"
    else
        echo "FALLA    ${names[$i]}"; tail -6 "$out/${names[$i]}" | sed 's/^/           /'
        fails=$((fails + 1))
    fi
done
if [ -f "$out/test" ]; then
    awk '/^test result/ {p += $4; f += $6} END {print "pruebas: " p " aprobadas, " f + 0 " fallidas"}' "$out/test"
fi
echo "total: ${SECONDS} s, fallos: ${fails}"
[ "$fails" -eq 0 ]
