# GAN protocol coverage and provenance

Cube Libre includes **all four known GAN generations implemented in the two
credited references**, selected by their discovered GATT services—not a guessed
model name. This is protocol-driver coverage, **not a promise that every cube or
firmware revision has been tested**. Real-cube testing of the Rust app remains
pending; all new fixtures are synthetic.

| Generation | Upstream model examples / family | Transport and key | Current app behavior |
|---|---|---|---|
| Gen1 | Original GAN 356i API v1 | `fff0` + Device Information; firmware/hardware-derived AES, overlapping ECB | Poll rolling six-move window, including half turns; read battery/facelets; briefly hold still for an authoritative snapshot |
| Gen2 | GAN Mini ui FreePlay, GAN12 ui/FreePlay, iCarry/iCarry S, GAN356 i3, Monster Go 3Ai | `6e400001-…`; MAC-salted overlapping CBC | Move notifications with seven-move catch-up, state/battery/hardware requests |
| Gen3 | GAN356 iCarry 2 | `8653000a-…`; same GAN MAC-salted CBC | Move notifications, state/battery/hardware requests |
| Gen4 | GAN12 ui Maglev, GAN14 ui FreePlay, iCarry E / GANicE3 family | `00000010-…`; same GAN MAC-salted CBC | Move notifications, state/battery/hardware requests |

The upstream **AiCube / MoYu AI 2023 Gen2-compatible key variant** is included.
It is selected only when the actual BLE advertisement says `AiCube` **and** the
Gen2 service is found. A user-edited saved name never selects an encryption key.
This is not general support for other MoYu protocols or other cube brands.
Scanning includes the upstream `GAN`, `MG` and `AiCube` name prefixes; services
are verified after connection. Some cubes omit service UUIDs from advertisements,
so scanning does not require an advertised service UUID.

## Safety and state reconciliation

- Outbound Gen2–4 commands expose only state, battery and hardware **read
  requests**. Gen1 uses GATT reads. No reset, calibration, pairing, firmware or
  gyro-configuration commands are implemented.
- Gen1 supports the reference's known encrypted v1 firmware/key families (1.0.8+
  in the 1.0 family, and the 1.1 family). Other firmware is rejected, not decoded
  with a guessed key. It derives the key from device information, **not a MAC**.
- Gen2–4 need the real hardware MAC for salting. Manufacturer-data extraction and
  manual entry are supported. Apple-native peripheral IDs are never treated as
  MACs. Existing saved MAC-based records still load unchanged; Gen1 may save an
  opaque native identity without a MAC.
- Gen1 facelets contain no move serial. We bracket their read with two counter
  reads and reject a snapshot if the counter changed. We do not attach serial
  zero or a guessed live state to a facelet frame. Facelets are validated for
  fixed centers, color counts, cubie uniqueness, orientation sums and parity.
- Gen2 replays only the newly received portion of its rolling seven-move window,
  in chronological order. Duplicate/older windows do not advance the decoder.
- All drivers use the shared modulo-256 history/reconciliation window, including
  the low byte of the wider Gen3/4 wire fields, as in the reference implementations.
- A gap beyond an available window freezes predictions and requests an
  authoritative snapshot. Gen3/4 move-history commands are deliberately not
  needed for recovery: state is repaired from a snapshot, and missing turn-log
  entries are not fabricated. This is not a complete speedcubing timer/history
  implementation.
- The view remains cube-fixed (white up, green front). Optional gyro data is
  ignored; automatic whole-cube orientation tracking is not implemented.
- Gen1 polls on a 50 ms target interval (actual rate depends on GATT latency).
  All drivers check authoritative state every 5 seconds and battery every minute;
  interrupted connections are cancelled/cleaned up and retried with bounded backoff.

## References and thanks

**Andy Fedotov**'s [gan-web-bluetooth](https://github.com/afedotov/gan-web-bluetooth)
provided the Gen2/3/4 layouts, key salting/overlapping CBC, read-request framing
and cubie-to-facelet maps. Reference revision:
[`65173e2`](https://github.com/afedotov/gan-web-bluetooth/tree/65173e2cdd0fa38ef384f237f6da8d76c177ed1e/src).

**Pau Oliva**'s [smartcube-web-bluetooth](https://github.com/poliva/smartcube-web-bluetooth),
an MIT fork retaining Andy's attribution, provided the additional Gen1 UUIDs,
firmware/key derivation, overlapping ECB, facelet packing and polling/move-window
reference. Its structural packet validation was also consulted. Reference revision:
[`44f1f09`](https://github.com/poliva/smartcube-web-bluetooth/tree/44f1f091c6e980d9cc31e6d2863c4437eca3ab3c/src).

Their work—and the community's protocol reverse engineering, including
**Chen Shuang** / [csTimer](https://github.com/cs0x7f/cstimer), acknowledged by the
upstreams—makes interoperable cube apps possible. Thank you. No csTimer source
code is vendored in Cube Libre. The Rust drivers are adaptations of the credited
MIT implementations, with native BLE and state-safety handling written here.

Full applicable copyright and license notices are preserved in
[THIRD_PARTY_NOTICES.md](../THIRD_PARTY_NOTICES.md).
