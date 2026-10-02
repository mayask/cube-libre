#!/usr/bin/env bash
# Optional libvirt USB handoff. Inspect is read-only; attach requires explicit consent.
# Never put a domain name, radio identifier or hypervisor URI in tracked defaults.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
[[ ! -f "$ROOT/.dev/vm.env" ]] || source "$ROOT/.dev/vm.env"
export LC_ALL=C
VIRSH=(virsh)
[[ -z "${VM_LIBVIRT_URI:-}" ]] || VIRSH+=(--connect "$VM_LIBVIRT_URI")
MODE="${1:-inspect}"
case "$MODE" in
    inspect)
        [[ $# -le 1 ]] || { echo "inspect accepts no extra arguments" >&2; exit 2; }
        "${VIRSH[@]}" uri
        "${VIRSH[@]}" list --all
        lsusb
        echo "Inspection only: no radio was detached or assigned."
        exit 0;;
    attach)
        [[ $# -eq 2 && "$2" == --allow-host-bluetooth-interruption ]] || {
            echo "Attaching a host radio interrupts host Bluetooth devices." >&2
            echo "After obtaining permission, use: $0 attach --allow-host-bluetooth-interruption" >&2
            exit 2
        };;
    detach)
        [[ $# -eq 1 ]] || { echo "detach accepts no extra arguments" >&2; exit 2; };;
    *) echo "Usage: $0 [inspect|attach --allow-host-bluetooth-interruption|detach]" >&2; exit 2;;
esac
: "${VM_LIBVIRT_DOMAIN:?Set VM_LIBVIRT_DOMAIN privately in .dev/vm.env}"
: "${VM_BLUETOOTH_VENDOR:?Set the Bluetooth USB vendor ID privately}"
: "${VM_BLUETOOTH_PRODUCT:?Set the Bluetooth USB product ID privately}"
vendor="${VM_BLUETOOTH_VENDOR,,}"; vendor="${vendor#0x}"
product="${VM_BLUETOOTH_PRODUCT,,}"; product="${product#0x}"
[[ "$vendor" =~ ^[0-9a-f]{4}$ && "$product" =~ ^[0-9a-f]{4}$ ]] || { echo "USB IDs must be four hexadecimal digits" >&2; exit 2; }
[[ "$("${VIRSH[@]}" domstate --domain "$VM_LIBVIRT_DOMAIN")" == running ]] || { echo "Start the VM first; this helper makes live-only changes" >&2; exit 2; }
if [[ "$MODE" == attach ]]; then
    mapfile -t radios < <(lsusb -d "$vendor:$product")
    [[ ${#radios[@]} -eq 1 ]] || { echo "Expected exactly one matching USB radio; refusing an ambiguous handoff" >&2; exit 2; }
    # An unprivileged session QEMU needs device-node access. Do not change ACLs,
    # sudo permissions, udev rules or kernel drivers automatically.
    if [[ "$("${VIRSH[@]}" uri)" == qemu:///session ]]; then
        read -r _ bus _ address _ <<< "${radios[0]}"
        node="/dev/bus/usb/$bus/${address%:}"
        [[ -r "$node" && -w "$node" ]] || { echo "Session QEMU needs read/write access to $node. Arrange a narrowly scoped temporary ACL or use an appropriately configured system VM." >&2; exit 2; }
    fi
fi
xml="$(mktemp)"
trap 'rm -f "$xml"' EXIT
printf '<hostdev mode="subsystem" type="usb"><source><vendor id="0x%s"/><product id="0x%s"/></source></hostdev>\n' "$vendor" "$product" > "$xml"
if [[ "$MODE" == attach ]]; then
    echo "Passing the entire USB radio to the guest. Host Bluetooth will be unavailable."
    "${VIRSH[@]}" attach-device --domain "$VM_LIBVIRT_DOMAIN" --file "$xml" --live
else
    "${VIRSH[@]}" detach-device --domain "$VM_LIBVIRT_DOMAIN" --file "$xml" --live
    echo "Radio returned to the host. Check host Bluetooth and reconnect peripherals if needed."
fi
