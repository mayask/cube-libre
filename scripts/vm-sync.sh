#!/usr/bin/env bash
# The host checkout is canonical. The VM directory is a disposable build mirror.
source "$(dirname "$0")/vm-common.sh"
"${SSH[@]}" "$VM_HOST" "mkdir -p '$VM_DIR'; printf '\\n=== Syncing host sources ===\\n' >> ~/cube-libre-dev.log"
# Use guest mtimes and checksums so clock differences cannot hide edits from Cargo.
rsync -az --no-times --omit-dir-times --checksum --delete --itemize-changes \
    --exclude .git/ --exclude target/ --exclude .dev/ --exclude dist/ \
    --exclude '.env' --exclude '.env.*' --exclude '*.log' --exclude '*.bundle' \
    -e "$RSYNC_SSH" "$ROOT/" "$VM_HOST:$VM_DIR/" \
    | "${SSH[@]}" "$VM_HOST" 'tee -a ~/cube-libre-dev.log'
