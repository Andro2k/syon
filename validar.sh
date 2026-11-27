#!/usr/bin/env bash
# Uso: validar.sh <directorio>   (cada copia de trabajo usa SU propio target; no compartir entre copias)
set -uo pipefail
export PATH=/usr/lib/rust-1.91/bin:/tmp/tools/cargo-deny-0.22.1-x86_64-unknown-linux-musl:/tmp/tools/cargo-machete-v0.10.1-x86_64-unknown-linux-musl:$PATH
unset CARGO_TARGET_DIR
cd "${1:?directorio}" || exit 1
out=$(mktemp -d); fails=0
now() { date +%s.%N; }; t0=$(now)
# 1) Dependencias en línea una sola vez (rápido si ya están en caché) y luego todo sin red.
cargo fetch --locked -q 2>"$out/fetch" || { echo "FALLA fetch"; tail -3 "$out/fetch"; exit 1; }
export CARGO_NET_OFFLINE=true
# 2) Ligeras en segundo plano (no compilan); cada una con su PID para conservar su código de salida.
cargo fmt --all --check            >"$out/fmt"     2>&1 & p_fmt=$!
cargo deny --manifest-path Cargo.toml check >"$out/deny" 2>&1 & p_deny=$!   # en línea: advisories frescos
cargo machete .                    >"$out/machete" 2>&1 & p_mach=$!
# 3) Pesadas en serie con fail-fast real (pipefail + &&); las xtask son baratas y van siempre.
step() { local name=$1; shift; if "$@" >"$out/$name" 2>&1; then echo "ok     $name"; else echo "FALLA  $name"; tail -8 "$out/$name" | sed 's/^/         /'; fails=$((fails+1)); return 1; fi; }
{ step clippy cargo clippy --workspace --all-targets --locked -- -D warnings \
  && step test cargo test --workspace --locked; } || true
step docs-check cargo xtask docs-check || true
step arch-check cargo xtask arch-check || true
# 4) Recoger las ligeras con su estado real.
for j in fmt:$p_fmt deny:$p_deny machete:$p_mach; do n=${j%%:*}; p=${j##*:}
  if wait "$p"; then echo "ok     $n"; else echo "FALLA  $n"; tail -6 "$out/$n" | sed 's/^/         /'; fails=$((fails+1)); fi; done
[ -f "$out/test" ] && awk '/^test result/ {p+=$4; f+=$6} END {print "pruebas: " p " aprobadas, " f+0 " fallidas"}' "$out/test"
printf "total: %.0f s, fallos: %d\n" "$(echo "$(now)-$t0" | bc -l)" "$fails"; [ "$fails" -eq 0 ]
