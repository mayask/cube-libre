# Testing a real cube in a Linux VM

A native BLE app needs a **Bluetooth controller inside the guest**. Ordinary VM
networking, SSH and forwarding a TCP port do not forward the host's Bluetooth
connection. The cube will make a new BLE connection to the guest; an existing
host connection is not shared or migrated.

## Recommended: a separate USB BLE adapter

Assign a spare USB Bluetooth adapter to the VM. This leaves the host's main
Bluetooth radio and peripherals alone. In a graphical VM manager, add the
specific adapter as a USB host device, not the whole USB/PCI controller.

## Optional: temporarily hand over the host radio

**Obtain permission before doing this.** Passing the main radio to the guest
interrupts host Bluetooth headphones, keyboards, mice and other devices. Have
a non-Bluetooth input method available. Avoid detaching an entire Wi-Fi/PCI
controller just because it also contains Bluetooth.

For a running libvirt VM with a USB controller, an opt-in helper is provided:

```sh
scripts/vm-bluetooth.sh inspect   # Inventory only; changes nothing
```

Set `VM_LIBVIRT_DOMAIN`, optional `VM_LIBVIRT_URI`, `VM_BLUETOOTH_VENDOR` and
`VM_BLUETOOTH_PRODUCT` in your ignored `.dev/vm.env`. Get the exact USB IDs from
`lsusb`; verify that they identify the Bluetooth radio, not another device.
Machine names, IDs, USB paths and diagnostic output must stay private.

Before handoff, close host cube apps and disconnect their BLE link. After
permission has been obtained:

```sh
scripts/vm-bluetooth.sh attach --allow-host-bluetooth-interruption
# In the visible guest desktop: enable Bluetooth, wake the cube, then Add & connect.
# When finished, disconnect/close the guest app before returning the radio:
scripts/vm-bluetooth.sh detach
```

The helper changes only the **running** VM, not its permanent configuration.
It refuses ambiguous adapters and does not grant permissions, alter ACLs,
unload drivers, pair with a cube or send cube reset/calibration/firmware commands.

### Device permissions

A `qemu:///session` VM runs without root privileges. Its QEMU process may need
read/write permission on the selected `/dev/bus/usb` device node before USB
passthrough works. The helper checks this and refuses if permission is missing.
An administrator can arrange a narrowly scoped temporary device ACL, or use an
appropriately configured system libvirt VM. Do not make every USB node
world-writable. Remove any temporary permission grant after testing. Host
security policy, USB resets or re-enumeration may require additional setup.

### Guest readiness and recovery

- Install BlueZ and required USB Bluetooth firmware in the guest. Start/enable
  its Bluetooth service through the guest's normal settings/service manager.
- Verify `bluetoothctl list` inside the guest now shows a controller, and enable
  Bluetooth there. A detected USB device alone does not prove a working BLE radio.
- Use the same desktop and visible development console as other manual tests.
- Close competing cube apps, wake the physical cube and compare every face in
  Cube Libre. Do not pair/reset/calibrate the cube to make a test pass.
- After detach, check that the host controller and peripherals return. A stopped
  VM also releases a live-only assignment. If recovery fails, use your VM
  manager to remove only that USB host device and troubleshoot locally.

This workflow is prepared for manual testing, **not a claim of successful radio
handoff or end-to-end hardware validation**. Follow the full checklist in
[TESTING.md](TESTING.md) and record actual results privately.
