# Cube Libre

<img src="assets/cube-libre.svg" width="112" alt="Cube Libre: a cocktail with lime and miniature twisty cubes">

A small, local-first smart-cube workspace in **Rust + Dioxus 0.7**. Native BLE
through **btleplug**, not a browser Bluetooth API. No account, telemetry, cloud,
external fonts, or CubeStation dependency during normal use.

## First release scope

- Discover, save, rename, connect and forget GAN cubes. Multiple saved devices,
  **one active cube at a time**.
- GAN **Gen1–Gen4**, including original 356i, iCarry/iCarry S/iCarry 2/iCarry E,
  GAN i-series and compatible MG/AiCube models. Drivers are selected by GATT
  service, never guessed from the model name. See [protocol coverage](docs/GAN_PROTOCOLS.md)
  for firmware limits, model examples and hardware-validation status.
- Real cube-fixed state in a rotatable CSS 3D view or all-six-faces net.
- Live face turns, battery and hardware information, a bounded move log.
- Gen1 move-window polling; read-only keep-alive / state checks every **5 seconds**,
  battery checks every
  minute, automatic reconnect with capped backoff, startup reconnect preference.
- Explicit unknown, last-known, synchronizing and live states. Missed events trigger
  an authoritative cube snapshot; missing move-log entries are **not invented**.
- An explicitly labelled simulator for offline UI and cube-maths testing.

**No radio connection can be guaranteed never to disconnect.** Range, battery,
radio loss, OS suspension and another app taking the cube can interrupt it. The
worker runs independently of the UI while the app remains open. It detects a
silent connection, reconnects, and reads state again. It cannot wake a sleeping,
out-of-range cube over a disconnected link. Keeping a cube awake uses its battery.

Use **white up / green front** as the cube's fixed reference frame. View controls
rotate only the camera; they never rotate or recalibrate the cube's internal state.
The iCarry E has no gyro. Optional gyro data from other models is currently ignored;
automatic whole-cube orientation tracking is not implemented.

## Run on desktop

Install stable Rust using [rustup](https://rustup.rs). Sources use Rust 2024.
The initial toolchain used is Rust 1.99.0. Dependencies are pinned by Cargo.lock.

Debian 13 / Ubuntu native build dependencies:

```sh
sudo apt-get install build-essential pkg-config libwebkit2gtk-4.1-dev \
  libxdo-dev libssl-dev libdbus-1-dev libayatana-appindicator3-dev librsvg2-dev bluez
cargo run --locked
```

Fedora build dependencies (package names may vary with Fedora version):

```sh
sudo dnf install gcc gcc-c++ pkgconf-pkg-config gtk3-devel webkit2gtk4.1-devel \
  libxdo-devel openssl-devel dbus-devel libappindicator-gtk3-devel librsvg2-devel
cargo run --locked
```

Windows requires the MSVC Rust toolchain and WebView2 runtime. macOS requires
Xcode command-line tools and Bluetooth permission. **Bundle the macOS app with
`NSBluetoothAlwaysUsageDescription`** before distributing it; see
`packaging/macos/Info.plist`. A bare development binary may require granting
Bluetooth access to the terminal that launched it.

Dioxus CLI is optional for development: CSS is embedded, so plain `cargo run`
works without a separate asset server or frontend toolchain.

### Connect a cube

1. Enable Bluetooth and attach the computer's Bluetooth/Wi-Fi antenna.
2. Close CubeStation and other apps using the cube. Do not pair in system settings.
3. Wake the cube (iCarry E: four quick quarter turns of the white face).
4. **Add a device** → select the discovered cube → **Add & connect**.
5. Wait for **LIVE STATE** before relying on the display.

**Gen2–Gen4** require the real hardware MAC for key derivation. It is normally
obtained from BLE manufacturer data (including on Apple platforms), with a
manual-entry fallback. **Gen1** reads its key from device information and does
not require a MAC; briefly hold it still for the initial snapshot. Existing saved
MAC-based records still work. No pairing, reset, calibration or firmware-update
commands are sent.

**Disconnect** pauses the current connection until you reconnect (or relaunch with
Reconnect on launch enabled). **Forget** removes only the local device record.
Closing the app ends its worker; no service is installed or left running.

## Architecture

- `crates/cube-core`: AES codec, strictly validated GAN packets, cube mathematics,
  modulo-counter reconciliation. Independent of UI, OS, storage and Bluetooth.
- `crates/cube-ble`: native transport, device registry, cancellable connection/
  discovery jobs, keep-alive and reconnect. Immutable snapshots published to UI.
- `src`: Dioxus UI, device management, accessible controls, CSS cube/net.
- `scripts`: host-to-Debian build/test mirror helpers.
- `docs`: platform and hardware-test notes.

Native BLE is **not** run inside the WebView. It lives on a dedicated Rust/Tokio
worker. The UI never blocks on scanning, GATT or reconnect delays. Session
generations prevent cancelled workers from overwriting a newer connection.

## Persistence and privacy

Device records are atomically stored in the OS application configuration
directory under `CubeLibre/devices.json` (the Linux directory spelling follows
the `directories` crate). Only device name, native identity, MAC, selected device,
and startup reconnect preference are persisted; cube state and turn logs are not.
`CUBE_LIBRE_CONFIG=/path/to/devices.json` overrides the file for testing.
Corrupt/future-version files are reported and **not silently replaced**.
No network services or analytics are used by the app.

## Platform roadmap (not completed support)

| Target | Current scope |
|---|---|
| Linux desktop | Initial development/manual-test target: Debian 13 |
| Windows / macOS | Native desktop code paths via Dioxus + btleplug; require platform builds and hardware validation |
| Android | Future: JNI/Java btleplug integration, runtime Nearby Devices permissions, Dioxus mobile packaging |
| iOS | Future: CoreBluetooth permissions, Xcode signing/packaging and lifecycle testing |
| Web | Future: separate Web Bluetooth transport; btleplug is not a released WASM backend |

Dioxus is suitable for this project. Mobile does not mean 'compile the desktop
binary unchanged': permissions and lifecycle integration still need work. Mobile
background BLE is explicitly **not** promised. On web, Safari/Firefox do not
natively offer the required Web Bluetooth API.

## Development and verification

```sh
cargo fmt --all -- --check
cargo test -p cube-core --locked
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
# Linux/libvirt helper tests use mocks only—no real adapter is touched:
python3 -m unittest discover -s tests -p 'test_*.py' -v
```

Protocol tests include independent OpenSSL encryption vectors; cube tests compare
moves with an independent cubie reference. State-machine tests cover dropped,
duplicated, out-of-order and wrapped counters; storage and backend tests do not
need a physical cube.

### Optional VM workflow

**Your local host checkout is the source of truth.** The VM copy is disposable.
Keep connection details in an ignored `.dev/vm.env` file, not in source control.
For example, configure an SSH alias in your own SSH config and set:

```sh
export VM_HOST=cube-test
export VM_DIR=/opt/dev/cube-libre  # Must be writable by the SSH user
# Set VM_DISPLAY to the guest X11 display you are actually viewing.
```

Optional variables: `VM_PORT`, `VM_KEY`, `VM_KNOWN_HOSTS`, `VM_OFFLINE`.
SSH normally uses your SSH config, agent and known-hosts file. No machine-specific
defaults or credentials are shipped.

```sh
# If the VM cannot reach crates.io, seed its dependency cache, then set
# VM_OFFLINE=true in your private .dev/vm.env:
scripts/vm-cache.sh
scripts/vm-check.sh test
scripts/vm-check.sh build
scripts/vm-check.sh clippy
# Set VM_DISPLAY, arrange Bluetooth passthrough, and wake the cube:
scripts/vm-run.sh
```

These helpers sync one-way from host to VM and append build/test/app output to
the guest's `~/cube-libre-dev.log`. Keep a terminal with
`tail -F ~/cube-libre-dev.log` open in the same desktop during manual testing.
Private configuration, logs, Git history and build artifacts are excluded from
source sync. See [VM Bluetooth setup](docs/VM_BLUETOOTH.md) for reversible,
permission-gated USB passthrough and `docs/TESTING.md` for the hardware checklist.
`scripts/vm-bluetooth.sh inspect` inventories only; it does not hand over a radio.

### Artwork

The cocktail-and-lime icon contains **two miniature 3×3 twisty cubes instead of
ice**. It is original SVG artwork, licensed with this project.
The same mark is used in the app, desktop window and packaging assets. Regenerate
the PNG/ICO/ICNS variants with `scripts/generate-icons.sh` (ImageMagick and Python
Pillow are development-only dependencies; the app does not require them).

### Publishing safely

Device registries, SSH configuration, local paths, screenshots and diagnostic
logs must not be committed. `.dev/`, environment files, key files and logs are
ignored by default. All example MAC addresses in tests are synthetic.
Review both file contents and Git author metadata before making a fork public.

## Protocol credits

A big thank-you to **Andy Fedotov** ([gan-web-bluetooth](https://github.com/afedotov/gan-web-bluetooth))
for the Gen2–Gen4 implementations, and **Pau Oliva** ([smartcube-web-bluetooth](https://github.com/poliva/smartcube-web-bluetooth))
for the Gen1 extension and additional validation. The upstreams also recognize
**Chen Shuang / csTimer** for earlier reverse-engineering work. These community
implementations made broader compatibility possible. See
[coverage and provenance](docs/GAN_PROTOCOLS.md) for the exact adaptations.

## License

MIT. GAN protocol portions retain the original MIT copyright/license notices for
Andy Fedotov and Pau Oliva; see [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).
This is an independent project, not an official GAN product.
