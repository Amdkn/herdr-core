#!/usr/bin/env bash
# Smoke test adapter herdr-core — D-N053 pas F1 (H-F1-3).
# Builds the adapter crate, runs its test suite, checks the adapter `hc`
# binary's client surface, and proves the framed protocol against a local
# stub. Never starts a real core, never touches the machine's real
# config/state (scratch XDG paths + scratch HC_SESSION throughout).
set -euo pipefail
cd "$(dirname "$0")/.."
export PATH="$HOME/.cargo/bin:$HOME/workspace/.tmp/zig-x86_64-linux-0.16.0:$PATH"

SCRATCH="$(mktemp -d /var/tmp/aspace-hc-smoke-XXXXXX)"
trap 'rm -rf "$SCRATCH"' EXIT
export HC_SESSION=aspace-f1-smoke
export XDG_CONFIG_HOME="$SCRATCH/config"
export XDG_STATE_HOME="$SCRATCH/state"
export XDG_RUNTIME_DIR="$SCRATCH/run"
mkdir -p "$XDG_CONFIG_HOME/herdr-core" "$XDG_STATE_HOME" "$XDG_RUNTIME_DIR"
cp aspace-herdr-core-adapter/config.toml "$XDG_CONFIG_HOME/herdr-core/config.toml"

# Build act of the fiche (E-F1-8): the ghostty submodule is uninitialised
# in a fresh clone.
git submodule update --init --depth 1 vendor/ghostty >/dev/null

# Adapter crate: build + its own test suite (protocol round-trip).
cargo build --offline --manifest-path aspace-herdr-core-adapter/Cargo.toml
cargo test --offline --manifest-path aspace-herdr-core-adapter/Cargo.toml
HC="aspace-herdr-core-adapter/target/debug/hc"

# Adapter CLI surface (no server is contacted).
"$HC" --version | grep -q "0.1.0"
"$HC" --help | grep -qi "terminal"
"$HC" doctor | grep -q "fiber:a0:herdr-core"
"$HC" doctor | grep -q '"notes_forced": false'

# No auto-start: server status with no core must fail as unavailable,
# and no socket file may appear at the scratch socket path.
if "$HC" server status 2>"$SCRATCH/status.err"; then
  echo "FAIL: server status succeeded with no core" >&2; exit 1
fi
grep -qi "unavailable" "$SCRATCH/status.err"
test ! -e "$XDG_RUNTIME_DIR/herdr-core/default.sock"

# server start and self-update do not exist in this binary at all
# (clap rejects them before any code runs).
if "$HC" server start 2>/dev/null; then echo "FAIL: server start exists" >&2; exit 1; fi
if "$HC" update 2>/dev/null; then echo "FAIL: update exists" >&2; exit 1; fi

# Contract surface: server start and self-update are NOT in the adapter
# surface; the library re-exports 33 of the 35 upstream pub modules
# (all except server and cli).
python3 - <<'EOF'
import json
c = json.load(open("aspace-herdr-core-adapter/contract.json"))
surface = c["cli"]["surface"]
for banned in ("hc server start", "hc update"):
    assert banned not in surface, f"{banned} must not be in the adapter surface"
assert c["switches_forced"]["notes"] is False
assert c["protocol"]["openrpc_methods"] == 32
assert c["library"]["reexported_modules"] == 33
assert c["library"]["excluded_modules"] == ["server", "cli"]
print("contract OK: surface excludes server start + update, notes=false, 32 methods, lib 33/35")
EOF

# Forced switch + protocol proof, through the adapter library itself.
cargo run --offline --quiet \
  --manifest-path aspace-herdr-core-adapter/protocol-proof/Cargo.toml -- \
  aspace-herdr-core-adapter/config.toml "$SCRATCH"

echo "smoke herdr-core OK — adapter hc 0.1.0, doctor, no auto-start, notes=false, protocol proof green"
