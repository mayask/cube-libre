#!/usr/bin/env bash
# Optional: seed only the crate index/cache (never credentials) for offline VM builds.
source "$(dirname "$0")/vm-common.sh"
cd "$ROOT"
"${CARGO_HOME:-$HOME/.cargo}/bin/cargo" fetch --locked --target x86_64-unknown-linux-gnu
"${SSH[@]}" "$VM_HOST" 'mkdir -p ~/.cargo/registry/index ~/.cargo/registry/cache; printf "\n=== Importing dependency cache ===\n" >> ~/cube-libre-dev.log'
for dir in index cache; do
    rsync -az -e "$RSYNC_SSH" "${CARGO_HOME:-$HOME/.cargo}/registry/$dir/" \
        "$VM_HOST:.cargo/registry/$dir/"
done
