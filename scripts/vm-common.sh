#!/usr/bin/env bash
# Per-developer connection details belong in the ignored .dev/vm.env file.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
if [[ -f "$ROOT/.dev/vm.env" ]]; then
    source "$ROOT/.dev/vm.env"
fi
: "${VM_HOST:?Set VM_HOST to your SSH alias in .dev/vm.env}"
: "${VM_DIR:?Set VM_DIR to an absolute guest path ending in /cube-libre}"
VM_PORT="${VM_PORT:-22}"
VM_KEY="${VM_KEY:-}"
VM_KNOWN_HOSTS="${VM_KNOWN_HOSTS:-}"
# Constrain values passed through the remote shell, especially the --delete target.
[[ "$VM_HOST" =~ ^[a-zA-Z0-9_][a-zA-Z0-9_.@:-]*$ ]] || { echo "Invalid VM_HOST" >&2; exit 2; }
[[ "$VM_PORT" =~ ^[0-9]+$ ]] || { echo "Invalid VM_PORT" >&2; exit 2; }
[[ "$VM_DIR" =~ ^/[a-zA-Z0-9_./-]+/cube-libre$ && "$VM_DIR" != *".."* ]] || { echo "Unsafe VM_DIR" >&2; exit 2; }
SSH=(ssh -p "$VM_PORT" -o BatchMode=yes)
[[ -z "$VM_KEY" ]] || SSH+=(-i "$VM_KEY" -o IdentitiesOnly=yes)
[[ -z "$VM_KNOWN_HOSTS" ]] || SSH+=(-o "UserKnownHostsFile=$VM_KNOWN_HOSTS")
# rsync parses a shell-like command string; quote optional key/known-hosts paths.
printf -v RSYNC_SSH '%q ' "${SSH[@]}"
