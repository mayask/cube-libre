#!/usr/bin/env bash
# Build/test in the VM with output mirrored into its visible development console.
source "$(dirname "$0")/vm-common.sh"
"$ROOT/scripts/vm-sync.sh"
MODE="${1:-test}"
case "$MODE" in test|build|clippy) ;; *) echo "Usage: $0 [test|build|clippy]" >&2; exit 2;; esac
VM_OFFLINE="${VM_OFFLINE:-false}"
case "$VM_OFFLINE" in true|false) ;; *) echo "VM_OFFLINE must be true or false" >&2; exit 2;; esac
"${SSH[@]}" "$VM_HOST" "bash -s -- '$VM_DIR' '$MODE' '$VM_OFFLINE'" <<'REMOTE'
set -euo pipefail
cd "$1"
source "$HOME/.cargo/env"
export CARGO_BUILD_JOBS=3 CARGO_NET_OFFLINE="$3"
printf '\n=== %s · %s ===\n' "$2" "$(date -Is)" | tee -a "$HOME/cube-libre-dev.log"
case "$2" in
 test) cargo test --workspace --locked 2>&1 | tee -a "$HOME/cube-libre-dev.log";;
 build) cargo build --locked 2>&1 | tee -a "$HOME/cube-libre-dev.log";;
 clippy) cargo clippy --workspace --all-targets --locked -- -D warnings 2>&1 | tee -a "$HOME/cube-libre-dev.log";;
esac
printf '\n=== Completed %s successfully ===\n' "$2" | tee -a "$HOME/cube-libre-dev.log"
REMOTE
