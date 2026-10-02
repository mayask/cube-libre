# Verification and hardware test plan

## Reference test environment

The desktop preview has been built and manually exercised on Debian 13 with
Rust 1.99.0, Dioxus 0.7.10, btleplug 0.13.3 and WebKitGTK 2.52.6.
These are software compatibility references, not required machine identities.

Keep the local host checkout canonical and use any VM as a disposable mirror.
Configure SSH/display details privately in `.dev/vm.env`; see the README.
The helper scripts use checksums and guest file timestamps so host/guest clock
differences do not hide source changes from Cargo. The dependency-cache helper
supports offline builds without copying Cargo credentials.

## Automated checks

- `cargo test --workspace --locked`: **42 tests passed** on Debian 13.
- `cargo clippy --workspace --all-targets --locked -- -D warnings`: passed.
- `cargo build --locked`: passed.
- `cargo fmt --all -- --check`: passed.
- `python3 -m unittest discover -s tests -p 'test_*.py' -v`: **6 tests passed**;
  mock libvirt/USB commands only, never a real adapter handoff.

Coverage: OpenSSL golden AES vectors, all six move encodings, truncated/invalid
packets, impossible cubie states, independent F/R turn reference, long reversible
sequences, duplicate and stale events, modulo-256 wrap, initial unknown state,
gap-triggered resynchronization and buffered moves, atomic persistence,
corrupt/future registry protection, cancellation/backoff, old-session rejection,
simulator state and no persistence. Additional generation coverage includes
Gen1 ECB vectors for both known firmware keys, validated Gen1 facelet packing,
stable-counter snapshot bracketing, six-move catch-up/half turns, Gen2 CBC and
AiCube key vectors, seven-move catch-up, Gen3 layouts, all generation-specific
read requests, metadata, old registry compatibility and MAC-free Gen1 identities.

A Linux/Windows/macOS CI matrix is included. Consult its actual run results;
Linux is the only platform manually exercised so far.

## Manual UI checks

- Launch, close and reopen the actual Dioxus desktop app.
- Initial display is explicitly unknown, not falsely solved/live.
- Simulator is explicitly labelled and shows the six-color cube.
- R then R' updates the view/log and restores the solved indicator.
- Net view exposes all six faces; F changes the net and turn stream.
- Device manager opens without blocking the cube UI.
- A missing adapter produces an actionable error, including VM passthrough advice.
- Modal is keyboard-focused, background inert, Escape dismisses it.
- 3D/net layouts visually reviewed; corrected WebKit preserved-3D flattening.
- Compact-height layout checked at 1280×800.
- Startup reconnect preference verified in the actual local JSON file.
- Cube Libre name and cocktail/lime icon with miniature 3×3 twisty cubes
  verified in the visible app; build, tests and strict Clippy checks pass.

## Physical hardware verification status

GAN Gen4 protocol selection was established with an independent BLE probe of
the iCarry E hardware family. That is **not** an end-to-end test of this Rust app.
Physical-cube tests below remain **pending** for every generation until performed
and recorded. Gen1–Gen4 driver coverage is documented in [GAN_PROTOCOLS.md](GAN_PROTOCOLS.md);
it is not evidence of successful hardware connections on those models.
Do not infer a hardware pass from the simulator or service UUIDs alone.

## Hardware checklist

1. Enable the BLE adapter. For a VM, obtain permission before USB passthrough:
   host Bluetooth peripherals will be unavailable while its adapter is assigned
   to the guest. A separate USB adapter avoids disrupting the host. See
   [VM_BLUETOOTH.md](VM_BLUETOOTH.md) for an opt-in, live-only handoff helper.
   Actual radio handoff remains unverified; inspection/guard checks change nothing.
2. Close CubeStation / disconnect other cube apps. Wake the cube near the antenna.
3. Add a device, select the cube, confirm its MAC for Gen2–4, then Add & connect.
   Gen1 does not need a MAC; briefly hold still to establish a bracketed snapshot.
4. Verify live-state badge, selected **protocol generation**, battery, firmware
   and available hardware information (Gen1 has less metadata).
5. Compare **every face** in Net view to the real cube (white up, green front).
6. Do one clockwise and one inverse turn on each face. Check notation and
   resulting state; then a mixed sequence and faster turns.
7. Cross counter 255 → 0. Check no jump, stuck sync or duplicated moves.
8. Leave untouched beyond the cube's normal idle timeout. Confirm keep-alive and
   immediate next-turn response before claiming idle-sleep prevention.
9. Interrupt the radio / go out of range. The view must become stale, show retry
   state, reconnect after recovery and match the real cube again.
10. Disconnect explicitly: no reconnect until Connect or a permitted relaunch.
11. Close/reopen: saved device/name/preference persist and startup reconnect
    follows its setting. No solved state shown before the first snapshot.
12. Rename, cancel forgetting, confirm forgetting. Verify no cube reset.
13. Close the app; verify it releases the connection.
14. Restore any temporarily assigned host adapter.

Record actual observations, software/model versions and pass/fail results.
Do not publish real device MACs, personal names, screenshots containing private
desktop content, local log paths or identifying machine configuration.
