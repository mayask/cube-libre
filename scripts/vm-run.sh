#!/usr/bin/env bash
# Select the same guest desktop the tester is viewing. Do not assume a display.
source "$(dirname "$0")/vm-common.sh"
: "${VM_DISPLAY:?Set VM_DISPLAY to your visible guest X11 display in .dev/vm.env}"
[[ "$VM_DISPLAY" =~ ^:[0-9]+(\.[0-9]+)?$ ]] || { echo "Invalid VM_DISPLAY" >&2; exit 2; }
"${SSH[@]}" "$VM_HOST" "bash -s -- '$VM_DIR' '$VM_DISPLAY'" <<'REMOTE'
set -euo pipefail
cd "$1"
test -x target/debug/cube-libre || { echo "Run scripts/vm-check.sh build first" >&2; exit 1; }
mkdir -p .dev
if [[ -f .dev/app.pid ]]; then
    old_pid="$(< .dev/app.pid)"
    if [[ "$old_pid" =~ ^[0-9]+$ ]] && [[ "$(readlink /proc/"$old_pid"/exe 2>/dev/null || true)" == "$PWD/target/debug/cube-libre" ]]; then
        echo "Cube Libre is already running (PID $old_pid). Close its window first." >&2
        exit 1
    fi
fi
export DISPLAY="$2" XAUTHORITY="$HOME/.Xauthority" DBUS_SESSION_BUS_ADDRESS="unix:path=/run/user/$(id -u)/bus"
export RUST_LOG=cube_ble=debug
printf '\n=== Launching Cube Libre · %s ===\n' "$(date -Is)" >> "$HOME/cube-libre-dev.log"
nohup target/debug/cube-libre >> "$HOME/cube-libre-dev.log" 2>&1 < /dev/null &
echo $! > .dev/app.pid
printf 'App PID %s on desktop %s\n' "$(< .dev/app.pid)" "$DISPLAY"
REMOTE
